//! Host tests for the opt-in, cloneable v2 IDE volume marker contract.
//! Compile with: rustc --edition=2021 --test tests/store_identity.rs -o /tmp/test-id && /tmp/test-id
#[path = "../src/store_identity.rs"]
mod store_identity;

use store_identity::{requested_id, marker_sector, matches_marker, MARKER_V2, ID_OFFSET};
const SELECTOR_A: &str = "0123456789abcdeffedcba9876543210";
const SELECTOR_B: &str = "123456789abcdeffedcba9876543211";

fn parsed(hex: &str) -> [u8; 16] {
    requested_id(&format!("auto=action1.elf store=ide0 store_id={hex} then=off")).unwrap()
}
#[test]
fn accepts_one_exact_lowercase_selected_volume_id() {
    assert_eq!(parsed(SELECTOR_A), [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
        0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10
    ]);
}
#[test]
fn refuses_missing_id() {
    assert_eq!(requested_id("store=ide0 auto=action1.elf"), None);
}
#[test]
fn refuses_duplicate_id_even_when_identical() {
    assert_eq!(requested_id(&format!("store_id={SELECTOR_A} store_id={SELECTOR_A}")),None);
}
#[test]
fn refuses_conflicting_id_tokens() {
    assert_eq!(requested_id(&format!("store_id={SELECTOR_A} store_id={SELECTOR_B}")),None);
}
#[test]
fn refuses_noncanonical_uppercase() {
    assert_eq!(requested_id(&format!("store_id={}",SELECTOR_A.to_uppercase())),None);
}
#[test]
fn refuses_overlong_id() {
    assert_eq!(requested_id(&format!("store_id={SELECTOR_A}00")),None);
}
#[test]
fn refuses_short_id() {
    assert_eq!(requested_id(&format!("store_id={}",&SELECTOR_A[..30])),None);
}
#[test]
fn refuses_odd_length() {
    assert_eq!(requested_id(&format!("store_id={}",&SELECTOR_A[..31])),None);
}
#[test]
fn refuses_invalid_character() {
    assert_eq!(requested_id("store_id=0123456789abcdefgedcba9876543210"),None);
}
#[test]
fn refuses_zero_id_as_not_provisioned() {
    assert_eq!(requested_id("store_id=00000000000000000000000000000000"),None);
}
#[test]
fn refuses_prefixed_or_suffix_obscured_token_as_id() {
    assert_eq!(requested_id(&format!("other_store_id={SELECTOR_A}")),None);
    assert_eq!(requested_id(&format!("store_id={SELECTOR_A}x")),None);
}
#[test]
fn marker_uses_reserved_zero_bytes_and_exact_id() {
    let id=parsed(SELECTOR_A);
    let marker=marker_sector(&id).unwrap();
    assert_eq!(&marker[..MARKER_V2.len()],MARKER_V2);
    assert!(marker[MARKER_V2.len()..ID_OFFSET].iter().all(|&b|b==0));
    assert_eq!(&marker[ID_OFFSET..ID_OFFSET+16],&id);
    assert!(marker[ID_OFFSET+16..].iter().all(|&b|b==0));
    assert!(matches_marker(&marker,&id));
}
#[test]
fn refuses_wrong_volume_id_even_with_correct_version_marker() {
    let a=parsed(SELECTOR_A);
    let b=parsed(SELECTOR_B);
    assert!(!matches_marker(&marker_sector(&b).unwrap(),&a));
}
#[test]
fn refuses_legacy_unpinned_marker() {
    let id=parsed(SELECTOR_A);
    let mut legacy=[0u8;512];
    legacy[..b"PCPOS USER STORE 1\n".len()].copy_from_slice(b"PCPOS USER STORE 1\n");
    assert!(!matches_marker(&legacy,&id));
}
#[test]
fn refuses_reserved_bytes_nonzero() {
    let id=parsed(SELECTOR_A);
    let mut marker=marker_sector(&id).unwrap();
    marker[510]^=1;
    assert!(!matches_marker(&marker,&id));
}
#[test]
fn refuses_marker_version_corruption() {
    let id=parsed(SELECTOR_A);
    let mut marker=marker_sector(&id).unwrap();
    marker[MARKER_V2.len()-2]^=1;
    assert!(!matches_marker(&marker,&id));
}
#[test]
fn marker_sector_never_accepts_unprovisioned_zero_id() {
    assert!(marker_sector(&[0u8;16]).is_none());
}
#[test]
fn copied_valid_id_marker_still_passes_this_untrusted_gate() {
    let id=parsed(SELECTOR_A);
    let wrong_disk_sector=marker_sector(&id).unwrap();
    let copied_wrong_disk_sector=wrong_disk_sector;
    assert!(matches_marker(&copied_wrong_disk_sector,&id));
    // This is the *known weakness*: copied metadata is not authenticated.
}
