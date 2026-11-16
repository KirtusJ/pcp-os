//! Source-authored positive/adverse Multiboot2 EFI-amd64 information fixtures.
//! Tests do NOT prove that GRUB, ExitBootServices or a kernel actually ran.
use pcpos_mb2_inspect::{
    inspect_efi_amd64, Error, PixelKind, ENTRY_MAGIC, MAX_TAGS,
};

const LOW: u64 = 0x0010_0000;

fn tag(ty: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&(8u32 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    while out.len() % 8 != 0 {
        out.push(0);
    }
    out
}
fn mbi(tags: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; 8];
    for t in tags {
        out.extend_from_slice(t);
    }
    out.extend_from_slice(&tag(0, &[]));
    let total = out.len() as u32;
    out[..4].copy_from_slice(&total.to_le_bytes());
    out
}
fn inspect<'a>(raw: &'a [u8]) -> Result<pcpos_mb2_inspect::Inspection<'a>, Error> {
    inspect_efi_amd64(raw, LOW, ENTRY_MAGIC)
}
fn rgb(base: u64, pitch: u32, width: u32, height: u32, bpp: u8, chan: [u8; 6]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&base.to_le_bytes());
    data.extend_from_slice(&pitch.to_le_bytes());
    data.extend_from_slice(&width.to_le_bytes());
    data.extend_from_slice(&height.to_le_bytes());
    data.push(bpp);
    data.push(1);
    data.extend_from_slice(&[0, 0]); // must be ignored, per spec
    data.extend_from_slice(&chan);
    tag(8, &data)
}
fn bgrx(base: u64) -> Vec<u8> {
    rgb(base, 8192, 1920, 1080, 32, [16, 8, 8, 8, 0, 8])
}
fn module(start: u32, end: u32, command: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&start.to_le_bytes());
    data.extend_from_slice(&end.to_le_bytes());
    data.extend_from_slice(command);
    tag(3, &data)
}
fn mm_entry(base: u64, length: u64, kind: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&base.to_le_bytes());
    d.extend_from_slice(&length.to_le_bytes());
    d.extend_from_slice(&kind.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d
}
fn mmap(entry_bytes: u32, version: u32, entries: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&entry_bytes.to_le_bytes());
    data.extend_from_slice(&version.to_le_bytes());
    data.extend_from_slice(entries);
    tag(6, &data)
}
fn efi_mmap(entry_bytes: u32, version: u32, entries: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&entry_bytes.to_le_bytes());
    data.extend_from_slice(&version.to_le_bytes());
    data.extend_from_slice(entries);
    tag(17, &data)
}

#[test]
fn empty_information_is_valid_but_proves_no_boot_services_state() {
    let data = mbi(&[]);
    let result = inspect(&data).unwrap();
    assert!(result.framebuffer.is_none());
    assert!(result.efi_memory_map.is_none());
    assert!(!result.efi_boot_services_not_terminated);
    assert_eq!(result.tag_count, 0);
}

#[test]
fn valid_rgb_framebuffer_above_four_gib_has_distinct_pitch() {
    let data = mbi(&[bgrx(0x1_1000_0000)]);
    let result = inspect(&data).unwrap();
    let fb = result.framebuffer.unwrap();
    assert!(fb.physical_base > u32::MAX as u64);
    assert_eq!(fb.pitch_bytes, 8192);
    assert_eq!(fb.width, 1920);
    assert_eq!(fb.height, 1080);
    assert!(fb.matches_bgrx32());
    assert!(!fb.matches_rgbx32());
}

#[test]
fn same_high_framebuffer_with_reversed_channels_is_not_bgrx() {
    let tag = rgb(0x1_1000_0000, 8192, 1920, 1080, 32, [0, 8, 8, 8, 16, 8]);
    let data = mbi(&[tag]);
    let result = inspect(&data).unwrap();
    let fb = result.framebuffer.unwrap();
    assert!(!fb.matches_bgrx32());
    assert!(fb.matches_rgbx32());
}

#[test]
fn valid_rgb565_is_preserved_as_non_bgrx() {
    let tag = rgb(0xB8000, 2560, 1280, 720, 16, [11, 5, 5, 6, 0, 5]);
    let data = mbi(&[tag]);
    let result = inspect(&data).unwrap();
    let fb = result.framebuffer.unwrap();
    assert_eq!(fb.kind, PixelKind::DirectRgb);
    assert_eq!(fb.bits_per_pixel, 16);
    assert!(!fb.matches_bgrx32());
}

#[test]
fn unsupported_unknown_framebuffer_subtype_is_not_implicitly_absent() {
    let mut t = bgrx(0x200000);
    t[8 + 8 + 4 + 4 + 4 + 1] = 99;
    let data = mbi(&[t]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.framebuffer.unwrap().kind, PixelKind::Unknown(99));
}

