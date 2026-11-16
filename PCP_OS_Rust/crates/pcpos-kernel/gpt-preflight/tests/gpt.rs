//! Synthetic host tests, bytes produced independently by Python/zlib.
//! The Rust library is read-only and does not communicate with virtio PCI.

use std::{cell::Cell, env, fs, path::PathBuf};
use pcpos_gpt_preflight::{
    choose_unique, crc32, inspect, still_writable,
    Error, Expected, Inspected, Media, ReadSector,
};

const CAP: u64 = 512;

fn guid(first:u8)->[u8;16] {
    std::array::from_fn(|i| first+i as u8)
}
fn target()->Expected{
    Expected{
        disk_guid:guid(1),
        partition_guid:guid(17),
        partition_type:guid(33),
    }
}
fn media()->Media{Media{capacity_512:CAP,read_only:false,generation:7}}

struct Disk{
    bytes:Vec<u8>,
    reads:Cell<u32>,
    fail_at:Option<u64>,
}
impl ReadSector for Disk{
    fn read_sector(&self,lba:u64,out:&mut [u8;512])->Result<(),()>{
        self.reads.set(self.reads.get()+1);
        if self.fail_at==Some(lba){return Err(());}
        let index=usize::try_from(lba).map_err(|_|())?
            .checked_mul(512).ok_or(())?;
        let data=self.bytes.get(index..index+512).ok_or(())?;
        out.copy_from_slice(data);
        Ok(())
    }
}
fn fixture(name:&str)->Disk{
    let base=PathBuf::from(env::var("PCPOS_GPT_FIXTURES")
        .expect("Generate Python fixtures and set PCPOS_GPT_FIXTURES"));
    let bytes=fs::read(base.join(format!("{name}.raw")))
        .expect("fixture bytes absent");
    Disk{bytes,reads:Cell::new(0),fail_at:None}
}
fn inspect_case(name:&str)->Result<Inspected,Error>{
    inspect(&fixture(name),media(),target())
}

