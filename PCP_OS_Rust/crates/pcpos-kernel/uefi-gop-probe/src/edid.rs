//! Pure, bounded EDID 1.3/1.4 base timing inspection.
//!
//! This does NOT read an EFI_EDID_*_PROTOCOL pointer, identify a GOP handle,
//! honor all CTA/DisplayID extension semantics, or select any GOP mode.
//! It accepts an *owned or borrowed* byte slice provided by a caller who must
//! independently establish which display/EDID protocol supplied it.
//!
//! Intentional conservative profile: complete EDID 1.x blocks only, at most
//! 16 extensions, checksum-verified. All extension bytes are checked for
//! length/checksum but their timing and override meanings are uninterpreted.

pub const EDID_BLOCK_BYTES: usize = 128;
pub const MAX_EXTENSIONS: usize = 16;
const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
const FIRST_DTD: usize = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    WrongLength,
    TooManyExtensions,
    BadHeader,
    BadChecksum { block_index: u8 },
    UnsupportedVersion,
    /// EDID 1.3's preference bit was not present. EDID 1.4 instead
    /// mandates the first DTD and repurposes the same bit's meaning.
    NoDeclaredPreferredTiming,
    MissingPreferredDetailedTiming,
    InvalidPreferredDetailedTiming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetailedTiming {
    /// The raw EDID DTD pixel-clock value in 10 kHz units.
    pub pixel_clock_10khz: u16,
    pub horizontal_active: u16,
    pub horizontal_blank: u16,
    pub vertical_active: u16,
    pub vertical_blank: u16,
    /// DTD flags bit 7. The vertical active field is not automatically
    /// expanded to a progressive full-frame size for an interlaced mode.
    pub interlaced: bool,
}

impl DetailedTiming {
    /// Tests only the encoded active pixel counts; no refresh/timing,
    /// mode-index, physical-output or EDID provenance claim follows.
    pub fn matches_dimensions_only(self, width: u32, height: u32) -> bool {
        u32::from(self.horizontal_active) == width
            && u32::from(self.vertical_active) == height
    }

    /// Pixel clock and all blanking intervals remain separate fields.
    /// This is NOT a claim about an actual active GOP refresh rate.
    pub fn total_dimensions(self) -> (u16, u16) {
        (
            self.horizontal_active + self.horizontal_blank,
            self.vertical_active + self.vertical_blank,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub revision: u8,
    pub extension_count: u8,
    pub preferred: DetailedTiming,
    /// Identity here means only validated EDID *bytes*; these fields
    /// are not a device ID, signature, or proof of source authenticity.
    pub manufacturer_raw: u16,
    pub product_code_raw: u16,
}

pub fn parse_preferred(raw: &[u8]) -> Result<Report, Error> {
    if raw.len() < EDID_BLOCK_BYTES || raw.len() % EDID_BLOCK_BYTES != 0 {
        return Err(Error::WrongLength);
    }
    if raw[..HEADER.len()] != HEADER {
        return Err(Error::BadHeader);
    }
    let extension_count = raw[126] as usize;
    if extension_count > MAX_EXTENSIONS {
        return Err(Error::TooManyExtensions);
    }
    let expected = (extension_count + 1) * EDID_BLOCK_BYTES;
    if raw.len() != expected {
        // A valid base block may advertise extensions. A base-only slice
        // cannot be presented as a complete checksum-validated EDID.
        return Err(Error::WrongLength);
    }
    for (index, block) in raw.chunks_exact(EDID_BLOCK_BYTES).enumerate() {
        let sum = block.iter().fold(0u8, |n, &b| n.wrapping_add(b));
        if sum != 0 {
            return Err(Error::BadChecksum { block_index: index as u8 });
        }
    }
    if raw[18] != 1 || !(raw[19] == 3 || raw[19] == 4) {
        return Err(Error::UnsupportedVersion);
    }
    let revision = raw[19];
    // In EDID 1.3, byte 24 bit 1 identifies a preferred timing mode.
    // In 1.4, the first 18-byte DTD is mandatory preferred timing and
    // bit 1 instead identifies whether it includes native pixel format.
    if revision == 3 && raw[24] & 0x02 == 0 {
        return Err(Error::NoDeclaredPreferredTiming);
    }

    let d = &raw[FIRST_DTD..FIRST_DTD + 18];
    let pixel_clock_10khz = u16::from_le_bytes([d[0], d[1]]);
    if pixel_clock_10khz == 0 {
        // This is a display descriptor, not a detailed timing descriptor.
        return Err(Error::MissingPreferredDetailedTiming);
    }
    let hactive = u16::from(d[2]) | (u16::from(d[4] & 0xf0) << 4);
    let hblank = u16::from(d[3]) | (u16::from(d[4] & 0x0f) << 8);
    let vactive = u16::from(d[5]) | (u16::from(d[7] & 0xf0) << 4);
    let vblank = u16::from(d[6]) | (u16::from(d[7] & 0x0f) << 8);
    if hactive == 0 || vactive == 0 || hblank == 0 || vblank == 0 {
        return Err(Error::InvalidPreferredDetailedTiming);
    }
    let preferred = DetailedTiming {
        pixel_clock_10khz,
        horizontal_active: hactive,
        horizontal_blank: hblank,
        vertical_active: vactive,
        vertical_blank: vblank,
        interlaced: d[17] & 0x80 != 0,
    };
    Ok(Report {
        revision,
        extension_count: extension_count as u8,
        preferred,
        manufacturer_raw: u16::from_be_bytes([raw[8], raw[9]]),
        product_code_raw: u16::from_le_bytes([raw[10], raw[11]]),
    })
}
