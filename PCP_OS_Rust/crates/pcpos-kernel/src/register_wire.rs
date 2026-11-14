//! A transfer fixture for one previously published record transition.
use crate::dev::{virtio_tx, BusDevice};
use crate::{log, EXERCISE_HEAD, EXERCISE_REGISTER};
use pcpos_core::acr::Record;

const BASE: &str = "3112244aad5d5015c2d9b86a0b96e2317adcc537bc2e641d221f0622b86cb56f";
const PRIOR_EVENTS: u64 = 12;
const MAX_TAIL: usize = 1024;
const ENVELOPE: usize = 28;
const MACS: [u8; 14] = [2, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 1, 0x88, 0xb5];

fn prior() -> Result<Record, &'static str> {
    // The existing kernel self-test also embeds the full published vector. Only these
    // first twelve lines initialize the replica; the transfer never reads the final line.
    let n = EXERCISE_REGISTER
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .nth(PRIOR_EVENTS as usize - 1)
        .ok_or("missing prefix")?
        .0
        + 1;
    let (record, problems) =
        Record::open(&EXERCISE_REGISTER[..n]).map_err(|_| "invalid local prefix")?;
    if !problems.is_empty()
        || record.ledger().len() != PRIOR_EVENTS
        || record.ledger().head() != BASE
    {
        return Err("invalid local prefix");
    }
    Ok(record)
}

fn stage(record: &mut Record, tail: &[u8]) -> Result<(), &'static str> {
    let live = record.ledger();
    if live.len() != PRIOR_EVENTS || live.head() != BASE {
        return Err("stale base");
    }
    if tail.is_empty() || tail.len() > MAX_TAIL || tail.last() != Some(&b'\n') {
        return Err("incomplete tail");
    }
    let mut bytes = live.bytes().to_vec();
    bytes.extend_from_slice(tail);
    // Record::open validates the canonical chain and folds all guards; a successful
    // return may still list replay problems. All work happens on a candidate.
    let (candidate, problems) = Record::open(&bytes).map_err(|_| "invalid extension")?;
    if !problems.is_empty() {
        return Err("guard replay");
    }
    if candidate.ledger().len() != PRIOR_EVENTS + 1 || candidate.ledger().head() != EXERCISE_HEAD {
        return Err("unexpected target");
    }
    *record = candidate;
    Ok(())
}

fn request(challenge: [u8; 8], attempt: u8) -> [u8; 60] {
    let mut bytes = [0; 60];
    bytes[..14].copy_from_slice(&MACS);
    bytes[14..18].copy_from_slice(b"PCPR");
    bytes[18] = 1; // protocol version
    bytes[19] = attempt; // one-based, never re-used in a boot
    bytes[20..28].copy_from_slice(&challenge);
    bytes
}

fn tail<'a>(frame: &'a [u8], request: &[u8; 60]) -> Result<&'a [u8], &'static str> {
    if frame.len() < ENVELOPE + 2
        || frame[..6] != request[6..12]
        || frame[6..12] != request[..6]
        || frame[12..28] != request[12..28]
    {
        return Err("wrong envelope");
    }
    let n = u16::from_be_bytes([frame[28], frame[29]]) as usize;
    if n == 0 || n > MAX_TAIL || frame.len() != ENVELOPE + 2 + n {
        return Err("wrong extent");
    }
    Ok(&frame[30..])
}

pub fn fixture(bus: &[BusDevice], cmdline: &str) -> Result<(), &'static str> {
    let nonce = virtio_tx::completion::parse_nonce(cmdline)?;
    let mut record = prior()?;
    log(format_args!(
        "register replica: base {} {}",
        record.ledger().len(),
        record.ledger().head()
    ));
    for attempt in 1..=2 {
        let request = request(nonce, attempt);
        let mut reply = [0; ENVELOPE + 2 + MAX_TAIL];
        // A transport error also covers an unconfirmed stop. End the fixture immediately;
        // never issue another transfer to a device which failed to quiesce.
        let n = virtio_tx::register_frame(bus, &request, &mut reply)?;
        let accepted = tail(&reply[..n], &request).and_then(|bytes| stage(&mut record, bytes));
        log(format_args!(
            "register replica: attempt {} {:?} {} {}",
            attempt,
            accepted,
            record.ledger().len(),
            record.ledger().head()
        ));
    }
    if record.ledger().head() == EXERCISE_HEAD && record.ledger().len() == PRIOR_EVENTS + 1 {
        Ok(())
    } else {
        Err("target absent")
    }
}