#[test]
fn gpt_crc32_matches_independent_standard_check_value(){
    assert_eq!(crc32(b"123456789"),0xcbf4_3926);
}
#[test]
fn valid_protective_mbr_both_headers_both_arrays_and_selected_partition(){
    let disk=fixture("valid");
    let result=inspect(&disk,media(),target()).unwrap();
    assert_eq!(result.selected.first,40);
    assert_eq!(result.selected.last,100);
    assert_eq!(result.selected.type_guid,guid(33));
    assert_eq!(result.selected.unique_guid,guid(17));
    assert_eq!(result.disk_guid,guid(1));
    assert_eq!(result.populated,1);
    assert_eq!(disk.reads.get(),67); // LBA0 + 2 headers + 2*32 table blocks
}
#[test]
fn cloned_guid_and_tables_cannot_authenticate_a_distinct_device(){
    let a=inspect_case("valid").unwrap();
    let b=inspect_case("clone").unwrap();
    assert_eq!(a,b);
    assert_eq!(choose_unique(&[a,b],guid(1),true),Err(Error::AmbiguousDevice));
    // An incomplete inventory or a forged generation can still fool a caller.
}
#[test]
fn unique_selection_requires_explicit_completeness_claim(){
    let valid=inspect_case("valid").unwrap();
    assert_eq!(choose_unique(&[valid],guid(1),false),Err(Error::InventoryIncomplete));
    assert_eq!(choose_unique(&[valid],guid(1),true),Ok(0));
    assert_eq!(choose_unique(&[],guid(1),true),Err(Error::WrongDisk));
}
#[test]
fn rejects_corrupt_primary_even_when_backup_remains_valid(){
    assert_eq!(inspect_case("primary_crc_bad"),Err(Error::BadPrimary));
}
#[test]
fn rejects_corrupt_backup_even_when_primary_remains_valid(){
    assert_eq!(inspect_case("backup_crc_bad"),Err(Error::BadBackup));
}
#[test]
fn rejects_nonprotective_or_corrupt_mbr(){
    assert_eq!(inspect_case("protective_mbr_bad"),Err(Error::BadProtectiveMbr));
}
#[test]
fn rejects_reserved_bytes_outside_supported_header(){
    assert_eq!(inspect_case("primary_reserved_bad"),Err(Error::BadPrimary));
}
#[test]
fn requires_identical_primary_backup_partition_arrays(){
    assert_eq!(inspect_case("primary_array_mismatch"),Err(Error::DifferentEntryArrays));
}
#[test]
fn recomputed_array_bytes_with_stale_crc_do_not_verify(){
    assert_eq!(inspect_case("both_arrays_wrong_crc"),Err(Error::BadEntryCrc));
}
#[test]
fn overlapping_partitions_are_not_safe_install_targets(){
    assert_eq!(inspect_case("overlap"),Err(Error::Overlap));
}
#[test]
fn duplicate_partition_guid_is_refused(){
    assert_eq!(inspect_case("duplicate_guid"),Err(Error::DuplicatePartitionGuid));
}
#[test]
fn partition_before_first_usable_lba_is_refused(){
    assert_eq!(inspect_case("out_of_range"),Err(Error::InvalidPartition));
}
#[test]
fn rejects_malformed_64_bit_partition_lba_above_media_capacity(){
    assert_eq!(inspect_case("large_lba_out_of_range"),Err(Error::InvalidPartition));
}
#[test]
fn missing_explicitly_selected_partition_is_refused(){
    assert_eq!(inspect_case("missing_partition"),Err(Error::SelectedMissing));
}
#[test]
fn wrong_selected_partition_type_is_refused(){
    assert_eq!(inspect_case("wrong_type"),Err(Error::WrongType));
}
#[test]
fn wrong_disk_guid_is_not_the_expected_disk(){
    assert_eq!(inspect_case("wrong_disk_guid"),Err(Error::WrongDisk));
}
#[test]
fn contradictory_gpt_disk_ids_cannot_be_reconciled_silently(){
    assert_eq!(inspect_case("mismatched_header_guids"),Err(Error::HeaderDisagreement));
}
#[test]
fn unsupported_256_entry_gpt_is_not_called_corrupt_or_accepted(){
    assert_eq!(inspect_case("unsupported_entries"),Err(Error::UnsupportedProfile));
}
#[test]
fn missing_last_sector_yields_io_failure_not_a_fabricated_recovery(){
    assert_eq!(inspect_case("truncated_last_sector"),Err(Error::ReadFailed));
}
#[test]
fn driver_read_error_fails_closed(){
    let mut disk=fixture("valid");
    disk.fail_at=Some(2);
    assert_eq!(inspect(&disk,media(),target()),Err(Error::ReadFailed));
}
#[test]
fn undercapacity_is_refused_before_first_disk_access(){
    let disk=fixture("valid");
    let mut bad=media();
    bad.capacity_512=67;
    assert_eq!(inspect(&disk,bad,target()),Err(Error::TooSmall));
    assert_eq!(disk.reads.get(),0);
}
#[test]
fn zero_selected_guids_are_not_accepted(){
    let disk=fixture("valid");
    let mut invalid=target();
    invalid.partition_guid=[0;16];
    assert_eq!(inspect(&disk,media(),invalid),Err(Error::InvalidSelection));
    assert_eq!(disk.reads.get(),0);
}
#[test]
fn reads_on_read_only_media_are_possible_but_writes_are_not(){
    let disk=fixture("valid");
    let mut ro=media();
    ro.read_only=true;
    let found=inspect(&disk,ro,target()).unwrap();
    assert_eq!(still_writable(found,ro),Err(Error::ReadOnly));
}
#[test]
fn changed_generation_or_capacity_refuses_write_plan(){
    let old=inspect_case("valid").unwrap();
    let mut new=media();
    new.generation=8;
    assert_eq!(still_writable(old,new),Err(Error::MediaChanged));
    new=media();
    new.capacity_512=511;
    assert_eq!(still_writable(old,new),Err(Error::MediaChanged));
    assert_eq!(still_writable(old,media()),Ok(()));
}
#[test]
fn newly_read_only_device_refuses_write_plan(){
    let old=inspect_case("valid").unwrap();
    let mut new=media();
    new.read_only=true;
    assert_eq!(still_writable(old,new),Err(Error::ReadOnly));
}
