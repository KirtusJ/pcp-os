//! Host-side ABI and pure-value tests. Nothing here invokes firmware.
use core::{ffi::c_void, mem::{offset_of, size_of}, ptr};
use pcpos_gop_probe::{
    BootServices, GraphicsOutput, GraphicsOutputMode, Guid, ModeInformation,
    PixelBitmask, SimpleTextOutput, Snapshot, SystemTable, BS_FREE_POOL,
    BS_LOCATE_PROTOCOL, GOP_GUID, observed, report_changed,
    safe_alternate_candidate,
};

const RGB32: ModeInformation = ModeInformation {
    version: 0, horizontal_resolution: 1280, vertical_resolution: 720,
    pixel_format: 1,
    pixel_information: PixelBitmask { red: 0, green: 0, blue: 0, reserved: 0 },
    pixels_per_scan_line: 1280,
};
fn mode(info: &ModeInformation) -> GraphicsOutputMode {
    GraphicsOutputMode {
        max_mode: 8, mode: 2,
        info,
        size_of_info: size_of::<ModeInformation>(),
        framebuffer_base: 0x1_0000_0000,
        framebuffer_size: 1280 * 720 * 4,
    }
}
#[test]
fn exact_uefi_table_layout_on_x86_64() {
    assert_eq!(size_of::<Guid>(), 16);
    assert_eq!(size_of::<pcpos_gop_probe::TableHeader>(), 24);
    assert_eq!(size_of::<SimpleTextOutput>(), 16);
    assert_eq!(offset_of!(SystemTable, con_out), 64);
    assert_eq!(offset_of!(SystemTable, boot_services), 96);
    assert_eq!(size_of::<SystemTable>(), 120);
    assert_eq!(offset_of!(BootServices, function_slots), 24);
    assert_eq!(BS_FREE_POOL, 6);
    assert_eq!(BS_LOCATE_PROTOCOL, 37);
    assert_eq!(24 + BS_LOCATE_PROTOCOL * 8, 320);
    assert_eq!(size_of::<BootServices>(), 328);
}
#[test]
fn exact_gop_wire_layout_on_x86_64() {
    assert_eq!(size_of::<PixelBitmask>(), 16);
    assert_eq!(size_of::<ModeInformation>(), 36);
    assert_eq!(offset_of!(ModeInformation, pixel_information), 16);
    assert_eq!(offset_of!(ModeInformation, pixels_per_scan_line), 32);
    assert_eq!(size_of::<GraphicsOutputMode>(), 40);
    assert_eq!(offset_of!(GraphicsOutputMode, framebuffer_base), 24);
    assert_eq!(offset_of!(GraphicsOutputMode, framebuffer_size), 32);
    assert_eq!(size_of::<GraphicsOutput>(), 32);
    assert_eq!(offset_of!(GraphicsOutput, mode), 24);
}
#[test]
fn official_gop_guid_bytes_are_preserved() {
    assert_eq!(GOP_GUID.data1, 0x9042a9de);
    assert_eq!(GOP_GUID.data2, 0x23dc);
    assert_eq!(GOP_GUID.data3, 0x4a38);
    assert_eq!(GOP_GUID.data4, [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a]);
}
#[test]
fn accepts_64_bit_framebuffer_and_explicit_pixel_stride() {
    let info = ModeInformation { pixels_per_scan_line: 1408, ..RGB32 };
    let m = mode(&info);
    let o = observed(&m, &info).unwrap();
    assert_eq!(o.framebuffer_base, 0x1_0000_0000);
    assert_eq!(o.pitch_pixels, 1408);
    assert_eq!(o.width, 1280);
}
#[test]
fn refuses_unknown_info_revision_instead_of_guessing() {
    let info = ModeInformation { version: 2, ..RGB32 };
    assert!(observed(&mode(&info), &info).is_none());
}
#[test]
fn refuses_mode_outside_maxmode() {
    let mut m = mode(&RGB32);
    m.mode = m.max_mode;
    assert!(observed(&m, &RGB32).is_none());
}
#[test]
fn refuses_linear_pitch_smaller_than_width() {
    let info = ModeInformation { pixels_per_scan_line: 1279, ..RGB32 };
    assert!(observed(&mode(&info), &info).is_none());
}
#[test]
fn permits_pixelbltonly_without_linear_stride_but_does_not_imply_writability() {
    let info = ModeInformation { pixel_format: 3, pixels_per_scan_line: 0, ..RGB32 };
    let mut m = mode(&info);
    m.framebuffer_base = 0;
    m.framebuffer_size = 0;
    assert_eq!(observed(&m, &info).unwrap().pixel_format, 3);
}
#[test]
fn never_recommends_current_index_for_optional_probe() {
    assert!(!safe_alternate_candidate(2, 8, 2, RGB32));
}
#[test]
fn never_recommends_unavailable_or_unknown_mode_version() {
    assert!(!safe_alternate_candidate(2, 3, 4, RGB32));
    assert!(!safe_alternate_candidate(2, 8, 4, ModeInformation { version: 1, ..RGB32 }));
}
#[test]
fn accepts_valid_distinct_fixture_without_claiming_edid() {
    assert!(safe_alternate_candidate(2, 8, 4, RGB32));
}
#[test]
fn recognizes_mode_metadata_changes_separately_from_index_changes() {
    let m = mode(&RGB32);
    let first = observed(&m, &RGB32).unwrap();
    let second = Snapshot { framebuffer_base: first.framebuffer_base + 0x1000, ..first };
    assert!(report_changed(first, second));
    assert!(!report_changed(first, first));
}