#[test]
fn palette_framebuffer_is_present_but_not_direct_rgb() {
    let mut data = Vec::new();
    data.extend_from_slice(&0xB0000u64.to_le_bytes());
    data.extend_from_slice(&640u32.to_le_bytes());
    data.extend_from_slice(&640u32.to_le_bytes());
    data.extend_from_slice(&480u32.to_le_bytes());
    data.extend_from_slice(&[8, 0, 0, 0]);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&[255, 0, 0, 0, 255, 0]);
    let data = mbi(&[tag(8, &data)]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.framebuffer.unwrap().kind, PixelKind::Indexed);
}

#[test]
fn ega_text_width_and_height_are_not_pixels() {
    let mut data = Vec::new();
    data.extend_from_slice(&0xB8000u64.to_le_bytes());
    data.extend_from_slice(&160u32.to_le_bytes());
    data.extend_from_slice(&80u32.to_le_bytes());
    data.extend_from_slice(&25u32.to_le_bytes());
    data.extend_from_slice(&[16, 2, 0, 0]);
    let data = mbi(&[tag(8, &data)]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.framebuffer.unwrap().kind, PixelKind::Text);
}

#[test]
fn accepts_efi_memory_map_and_basic_map_as_distinct_sources() {
    let memory = mm_entry(0, 640 * 1024, 2);
    let data = mbi(&[
        mmap(24, 0, &memory),
        efi_mmap(48, 1, &[0; 48]),
    ]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.basic_memory_map.unwrap().entry_bytes, 24);
    assert_eq!(result.efi_memory_map.unwrap().entry_bytes, 48);
    assert_eq!(result.efi_memory_map.unwrap().entry_version, 1);
}

#[test]
fn efi_boot_services_tag_reports_not_exited() {
    let data = mbi(&[tag(18, &[])]);
    let result = inspect(&data).unwrap();
    assert!(result.efi_boot_services_not_terminated);
    assert!(result.efi_memory_map.is_none());
}

#[test]
fn command_line_and_module_use_nul_delimited_bytes() {
    let data = mbi(&[
        tag(1, b"auto=probe\0"),
        module(0x400000, 0x500000, b"probe\0"),
    ]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.command_line, Some(&b"auto=probe"[..]));
    assert_eq!(result.module_count, 1);
    assert_eq!(result.modules[0].unwrap().command, b"probe");
}

#[test]
fn efi_system_table_pointer_is_64_bit_without_map_authority() {
    let data = mbi(&[tag(12, &0x1_0000_1000u64.to_le_bytes())]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.efi64_system_table, Some(0x1_0000_1000));
    assert!(result.efi_memory_map.is_none());
}

#[test]
fn elf_header_tags_are_not_mb2_information_tags() {
    // Byte pattern for an EFI image HEADER tag type 9 is not evidence of
    // the separate OS-image header. The information parser just skips it.
    let data = mbi(&[tag(9, &[0x11; 12])]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.unknown_tag_count, 1);
}

#[test]
fn reject_bad_magic() {
    assert_eq!(inspect_efi_amd64(&mbi(&[]), LOW, 0), Err(Error::BadEntryMagic));
}

#[test]
fn reject_mbi_above_four_gib_even_if_framebuffer_may_be_high() {
    let raw = mbi(&[bgrx(0x1_1000_0000)]);
    assert_eq!(inspect_efi_amd64(&raw, 0x1_0000_0000, ENTRY_MAGIC),
               Err(Error::BadLocation));
}

#[test]
fn reject_unaligned_mbi_physical_address() {
    let raw = mbi(&[]);
    assert_eq!(inspect_efi_amd64(&raw, LOW + 1, ENTRY_MAGIC), Err(Error::BadLocation));
}

#[test]
fn reject_mbi_crossing_exclusive_four_gib_limit() {
    let raw = mbi(&[]);
    assert_eq!(inspect_efi_amd64(&raw, 0xffff_fff8, ENTRY_MAGIC), Err(Error::BadLocation));
}

#[test]
fn reject_total_size_mismatch() {
    let mut raw = mbi(&[]);
    raw[..4].copy_from_slice(&999u32.to_le_bytes());
    assert_eq!(inspect(&raw), Err(Error::BadTotal));
}

#[test]
fn reject_truncated_tag() {
    let mut raw = mbi(&[tag(1, b"x\0")]);
    // First information tag is at offset 8; an oversized length must fail.
    raw[12..16].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
    assert_eq!(inspect(&raw), Err(Error::MalformedTag));
}

#[test]
fn reject_zero_sized_tag_without_looping() {
    let mut raw = mbi(&[tag(33, &[])]);
    raw[12..16].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(inspect(&raw), Err(Error::MalformedTag));
}

#[test]
fn reject_missing_end_tag() {
    let mut raw = mbi(&[]);
    raw[8..12].copy_from_slice(&22u32.to_le_bytes());
    assert_eq!(inspect(&raw), Err(Error::MissingEnd));
}

#[test]
fn reject_duplicate_framebuffers() {
    let t = bgrx(0x100000);
    assert_eq!(inspect(&mbi(&[t.clone(), t])), Err(Error::DuplicateTag));
}

