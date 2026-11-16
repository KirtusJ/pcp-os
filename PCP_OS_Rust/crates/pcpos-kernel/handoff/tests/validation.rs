//! Positive and adverse controls for a proposed UEFI-to-kernel data contract.
//! These tests validate *values only*, not pointer ownership, EFI calls, a boot,
//! the physical framebuffer, or successful installation.

use pcpos_boot_handoff::{
    validate, Display, Error, Framebuffer, Handoff, MemoryRegion, Module,
    DISPLAY_BLT_ONLY, DISPLAY_LINEAR, MAGIC, PIXEL_BGRX8, PIXEL_BITMASK,
    PIXEL_BLT_ONLY, PIXEL_RGBX8, VERSION,
};
use core::mem::size_of;

const LIMIT: u64 = 1 << 48;

fn fixture() -> (Handoff, [MemoryRegion; 2], [Module; 1]) {
    let regions = [
        MemoryRegion { physical_start: 0, page_count: 256, firmware_type: 0,
                       reserved: 0, attributes: 0 },
        MemoryRegion { physical_start: 0x100000, page_count: 2048, firmware_type: 7,
                       reserved: 0, attributes: 0 },
    ];
    let modules = [
        Module {
            physical_start: 0x2000000,
            length_bytes: 0x10000,
            name_physical: 0x2100000,
            name_length: 5,
            kind: 1,
            sha256: [0x19; 32],
        }
    ];
    let header = Handoff {
        magic: MAGIC,
        version: VERSION,
        record_bytes: size_of::<Handoff>() as u32,
        flags: 0,
        kernel_physical: 0x100000,
        kernel_length: 0x200000,
        map_physical: 0x800000,
        map_count: 2,
        map_stride: size_of::<MemoryRegion>() as u32,
        module_table_physical: 0x900000,
        module_count: 1,
        module_stride: size_of::<Module>() as u32,
        command_line_physical: 0,
        command_line_length: 0,
        reserved: 0,
        framebuffer: Framebuffer::default(),
    };
    (header, regions, modules)
}

fn bgr(width: u32, height: u32, stride_bytes: u32) -> Framebuffer {
    Framebuffer {
        state: DISPLAY_LINEAR,
        pixel_format: PIXEL_BGRX8,
        physical_start: 0x1_2000_0000,  // intentionally above 4 GiB
        length_bytes: u64::from(height) * u64::from(stride_bytes),
        width,
        height,
        stride_bytes,
        ..Framebuffer::default()
    }
}

#[test]
fn fixed_wire_sizes_are_visible() {
    // Compile-time ABI changes must require a new version, not reinterpret old data.
    assert_eq!(size_of::<MemoryRegion>(), 32);
    assert_eq!(size_of::<Module>(), 64);
    assert_eq!(size_of::<Framebuffer>(), 56);
    assert_eq!(size_of::<Handoff>(), 144);
}

#[test]
fn no_display_is_explicit() {
    let (h, regions, modules) = fixture();
    let checked = validate(&h, &regions, &modules, LIMIT).unwrap();
    assert_eq!(checked.display, Display::Absent);
    assert_eq!(checked.regions, 2);
    assert_eq!(checked.modules, 1);
}

#[test]
fn accepts_bgrx_above_four_gib() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1920, 1080, 7680);
    let checked = validate(&h, &regions, &modules, LIMIT).unwrap();
    assert!(matches!(checked.display, Display::Linear(f) if f.physical_start > u32::MAX as u64));
}

#[test]
fn accepts_padded_stride_not_width_times_four() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1600, 900, 8192);
    assert!(validate(&h, &regions, &modules, LIMIT).is_ok());
}

#[test]
fn accepts_rgbx_without_conflating_channel_order() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1280, 720, 5120);
    h.framebuffer.pixel_format = PIXEL_RGBX8;
    assert!(matches!(validate(&h, &regions, &modules, LIMIT).unwrap().display,
                     Display::Linear(f) if f.pixel_format == PIXEL_RGBX8));
}

