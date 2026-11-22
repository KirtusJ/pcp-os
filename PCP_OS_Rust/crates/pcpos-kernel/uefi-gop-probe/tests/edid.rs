//! Copy-only, synthetic EDID 1.3/1.4 decoder controls.
//! No UEFI protocol, real monitor or GOP association is tested here.

use pcpos_gop_probe::edid::{parse_preferred, Error, EDID_BLOCK_BYTES, MAX_EXTENSIONS};

const FULL_HD_60_DTD: [u8; 18] = [
    0x02, 0x3a, // 148.50 MHz = 14850*10 kHz
    0x80, 0x18, 0x71, // 1920 active, 280 horizontal blanking
    0x38, 0x2d, 0x40, // 1080 active, 45 vertical blanking
    0x58, 0x2c, 0x45, 0x00, 0xfd, 0x1e, 0x11, 0x00, 0x00, 0x1e,
];

fn checksum(block: &mut [u8; EDID_BLOCK_BYTES]) {
    let sum = block[..127].iter().fold(0u8, |a, x| a.wrapping_add(*x));
    block[127] = 0u8.wrapping_sub(sum);
}
fn base() -> [u8; EDID_BLOCK_BYTES] {
    let mut block = [0u8; EDID_BLOCK_BYTES];
    block[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    block[8] = 0x4c;
    block[9] = 0x2d;
    block[10] = 0x78;
    block[11] = 0x56;
    block[18] = 1;
    block[19] = 4;
    // For version 1.4, bit 1 is *not* the preferred-timing-presence bit.
    block[24] = 0;
    block[54..72].copy_from_slice(&FULL_HD_60_DTD);
    checksum(&mut block);
    block
}

#[test]
fn accepts_v14_preferred_even_if_feature_bit_one_is_unset() {
    let report = parse_preferred(&base()).unwrap();
    assert_eq!(report.revision, 4);
    assert_eq!(report.extension_count, 0);
    assert_eq!(report.preferred.horizontal_active, 1920);
    assert_eq!(report.preferred.vertical_active, 1080);
    assert_eq!(report.preferred.horizontal_blank, 280);
    assert_eq!(report.preferred.vertical_blank, 45);
    assert_eq!(report.preferred.pixel_clock_10khz, 14850);
    assert_eq!(report.preferred.total_dimensions(), (2200, 1125));
    assert!(!report.preferred.interlaced);
    assert_eq!(report.manufacturer_raw, 0x4c2d);
    assert_eq!(report.product_code_raw, 0x5678);
}

#[test]
fn v13_preference_bit_one_is_required() {
    let mut edid = base();
    edid[19] = 3;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::NoDeclaredPreferredTiming));
    edid[24] = 0x02;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid).unwrap().revision, 3);
}

#[test]
fn v14_does_not_misread_native_pixel_format_bit_as_preferred_presence() {
    let mut edid = base();
    edid[24] |= 0x02;
    checksum(&mut edid);
    let a = parse_preferred(&edid).unwrap();
    edid[24] &= !0x02;
    checksum(&mut edid);
    let b = parse_preferred(&edid).unwrap();
    assert_eq!(a.preferred, b.preferred);
}

#[test]
fn dimensional_comparison_is_only_dimensions_and_cannot_select_gop() {
    let preferred = parse_preferred(&base()).unwrap().preferred;
    assert!(preferred.matches_dimensions_only(1920, 1080));
    assert!(!preferred.matches_dimensions_only(1280, 720));
    assert!(!preferred.matches_dimensions_only(1920, 1200));
}

#[test]
fn rejects_bad_header_even_when_checksum_is_recomputed() {
    let mut edid = base();
    edid[1] = 0;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::BadHeader));
}

#[test]
fn rejects_bad_base_checksum() {
    let mut edid = base();
    edid[15] ^= 0x40;
    assert_eq!(parse_preferred(&edid), Err(Error::BadChecksum { block_index: 0 }));
}

#[test]
fn accepts_advertised_extension_only_with_full_checksum_verified_bytes() {
    let mut base = base();
    base[126] = 1;
    checksum(&mut base);
    let mut full = Vec::from(base);
    let mut extension = [0u8; EDID_BLOCK_BYTES];
    extension[0] = 0x02;
    extension[1] = 0x03;
    extension[2] = 4;
    checksum(&mut extension);
    full.extend_from_slice(&extension);
    let result = parse_preferred(&full).unwrap();
    assert_eq!(result.extension_count, 1);
    assert_eq!(result.preferred.pixel_clock_10khz, 14850);
    // No CTA/extension mode is inferred or used to override this value.
}

