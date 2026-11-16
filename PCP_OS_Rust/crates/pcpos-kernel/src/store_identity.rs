//! Explicit, *non-authenticating* volume identifier for an opt-in legacy IDE fixture.
//!
//! This is a fail-closed accidental-disk-selection guard, not a security
//! boundary. Anyone who copies the 512-byte format sector also copies the ID.
//! It does not identify a physical drive, detect duplicate devices, choose a
//! partition, protect secrets, prevent malicious rollback or authorize writes.
//!
//! This test-only v2 marker must NEVER be used in lieu of a device/partition
//! path, capacity check, user approval, integrity protection or full installer.

pub const MARKER_V2: &[u8] = b"PCPOS USER STORE 2\n";
pub const ID_OFFSET: usize = 32;
pub const ID_BYTES: usize = 16;
pub const SECTOR_BYTES: usize = 512;

#[inline]
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// An exact one-token, canonical lowercase 128-bit ID supplied by a loader.
/// Duplicate id tokens, zero ID, uppercase, odd length or invalid characters
/// fail closed. The trust of the command line is external to this function.
pub fn requested_id(command_line: &str) -> Option<[u8; ID_BYTES]> {
    let mut id: Option<[u8; ID_BYTES]> = None;
    for token in command_line.split_ascii_whitespace() {
        if let Some(hex) = token.strip_prefix("store_id=") {
            if id.is_some() {
                return None;
            }
            let bytes = hex.as_bytes();
            if bytes.len() != ID_BYTES * 2 {
                return None;
            }
            let mut next = [0u8; ID_BYTES];
            for i in 0..ID_BYTES {
                let a = nibble(bytes[i * 2])?;
                let b = nibble(bytes[i * 2 + 1])?;
                next[i] = a << 4 | b;
            }
            if next == [0; ID_BYTES] {
                return None;
            }
            id = Some(next);
        }
    }
    id
}

/// A versioned raw *fixture* marker with a chosen nonzero 128-bit ID.
/// Reserved bytes are zero, and the marker occupies the entire 512-byte
/// sector. The ID must be generated and explicitly selected externally;
/// there is no entropy source in this code.
pub fn marker_sector(id: &[u8; ID_BYTES]) -> Option<[u8; SECTOR_BYTES]> {
    if *id == [0u8; ID_BYTES] {
        return None;
    }
    let mut sector = [0u8; SECTOR_BYTES];
    sector[..MARKER_V2.len()].copy_from_slice(MARKER_V2);
    sector[ID_OFFSET..ID_OFFSET + ID_BYTES].copy_from_slice(id);
    Some(sector)
}

/// Equality is deterministic checking, not authentication against cloning.
pub fn matches_marker(sector: &[u8; SECTOR_BYTES], id: &[u8; ID_BYTES]) -> bool {
    marker_sector(id).is_some_and(|expected| sector == &expected)
}
