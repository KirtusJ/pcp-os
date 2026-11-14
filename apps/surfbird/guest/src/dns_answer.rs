//! Matching rules for the local direct-A DNS profile.
use smoltcp::wire::{
    DnsFlags, DnsOpcode, DnsPacket, DnsQueryType, DnsQuestion, DnsRcode, DnsRecord, DnsRecordData,
    Ipv4Address,
};

fn name_matches(packet: &DnsPacket<&[u8]>, encoded: &[u8], host: &str) -> bool {
    let mut labels = packet.parse_name(encoded);
    for wanted in host.as_bytes().split(|&b| b == b'.') {
        if !matches!(labels.next(), Some(Ok(label)) if label.eq_ignore_ascii_case(wanted)) {
            return false;
        }
    }
    labels.next().is_none()
}

pub fn answer(bytes: &[u8], id: u16, host: &str) -> Result<Option<Ipv4Address>, &'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    const RESPONSE: &[u8] = b"\x12\x34\x85\x00\x00\x01\x00\x01\x00\x00\x00\x00\x07fixture\x04test\0\0\x01\0\x01\xc0\x0c\0\x01\0\x01\0\0\0\x3c\0\x04\x0a\0\x02\x55";

    #[test]
    fn binds_the_response_to_the_transaction_question_and_owner() {
        let check = |bytes: &[u8]| answer(bytes, 0x1234, "FIXTURE.test");
        assert_eq!(check(RESPONSE), Ok(Some(Ipv4Address::new(10, 0, 2, 85))));
        for (position, value) in [
            (0, 0),
            (2, 0),
            (5, 2),
            (13, b'x'),
            (27, 28),
            (29, 3),
            (31, 20),
            (35, 3),
        ] {
            let mut changed = [0; 46];
            changed.copy_from_slice(RESPONSE);
            changed[position] = value;
            assert_eq!(check(&changed), Ok(None), "field at {position}");
        }
        for len in 0..RESPONSE.len() {
            assert_eq!(check(&RESPONSE[..len]), Ok(None), "length {len}");
        }
    }

    #[test]
    fn matched_truncation_and_negative_answers_end_resolution() {
        let mut msg = [0; 46];
        msg.copy_from_slice(RESPONSE);
        msg[2] |= 2;
        assert_eq!(
            answer(&msg, 0x1234, "fixture.test"),
            Err("DNS truncated response")
        );
        msg[2] &= !2;
        msg[3] = 3;
        msg[7] = 0;
        assert_eq!(
            answer(&msg[..30], 0x1234, "fixture.test"),
            Err("DNS name does not exist")
        );
        assert_eq!(answer(&msg[..30], 0x1234, "another.test"), Ok(None));
    }
}