#[test]
fn rejects_base_only_when_extension_is_advertised() {
    let mut edid = base();
    edid[126] = 1;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::WrongLength));
}

#[test]
fn rejects_corrupt_extension_checksum() {
    let mut edid = base();
    edid[126] = 1;
    checksum(&mut edid);
    let mut full = Vec::from(edid);
    let mut ext = [0u8; EDID_BLOCK_BYTES];
    ext[0] = 0x02; // invalid checksum, deliberately never fixed
    full.extend_from_slice(&ext);
    assert_eq!(parse_preferred(&full), Err(Error::BadChecksum { block_index: 1 }));
}

#[test]
fn rejects_truncated_advertised_extension_block() {
    let mut edid = base();
    edid[126] = 1;
    checksum(&mut edid);
    let mut full = Vec::from(edid);
    full.extend_from_slice(&[0u8; 64]);
    assert_eq!(parse_preferred(&full), Err(Error::WrongLength));
}

#[test]
fn rejects_trailing_undeclared_full_block_even_with_good_checksum() {
    let mut full = Vec::from(base());
    full.extend_from_slice(&[0u8; EDID_BLOCK_BYTES]);
    assert_eq!(parse_preferred(&full), Err(Error::WrongLength));
}

#[test]
fn rejects_extension_count_above_documented_bounded_profile() {
    let mut edid = base();
    edid[126] = (MAX_EXTENSIONS + 1) as u8;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::TooManyExtensions));
}

#[test]
fn accepts_exactly_maximum_extensions_subject_to_checksum_only() {
    let mut edid = base();
    edid[126] = MAX_EXTENSIONS as u8;
    checksum(&mut edid);
    let mut full = Vec::from(edid);
    for _ in 0..MAX_EXTENSIONS {
        full.extend_from_slice(&[0u8; EDID_BLOCK_BYTES]);
    }
    let result = parse_preferred(&full).unwrap();
    assert_eq!(usize::from(result.extension_count), MAX_EXTENSIONS);
}

#[test]
fn rejects_non_block_multiple_input() {
    let mut full = Vec::from(base());
    full.push(0);
    assert_eq!(parse_preferred(&full), Err(Error::WrongLength));
}

#[test]
fn rejects_unsupported_edid_major_version() {
    let mut edid = base();
    edid[18] = 2;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::UnsupportedVersion));
}

#[test]
fn rejects_unsupported_old_or_future_minor_revision() {
    for revision in [0u8, 1u8, 2u8, 5u8] {
        let mut edid = base();
        edid[19] = revision;
        checksum(&mut edid);
        assert_eq!(parse_preferred(&edid), Err(Error::UnsupportedVersion));
    }
}

#[test]
fn rejects_missing_preferred_first_dtd_even_if_other_descriptor_exists() {
    let mut edid = base();
    edid[54..72].fill(0);
    edid[72..90].copy_from_slice(&FULL_HD_60_DTD);
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::MissingPreferredDetailedTiming));
}

#[test]
fn rejects_zero_horizontal_active_from_first_dtd() {
    let mut edid = base();
    edid[56] = 0;
    edid[58] &= 0x0f;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::InvalidPreferredDetailedTiming));
}

#[test]
fn rejects_zero_vertical_active_from_first_dtd() {
    let mut edid = base();
    edid[59] = 0;
    edid[61] &= 0x0f;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::InvalidPreferredDetailedTiming));
}

#[test]
fn rejects_zero_horizontal_blanking_from_first_dtd() {
    let mut edid = base();
    edid[57] = 0;
    edid[58] &= 0xf0;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::InvalidPreferredDetailedTiming));
}

#[test]
fn rejects_zero_vertical_blanking_from_first_dtd() {
    let mut edid = base();
    edid[60] = 0;
    edid[61] &= 0xf0;
    checksum(&mut edid);
    assert_eq!(parse_preferred(&edid), Err(Error::InvalidPreferredDetailedTiming));
}

#[test]
fn interlace_bit_is_preserved_without_claiming_progressive_dimensions() {
    let mut edid = base();
    edid[71] |= 0x80; // DTD flags byte 17
    checksum(&mut edid);
    let report = parse_preferred(&edid).unwrap();
    assert!(report.preferred.interlaced);
    assert_eq!(report.preferred.vertical_active, 1080);
    // A genuine interlaced mode needs a separate timing policy.
}

#[test]
fn minimum_input_is_one_complete_block_not_zero_or_seven_bytes() {
    assert_eq!(parse_preferred(&[]), Err(Error::WrongLength));
    assert_eq!(parse_preferred(&[0; 7]), Err(Error::WrongLength));
    assert_eq!(parse_preferred(&[0; 127]), Err(Error::WrongLength));
}