#[test]
fn accepts_valid_disjoint_8_bit_masks() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1280, 720, 5120);
    h.framebuffer.pixel_format = PIXEL_BITMASK;
    h.framebuffer.red_mask = 0x00ff_0000;
    h.framebuffer.green_mask = 0x0000_ff00;
    h.framebuffer.blue_mask = 0x0000_00ff;
    h.framebuffer.reserved_mask = 0xff00_0000;
    assert!(validate(&h, &regions, &modules, LIMIT).is_ok());
}

#[test]
fn blt_only_exposes_no_linear_framebuffer() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = Framebuffer {
        state: DISPLAY_BLT_ONLY, pixel_format: PIXEL_BLT_ONLY,
        width: 1024, height: 768, ..Framebuffer::default()
    };
    assert_eq!(validate(&h, &regions, &modules, LIMIT).unwrap().display,
               Display::BltOnly { width: 1024, height: 768 });
}

#[test]
fn rejects_nonzero_fields_for_absent_display() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer.width = 1;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayState));
}

#[test]
fn rejects_unknown_display_state() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer.state = 9;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayState));
}

#[test]
fn rejects_zero_display_dimensions() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(0, 900, 4096);
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayDimensions));
}

#[test]
fn rejects_narrow_stride() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1920, 1080, 4096);
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayStride));
}

#[test]
fn rejects_unaligned_stride() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(100, 100, 403);
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayStride));
}

#[test]
fn rejects_short_framebuffer_mapping() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1920, 1080, 7680);
    h.framebuffer.length_bytes -= 1;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayLength));
}

#[test]
fn rejects_framebuffer_overflow_of_cpu_physical_limit() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1920, 1080, 7680);
    h.framebuffer.physical_start = LIMIT - 4096;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::SpanOutOfRange));
}

#[test]
fn rejects_renderer_unknown_pixel_format() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1920, 1080, 7680);
    h.framebuffer.pixel_format = 77;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayFormat));
}

#[test]
fn rejects_bitmask_overlap() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1280, 720, 5120);
    h.framebuffer.pixel_format = PIXEL_BITMASK;
    h.framebuffer.red_mask = 0xff;
    h.framebuffer.green_mask = 0xff;
    h.framebuffer.blue_mask = 0xff00;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayMasks));
}

#[test]
fn rejects_unsupported_five_six_five_bitmask_without_guessing() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1280, 720, 5120);
    h.framebuffer.pixel_format = PIXEL_BITMASK;
    h.framebuffer.red_mask = 0xf800;
    h.framebuffer.green_mask = 0x07e0;
    h.framebuffer.blue_mask = 0x001f;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayMasks));
}

#[test]
fn rejects_unexpected_masks_for_packed_format() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(1280, 720, 5120);
    h.framebuffer.red_mask = 0xff;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidDisplayMasks));
}

#[test]
fn rejects_bad_magic_and_version() {
    let (mut h, regions, modules) = fixture();
    h.magic = 0;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::InvalidMagic));
    h.magic = MAGIC;
    h.version = 2;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::UnsupportedVersion));
}

#[test]
fn rejects_wrong_header_size_and_reserved_bits() {
    let (mut h, regions, modules) = fixture();
    h.record_bytes = 0;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::WrongRecordLength));
    h.record_bytes = size_of::<Handoff>() as u32;
    h.flags = 1;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::ReservedField));
}

#[test]
fn rejects_bogus_physical_limit() {
    let (h, regions, modules) = fixture();
    assert_eq!(validate(&h, &regions, &modules, 12345), Err(Error::InvalidPhysicalLimit));
}

