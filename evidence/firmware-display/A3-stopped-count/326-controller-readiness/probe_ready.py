"""Run the kernel's keyboard replay probe against a host-side controller model.

Usage: python3 probe_ready.py /path/to/pcp-os
RUSTC may name an absolute path to the host compiler. No devices are opened.
"""
import argparse
import os
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("repo", type=Path)
parser.add_argument("--expect", choices=("before", "corrected"))
args = parser.parse_args()
repo = args.repo.resolve()
source = (repo / "PCP_OS_Rust/crates/pcpos-kernel/src/dev.rs").read_text()


def function(name):
    start = source.index("fn " + name + "(")
    end = source.index("\n}", start) + 2
    return source[start:end]


model = """
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
static PROBE: AtomicU32 = AtomicU32::new(0);
static PROBE_HIT: AtomicBool = AtomicBool::new(false);
static BEFORE: AtomicU32 = AtomicU32::new(0);
static AFTER: AtomicU32 = AtomicU32::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);
static WRITES: Mutex<Vec<(u16,u8)>> = Mutex::new(Vec::new());
static MODE: Mutex<String> = Mutex::new(String::new());
fn keys_forget() {}
mod arch {
    use super::*;
    pub fn ticks() -> u64 { TICKS.load(Ordering::SeqCst) }
    pub fn wait_until(n: u64) { TICKS.store(n, Ordering::SeqCst); }
    pub fn without_interrupts<F: FnOnce() -> T, T>(f: F) -> T { f() }
}
fn in8(port: u16) -> u8 {
    assert_eq!(port, 0x64);
    let after_command = !WRITES.lock().unwrap().is_empty();
    let n = if after_command { AFTER.fetch_add(1, Ordering::SeqCst) } else { BEFORE.fetch_add(1, Ordering::SeqCst) };
    match MODE.lock().unwrap().as_str() {
        "input-busy" => 2,
        "output-full" => 1,
        "busy-after-command" if after_command => 2,
        "ready-after-three-ticks" if arch::ticks() < 3 => 2,
        "busy-on-recheck" if !after_command && n > 0 => 2,
        "full-on-recheck" if !after_command && n > 0 => 1,
        "ready-at-deadline" if arch::ticks() < 20 => 2,
        "ready-after-deadline" if arch::ticks() < 21 => 2,
        "ready-on-last-poll" if after_command && n < 99_999 => 2,
        "ready-one-poll-late" if after_command && n < 100_000 => 2,
        _ => 0,
    }
}
fn out8(port: u16, value: u8) {
    WRITES.lock().unwrap().push((port,value));
    if (port,value)==(0x60,0xf4) && matches!(MODE.lock().unwrap().as_str(), "ready-replay" | "ready-after-three-ticks" | "ready-at-deadline" | "ready-after-deadline" | "ready-on-last-poll" | "ready-one-poll-late" | "busy-on-recheck" | "full-on-recheck") {
        PROBE_HIT.store(true, Ordering::SeqCst);
    }
}
fn main() {
    for case in ["ready-replay", "ready-no-reply", "ready-after-three-ticks", "input-busy", "output-full", "busy-after-command", "busy-on-recheck", "full-on-recheck", "ready-at-deadline", "ready-after-deadline", "ready-on-last-poll", "ready-one-poll-late"] {
        *MODE.lock().unwrap() = case.to_string();
        WRITES.lock().unwrap().clear();
        BEFORE.store(0, Ordering::SeqCst);
        AFTER.store(0, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        PROBE_HIT.store(false, Ordering::SeqCst);
        PROBE.store(0, Ordering::SeqCst);
        let result = keyboard_replays();
        let writes = WRITES.lock().unwrap().iter().map(|(p,v)| format!("{:02X}={:02X}",p,v)).collect::<Vec<_>>().join(",");
        println!("{case} replay={result} ticks={} writes=[{writes}]",arch::ticks());
    }
}
"""
with tempfile.TemporaryDirectory() as d:
    work = Path(d)
    (work / "probe.rs").write_text(model + function("controller_wait_write") + "\n" + function("keyboard_replays"))
    subprocess.run([os.environ.get("RUSTC", "rustc"), "--edition=2021", str(work / "probe.rs"), "-o", str(work / "probe")], check=True)
    result = subprocess.check_output([str(work / "probe")], text=True)
print(result, end="")
if args.expect:
    # Entries give the expected echo, elapsed model ticks and number of port writes.
    expected = {
        "ready-replay": (True, 0, 2), "ready-no-reply": (False, 20, 2),
        "ready-after-three-ticks": (True, 3, 2), "input-busy": (False, 20, 0),
        "output-full": (False, 20, 0), "busy-after-command": (False, 0, 1),
        "busy-on-recheck": (False, 0, 0), "full-on-recheck": (False, 0, 0),
        "ready-at-deadline": (True, 20, 2), "ready-after-deadline": (False, 20, 0),
        "ready-on-last-poll": (True, 0, 2), "ready-one-poll-late": (False, 0, 1),
    }
    if args.expect == "before":
        expected.update({"input-busy": (False, 40, 2), "output-full": (False, 40, 2),
                         "busy-after-command": (False, 20, 2), "busy-on-recheck": (True, 0, 2),
                         "full-on-recheck": (True, 0, 2), "ready-after-deadline": (True, 20, 2),
                         "ready-one-poll-late": (True, 0, 2)})
    writes = ("", "64=D2", "64=D2,60=F4")
    lines = [f"{case} replay={str(echo).lower()} ticks={ticks} writes=[{writes[count]}]"
             for case, (echo, ticks, count) in expected.items()]
    if result.splitlines() != lines:
        raise SystemExit("one or more observed results differed from " + args.expect)
    print(len(lines), "observations matched", args.expect)
