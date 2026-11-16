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

const MAX_SERIAL_BYTES: u64 = 1024 * 1024;

fn attest_guest_serial(serial: &str, measurement: &str) -> Result<(), &'static str> {
    // This is independent checking of QEMU's actual serial, not the proof of
    // authenticity of the guest or of a person/ACR participant.
    let native_banner = serial.lines().any(|line| line.starts_with("PCP OS kernel ") && line.contains("x86_64"));
    if !native_banner {
        return Err("no PCP OS x86_64 kernel banner");
    }
    let expected = format!(
        "running image (code and read-only data), SHA-256: {} (measured; equal to the value the loader named)",
        measurement
    );
    if serial.lines().filter(|line| *line == expected).count() != 1 {
        return Err("kernel did not exactly confirm this externally named image");
    }
    if serial.lines().filter(|line| line.starts_with("ALL SELF-TESTS PASS.")).count() != 1
        || serial.contains("SELF-TEST(S) FAILED")
    {
        return Err("native self-test success absent or contradicted");
    }
    let mut heads = serial.lines().filter(|line| line.starts_with("kernel register: "));
    let head = heads.next().ok_or("native register receipt absent")?;
    if heads.next().is_some() {
        return Err("ambiguous duplicate register receipt");
    }
    let after = head.strip_prefix("kernel register: ").ok_or("native register prefix missing")?;
    let (count, digest) = after.split_once(" events, head ").ok_or("invalid register receipt")?;
    if count.parse::<usize>().map_err(|_| "invalid register event count")? < 7
        || digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("invalid register count or SHA-256");
    }
    Ok(())
}

fn attest_guest_serial_file(path: &Path, measurement: &str) -> io::Result<()> {
    let file = File::open(path)?;
    let mut data = String::new();
    file.take(MAX_SERIAL_BYTES + 1).read_to_string(&mut data)?;
    if data.len() as u64 > MAX_SERIAL_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "native serial exceeds 1MiB limit"));
    }
    attest_guest_serial(&data, measurement).map_err(|why| io::Error::new(io::ErrorKind::InvalidData, why))
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

    let serial_path = base.join(format!("qemu-{next}.serial"));
    let stderr_path = base.join(format!("qemu-{next}.stderr"));
    // A preexisting output is an ambiguous crash residue, not permission
    // to truncate somebody else's previous evidence.
    let serial = OpenOptions::new().write(true).create_new(true).open(&serial_path)?;
    let stderr = OpenOptions::new().write(true).create_new(true).open(&stderr_path)?;
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
    // Adversarial harness-only arm: simulate an exit code of 33 without
    // launching QEMU, and leave a clearly non-native serial. This checks
    // whether an exit-code-only end oracle could fabricate completion.
    let spoofed = env::var("D12_SPOOF_EXIT33").as_deref() == Ok("1");
    let status = if spoofed { None } else { Some(qemu.status()?) };
    if spoofed {
        fs::write(&serial_path, b"SYNTHETIC TEST: fabricated QEMU exit code 33 without native guest\\n")?;
    }
    if !spoofed && status.as_ref().and_then(|status| status.code()) != Some(33) {
        durable_attention(&base, &format!(
            "UNCONFIRMED: QEMU result {status:?}; no automatic duplicate launch\n"
        ))?;
        return Ok(0);
    }
    // QEMU's diagnostic exit status alone is insufficient: another process
    // could produce it without executing the expected measured guest.
    if let Err(err) = attest_guest_serial_file(&serial_path, &measurement) {
        durable_attention(&base, &format!(
            "UNCONFIRMED: QEMU exited 33 but native receipt invalid: {err}; no END\n"
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

#[cfg(test)]
mod tests {
    use super::attest_guest_serial;

    fn candidate(digest: &str, measurement: &str) -> String {
        format!(
            "PCP OS kernel 0.3.1 | x86_64 | early\n\
             running image (code and read-only data), SHA-256: {measurement} (measured; equal to the value the loader named)\n\
             kernel register: 43 events, head {digest}\n\
             ALL SELF-TESTS PASS. Present: native user mode\n"
        )
    }

    #[test]
    fn accepts_only_one_exact_native_receipt_after_measured_image_pass() {
        let digest = "a".repeat(64);
        let measurement = "b".repeat(64);
        let serial = candidate(&digest, &measurement);
        assert!(attest_guest_serial(&serial, &measurement).is_ok());
        for adversary in [
            serial.replace(&measurement, &"c".repeat(64)),
            serial.replace("43 events", "0 events"),
            serial.replace(&digest, &"z".repeat(64)),
            serial.replace("PCP OS kernel", "Other kernel"),
            serial.replace("ALL SELF-TESTS PASS.", "1 SELF-TEST(S) FAILED"),
            format!("{serial}kernel register: 43 events, head {digest}\n"),
            serial.replace("measured; equal", "measured; NOT equal"),
            serial.replace("kernel register: ", "unproven register: "),
        ] {
            assert!(
                attest_guest_serial(&adversary, &measurement).is_err(),
                "a modified receipt was falsely accepted"
            );
        }
    }
}
