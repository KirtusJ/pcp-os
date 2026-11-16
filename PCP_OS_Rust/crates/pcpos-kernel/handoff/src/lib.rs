#![no_std]
//! Versioned, pointer-free *validation* of a proposed native x86-64 firmware handoff.
//!
//! This crate does not implement an EFI loader, map physical memory, leave firmware
//! boot services, launch a kernel, authenticate an image, or parse raw firmware
//! pointers. A trusted loader must copy descriptors into checked, owned memory
//! before borrowing slices and calling `validate`.
//!
//! All wire fields are fixed-width integers. There are no Rust enum discriminants
//! or Rust references in the binary record. Unknown values are rejected, never
//! transmuted. The existing Multiboot 1 entry remains unchanged.

use core::mem::size_of;

pub const MAGIC: u64 = u64::from_le_bytes(*b"PCPHOF01");
pub const VERSION: u32 = 1;
pub const MAX_REGIONS: usize = 4096;
pub const MAX_MODULES: usize = 32;
pub const MAX_COMMAND_LINE: u32 = 4096;
const PAGE: u64 = 4096;

// These three values are the UEFI Graphics Output Protocol's direct pixel
// formats. BLT-only has no linear frame buffer after ExitBootServices.
pub const PIXEL_RGBX8: u32 = 0;
pub const PIXEL_BGRX8: u32 = 1;
pub const PIXEL_BITMASK: u32 = 2;
pub const PIXEL_BLT_ONLY: u32 = 3;

pub const DISPLAY_ABSENT: u32 = 0;
pub const DISPLAY_LINEAR: u32 = 1;
pub const DISPLAY_BLT_ONLY: u32 = 2;

/// The firmware's original memory type and attributes, deliberately not
/// reinterpreted as post-ExitBootServices allocations or grants.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryRegion {
    pub physical_start: u64,
    pub page_count: u64,
    pub firmware_type: u32,
    pub reserved: u32,
    pub attributes: u64,
}

/// A loader-owned copy of a named module. Hash bytes are an identification
/// field, NOT proof that anyone checked them or authenticated the publisher.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Module {
    pub physical_start: u64,
    pub length_bytes: u64,
    pub name_physical: u64,
    pub name_length: u32,
    pub kind: u32,
    pub sha256: [u8; 32],
}

/// GOP geometry plus an explicit absence/BLT-only outcome. For linear formats
/// `stride_bytes` may exceed width*4. Masks are zero for RGBX/BGRX; for a
/// bitmask layout they identify the actual color-channel positions. The current
/// PCP OS renderer is BGRX-specific and must not draw into other formats without
/// conversion.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Framebuffer {
    pub state: u32,
    pub pixel_format: u32,
    pub physical_start: u64,
    pub length_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub stride_bytes: u32,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub reserved_mask: u32,
    pub reserved: u32,
}

/// Proposed v1 loader-to-kernel record. `physical_*` are raw physical
/// addresses; they MUST NOT be dereferenced merely because this record passes
/// validation. The loader must pin every referenced byte against reuse, copy
/// the firmware memory map with its actual descriptor stride, and hand off only
/// after an appropriate ExitBootServices procedure. Those acts are not here.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Handoff {
    pub magic: u64,
    pub version: u32,
    pub record_bytes: u32,
    pub flags: u64,
    pub kernel_physical: u64,
    pub kernel_length: u64,
    pub map_physical: u64,
    pub map_count: u32,
    pub map_stride: u32,
    pub module_table_physical: u64,
    pub module_count: u32,
    pub module_stride: u32,
    pub command_line_physical: u64,
    pub command_line_length: u32,
    pub reserved: u32,
    pub framebuffer: Framebuffer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidMagic,
    UnsupportedVersion,
    WrongRecordLength,
    ReservedField,
    InvalidPhysicalLimit,
    MissingOrTooManyRegions,
    MemoryMapMetadata,
    MemoryRegion,
    MemoryRegionOrder,
    ModuleTableMetadata,
    ModuleRange,
    CommandLineMetadata,
    KernelRange,
    SpanOverflow,
    SpanOutOfRange,
    RangeOverlap,
    InvalidDisplayState,
    InvalidDisplayDimensions,
    InvalidDisplayStride,
    InvalidDisplayLength,
    InvalidDisplayFormat,
    InvalidDisplayMasks,
    UnsupportedPixelLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Absent,
    /// GOP `PixelBltOnly` has no physical framebuffer to map or draw into.
    BltOnly { width: u32, height: u32 },
    /// Geometry was checked, but physical mapping and actual pixel output
    /// have NOT been established.
    Linear(Framebuffer),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Validated {
    pub kernel_end: u64,
    pub regions: usize,
    pub modules: usize,
    pub display: Display,
}

