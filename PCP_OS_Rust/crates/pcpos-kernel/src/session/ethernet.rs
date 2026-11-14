//! Only one exact image pin can authorize raw Ethernet. An explicit denial always wins.
pub(crate) fn ethernet_grant(cmd: &str, digest: &str) -> bool {
    let mut pins = cmd
        .split_ascii_whitespace()
        .filter_map(|word| word.strip_prefix("net_image="));
    !cmd.split_ascii_whitespace()
        .any(|word| word == "grant=none")
        && pins.next() == Some(digest)
        && pins.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_a_single_matching_pin() {
        let digest = "d".repeat(64);
        let pin = format!("net_image={digest}");
        assert!(ethernet_grant(
            &format!("auto=surf.elf {pin} then=off"),
            &digest
        ));
        for cmd in [
            "",
            "net_image=",
            "link_image=d",
            "net_image=wrong",
            "other_net_image=d",
        ] {
            assert!(!ethernet_grant(cmd, &digest), "{cmd}");
        }
        for cmd in [
            format!("{pin} {pin}"),
            format!("{pin} net_image=other"),
            format!("net_image=other {pin}"),
        ] {
            assert!(!ethernet_grant(&cmd, &digest), "{cmd}");
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
            assert!(!ethernet_grant(cmd, "d"), "{cmd}");
        }
    }
}
