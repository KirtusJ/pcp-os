//! Only one exact image pin can authorize raw Ethernet. An explicit denial always wins.
pub(crate) fn ethernet_grant(cmd: Option<&str>, digest: &str) -> bool {
    let Some(cmd) = cmd else { return false };
    let mut pins = cmd
        .split_ascii_whitespace()
        .filter_map(|word| word.strip_prefix("net_image="));
    !cmd.split_ascii_whitespace()
        .any(|word| word == "grant=none")
        && pins.next() == Some(digest)
        && pins.next().is_none()
}

#[cfg(test)]
#[path = "../arch/boot_text.rs"]
mod boot_text;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_a_single_matching_pin() {
        let digest = "d".repeat(64);
        let pin = format!("net_image={digest}");
        assert!(ethernet_grant(
            Some(&format!("auto=surf.elf {pin} then=off")),
            &digest
        ));
        for cmd in [
            "",
            "net_image=",
            "link_image=d",
            "net_image=wrong",
            "other_net_image=d",
        ] {
            assert!(!ethernet_grant(Some(cmd), &digest), "{cmd}");
        }
        for cmd in [
            format!("{pin} {pin}"),
            format!("{pin} net_image=other"),
            format!("net_image=other {pin}"),
        ] {
            assert!(!ethernet_grant(Some(&cmd), &digest), "{cmd}");
        }
    }

    #[test]
    fn explicit_denial_wins_in_any_position() {
        for cmd in [
            "grant=none net_image=d",
            "net_image=d grant=none",
            "grant=full net_image=d grant=none",
            "grant=none grant=full net_image=d",
        ] {
            assert!(!ethernet_grant(Some(cmd), "d"), "{cmd}");
        }
    }

    #[test]
    fn loader_must_see_an_unmodified_terminated_command() {
        let granted = |bytes: &[u8]| {
            let (prefix, exact) = boot_text::read(|i| bytes.get(i).copied());
            ethernet_grant(exact.then_some(prefix.as_str()), "d")
        };
        assert!(!granted(b""));
        assert!(!granted(b"net_image=d pre\tgrant=none\0"));
        for len in [254, 255, 256] {
            let mut cmd = b"net_image=d ".to_vec();
            cmd.resize(len, b' ');
            assert!(!granted(&cmd), "missing or inaccessible NUL");
            cmd.push(0);
            assert_eq!(granted(&cmd), len <= 255, "NUL at byte {len}");
        }
        for suffix in [b"grant=none".as_slice(), b"net_image=d"] {
            let mut cmd = b"net_image=d ".to_vec();
            cmd.resize(255, b' ');
            cmd.extend_from_slice(suffix);
            cmd.push(0);
            assert!(!granted(&cmd));
        }
        for bad in [b'\t', 0x7f, 0xff] {
            let mut cmd = b"net_image=d x".to_vec();
            cmd.extend_from_slice(&[bad, b'y', 0]);
            assert!(!granted(&cmd));
        }
    }
}