fn span_end(start: u64, length: u64, limit: u64) -> Result<u64, Error> {
    let end = start.checked_add(length).ok_or(Error::SpanOverflow)?;
    if length == 0 || end > limit || start >= limit {
        return Err(Error::SpanOutOfRange);
    }
    Ok(end)
}

fn span_end_nonzero(start: u64, length: u64, limit: u64) -> Result<u64, Error> {
    if start == 0 {
        return Err(Error::SpanOutOfRange);
    }
    span_end(start, length, limit)
}

fn intersects(a: u64, b: u64, c: u64, d: u64) -> bool {
    a < d && c < b
}

fn byte_mask(m: u32) -> bool {
    m != 0 && m.count_ones() == 8 && (m >> m.trailing_zeros()) == 0xff
}

impl Framebuffer {
    pub fn checked(&self, physical_limit: u64) -> Result<Display, Error> {
        if self.reserved != 0 {
            return Err(Error::ReservedField);
        }
        match self.state {
            DISPLAY_ABSENT => {
                if *self != Self::default() {
                    return Err(Error::InvalidDisplayState);
                }
                return Ok(Display::Absent);
            }
            DISPLAY_BLT_ONLY => {
                if self.pixel_format != PIXEL_BLT_ONLY
                    || self.width == 0 || self.height == 0
                    || self.width > 16384 || self.height > 16384
                    || self.physical_start != 0 || self.length_bytes != 0
                    || self.stride_bytes != 0
                    || self.red_mask != 0 || self.green_mask != 0
                    || self.blue_mask != 0 || self.reserved_mask != 0
                {
                    return Err(Error::InvalidDisplayState);
                }
                return Ok(Display::BltOnly { width: self.width, height: self.height });
            }
            DISPLAY_LINEAR => {}
            _ => return Err(Error::InvalidDisplayState),
        }
        if self.width == 0 || self.height == 0
            || self.width > 16384 || self.height > 16384
        {
            return Err(Error::InvalidDisplayDimensions);
        }
        match self.pixel_format {
            PIXEL_RGBX8 | PIXEL_BGRX8 => {
                if self.red_mask != 0 || self.green_mask != 0
                    || self.blue_mask != 0 || self.reserved_mask != 0
                {
                    return Err(Error::InvalidDisplayMasks);
                }
            }
            PIXEL_BITMASK => {
                let (r, g, b, x) =
                    (self.red_mask, self.green_mask, self.blue_mask, self.reserved_mask);
                if r == 0 || g == 0 || b == 0
                    || (r & g) != 0 || (r & b) != 0 || (r & x) != 0
                    || (g & b) != 0 || (g & x) != 0 || (b & x) != 0
                {
                    return Err(Error::InvalidDisplayMasks);
                }
                // Other non-overlapping UEFI BitMask formats can be valid
                // firmware modes, but this proposed v1 rasterizer profile
                // deliberately supports only three 8-bit color channels.
                if !byte_mask(r) || !byte_mask(g) || !byte_mask(b)
                    || (x != 0 && !byte_mask(x))
                {
                    return Err(Error::UnsupportedPixelLayout);
                }
            }
            _ => return Err(Error::InvalidDisplayFormat),
        }
        // Check pixel-format support *before* imposing the 32-bit row
        // contract. A valid 16-bit GOP bitmask mode must be reported as
        // unsupported, not mislabeled as a malformed 32-bit stride.
        let min_stride = self.width.checked_mul(4).ok_or(Error::SpanOverflow)?;
        if self.stride_bytes < min_stride || self.stride_bytes % 4 != 0 {
            return Err(Error::InvalidDisplayStride);
        }
        // The GOP frame-buffer size includes row padding, including the last
        // row. Do not infer stride from horizontal resolution.
        let need = u64::from(self.stride_bytes)
            .checked_mul(u64::from(self.height)).ok_or(Error::SpanOverflow)?;
        if self.length_bytes < need {
            return Err(Error::InvalidDisplayLength);
        }
        span_end_nonzero(self.physical_start, self.length_bytes, physical_limit)?;
        Ok(Display::Linear(*self))
    }
}

