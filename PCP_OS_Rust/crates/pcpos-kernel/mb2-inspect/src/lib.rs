#![no_std]
//! Bounded inspection of copied Multiboot2 boot INFORMATION (not image-header tags).
//! No pointers are dereferenced and no firmware, kernel, mapping or I/O is invoked.
//! This is an inspection-only candidate, not a boot path or authority mechanism.

pub const ENTRY_MAGIC: u32 = 0x36d7_6289;
pub const MAX_INFO_BYTES: usize = 1024 * 1024;
pub const MAX_TAGS: usize = 512;
pub const MAX_MODULES: usize = 16;
const EFI_AMD64_LOW_LIMIT: u64 = 1u64 << 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    BadEntryMagic,
    WrongLength,
    BadLocation,
    BadTotal,
    MalformedTag,
    TagOverflow,
    MissingEnd,
    DuplicateTag,
    TooManyTags,
    TooManyModules,
    BadModule,
    OverlappingModule,
    BadMemoryMap,
    BadEfiMemoryMap,
    BadFramebuffer,
    BadRgbFields,
    UnsupportedFramebufferDepth,
    BadString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub position: u8,
    pub size: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelKind {
    DirectRgb,
    Indexed,
    Text,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub physical_base: u64,
    pub pitch_bytes: u32,
    pub width: u32,
    pub height: u32,
    pub bits_per_pixel: u8,
    pub kind: PixelKind,
    pub rgb_channels: Option<[Channel; 3]>,
}

impl Framebuffer {
    /// Only a known, linear 32-bit B,G,R byte order matches the existing
    /// kernel renderer's numerical color representation.
    pub fn matches_bgrx32(&self) -> bool {
        self.bits_per_pixel == 32 && self.kind == PixelKind::DirectRgb
            && self.rgb_channels == Some([
                Channel { position: 16, size: 8 },
                Channel { position: 8, size: 8 },
                Channel { position: 0, size: 8 },
            ])
    }

