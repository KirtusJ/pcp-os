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

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: [u8; 16] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
    ];
    const HEX: &str = "00112233445566778899aabbccddeeff";

    #[test]
    fn exactly_one_canonical_id_is_recognized() {
        assert_eq!(
            requested_id("auto=program then=off store=ide0 store_id=00112233445566778899aabbccddeeff"),
            Some(EXPECTED)
        );
    }

    #[test]
    fn missing_id_fails_closed() {
        assert_eq!(requested_id("auto=program store=ide0"), None);
    }

    #[test]
    fn duplicate_id_tokens_fail_even_if_both_equal() {
        assert_eq!(requested_id(&format!("store_id={HEX} store_id={HEX}")), None);
    }

    #[test]
    fn zero_id_and_wrong_length_fail() {
        assert_eq!(requested_id("store_id=00000000000000000000000000000000"), None);
        assert_eq!(requested_id("store_id=abcd"), None);
        assert_eq!(requested_id(&format!("store_id={HEX}0")), None);
        assert_eq!(requested_id("store_id="), None);
    }

    #[test]
    fn uppercase_and_invalid_hex_fail() {
        assert_eq!(requested_id("store_id=00112233445566778899AABBCCDDEEFF"), None);
        assert_eq!(requested_id("store_id=00112233445566778899aabbccddeezz"), None);
    }

    #[test]
    fn marker_contains_exact_version_two_id_and_zero_reserved_bytes() {
        let marker = marker_sector(&EXPECTED).unwrap();
        assert_eq!(&marker[..MARKER_V2.len()], MARKER_V2);
        assert_eq!(&marker[ID_OFFSET..ID_OFFSET + 16], &EXPECTED);
        assert!(marker[MARKER_V2.len()..ID_OFFSET].iter().all(|x| *x == 0));
        assert!(marker[ID_OFFSET + ID_BYTES..].iter().all(|x| *x == 0));
        assert!(matches_marker(&marker, &EXPECTED));
    }

    #[test]
    fn wrong_requested_id_is_rejected() {
        let mut wrong = EXPECTED;
        wrong[0] ^= 0x01;
        assert!(!matches_marker(&marker_sector(&EXPECTED).unwrap(), &wrong));
    }

    #[test]
    fn version_one_marker_cannot_be_silent_downgrade() {
        let mut v1 = [0u8; SECTOR_BYTES];
        v1[..b"PCPOS USER STORE 1\n".len()].copy_from_slice(b"PCPOS USER STORE 1\n");
        assert!(!matches_marker(&v1, &EXPECTED));
    }

    #[test]
    fn clone_of_same_marker_remains_accepted_and_is_not_authentication() {
        let marker = marker_sector(&EXPECTED).unwrap();
        let copied = marker;
        assert!(matches_marker(&copied, &EXPECTED));
    }

    #[test]
    fn reserved_byte_mutation_fails_closed() {
        let mut marker = marker_sector(&EXPECTED).unwrap();
        marker[ID_OFFSET + ID_BYTES] = 1;
        assert!(!matches_marker(&marker, &EXPECTED));
    }

    #[test]
    fn zero_id_cannot_be_formatted() {
        assert!(marker_sector(&[0u8; ID_BYTES]).is_none());
    }

    #[test]
    fn stray_text_cannot_make_id_match() {
        assert_eq!(requested_id(&format!("not_store_id={HEX}")), None);
        assert_eq!(requested_id(&format!("other={HEX}")), None);
    }
}
