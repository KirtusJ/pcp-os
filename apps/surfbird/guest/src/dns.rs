//! A bounded UDP stub for one direct Internet-class A answer from the fixture resolver.
use super::Ethernet;
use pcp_game_guest::{load, sleep_us, time_us};
use smoltcp::{
    iface::{Interface, SocketSet, SocketStorage},
    socket::udp,
    wire::{
        DnsFlags, DnsOpcode, DnsPacket, DnsQueryType, DnsQuestion, DnsRcode, DnsRecord,
        DnsRecordData, DnsRepr, IpEndpoint, Ipv4Address,
    },
};

type Failure = &'static str;

fn name_matches(packet: &DnsPacket<&[u8]>, encoded: &[u8], host: &str) -> bool {
    let mut labels = packet.parse_name(encoded);
    for wanted in host.as_bytes().split(|&b| b == b'.') {
        if !matches!(labels.next(), Some(Ok(label)) if label.eq_ignore_ascii_case(wanted)) {
            return false;
        }
    }
    labels.next().is_none()
}

fn answer(bytes: &[u8], id: u16, host: &str) -> Result<Option<Ipv4Address>, Failure> {
    let Ok(packet) = DnsPacket::new_checked(bytes) else {
        return Ok(None);
    };
    if packet.transaction_id() != id
        || packet.opcode() != DnsOpcode::Query
        || !packet.flags().contains(DnsFlags::RESPONSE)
        || packet.question_count() != 1
    {
        return Ok(None);
    }
    let Ok((records, question)) = DnsQuestion::parse(packet.payload()) else {
        return Ok(None);
    };
    if question.type_ != DnsQueryType::A || !name_matches(&packet, question.name, host) {
        return Ok(None);
    }
    if packet.flags().contains(DnsFlags::TRUNCATED) {
        return Err("DNS truncated response");
    }
    match packet.rcode() {
        DnsRcode::NXDomain => return Err("DNS name does not exist"),
        DnsRcode::NoError => (),
        _ => return Err("DNS resolver error"),
    }
    if packet.answer_record_count() != 1 {
        return Err("DNS expected one address");
    }
    let Ok((_, record)) = DnsRecord::parse(records) else {
        return Ok(None);
    };
    if !name_matches(&packet, record.name, host) {
        return Ok(None);
    }
    match record.data {
        DnsRecordData::A(ip) => Ok(Some(ip)),
        _ => Err("DNS expected an A record"),
    }
}

pub(super) fn resolve(
    host: &str,
    device: &mut Ethernet,
    iface: &mut Interface,
) -> Result<Ipv4Address, Failure> {
    // Only the resolver port and fresh query entropy are injected. The HTTP address arrives on the NIC.
    let mut config = [0; 6];
    if !load(b"dns.cfg\0", &mut config) || config[..2] == [0, 0] {
        return Err("invalid DNS configuration");
    }
    let resolver = IpEndpoint::new(
        Ipv4Address::new(10, 0, 2, 2).into(),
        u16::from_be_bytes([config[0], config[1]]),
    );
    let id = u16::from_be_bytes([config[2], config[3]]);
    let local = 49152 + u16::from_be_bytes([config[4], config[5]]) % 16384;
    let mut name = [0u8; 255];
    let mut used = 0;
    for label in host.as_bytes().split(|&b| b == b'.') {
        if label.is_empty() || label.len() > 63 || used + label.len() + 2 > name.len() {
            return Err("invalid DNS name");
        }
        name[used] = label.len() as u8;
        name[used + 1..used + 1 + label.len()].copy_from_slice(label);
        used += 1 + label.len();
    }
    let query = DnsRepr {
        transaction_id: id,
        opcode: DnsOpcode::Query,
        flags: DnsFlags::RECURSION_DESIRED,
        question: DnsQuestion {
            name: &name[..=used],
            type_: DnsQueryType::A,
        },
    };
    let mut query_buf = [0u8; 512];
    let wire = &mut query_buf[..query.buffer_len()];
    query.emit(&mut DnsPacket::new_unchecked(&mut *wire));

    let mut rx_meta = [udp::PacketMetadata::EMPTY; 2];
    let mut tx_meta = [udp::PacketMetadata::EMPTY; 1];
    let mut rx_buf = [0; 1024];
    let mut tx_buf = [0; 512];
    let mut socket = udp::Socket::new(
        udp::PacketBuffer::new(&mut rx_meta[..], &mut rx_buf[..]),
        udp::PacketBuffer::new(&mut tx_meta[..], &mut tx_buf[..]),
    );
    socket
        .bind((Ipv4Address::new(10, 0, 2, 15), local))
        .map_err(|_| "DNS bind failed")?;
    let mut storage = [SocketStorage::EMPTY];
    let mut sockets = SocketSet::new(&mut storage[..]);
    let handle = sockets.add(socket);
    let start = time_us();
    let mut next_send = start;
    let mut retry_us = 1_000_000;
    while time_us() - start < 5_000_000 {
        device.poll(iface, &mut sockets)?;
        let socket = sockets.get_mut::<udp::Socket>(handle);
        while let Ok((data, peer)) = socket.recv() {
            if peer.endpoint == resolver && data.len() <= 512 {
                if let Some(address) = answer(data, id, host)? {
                    return Ok(address);
                }
            }
        }
        let now = time_us();
        if now >= next_send && socket.can_send() {
            socket
                .send_slice(wire, resolver)
                .map_err(|_| "DNS send failed")?;
            next_send = now + retry_us;
            retry_us *= 2;
        }
        sleep_us(1000);
    }
    Err("DNS timeout")
}