/// Validate value/layout invariants without dereferencing a physical address.
///
/// `regions` and `modules` are already bounded, correctly copied slices
/// provided by a *trusted* loader. Passing unrelated slices with matching
/// lengths is not detectable here. Verify pointer provenance, actual mappings,
/// ownership, descriptor source, and the ExitBootServices map key separately.
///
/// `physical_limit` must be an externally verified **exclusive** CPU physical
/// address limit. In particular, this API must not infer it from the handoff.
pub fn validate(
    handoff: &Handoff,
    regions: &[MemoryRegion],
    modules: &[Module],
    physical_limit: u64,
) -> Result<Validated, Error> {
    if handoff.magic != MAGIC { return Err(Error::InvalidMagic); }
    if handoff.version != VERSION { return Err(Error::UnsupportedVersion); }
    if handoff.record_bytes as usize != size_of::<Handoff>() {
        return Err(Error::WrongRecordLength);
    }
    if handoff.flags != 0 || handoff.reserved != 0 { return Err(Error::ReservedField); }
    if physical_limit <= PAGE || !physical_limit.is_power_of_two() {
        return Err(Error::InvalidPhysicalLimit);
    }
    let kernel_end = span_end_nonzero(
        handoff.kernel_physical, handoff.kernel_length, physical_limit
    ).map_err(|_| Error::KernelRange)?;
    if handoff.kernel_physical % PAGE != 0 || handoff.kernel_length % PAGE != 0 {
        return Err(Error::KernelRange);
    }

    if regions.is_empty() || regions.len() > MAX_REGIONS || handoff.map_count as usize != regions.len() {
        return Err(Error::MissingOrTooManyRegions);
    }
    if handoff.map_stride as usize != size_of::<MemoryRegion>() {
        return Err(Error::MemoryMapMetadata);
    }
    let map_len = u64::from(handoff.map_count)
        .checked_mul(u64::from(handoff.map_stride)).ok_or(Error::SpanOverflow)?;
    let map_end = span_end_nonzero(handoff.map_physical, map_len, physical_limit)?;
    if handoff.map_physical % 8 != 0 {
        return Err(Error::MemoryMapMetadata);
    }
    if intersects(handoff.kernel_physical, kernel_end, handoff.map_physical, map_end) {
        return Err(Error::RangeOverlap);
    }
    let mut previous_end: u64 = 0;
    for (i, region) in regions.iter().enumerate() {
        if region.reserved != 0 || region.page_count == 0 || region.physical_start % PAGE != 0 {
            return Err(Error::MemoryRegion);
        }
        let len = region.page_count.checked_mul(PAGE).ok_or(Error::SpanOverflow)?;
        let end = span_end(region.physical_start, len, physical_limit)?;
        if i > 0 && region.physical_start < previous_end {
            return Err(Error::MemoryRegionOrder);
        }
        previous_end = end;
    }

    if handoff.module_count as usize != modules.len() || modules.len() > MAX_MODULES {
        return Err(Error::ModuleTableMetadata);
    }
    let module_table_end = if modules.is_empty() {
        if handoff.module_table_physical != 0 || handoff.module_stride != 0 {
            return Err(Error::ModuleTableMetadata);
        }
        0
    } else {
        if handoff.module_stride as usize != size_of::<Module>() {
            return Err(Error::ModuleTableMetadata);
        }
        let bytes = u64::from(handoff.module_count)
            .checked_mul(u64::from(handoff.module_stride)).ok_or(Error::SpanOverflow)?;
        let end = span_end_nonzero(handoff.module_table_physical, bytes, physical_limit)?;
        if handoff.module_table_physical % 8 != 0 {
            return Err(Error::ModuleTableMetadata);
        }
        if intersects(handoff.kernel_physical, kernel_end, handoff.module_table_physical, end)
            || intersects(handoff.map_physical, map_end, handoff.module_table_physical, end)
        {
            return Err(Error::RangeOverlap);
        }
        end
    };
    let command_end = if handoff.command_line_length == 0 {
        if handoff.command_line_physical != 0 { return Err(Error::CommandLineMetadata); }
        0
    } else {
        if handoff.command_line_length > MAX_COMMAND_LINE {
            return Err(Error::CommandLineMetadata);
        }
        let end = span_end_nonzero(
            handoff.command_line_physical, u64::from(handoff.command_line_length), physical_limit
        )?;
        if intersects(handoff.kernel_physical, kernel_end, handoff.command_line_physical, end)
            || intersects(handoff.map_physical, map_end, handoff.command_line_physical, end)
            || (module_table_end != 0 && intersects(
                handoff.module_table_physical, module_table_end,
                handoff.command_line_physical, end
            ))
        {
            return Err(Error::RangeOverlap);
        }
        end
    };

    let display = handoff.framebuffer.checked(physical_limit)?;
    if let Display::Linear(fb) = display {
        let fb_end = fb.physical_start + fb.length_bytes; // checked by Framebuffer::checked
        if intersects(handoff.kernel_physical, kernel_end, fb.physical_start, fb_end)
            || intersects(handoff.map_physical, map_end, fb.physical_start, fb_end)
            || (module_table_end != 0 && intersects(
                handoff.module_table_physical, module_table_end, fb.physical_start, fb_end
            ))
            || (command_end != 0 && intersects(
                handoff.command_line_physical, command_end, fb.physical_start, fb_end
            ))
        {
            return Err(Error::RangeOverlap);
        }
    }
    for (i, module) in modules.iter().enumerate() {
        if module.name_length == 0 || module.name_length > 128 {
            return Err(Error::ModuleRange);
        }
        let end = span_end_nonzero(
            module.physical_start, module.length_bytes, physical_limit
        ).map_err(|_| Error::ModuleRange)?;
        let name_end = span_end_nonzero(
            module.name_physical, u64::from(module.name_length), physical_limit
        ).map_err(|_| Error::ModuleRange)?;
        if intersects(module.physical_start, end, module.name_physical, name_end)
            || intersects(module.physical_start, end, handoff.kernel_physical, kernel_end)
            || intersects(module.name_physical, name_end, handoff.kernel_physical, kernel_end)
            || intersects(module.physical_start, end, handoff.map_physical, map_end)
            || intersects(module.name_physical, name_end, handoff.map_physical, map_end)
            || (module_table_end != 0 && (
                intersects(module.physical_start, end, handoff.module_table_physical, module_table_end)
                || intersects(module.name_physical, name_end, handoff.module_table_physical, module_table_end)))
            || (command_end != 0 && (
                intersects(module.physical_start, end, handoff.command_line_physical, command_end)
                || intersects(module.name_physical, name_end, handoff.command_line_physical, command_end)))
        {
            return Err(Error::RangeOverlap);
        }
        if let Display::Linear(fb) = display {
            let fb_end = fb.physical_start + fb.length_bytes;
            if intersects(module.physical_start, end, fb.physical_start, fb_end)
                || intersects(module.name_physical, name_end, fb.physical_start, fb_end)
            {
                return Err(Error::RangeOverlap);
            }
        }
        for older in &modules[..i] {
            let older_end = older.physical_start + older.length_bytes;
            let older_name_end = older.name_physical + u64::from(older.name_length);
            if intersects(module.physical_start, end, older.physical_start, older_end)
                || intersects(module.physical_start, end, older.name_physical, older_name_end)
                || intersects(module.name_physical, name_end, older.physical_start, older_end)
                || intersects(module.name_physical, name_end, older.name_physical, older_name_end)
            {
                return Err(Error::RangeOverlap);
            }
        }
    }
    Ok(Validated { kernel_end, regions: regions.len(), modules: modules.len(), display })
}
