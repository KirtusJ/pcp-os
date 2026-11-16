//! Optional raw-guest policy: one pinned IPv4/UDP pair and one timed reply.
use core::net::SocketAddrV4;

// with_queues verifies this MAC for the sole guest Ethernet device.
const LOCAL_MAC: [u8; 6] = [2, 0, 0, 0, 0, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rule {
    local: SocketAddrV4,
    remote: SocketAddrV4,
    peer_mac: [u8; 6],
}

/// An absent option preserves raw mode. A malformed or duplicate option refuses admission.
pub(crate) fn parse(line: &str) -> Result<Option<Rule>, &'static str> {
    let mut values = line.split_ascii_whitespace()
        .filter(|s| *s == "net_udp" || s.starts_with("net_udp="));
    let Some(option) = values.next() else { return Ok(None) };
    let value = option.strip_prefix("net_udp=").ok_or("missing net_udp value")?;
    if values.next().is_some() { return Err("duplicate net_udp"); }
    let mut parts = value.split(',');
    let mut endpoint = || {
        let addr: SocketAddrV4 = parts.next().and_then(|s| s.parse().ok()).ok_or("invalid UDP endpoint")?;
        if addr.port() == 0 || !matches!(addr.ip().octets()[0], 1..=223) || addr.ip().is_loopback() {
            return Err("expected unicast IPv4 endpoint");
        }
        Ok(addr)
    };
    let local = endpoint()?;
    let remote = endpoint()?;
    let mac = parts.next().ok_or("missing peer MAC")?;
    if parts.next().is_some() || mac.len() != 12 || !mac.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid peer MAC");
    }
    let mut peer_mac = [0u8; 6];
    for (i, byte) in peer_mac.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&mac[2*i..2*i+2], 16).map_err(|_| "invalid peer MAC")?;
    }
    if peer_mac[0] & 1 != 0 || peer_mac == [0; 6] || peer_mac == LOCAL_MAC || local.ip() == remote.ip() {
        return Err("expected distinct unicast peers");
    }
    Ok(Some(Rule { local, remote, peer_mac }))
}

fn sum_words(bytes: &[u8]) -> u32 {
    bytes.chunks(2).map(|b| u16::from_be_bytes([b[0], b.get(1).copied().unwrap_or(0)]) as u32).sum()
}

fn valid_sum(mut sum: u32) -> bool {
    while sum > 0xffff { sum = (sum & 0xffff) + (sum >> 16); }
    sum == 0xffff
}

impl Rule {
    pub(crate) fn permits(&self, frame: &[u8], inbound: bool) -> bool {
        if !(42..=1514).contains(&frame.len()) { return false; }
        let (from, to, src_mac, dst_mac) = if inbound {
            (self.remote, self.local, self.peer_mac, LOCAL_MAC)
        } else {
            (self.local, self.remote, LOCAL_MAC, self.peer_mac)
        };
        if frame[..6] != dst_mac || frame[6..12] != src_mac || frame[12..14] != [8, 0] {
            return false;
        }
        let ip = &frame[14..];
        let total = u16::from_be_bytes([ip[2], ip[3]]) as usize;
        if ip[0] != 0x45 || !(28..=ip.len()).contains(&total) || ip[8] == 0 || ip[9] != 17
            || !matches!(u16::from_be_bytes([ip[6], ip[7]]), 0 | 0x4000)
            || ip[12..16] != from.ip().octets() || ip[16..20] != to.ip().octets()
            || !valid_sum(sum_words(&ip[..20])) {
            return false;
        }
        // Permit only zero padding up to the Ethernet minimum frame size.
        if frame.len() > (14 + total).max(60) || ip[total..].iter().any(|&b| b != 0) { return false; }
        let udp = &ip[20..total];
        if udp[..2] != from.port().to_be_bytes() || udp[2..4] != to.port().to_be_bytes()
            || u16::from_be_bytes([udp[4], udp[5]]) as usize != udp.len() {
            return false;
        }
        udp[6..8] == [0, 0] || valid_sum(sum_words(&ip[12..20]) + 17 + udp.len() as u32 + sum_words(udp))
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub tx_pass: u64,
    pub tx_deny: u64,
    pub rx_pass: u64,
    pub rx_drop: u64,
}

pub(crate) struct Session {
    pub rule: Rule,
    pub counts: Counts,
    reply_until: Option<u64>,
}

impl Session {
    pub(crate) fn new(rule: Rule) -> Self {
        Self { rule, counts: Counts::default(), reply_until: None }
    }

