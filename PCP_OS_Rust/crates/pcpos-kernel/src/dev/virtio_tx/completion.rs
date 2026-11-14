//! Completion checks for the one-chain, non-offloaded QEMU probe.

pub(crate) fn validate_used(count: u16, head: u32) -> Result<(), &'static str> {
    if count == 1 && head == 0 {
        Ok(())
    } else {
        Err("invalid used entry")
    }
}

pub(crate) fn validate_reply_length(len: u32) -> Result<(), &'static str> {
    // The used length covers both writable descriptors: ten header bytes and
    // sixty Ethernet bytes. The physical alignment gap is outside both.
    if len == 10 + 60 {
        Ok(())
    } else {
        Err("RX used length")
    }
}

pub(crate) fn validate_header(header: &[u8; 10]) -> Result<(), &'static str> {
    // The remaining fields have no meaning for a packet without offloads.
    if header[0] == 0 && header[1] == 0 {
        Ok(())
    } else {
        Err("RX header mismatch")
    }
}

#[cfg(any(feature = "virtio-rx-probe", test))]
pub(crate) fn parse_nonce(cmdline: &str) -> Result<[u8; 8], &'static str> {
    let mut words = cmdline
        .split_ascii_whitespace()
        .filter_map(|s| s.strip_prefix("reply_nonce="));
    let hex = words.next().ok_or("missing reply nonce")?.as_bytes();
    if hex.len() != 16 || words.next().is_some() {
        return Err("invalid reply nonce");
    }
    let nibble = |c| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err("invalid reply nonce"),
    };
    let mut bytes = [0u8; 8];
    for (out, pair) in bytes.iter_mut().zip(hex.chunks_exact(2)) {
        *out = nibble(pair[0])? << 4 | nibble(pair[1])?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_one_exact_hex_challenge() {
        assert_eq!(
            parse_nonce("test=virtio-reply reply_nonce=dead0001facedeaf"),
            Ok([0xde, 0xad, 0, 1, 0xfa, 0xce, 0xde, 0xaf])
        );
        assert!(parse_nonce("").is_err());
        for s in [
            "reply_nonce=ef",
            "reply_nonce=000000000000000g",
            "reply_nonce=0000000000000000 reply_nonce=0000000000000000",
        ] {
            assert_eq!(parse_nonce(s), Err("invalid reply nonce"));
        }
    }

    #[test]
    fn only_the_single_posted_chain_completes() {
        assert_eq!(validate_used(1, 0), Ok(()));
        for count in [0, 2, 257, u16::MAX] {
            assert_eq!(validate_used(count, 0), Err("invalid used entry"));
        }
        for head in [1, 256, 65536, u32::MAX] {
            assert_eq!(validate_used(1, head), Err("invalid used entry"));
        }
    }

    #[test]
    fn reply_uses_exactly_the_header_and_frame() {
        assert_eq!(validate_reply_length(70), Ok(()));
        for len in [0, 9, 10, 69, 71, 1526, 1527, u32::MAX] {
            assert_eq!(validate_reply_length(len), Err("RX used length"));
        }
    }

    #[test]
    fn flags_and_gso_type_gate_the_inactive_fields() {
        assert_eq!(validate_header(&[0; 10]), Ok(()));
        for i in 0..10 {
            let mut header = [0; 10];
            header[i] = 0xff;
            let expected = if i < 2 {
                Err("RX header mismatch")
            } else {
                Ok(())
            };
            assert_eq!(validate_header(&header), expected);
        }
        assert_eq!(validate_header(&[0, 0, 3, 7, 6, 5, 4, 3, 2, 1]), Ok(()));
    }
}
