//! One bounded delegated computation between local QEMU guests. No network ELF loading.
use crate::{
    arch,
    dev::{self, virtio_tx::worker_link},
    log, user,
};
use alloc::format;

type Frame = [u8; 60];
const READY: u8 = 1;
const JOB: u8 = 2;
const RESULT: u8 = 3;
const DENIED: u8 = 4;

fn frame(sender: u8, kind: u8, job: u8, cached: bool, nonce: [u8; 8], n: u8, value: u32) -> Frame {
    let mut b = [0; 60];
    b[0] = 2;
    b[5] = 3 - sender;
    b[6] = 2;
    b[11] = sender;
    b[12..14].copy_from_slice(&0x88b5u16.to_be_bytes());
    b[14..18].copy_from_slice(b"PCPW");
    b[18] = 1;
    b[19] = kind;
    b[20] = job;
    b[21] = u8::from(cached);
    b[22..30].copy_from_slice(&nonce);
    b[30] = n;
    b[31..35].copy_from_slice(&value.to_be_bytes());
    b
}

fn option<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let mut found = line
        .split_ascii_whitespace()
        .filter_map(|s| s.strip_prefix(prefix));
    let value = found.next()?;
    if found.next().is_none() {
        Some(value)
    } else {
        None
    }
}

// Fast doubling is independent of the external worker's linear recurrence.
fn fib(n: u8) -> (u32, u32) {
    if n == 0 {
        return (0, 1);
    }
    let (a, b) = fib(n / 2);
    let c = a * ((2 * b + 251 - a) % 251) % 251;
    let d = (a * a + b * b) % 251;
    if n % 2 == 0 {
        (c, d)
    } else {
        (d, (c + d) % 251)
    }
}

pub fn director(bus: &[dev::BusDevice], line: &str) -> Result<(), &'static str> {
    let nonce = dev::virtio_tx::completion::parse_nonce(line)?;
    let n = 12 + (u16::from_be_bytes([nonce[0], nonce[1]]) % 25) as u8;
    let (job, kind, value) = match option(line, "worker_case=") {
        Some("run") => (1, RESULT, fib(n).0),
        Some("unknown") => (0xfe, DENIED, 1),
        Some("wrong-image") => (1, DENIED, 2),
        _ => return Err("invalid worker case"),
    };
    worker_link::connected(bus, 1, |link| {
        log(format_args!("worker wire: director RX ready"));
        if link.receive()? != frame(2, READY, 0, false, [0; 8], 0, 0) {
            return Err("worker readiness");
        }
        let request = frame(1, JOB, job, false, nonce, n, 0);
        for cached in [false, true] {
            link.send(&request)?;
            if link.receive()? != frame(2, kind, job, cached, nonce, n, value) {
                return Err("worker result");
            }
        }
        log(format_args!(
            "worker wire: director verified input={} kind={} value={} duplicate=1",
            n, kind, value
        ));
        Ok(())
    })
}

fn image(boot: &arch::BootInfo) -> Option<&'static [u8]> {
    let pin = option(&boot.command_line, "worker_image=")?;
    let mut workers = boot.modules.iter().filter(|m| m.name == "pcp-fib");
    let bytes = arch::module_bytes(workers.next()?)?;
    if workers.next().is_some() || bytes.len() > 256 * 1024 || pcpos_core::sha256_hex(bytes) != pin
    {
        return None;
    }
    Some(bytes)
}

fn launch(image: &[u8], n: u8) -> Result<u32, &'static str> {
    let arg = format!("{}", n);
    let program = user::Program {
        name: "pcp-fib",
        image,
        args: &["pcp-fib", &arg],
        grants: user::Grants::default(),
        budget: 50,
    };
    let world = user::empty_world();
    let before = user::frames_free();
    let (child, world) = arch::without_interrupts(|| user::run_program(&program, world));
    let p = child?;
    let value = match p.end {
        Some(user::End::Exited(code)) if code < 251 => code as u32,
        _ => return Err("worker termination"),
    };
    if p.name != "pcp-fib"
        || p.calls != 1
        || p.released == 0
        || p.census.1 != 0
        || p.census.3
        || p.written != 0
        || !world.unanswered.is_empty()
        || user::frames_free() != before
    {
        return Err("worker process audit");
    }
    log(format_args!("worker wire: child pcp-fib pid={} task={} input={} exit={} calls={} released={} grants=none",
        p.pid,p.task,n,value,p.calls,p.released));
    Ok(value)
}

pub fn receiver(bus: &[dev::BusDevice], boot: &arch::BootInfo) -> Result<(), &'static str> {
    let worker = image(boot);
    worker_link::connected(bus, 2, |link| {
        link.send(&frame(2, READY, 0, false, [0; 8], 0, 0))?;
        let request = link.receive()?;
        let job = request[20];
        let n = request[30];
        let mut nonce = [0u8; 8];
        nonce.copy_from_slice(&request[22..30]);
        if !(12..=36).contains(&n) || request != frame(1, JOB, job, false, nonce, n, 0) {
            return Err("worker malformed job");
        }
        let (kind, value, launches) = if job != 1 {
            (DENIED, 1, 0)
        } else if let Some(bytes) = worker {
            (RESULT, launch(bytes, n)?, 1)
        } else {
            (DENIED, 2, 0)
        };
        link.send(&frame(2, kind, job, false, nonce, n, value))?;
        if link.receive()? != request {
            return Err("worker duplicate changed");
        }
        link.send(&frame(2, kind, job, true, nonce, n, value))?;
        log(format_args!("worker wire: receiver launches={}", launches));
        Ok(())
    })
}