    pub(crate) fn completed_send(&mut self, now: u64, lifetime: u64) {
        self.reply_until = Some(now.saturating_add(lifetime));
        self.counts.tx_pass = self.counts.tx_pass.saturating_add(1);
    }

    pub(crate) fn receive(&mut self, bytes: &[u8], now: u64) -> bool {
        if self.reply_until.is_some_and(|end| now < end) && self.rule.permits(bytes, true) {
            self.reply_until = None;
            self.counts.rx_pass = self.counts.rx_pass.saturating_add(1);
            true
        } else {
            self.counts.rx_drop = self.counts.rx_drop.saturating_add(1);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const OPTION: &str = "net_udp=192.0.2.2:50001,198.51.100.99:31337,020000000002";

    fn frames() -> ([u8; 60], [u8; 60]) {
        // Exact outgoing bytes captured in the baseline raw-guest experiment.
        let encoded = b"02000000000202000000000108004500002e07544000401146d2c0000202c6336463c3517a69001a00005043504f532d5241572d5749544e45535321";
        let mut out = [0; 60];
        for (dst, hex) in out.iter_mut().zip(encoded.chunks_exact(2)) {
            *dst = u8::from_str_radix(core::str::from_utf8(hex).unwrap(), 16).unwrap();
        }
        let mut reply = out;
        reply[..6].copy_from_slice(&out[6..12]);
        reply[6..12].copy_from_slice(&out[..6]);
        reply[26..30].copy_from_slice(&out[30..34]);
        reply[30..34].copy_from_slice(&out[26..30]);
        reply[34..36].copy_from_slice(&out[36..38]);
        reply[36..38].copy_from_slice(&out[34..36]);
        (out, reply)
    }

    #[test]
    fn reply_credit_expires_collapses_and_is_consumed_once() {
        let mut s = Session::new(parse(OPTION).unwrap().unwrap());
        let (out, reply) = frames();
        assert!(s.rule.permits(&out, false));
        assert!(!s.receive(&reply, 1));
        s.completed_send(2, 5);
        assert!(!s.receive(&reply, 7)); // boundary is exclusive
        s.completed_send(8, 5);
        s.completed_send(9, 5);
        assert!(!s.receive(&out, 10)); // a wrong tuple does not consume credit
        assert!(s.receive(&reply, 13));
        assert!(!s.receive(&reply, 13));
        assert_eq!(s.counts, Counts { tx_pass: 3, tx_deny: 0, rx_pass: 1, rx_drop: 4 });
    }

    #[test]
    fn header_extent_and_boot_options_fail_closed() {
        let rule = parse(OPTION).unwrap().unwrap();
        let (out, _) = frames();
        for len in 0..out.len() { assert!(!rule.permits(&out[..len], false)); }
        assert!(!rule.permits(&[0; 1515], false));
        assert_eq!(parse("grant=none"), Ok(None));
        for bad in ["net_udp", "net_udp=", "net_udp=1,2,3", "net_udp=1.1.1.1:0,2.2.2.2:4,020000000002",
                    "net_udp=127.0.0.1:1,2.2.2.2:4,020000000002",
                    "net_udp=1.1.1.1:1,224.0.0.2:4,020000000002",
                    "net_udp=1.1.1.1:1,2.2.2.2:4,ffffffffffff"] {
            assert!(parse(bad).is_err());
        }
    }
}
