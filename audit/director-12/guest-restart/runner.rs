//! Disposable, fail-closed journaled QEMU launcher for one synthetic test.
//! It is NOT a production supervisor, service, ACR enforcement layer, or PCP kernel.
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
    thread,
    time::Duration,
};

fn persist_new(path: &Path, text: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

fn read(path: &Path) -> io::Result<String> {
    let mut output = String::new();
    File::open(path)?.read_to_string(&mut output)?;
    Ok(output)
}

fn hex64(input: &str) -> bool {
    input.len() == 64 && input.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn durable_attention(state: &Path, reason: &str) -> io::Result<()> {
    let path = state.join("attention.txt");
    if !path.exists() {
        persist_new(&path, reason)?;
    }
    Ok(())
}

fn test_service() -> io::Result<i32> {
    let base = PathBuf::from("/state");
    let request = env::var("D12_REQUEST").map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let measurement = env::var("D12_MEASUREMENT")
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    if request.len() != 24 || !request.bytes().all(|b| b.is_ascii_hexdigit()) || !hex64(&measurement) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid synthetic request or image pin"));
    }
    let pin = base.join("request.txt");
    if !pin.exists() {
        persist_new(&pin, &request)?;
    }
    if read(&pin)? != request {
        durable_attention(&base, "REQUEST_MISMATCH: stale unrelated test state was supplied\n")?;
        return Ok(0);
    }

    // Completion is not inferred from a START marker, a container restart,
    // or the mere presence of raw serial data. Only this test runner's END
    // marker authorizes the next synthetic run.
    let mut next = 1usize;
    for n in 1..=3 {
        let start = base.join(format!("start-{n}.txt"));
        let end = base.join(format!("end-{n}.txt"));
        if end.exists() && !start.exists() {
            durable_attention(&base, "INVALID_JOURNAL: END exists without START\n")?;
            return Ok(0);
        }
        if start.exists() && !end.exists() {
            durable_attention(&base, &format!("INDETERMINATE: run {n} START without END; no automatic retry\n"))?;
            return Ok(0);
        }
        if start.exists() && end.exists() {
            if read(&start)? != format!("REQUEST={request};ATTEMPT={n};START\n")
                || read(&end)? != format!("REQUEST={request};ATTEMPT={n};QEMU_EXIT=33;END\n")
            {
                durable_attention(&base, "INVALID_JOURNAL: original entries incompatible\n")?;
                return Ok(0);
            }
            next = n + 1;
        } else {
            break;
        }
    }

    if next > 3 {
        return Ok(0);
    }
    let start = base.join(format!("start-{next}.txt"));
    persist_new(&start, &format!("REQUEST={request};ATTEMPT={next};START\n"))?;

    // This fault occurs AFTER the START record is flushed and BEFORE
    // launching QEMU. A Docker-managed restart encounters unresolved prior
    // work and refuses to invent a completion or launch a duplicate guest.
    if env::var("D12_FAULT_PRESPAWN").as_deref() == Ok("1") {
        thread::sleep(Duration::from_millis(11_200));
        process::abort();
    }

    let serial = File::create(base.join(format!("qemu-{next}.serial")))?;
    let stderr = File::create(base.join(format!("qemu-{next}.stderr")))?;
    let mut qemu = Command::new("/usr/bin/qemu-system-x86_64");
    qemu.args([
        "-accel", "tcg", "-cpu", "max", "-m", "128M",
        "-display", "none", "-monitor", "none", "-no-reboot",
        "-kernel", "/kernel.elf32",
        "-serial", "stdio", "-net", "none",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ])
    .arg("-append").arg(format!("then=off image={measurement}"))
    .stdout(Stdio::from(serial))
    .stderr(Stdio::from(stderr));
    if env::var("D12_HOLD_GUEST").as_deref() == Ok("1") {
        // Deliberate test-only post-spawn crash window: QEMU remains alive
        // with vCPUs paused. This is *not* a completed or booted PCP guest.
        qemu.arg("-S");
    }
    let status = qemu.status()?;

    if status.code() != Some(33) {
        durable_attention(&base, &format!(
            "UNCONFIRMED: QEMU result {status:?}; no automatic duplicate launch\n"
        ))?;
        return Ok(0);
    }
    persist_new(
        &base.join(format!("end-{next}.txt")),
        &format!("REQUEST={request};ATTEMPT={next};QEMU_EXIT=33;END\n"),
    )?;

    // Docker's on-failure restart policy is deliberately exercised on a
    // synthetic nonzero wrapper exit. This is not a kernel crash or recovery.
    // Its >=10-second lifetime satisfies Docker's restart activation floor.
    if next < 3 {
        thread::sleep(Duration::from_millis(11_200));
        Ok(42)
    } else {
        Ok(0)
    }
}

fn main() {
    match test_service() {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("D12 disposable runner failed: {e}");
            process::exit(2);
        }
    }
}