    /// The opposite R,G,B byte order is NOT interchangeable with BGRX.
    pub fn matches_rgbx32(&self) -> bool {
        self.bits_per_pixel == 32 && self.kind == PixelKind::DirectRgb
            && self.rgb_channels == Some([
                Channel { position: 0, size: 8 },
                Channel { position: 8, size: 8 },
                Channel { position: 16, size: 8 },
            ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootModule<'a> {
    pub start: u32,
    pub end_exclusive: u32,
    /// Borrowed and NUL-trimmed. UTF-8/authorization has not been established.
    pub command: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapView<'a> {
    /// Entire entries, including descriptor-specific extensions. No casts.
    pub entries: &'a [u8],
    pub entry_bytes: u32,
    pub entry_version: u32,
    pub entry_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection<'a> {
    pub info_bytes: u32,
    pub tag_count: usize,
    /// Counts skipped tags, including standardized but unimplemented types
    /// (for example information tag 9, ELF sections) and future unknown
    /// types. It does not claim they have no defined Multiboot2 meaning.
    pub uninterpreted_tag_count: usize,
    pub framebuffer: Option<Framebuffer>,
    /// The generic Multiboot2 memory map (tag 6), not the EFI map.
    pub basic_memory_map: Option<MapView<'a>>,
    /// EFI descriptor array (tag 17), never an ExitBootServices receipt.
    pub efi_memory_map: Option<MapView<'a>>,
    /// Tag 18 says the boot loader had not called ExitBootServices
    /// when it transferred control. A later OS call does not remove it.
    /// Absence cannot prove a successful later ExitBootServices call.
    pub efi_boot_services_not_terminated: bool,
    pub efi64_system_table: Option<u64>,
    pub load_base32: Option<u32>,
    pub command_line: Option<&'a [u8]>,
    pub modules: [Option<BootModule<'a>>; MAX_MODULES],
    pub module_count: usize,
}

fn bytes(raw: &[u8], off: usize, len: usize) -> Result<&[u8], Error> {
    raw.get(off..off.checked_add(len).ok_or(Error::TagOverflow)?)
        .ok_or(Error::MalformedTag)
}
fn get32(raw: &[u8], off: usize) -> Result<u32, Error> {
    let v = bytes(raw, off, 4)?;
    Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
}
fn get64(raw: &[u8], off: usize) -> Result<u64, Error> {
    let v = bytes(raw, off, 8)?;
    Ok(u64::from_le_bytes([v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]]))
}
fn aligned8(n: usize) -> Result<usize, Error> {
    Ok(n.checked_add(7).ok_or(Error::TagOverflow)? & !7)
}
fn c_string(raw: &[u8]) -> Result<&[u8], Error> {
    let end = raw.iter().position(|c| *c == 0).ok_or(Error::BadString)?;
    Ok(&raw[..end])
}
fn overlaps(a: u64, b: u64, c: u64, d: u64) -> bool {
    a < d && c < b
}

fn map_tag<'a>(tag: &'a [u8], efi: bool) -> Result<MapView<'a>, Error> {
    let bad = if efi { Error::BadEfiMemoryMap } else { Error::BadMemoryMap };
    if tag.len() < 16 {
        return Err(bad);
    }
    let stride = get32(tag, 8).map_err(|_| bad)?;
    let version = get32(tag, 12).map_err(|_| bad)?;
    let minimum = if efi { 40 } else { 24 };
    // Multiboot2 *basic* memory map tag 6 guarantees entries are a
    // multiple of eight bytes. EFI tag 17 carries firmware-provided
    // DescriptorSize, which UEFI requires consumers to honor as returned;
    // it does not inherit the basic-map multiple-of-eight guarantee.
    // We use slices/byte reads, not pointer casts, so no alignment is
    // assumed for a future extended EFI descriptor.
    if stride < minimum || (!efi && stride % 8 != 0) {
        return Err(bad);
    }
    let entries = &tag[16..];
    if entries.is_empty() || entries.len() % (stride as usize) != 0
        || entries.len() / (stride as usize) > 4096
    {
        return Err(bad);
    }
    if !efi {
        for chunk in entries.chunks_exact(stride as usize) {
            let base = get64(chunk, 0).map_err(|_| bad)?;
            let size = get64(chunk, 8).map_err(|_| bad)?;
            if size == 0 || base.checked_add(size).is_none() {
                return Err(bad);
            }
        }
    }
    Ok(MapView {
        entries,
        entry_bytes: stride,
        entry_version: version,
        entry_count: (entries.len() / stride as usize) as u32,
    })
}

fn framebuffer_tag(tag: &[u8]) -> Result<Framebuffer, Error> {
    // Layout of Multiboot2 INFORMATION tag 8, not UEFI GOP's native mode
    // enum. The RGB layout is six field-position/size bytes at offset 32.
    if tag.len() < 32 {
        return Err(Error::BadFramebuffer);
    }
    let physical_base = get64(tag, 8)?;
    let pitch_bytes = get32(tag, 16)?;
    let width = get32(tag, 20)?;
    let height = get32(tag, 24)?;
    let bpp = tag[28];
    let ty = tag[29];
    if physical_base == 0 || pitch_bytes == 0
        || width == 0 || height == 0 || width > 16384 || height > 16384
    {
        return Err(Error::BadFramebuffer);
    }
    // Unknown pixel representations remain visible but cannot become an
    // admitted linear renderer mode. This does not establish the display
    // contents or real frame-buffer address.
    let mut rgb_channels = None;
    let kind = match ty {
        1 => {
            if tag.len() < 38 {
                return Err(Error::BadFramebuffer);
            }
            let channels = [
                Channel { position: tag[32], size: tag[33] },
                Channel { position: tag[34], size: tag[35] },
                Channel { position: tag[36], size: tag[37] },
            ];
            if bpp == 0 || bpp > 32 {
                return Err(Error::UnsupportedFramebufferDepth);
            }
            let mut occupied = 0u64;
            for channel in channels {
                let end = (channel.position as u16) + (channel.size as u16);
                if channel.size == 0 || end > bpp as u16 {
                    return Err(Error::BadRgbFields);
                }
                let mask = ((1u64 << channel.size) - 1) << channel.position;
                if occupied & mask != 0 {
                    return Err(Error::BadRgbFields);
                }
                occupied |= mask;
            }
            rgb_channels = Some(channels);
            PixelKind::DirectRgb
        }
        0 => {
            // Compatibility profile: GNU Multiboot2 §3.6.12's prose table
            // says u32, but the same publication's C struct uses u16, and
            // GRUB writes a u16 count followed by 3-byte RGB entries.
            // We intentionally decode the GRUB-produced form: count at
            // 32..34, RGB entries starting at 34. This is NOT an assertion
            // that the published specification is internally unambiguous.
            // Other layouts need a separately sourced/profiled parser.
            if tag.len() < 34 {
                return Err(Error::BadFramebuffer);
            }
            let count = u16::from_le_bytes([tag[32], tag[33]]) as usize;
            let expected = 34usize
                .checked_add(count.checked_mul(3).ok_or(Error::BadFramebuffer)?)
                .ok_or(Error::BadFramebuffer)?;
            // The GRUB-targeted u16 count is not limited to 256 by
            // the wire profile. This is NOT approval to draw indexed pixels
            // (PixelKind::Indexed is not the current BGRX32 renderer).
            if tag.len() != expected {
                return Err(Error::BadFramebuffer);
            }
            PixelKind::Indexed
        }
        2 => {
            if bpp != 16 {
                return Err(Error::BadFramebuffer);
            }
            PixelKind::Text
        }
        other => PixelKind::Unknown(other),
    };
    // This is a layout consistency bound, not proof that the framebuffer
    // memory is present, mapped or safe. Text dimensions are characters.
    if ty <= 2 {
        if bpp == 0 || bpp > 32 {
            return Err(Error::UnsupportedFramebufferDepth);
        }
        // Framebuffer pitch is bytes PER ROW; bpp is bits PER PIXEL.
        // For packed depths (1/2/4 bpp), rounding each pixel up to one
        // byte incorrectly rejects valid bit-packed rows. Round *once*
        // after computing all row bits.
        let row_bits = u64::from(width)
            .checked_mul(u64::from(bpp)).ok_or(Error::BadFramebuffer)?;
        let byte_width = row_bits.checked_add(7)
            .ok_or(Error::BadFramebuffer)? / 8;
        let end = physical_base.checked_add(
            u64::from(pitch_bytes).checked_mul(u64::from(height)).ok_or(Error::BadFramebuffer)?
        ).ok_or(Error::BadFramebuffer)?;
        if u64::from(pitch_bytes) < byte_width || end <= physical_base {
            return Err(Error::BadFramebuffer);
        }
    }
    Ok(Framebuffer { physical_base, pitch_bytes, width, height,
        bits_per_pixel: bpp, kind, rgb_channels })
}

/// Inspect a complete *copied* Multiboot2 information buffer.
///
/// The physical address is an externally observed parameter for the documented
/// EFI amd64 entry. This routine checks its alignment and the MB2 below-4-GiB
/// placement constraint; it cannot authenticate who supplied the parameter or
/// attest firmware ExitBootServices. It does not inspect the OS image header
/// tags 7 and 9; those are a DIFFERENT wire format.
pub fn inspect_efi_amd64<'a>(
    raw: &'a [u8],
    mbi_physical: u64,
    entry_magic: u32,
) -> Result<Inspection<'a>, Error> {
    if entry_magic != ENTRY_MAGIC {
        return Err(Error::BadEntryMagic);
    }
    if raw.len() < 16 || raw.len() > MAX_INFO_BYTES {
        return Err(Error::WrongLength);
    }
    if mbi_physical == 0 || mbi_physical % 8 != 0 ||
        mbi_physical.checked_add(raw.len() as u64)
            .filter(|end| *end <= EFI_AMD64_LOW_LIMIT).is_none()
    {
        return Err(Error::BadLocation);
    }
    let total = get32(raw, 0)? as usize;
    // The boot-information length must end at a terminating tag; the
    // parser does not accept any trailing unparsed bytes.
    if total != raw.len() || total < 16 || total % 8 != 0 {
        return Err(Error::BadTotal);
    }
    // Reserved fixed field is ignored as prescribed by Multiboot2.
    let mut state = Inspection {
        info_bytes: total as u32, tag_count: 0, uninterpreted_tag_count: 0,
        framebuffer: None, basic_memory_map: None, efi_memory_map: None,
        efi_boot_services_not_terminated: false, efi64_system_table: None,
        load_base32: None, command_line: None,
        modules: [None; MAX_MODULES], module_count: 0,
    };
    let mut pos = 8usize;
    let mut ended = false;
    while pos < total {
        let ty = get32(raw, pos)?;
        let size = get32(raw, pos + 4)? as usize;
        if size < 8 {
            return Err(Error::MalformedTag);
        }
        let end = pos.checked_add(size).ok_or(Error::TagOverflow)?;
        let tag = bytes(raw, pos, size)?;
        if ty == 0 {
            if size != 8 || end != total || ended {
                return Err(Error::MalformedTag);
            }
            ended = true;
            break;
        }
        // The terminal tag is not a data tag. Permit exactly MAX_TAGS
        // nonterminal tags, but refuse a (MAX_TAGS+1)th data tag.
        if state.tag_count >= MAX_TAGS {
            return Err(Error::TooManyTags);
        }
        match ty {
            1 => {
                if state.command_line.is_some() {
                    return Err(Error::DuplicateTag);
                }
                state.command_line = Some(c_string(&tag[8..])?);
            }
            3 => {
                if size < 17 {
                    return Err(Error::BadModule);
                }
                if state.module_count >= MAX_MODULES {
                    return Err(Error::TooManyModules);
                }
                let start = get32(tag, 8)?;
                let stop = get32(tag, 12)?;
                if start >= stop || overlaps(start as u64, stop as u64,
                    mbi_physical, mbi_physical + raw.len() as u64)
                {
                    return Err(Error::BadModule);
                }
                for prior in state.modules.iter().flatten() {
                    if overlaps(start as u64, stop as u64,
                        prior.start as u64, prior.end_exclusive as u64)
                    {
                        return Err(Error::OverlappingModule);
                    }
                }
                let entry = BootModule { start, end_exclusive: stop,
                    command: c_string(&tag[16..])? };
                state.modules[state.module_count] = Some(entry);
                state.module_count += 1;
            }
            6 => {
                if state.basic_memory_map.is_some() {
                    return Err(Error::DuplicateTag);
                }
                state.basic_memory_map = Some(map_tag(tag, false)?);
            }
            8 => {
                if state.framebuffer.is_some() {
                    return Err(Error::DuplicateTag);
                }
                state.framebuffer = Some(framebuffer_tag(tag)?);
            }
            12 => {
                if state.efi64_system_table.is_some() {
                    return Err(Error::DuplicateTag);
                }
                if size != 16 {
                    return Err(Error::MalformedTag);
                }
                state.efi64_system_table = Some(get64(tag, 8)?);
            }
            17 => {
                if state.efi_memory_map.is_some() {
                    return Err(Error::DuplicateTag);
                }
                state.efi_memory_map = Some(map_tag(tag, true)?);
            }
            18 => {
                if state.efi_boot_services_not_terminated {
                    return Err(Error::DuplicateTag);
                }
                if size != 8 {
                    return Err(Error::MalformedTag);
                }
                state.efi_boot_services_not_terminated = true;
            }
            21 => {
                if state.load_base32.is_some() {
                    return Err(Error::DuplicateTag);
                }
                if size != 12 {
                    return Err(Error::MalformedTag);
                }
                state.load_base32 = Some(get32(tag, 8)?);
            }
            _ => state.uninterpreted_tag_count += 1,
        }
        state.tag_count += 1;
        let next = aligned8(end)?;
        if next <= pos || next > total {
            return Err(Error::MalformedTag);
        }
        pos = next;
    }
    if !ended {
        return Err(Error::MissingEnd);
    }
    Ok(state)
}
