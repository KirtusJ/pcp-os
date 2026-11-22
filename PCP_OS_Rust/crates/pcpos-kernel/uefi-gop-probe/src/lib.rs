#![no_std]
//! UEFI 2.11 GOP probe types, x86_64 ABI layout and pure evidence helpers.
//!
//! This crate exposes a *standalone optional firmware probe*, not the PCP OS
//! loader, an authenticated measurement, or permission to use firmware after
//! ExitBootServices. Unsafe dereferences belong in src/main.rs only.

/// Parser for copied EDID bytes only; no source/handle association or SetMode.
pub mod edid;

use core::ffi::c_void;

pub const SYSTEM_TABLE_SIGNATURE: u64 = 0x5453_5953_2049_4249;
pub const BOOT_SERVICES_SIGNATURE: u64 = 0x5652_4553_544f_4f42;
pub const GOP_GUID: Guid = Guid {
    data1: 0x9042a9de, data2: 0x23dc, data3: 0x4a38,
    data4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};
 
/// UEFI EDID discovered for one possible physical video output child handle.
pub const EDID_DISCOVERED_GUID: Guid = Guid {
    data1: 0x1c0c34f6, data2: 0xd380, data3: 0x41fa,
    data4: [0xa0, 0x49, 0x8a, 0xd0, 0x6c, 0x1a, 0x66, 0xaa],
};
/// EDID of an *active* output, potentially supplied by an override.
/// This cannot be associated with a GOP merely via global LocateProtocol.
pub const EDID_ACTIVE_GUID: Guid = Guid {
    data1: 0xbd8c1056, data2: 0x9f36, data3: 0x44ec,
    data4: [0x92, 0xa8, 0xa6, 0x33, 0x7f, 0x81, 0x79, 0x86],
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

/// UEFI EFI_EDID_DISCOVERED_PROTOCOL / EFI_EDID_ACTIVE_PROTOCOL
/// layout on x86-64. The pointer has no proven lifetime or ownership here;
/// none of the pure EDID functions dereferences it.
#[repr(C)]
pub struct EdidProtocol {
    pub size_of_edid: u32,
    pub edid: *const u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

pub type EfiStatus = usize;
pub type EfiHandle = *mut c_void;

#[repr(C)]
pub struct SimpleTextOutput {
    pub reset: unsafe extern "efiapi" fn(*mut Self, bool) -> EfiStatus,
    pub output_string: unsafe extern "efiapi" fn(*mut Self, *const u16) -> EfiStatus,
}

/// Official UEFI EFI_BOOT_SERVICES table, through LocateProtocol inclusive.
///
/// Each pointer is 8 bytes on the x86_64 EFI ABI. Index 6 is FreePool;
/// index 37 is LocateProtocol. This is not portable to 32-bit targets.
/// Layout is checked in tests with offset_of! and sizeof. We do not claim
/// that a signature+size check authenticates a firmware-provided pointer.
#[repr(C)]
pub struct BootServices {
    pub hdr: TableHeader,
    pub function_slots: [usize; 38],
}

pub const BS_FREE_POOL: usize = 6;
pub const BS_LOCATE_PROTOCOL: usize = 37;

pub type FreePoolFn = unsafe extern "efiapi" fn(*mut c_void) -> EfiStatus;
pub type LocateProtocolFn = unsafe extern "efiapi" fn(
    *const Guid, *mut c_void, *mut *mut c_void,
) -> EfiStatus;

#[repr(C)]
pub struct SystemTable {
    pub hdr: TableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: EfiHandle,
    pub con_in: *mut c_void,
    pub console_out_handle: EfiHandle,
    pub con_out: *mut SimpleTextOutput,
    pub standard_error_handle: EfiHandle,
    pub std_err: *mut SimpleTextOutput,
    pub runtime_services: *mut c_void,
    pub boot_services: *mut BootServices,
    pub number_of_table_entries: usize,
    pub configuration_table: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelBitmask {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInformation {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: u32,
    pub pixel_information: PixelBitmask,
    pub pixels_per_scan_line: u32,
}

#[repr(C)]
pub struct GraphicsOutputMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *const ModeInformation,
    pub size_of_info: usize,
    pub framebuffer_base: u64,
    pub framebuffer_size: usize,
}

#[repr(C)]
pub struct GraphicsOutput {
    pub query_mode: unsafe extern "efiapi" fn(
        *mut Self, u32, *mut usize, *mut *mut ModeInformation
    ) -> EfiStatus,
    pub set_mode: unsafe extern "efiapi" fn(*mut Self, u32) -> EfiStatus,
    pub blt: usize,
    pub mode: *mut GraphicsOutputMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub mode: u32,
    pub max_mode: u32,
    pub width: u32,
    pub height: u32,
    pub pitch_pixels: u32,
    pub pixel_format: u32,
    pub framebuffer_base: u64,
    pub framebuffer_bytes: u64,
}

pub fn observed(g: &GraphicsOutputMode, info: &ModeInformation) -> Option<Snapshot> {
    if g.max_mode == 0 || g.mode >= g.max_mode {
        return None;
    }
    if info.version != 0 || info.horizontal_resolution == 0 ||
       info.vertical_resolution == 0 || info.horizontal_resolution > 16384 ||
       info.vertical_resolution > 16384 || info.pixel_format > 3 {
        return None;
    }
    if info.pixel_format != 3 &&
        info.pixels_per_scan_line < info.horizontal_resolution {
        return None;
    }
    Some(Snapshot {
        mode: g.mode, max_mode: g.max_mode,
        width: info.horizontal_resolution,
        height: info.vertical_resolution,
        pitch_pixels: info.pixels_per_scan_line,
        pixel_format: info.pixel_format,
        framebuffer_base: g.framebuffer_base,
        framebuffer_bytes: g.framebuffer_size as u64,
    })
}

pub fn report_changed(before: Snapshot, after: Snapshot) -> bool {
    before != after
}

pub fn safe_alternate_candidate(
    original_mode: u32, max_modes: u32, mode: u32, info: ModeInformation,
) -> bool {
    mode < max_modes && mode != original_mode && info.version == 0 &&
    info.horizontal_resolution > 0 && info.vertical_resolution > 0 &&
    info.horizontal_resolution <= 16384 && info.vertical_resolution <= 16384 &&
    info.pixel_format < 4 &&
    (info.pixel_format == 3 || info.pixels_per_scan_line >= info.horizontal_resolution)
}