#[test]
fn reject_duplicate_efi_boot_services_tag() {
    assert_eq!(inspect(&mbi(&[tag(18, &[]), tag(18, &[])])),
               Err(Error::DuplicateTag));
}

#[test]
fn reject_rgb_overlap() {
    let tag = rgb(0xB8000, 7680, 1920, 1080, 32, [16, 8, 16, 8, 0, 8]);
    assert_eq!(inspect(&mbi(&[tag])), Err(Error::BadRgbFields));
}

#[test]
fn reject_rgb_field_past_bpp() {
    let tag = rgb(0xB8000, 7680, 1920, 1080, 32, [28, 8, 8, 8, 0, 8]);
    assert_eq!(inspect(&mbi(&[tag])), Err(Error::BadRgbFields));
}

#[test]
fn reject_invalid_row_pitch_even_when_width_is_valid() {
    let tag = rgb(0xB8000, 6000, 1920, 1080, 32, [16, 8, 8, 8, 0, 8]);
    assert_eq!(inspect(&mbi(&[tag])), Err(Error::BadFramebuffer));
}

#[test]
fn reject_framebuffer_address_overflow() {
    let tag = rgb(u64::MAX - 10, 7680, 1920, 1080, 32, [16, 8, 8, 8, 0, 8]);
    assert_eq!(inspect(&mbi(&[tag])), Err(Error::BadFramebuffer));
}

#[test]
fn reject_truncated_direct_rgb_color_fields() {
    let mut t = bgrx(0xB8000);
    // Omit three bytes from the direct-RGB fields and revise the size.
    let new_size = 35u32;
    t[4..8].copy_from_slice(&new_size.to_le_bytes());
    assert_eq!(inspect(&mbi(&[t])), Err(Error::BadFramebuffer));
}

#[test]
fn reject_palette_with_declared_entries_not_present() {
    let mut data = Vec::new();
    data.extend_from_slice(&0xB0000u64.to_le_bytes());
    data.extend_from_slice(&640u32.to_le_bytes());
    data.extend_from_slice(&640u32.to_le_bytes());
    data.extend_from_slice(&480u32.to_le_bytes());
    data.extend_from_slice(&[8, 0, 0, 0]);
    data.extend_from_slice(&256u32.to_le_bytes());
    assert_eq!(inspect(&mbi(&[tag(8, &data)])), Err(Error::BadFramebuffer));
}

#[test]
fn reject_invalid_memory_map_descriptor_stride() {
    let entry = mm_entry(0, 0x1000, 1);
    assert_eq!(inspect(&mbi(&[mmap(23, 0, &entry)])), Err(Error::BadMemoryMap));
}

#[test]
fn reject_memory_map_range_overflow() {
    let entry = mm_entry(u64::MAX - 10, 20, 1);
    assert_eq!(inspect(&mbi(&[mmap(24, 0, &entry)])), Err(Error::BadMemoryMap));
}

#[test]
fn reject_efi_memory_map_misaligned_descriptor_stride() {
    assert_eq!(inspect(&mbi(&[efi_mmap(41, 1, &[0u8; 41])])),
               Err(Error::BadEfiMemoryMap));
}

#[test]
fn reject_oversized_efi_memory_map_item_count() {
    // An entry-size field cannot claim just 8 bytes of EFI descriptor.
    assert_eq!(inspect(&mbi(&[efi_mmap(8, 1, &[0; 8])])),
               Err(Error::BadEfiMemoryMap));
}

#[test]
fn reject_module_without_nul_delimited_command() {
    assert_eq!(inspect(&mbi(&[module(0x400000, 0x500000, b"no-nul")])),
               Err(Error::BadString));
}

#[test]
fn reject_module_overlapping_mbi() {
    assert_eq!(inspect(&mbi(&[module(LOW as u32, LOW as u32 + 4096, b"x\0")])),
               Err(Error::BadModule));
}

#[test]
fn reject_duplicate_overlapping_modules() {
    assert_eq!(inspect(&mbi(&[
        module(0x400000, 0x500000, b"a\0"),
        module(0x4f0000, 0x600000, b"b\0"),
    ])), Err(Error::OverlappingModule));
}

#[test]
fn accept_unknown_tags_without_granting_semantics() {
    let data = mbi(&[tag(1000, &[1, 2, 3])]);
    let result = inspect(&data).unwrap();
    assert_eq!(result.unknown_tag_count, 1);
}

#[test]
fn reject_excessive_tag_count() {
    let mut tags = Vec::new();
    for _ in 0..=MAX_TAGS {
        tags.push(tag(99, &[]));
    }
    assert_eq!(inspect(&mbi(&tags)), Err(Error::TooManyTags));
}

#[test]
fn never_panic_on_short_mutated_info_slices() {
    let original = mbi(&[bgrx(0x1_1000_0000), tag(18, &[])]);
    for i in 0..original.len() {
        let _ = inspect(&original[..i]);
    }
    for i in 0..original.len() {
        let mut b = original.clone();
        b[i] ^= 0xff;
        let _ = inspect(&b);
    }
}
