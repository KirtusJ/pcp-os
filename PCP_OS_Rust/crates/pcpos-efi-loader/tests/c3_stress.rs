//! C3 independent host controls for review head 87c627b8; not a native boot.
use pcpos_model_media::Manifest;
use pcpos_preloaded_loader::{payload::{load_model, Media}, Result};
use sha2::{Digest, Sha256};

fn digest(b: &[u8]) -> [u8; 32] { Sha256::digest(b).into() }
fn manifest(data: &[u8], width: usize) -> Vec<u8> {
    let mut b = b"PCPMDL01".to_vec();
    b.extend((data.len() as u64).to_le_bytes());
    b.extend((width as u32).to_le_bytes());
    b.extend((data.len().div_ceil(width) as u32).to_le_bytes());
    b.extend(digest(data));
    for s in data.chunks(width) { b.extend(digest(s)); }
    b
}
struct Files<'a> { shards: Vec<&'a [u8]>, pos: usize, width: usize, opened: usize, closed: usize, bad_read: bool, bad_close: bool, progress: Vec<u64> }
impl Media for Files<'_> {
    type File = usize;
    fn open(&mut self, name: &[u8]) -> Result<usize> {
        let i:usize=std::str::from_utf8(&name[6..11]).unwrap().parse().unwrap();
        if i >= self.shards.len() { return Err("missing"); }
        self.pos=0; self.opened+=1; Ok(i)
    }
    fn read(&mut self, f: usize, out: &mut [u8]) -> Result<usize> {
        if self.bad_read { return Ok(out.len()+1); }
        let n=self.width.min(out.len()).min(self.shards[f].len()-self.pos);
        out[..n].copy_from_slice(&self.shards[f][self.pos..self.pos+n]); self.pos+=n; Ok(n)
    }
    fn close(&mut self, _:usize) -> Result<()> { self.closed+=1; if self.bad_close {Err("close")} else {Ok(())} }
    fn progress(&mut self, done:u64, _:u64) {self.progress.push(done);}
}
fn files(data:&[u8], shard:usize, read:usize)->Files<'_> {
    Files{shards:data.chunks(shard).collect(),pos:0,width:read,opened:0,closed:0,bad_read:false,bad_close:false,progress:vec![]}
}

#[test]
fn c3_all_page_and_shard_boundary_placements() {
    let mut trials=0;
    for len in [1usize,1023,1024,1025,4095,4096,4097,8192,8193,20001] {
        let data:Vec<_>=(0..len).map(|i|((i*31+i/7)%251) as u8).collect();
        let pages=len.div_ceil(4096);
        for shard in [1024usize,4096,8192] {
            let raw=manifest(&data,shard); let m=Manifest::parse(&raw,digest(&raw)).unwrap();
            for split in 0..pages {
                let lengths=if split==0 {vec![pages*4096]} else {vec![split*4096,(pages-split)*4096]};
                for read in [1,3,257,1024,8192] {
                    let mut storage:Vec<_>=lengths.iter().map(|n|vec![0xa5;n+2]).collect();
                    let mut dest:Vec<_>=storage.iter_mut().zip(&lengths).map(|(p,n)|&mut p[1..n+1]).collect();
                    let mut f=files(&data,shard,read);
                    load_model(&mut f,&m,&mut dest).unwrap();
                    let flat:Vec<u8>=dest.iter().flat_map(|s|s.iter().copied()).collect();
                    assert_eq!(&flat[..len],&data); assert!(flat[len..].iter().all(|x|*x==0));
                    assert_eq!(f.opened,len.div_ceil(shard)); assert_eq!(f.closed,f.opened);
                    assert_eq!(f.progress.last().copied(),Some(len as u64));
                    drop(dest); for (p,n) in storage.iter().zip(&lengths) {assert_eq!([p[0],p[n+1]],[0xa5;2]);}
                    trials+=1;
                }
            }
        }
    }
    println!("C3 placement trials={trials}");
}

#[test]
fn c3_refusals_do_not_read_bad_destination_or_report_complete_on_io_failure() {
    let data=vec![7u8;4097];let raw=manifest(&data,1024);let m=Manifest::parse(&raw,digest(&raw)).unwrap();
    for size in [0usize,4095,4096,8191,12288] {
        let mut dst=vec![0xa5;size];let mut f=files(&data,1024,3);
        assert!(load_model(&mut f,&m,&mut [&mut dst]).is_err());
        assert_eq!(f.opened,0);assert!(dst.iter().all(|x|*x==0xa5));
    }
    for bad_read in [false,true] {
        let mut dst=vec![0xa5;8192];let mut f=files(&data,1024,3);
        f.bad_read=bad_read;f.bad_close=!bad_read;
        assert!(load_model(&mut f,&m,&mut [&mut dst]).is_err());
        assert_eq!(f.opened,1);assert_eq!(f.closed,1);assert!(f.progress.is_empty());
    }
    let mut f=files(&data,1024,3);f.shards.truncate(2);let mut dst=vec![0xa5;8192];
    assert!(load_model(&mut f,&m,&mut [&mut dst]).is_err());assert_eq!(f.closed,2);
    assert_eq!(f.progress,[1024,2048]);
}