#[test]
fn accepts_kernel_handoff_above_four_gib_with_no_graphics() {
    let (mut h, regions, modules) = fixture();
    h.kernel_physical = 0x1_0100_0000;
    let checked = validate(&h, &regions, &modules, LIMIT).unwrap();
    assert!(checked.kernel_end > u32::MAX as u64);
}

#[test]
fn rejects_invalid_kernel_extent() {
    let (mut h, regions, modules) = fixture();
    h.kernel_length = 0;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::KernelRange));
}

#[test]
fn rejects_memory_map_count_mismatch() {
    let (mut h, regions, modules) = fixture();
    h.map_count = 1;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::MissingOrTooManyRegions));
}

#[test]
fn rejects_wrong_memory_descriptor_stride() {
    let (mut h, regions, modules) = fixture();
    h.map_stride = 40;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::MemoryMapMetadata));
}

#[test]
fn rejects_misaligned_region() {
    let (h, mut regions, modules) = fixture();
    regions[1].physical_start += 1;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::MemoryRegion));
}

#[test]
fn rejects_overlapping_regions() {
    let (h, mut regions, modules) = fixture();
    regions[1].physical_start = 0xF0000;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::MemoryRegionOrder));
}

#[test]
fn rejects_bad_module_metadata() {
    let (mut h, regions, modules) = fixture();
    h.module_stride = 1;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::ModuleTableMetadata));
}

#[test]
fn rejects_zero_length_module() {
    let (h, regions, mut modules) = fixture();
    modules[0].length_bytes = 0;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::ModuleRange));
}

#[test]
fn rejects_module_overlapping_kernel() {
    let (h, regions, mut modules) = fixture();
    modules[0].physical_start = h.kernel_physical;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::RangeOverlap));
}

#[test]
fn rejects_module_name_overlapping_memory_map() {
    let (h, regions, mut modules) = fixture();
    modules[0].name_physical = h.map_physical;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::RangeOverlap));
}

#[test]
fn accepts_empty_module_table_when_explicitly_absent() {
    let (mut h, regions, _modules) = fixture();
    h.module_count = 0;
    h.module_stride = 0;
    h.module_table_physical = 0;
    assert!(validate(&h, &regions, &[], LIMIT).is_ok());
}

#[test]
fn rejects_stale_module_table_pointer_when_empty() {
    let (mut h, regions, _modules) = fixture();
    h.module_count = 0;
    h.module_stride = 0;
    assert_eq!(validate(&h, &regions, &[], LIMIT), Err(Error::ModuleTableMetadata));
}

#[test]
fn accepts_valid_bounded_command_line_span() {
    let (mut h, regions, modules) = fixture();
    h.command_line_physical = 0xA00000;
    h.command_line_length = 80;
    assert!(validate(&h, &regions, &modules, LIMIT).is_ok());
}

#[test]
fn rejects_stale_command_line_pointer_for_empty_text() {
    let (mut h, regions, modules) = fixture();
    h.command_line_physical = 0xA00000;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::CommandLineMetadata));
}

#[test]
fn rejects_too_large_command_line() {
    let (mut h, regions, modules) = fixture();
    h.command_line_physical = 0xA00000;
    h.command_line_length = 5000;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::CommandLineMetadata));
}

#[test]
fn rejects_overlap_of_command_line_and_module_table() {
    let (mut h, regions, modules) = fixture();
    h.command_line_physical = h.module_table_physical;
    h.command_line_length = 80;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::RangeOverlap));
}

#[test]
fn rejects_overflow_of_module_memory() {
    let (h, regions, mut modules) = fixture();
    modules[0].physical_start = u64::MAX - 8;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::ModuleRange));
}

#[test]
fn rejects_writing_framebuffer_into_kernel() {
    let (mut h, regions, modules) = fixture();
    h.framebuffer = bgr(100, 100, 400);
    h.framebuffer.physical_start = h.kernel_physical;
    assert_eq!(validate(&h, &regions, &modules, LIMIT), Err(Error::RangeOverlap));
}
