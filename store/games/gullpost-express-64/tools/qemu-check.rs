//! Run from the game folder: qemu-check <new-output-folder>
use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};
fn wait(ms: u64) {
    sleep(Duration::from_millis(ms))
}
struct Vm {
    child: Child,
    input: ChildStdin,
    out: BufReader<ChildStdout>,
    dir: PathBuf,
}
impl Drop for Vm {
    fn drop(&mut self) {
        let _ = writeln!(self.input, r#"{{"execute":"quit"}}"#);
        let _ = self.input.flush();
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            wait(50);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Vm {
    fn line(&mut self) -> io::Result<String> {
        let mut s = String::new();
        if self.out.read_line(&mut s)? == 0 {
            return Err(io::Error::other("QEMU monitor closed"));
        }
        Ok(s)
    }
    fn tell(&mut self, s: &str) -> io::Result<()> {
        writeln!(self.input, "{s}")?;
        self.input.flush()?;
        loop {
            let r = self.line()?;
            if r.contains("\"error\"") {
                return Err(io::Error::other(r));
            }
            if r.contains("\"return\"") {
                return Ok(());
            }
        }
    }
    fn key(&mut self, s: &str, hold: u64) -> io::Result<()> {
        self.tell(&format!(r#"{{"execute":"human-monitor-command","arguments":{{"command-line":"sendkey {s} {hold}"}}}}"#))
    }
    fn set_key(&mut self, key: &str, down: bool) -> io::Result<()> {
        self.tell(&format!(r#"{{"execute":"input-send-event","arguments":{{"events":[{{"type":"key","data":{{"down":{down},"key":{{"type":"qcode","data":"{key}"}}}}}}]}}}}"#))
    }
    fn snap(&mut self, name: &str) -> io::Result<()> {
        let path = self.dir.join(format!("{name}.ppm"));
        let path = path
            .to_string_lossy()
            .replace('\\', "/")
            .replace('"', "\\\"");
        self.tell(&format!(
            r#"{{"execute":"screendump","arguments":{{"filename":"{path}"}}}}"#
        ))
    }
    fn until(&mut self, s: &str, qty: usize, timeout: u64) -> io::Result<()> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(timeout) {
            let data = fs::read_to_string(self.dir.join("serial.txt")).unwrap_or_default();
            if data.contains("it is in SAFE") {
                return Err(io::Error::other(format!(
                    "kernel entered SAFE before launch: {}",
                    data.lines()
                        .filter(|l| l.starts_with("  FAIL"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                )));
            }
            if data.matches(s).count() >= qty {
                return Ok(());
            }
            if self.child.try_wait()?.is_some() {
                break;
            }
            wait(60)
        }
        Err(io::Error::other(format!(
            "guest marker missing: {s} x{qty}"
        )))
    }
}
fn main() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    let dir = PathBuf::from(args.next().expect("supply a new QA folder"));
    fs::create_dir_all(&dir)?;
    let dir = dir.canonicalize()?;
    let serial = dir.join("serial.txt");
    if serial.exists() {
        return Err(io::Error::other("Use a fresh evidence directory"));
    }
    let disk = |p: &Path| {
        p.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/")
    };
    // Use the public launcher so a missing or misnamed Store module fails this check.
    let mut command = Command::new("sh");
    command.arg("../../play-native.sh");
    if let Some(share) = env::var_os("PCPOS_QEMU_SHARE") {
        command.arg("-L").arg(share);
    }
    command
        .args(["-accel", "tcg", "-display", "none", "-audiodev"])
        .arg(format!("wav,id=out,path={}", disk(&dir.join("audio.wav"))))
        .args(["-device", "AC97,audiodev=out", "-qmp", "stdio", "-serial"])
        .arg(format!("file:{}", disk(&serial)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(fs::File::create(dir.join("qemu.txt"))?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn()?;
    let input = child.stdin.take().unwrap();
    let out = BufReader::new(child.stdout.take().unwrap());
    let mut vm = Vm {
        child,
        input,
        out,
        dir: PathBuf::from(disk(&dir)),
    };
    vm.line()?;
    vm.tell(r#"{"execute":"qmp_capabilities"}"#)?;

    vm.until("a program that cannot be admitted", 1, 25)?;
    wait(500);
    // The kernel presents modules in loader order. Discover the slot afresh for each run.
    let boot_log = fs::read_to_string(&serial)?;
    let slot = boot_log
        .lines()
        .filter_map(|line| line.strip_prefix("file from the loader: "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.ends_with(".elf"))
        .position(|name| name.rsplit('/').next() == Some("gull64.elf"))
        .ok_or_else(|| io::Error::other("gull64.elf is missing from the native Store menu"))?;
    for _ in 0..slot {
        vm.key("down", 60)?;
        wait(180);
    }
    vm.key("ret", 80)?;
    vm.until("G64: cover ready", 1, 15)?;
    wait(300);
    vm.snap("cover")?;
    vm.key("spc", 85)?;
    vm.until("G64: free play", 1, 5)?;
    wait(250);
    vm.snap("pier")?;
    vm.set_key("right", true)?;
    vm.until("G64: retry", 1, 15)?;
    vm.set_key("right", false)?;
    wait(250);
    vm.key("up", 350)?;
    wait(440);
    vm.key("up-spc", 225)?;
    vm.until("G64: jump", 1, 3)?;
    wait(270);
    vm.key("spc", 90)?;
    vm.until("G64: flap", 1, 3)?;
    wait(200);
    vm.key("e", 330)?;
    wait(420);
    vm.snap("orbit")?;
    vm.key("p", 80)?;
    vm.until("G64: paused", 1, 4)?;
    wait(120);
    vm.key("p", 80)?;
    vm.until("G64: free play", 2, 5)?;
    wait(160);
    vm.key("r", 80)?;
    vm.until("G64: retry", 2, 4)?;
    wait(250);
    vm.key("esc", 80)?;
    vm.until("G64: exit to PCP OS", 1, 4)?;
    wait(500);
    vm.key("ret", 80)?;
    vm.until("G64: cover ready", 2, 6)?;
    // Three independently restarted campaigns keep the island, input policy and timing scope fixed.
    for run in 1..=3 {
        vm.key("f2", 80)?;
        vm.until("G64: guided run", run, 6)?;
        vm.until("G64: postmarks mask=1", run, 15)?;
        if run == 1 {
            vm.snap("shore")?;
        }
        vm.until("G64: postmarks mask=3", run, 16)?;
        if run == 1 {
            vm.snap("rooftop")?;
        }
        vm.until("G64: postmarks mask=7", run, 16)?;
        if run == 1 {
            vm.snap("lantern")?;
        }
        vm.until("G64: letter delivered", run, 16)?;
        if run == 1 {
            wait(130);
            vm.snap("delivered")?;
        }
        println!("Delivery {run} completed.");
        vm.key("spc", 80)?;
        vm.until("G64: cover ready", run + 2, 6)?;
        wait(150);
    }
    vm.key("esc", 80)?;
    vm.until("G64: exit to PCP OS", 2, 5)?;
    wait(400);
    vm.key("p", 80)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = vm.child.try_wait()? {
            // play-native.sh translates the verified QEMU exit (33) into shell success.
            if !status.success() {
                return Err(io::Error::other(format!(
                    "unexpected Store launcher exit {status}"
                )));
            }
            break;
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("kernel shutdown timed out"));
        }
        wait(60);
    }
    let log = fs::read_to_string(serial)?;
    if !log.contains("ALL SELF-TESTS PASS") {
        return Err(io::Error::other("kernel did not verify shutdown"));
    }
    for l in log.lines().filter(|l| l.starts_with("G64 PERF")) {
        println!("{l}")
    }
    Ok(())
}
