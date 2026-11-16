#![no_std]
//! Read-only GPT validation for a future virtio-blk installer boundary.
//!
//! Pure supplied bytes only. No PCI, DMA, firmware, disk writes, approval,
//! hardware provenance, crypto, or caller authentication is implemented.
//! STRICT limited profile: 512-byte GPT LBAs, 92-byte v1 header, 128 entries
//! of 128 bytes and matching primary/backup GPT tables. Other valid GPT
//! shapes are UNSUPPORTED here, not necessarily invalid GPTs.

use core::convert::TryInto;

pub const SECTOR_BYTES: usize = 512;
pub const ENTRIES: usize = 128;
const ENTRY_BYTES: usize = 128;
const ENTRY_SECTORS: u64 = 32;
const MIN_CAPACITY: u64 = 68;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Media {
    /// virtio-blk reports capacity in 512-byte sectors, not physical blocks.
    pub capacity_512: u64,
    pub read_only: bool,
    /// Caller-owned generation, requiring a trusted driver for meaning.
    pub generation: u64,
}
pub trait ReadSector {
    fn read_sector(&self, lba: u64, out: &mut [u8; 512]) -> Result<(), ()>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expected {
    pub disk_guid: [u8;16],
    pub partition_guid: [u8;16],
    pub partition_type: [u8;16],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition {
    pub first: u64,
    pub last: u64,
    pub unique_guid: [u8;16],
    pub type_guid: [u8;16],
}
const EMPTY: Partition = Partition {
    first:0,last:0,unique_guid:[0;16],type_guid:[0;16]
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspected {
    pub media: Media,
    pub selected: Partition,
    pub disk_guid: [u8;16],
    pub populated: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidSelection, TooSmall, ReadFailed, BadProtectiveMbr,
    BadPrimary, BadBackup, UnsupportedProfile, HeaderDisagreement,
    WrongDisk, DifferentEntryArrays, BadEntryCrc, InvalidPartition,
    DuplicatePartitionGuid, Overlap, SelectedMissing, WrongType,
    InventoryIncomplete, AmbiguousDevice, MediaChanged, ReadOnly,
}
fn u32le(x:&[u8],p:usize)->u32 {u32::from_le_bytes(x[p..p+4].try_into().unwrap())}
fn u64le(x:&[u8],p:usize)->u64 {u64::from_le_bytes(x[p..p+8].try_into().unwrap())}
fn guid(x:&[u8],p:usize)->[u8;16] {x[p..p+16].try_into().unwrap()}

fn advance(mut crc:u32,b:u8)->u32 {
    crc ^= u32::from(b);
    for _ in 0..8 {crc=(crc>>1)^if crc&1!=0 {0xedb8_8320} else {0};}
    crc
}
pub fn crc32(bytes:&[u8])->u32 {
    !bytes.iter().fold(!0u32,|crc,&b|advance(crc,b))
}
fn check_mbr(x:&[u8;512],capacity:u64)->bool {
    if x[510..512]!=[0x55,0xaa] {return false;}
    let p=&x[446..462];
    if p[4]!=0xee || u32le(p,8)!=1 {return false;}
    if u32le(p,12)!=(capacity-1).min(u64::from(u32::MAX)) as u32 {return false;}
    x[462..510].iter().all(|&b|b==0)
}
#[derive(Clone,Copy)]
struct Header {guid:[u8;16],first:u64,last:u64,array_crc:u32}
fn parse_header(x:&[u8;512],capacity:u64,primary:bool)->Result<Header,Error>{
    let bad=if primary {Error::BadPrimary}else{Error::BadBackup};
    if &x[..8]!=b"EFI PART" || u32le(x,8)!=0x10000 {return Err(bad);}
    if u32le(x,12)!=92 || u32le(x,80)!=128 || u32le(x,84)!=128 {
        return Err(Error::UnsupportedProfile);
    }
    if u32le(x,20)!=0 || x[92..].iter().any(|&b|b!=0){return Err(bad);}
    let mut tmp=*x;
    tmp[16..20].fill(0);
    if crc32(&tmp[..92])!=u32le(x,16){return Err(bad);}
    let (want_my,want_alt,want_array)=if primary {
        (1,capacity-1,2)
    }else{
        (capacity-1,1,capacity-33)
    };
    let first=u64le(x,40);
    let last=u64le(x,48);
    if u64le(x,24)!=want_my || u64le(x,32)!=want_alt ||
       u64le(x,72)!=want_array || first<34 || first>last ||
       last>capacity-34 {return Err(bad);}
    let id=guid(x,56);
    if id==[0;16] {return Err(bad);}
    Ok(Header{guid:id,first,last,array_crc:u32le(x,88)})
}
/// Every operation is a read. An Inspected value is not an authorization.
pub fn inspect<R:ReadSector>(reader:&R,media:Media,want:Expected)->Result<Inspected,Error>{
    if want.disk_guid==[0;16] || want.partition_guid==[0;16] ||
       want.partition_type==[0;16] {return Err(Error::InvalidSelection);}
    if media.capacity_512<MIN_CAPACITY {return Err(Error::TooSmall);}
    let mut a=[0u8;512];
    let mut b=[0u8;512];
    reader.read_sector(0,&mut a).map_err(|_|Error::ReadFailed)?;
    if !check_mbr(&a,media.capacity_512){return Err(Error::BadProtectiveMbr);}
    reader.read_sector(1,&mut a).map_err(|_|Error::ReadFailed)?;
    reader.read_sector(media.capacity_512-1,&mut b)
        .map_err(|_|Error::ReadFailed)?;
    let primary=parse_header(&a,media.capacity_512,true)?;
    let backup=parse_header(&b,media.capacity_512,false)?;
    if primary.guid!=backup.guid || primary.first!=backup.first ||
       primary.last!=backup.last || primary.array_crc!=backup.array_crc {
        return Err(Error::HeaderDisagreement);
    }
    if primary.guid!=want.disk_guid {return Err(Error::WrongDisk);}
    let mut seen=[EMPTY;ENTRIES];
    let mut n=0usize;
    let mut selected=None;
    let mut ca=!0u32;
    let mut cb=!0u32;
    for sector_index in 0..ENTRY_SECTORS{
        reader.read_sector(2+sector_index,&mut a).map_err(|_|Error::ReadFailed)?;
        reader.read_sector(media.capacity_512-33+sector_index,&mut b)
            .map_err(|_|Error::ReadFailed)?;
        if a!=b {return Err(Error::DifferentEntryArrays);}
        for &v in a.iter(){ca=advance(ca,v);}
        for &v in b.iter(){cb=advance(cb,v);}
        for entry in a.chunks_exact(ENTRY_BYTES){
            let kind=guid(entry,0);
            if kind==[0;16] {
                if entry.iter().any(|&v|v!=0){return Err(Error::InvalidPartition);}
                continue;
            }
            let id=guid(entry,16);
            let first=u64le(entry,32);
            let last=u64le(entry,40);
            if id==[0;16] || first<primary.first || first>last ||
               last>primary.last {return Err(Error::InvalidPartition);}
            let current=Partition{first,last,unique_guid:id,type_guid:kind};
            for old in &seen[..n]{
                if old.unique_guid==id{return Err(Error::DuplicatePartitionGuid);}
                if !(last<old.first || old.last<first){return Err(Error::Overlap);}
            }
            seen[n]=current;
            n+=1;
            if id==want.partition_guid {
                if kind!=want.partition_type {return Err(Error::WrongType);}
                selected=Some(current);
            }
        }
    }
    if !ca!=primary.array_crc || !cb!=backup.array_crc {
        return Err(Error::BadEntryCrc);
    }
    Ok(Inspected{media,selected:selected.ok_or(Error::SelectedMissing)?,
                 disk_guid:primary.guid,populated:n})
}
/// This is a conditional planner check, NOT a write capability.
pub fn still_writable(old:Inspected,now:Media)->Result<(),Error>{
    if old.media.capacity_512!=now.capacity_512 ||
       old.media.generation!=now.generation {return Err(Error::MediaChanged);}
    if old.media.read_only || now.read_only {return Err(Error::ReadOnly);}
    Ok(())
}
/// A claimed complete inventory is data. It does not authenticate devices.
pub fn choose_unique(list:&[Inspected],disk_guid:[u8;16],complete:bool)->Result<usize,Error>{
    if !complete {return Err(Error::InventoryIncomplete);}
    let mut found=None;
    for (i,item) in list.iter().enumerate(){
        if item.disk_guid==disk_guid{
            if found.is_some(){return Err(Error::AmbiguousDevice);}
            found=Some(i);
        }
    }
    found.ok_or(Error::WrongDisk)
}
