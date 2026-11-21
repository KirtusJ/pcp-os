"""Observe keyboard_init on host-side controller replies without opening devices.

Run with python3 keyboard_init.py /path/to/pcp-os. RUSTC may select a host compiler.
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

repo = Path(sys.argv[1]).resolve()
source = (repo / "PCP_OS_Rust/crates/pcpos-kernel/src/dev.rs").read_text()


def function(name):
    start = source.index("fn " + name + "(")
    return source[start:source.index("\n}", start) + 2]


model = """
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool,Ordering};
#[derive(Default)]
struct Keyboard { present: bool }
struct Holder(Mutex<Keyboard>);
impl Holder {
    fn with<F: FnOnce(&mut Keyboard)>(&self, f: F) { f(&mut self.0.lock().unwrap()) }
}
static KEYBOARD: Holder = Holder(Mutex::new(Keyboard {present:false}));
static IRQ: AtomicBool = AtomicBool::new(false);
mod arch {
    use super::*;
    pub fn without_interrupts<F: FnOnce() -> T, T>(f: F) -> T { f() }
    pub fn interrupt_line_on(n: u8) { assert_eq!(n,1); IRQ.store(true,Ordering::SeqCst); }
}
struct Controller { case: &'static str, output: Option<u8>, writes: Vec<(u16,u8)> }
static CTRL: Mutex<Controller> = Mutex::new(Controller {case:"",output:None,writes:Vec::new()});
fn in8(port: u16) -> u8 {
    let mut c = CTRL.lock().unwrap();
    if port==0x60 { return c.output.take().expect("reading an empty output register"); }
    assert_eq!(port,0x64);
    if c.case=="no-controller" { return 0xff; }
    let busy = c.case=="busy-throughout" ||
        (c.case=="busy-after-config-command" && c.writes.contains(&(0x64,0x60)));
    (if busy { 2 } else { 0 }) | u8::from(c.output.is_some())
}
fn out8(port: u16, value: u8) {
    let mut c = CTRL.lock().unwrap();
    c.writes.push((port,value));
    if (port,value)==(0x64,0x20) && !matches!(c.case,"busy-throughout"|"no-config-reply") {
        c.output=Some(0x04);
    }
    if (port,value)==(0x60,0xf4) {
        c.output=match c.case {"ack"=>Some(0xfa),"resend"=>Some(0xfe),_=>None};
    }
}
fn main() {
    for case in ["no-controller","busy-throughout","no-config-reply","ack","no-keyboard-reply","resend","busy-after-config-command"] {
        *CTRL.lock().unwrap() = Controller {case,output:None,writes:Vec::new()};
        KEYBOARD.with(|k| k.present=false);
        IRQ.store(false,Ordering::SeqCst);
        let found=keyboard_init();
        let c=CTRL.lock().unwrap();
        let writes=c.writes.iter().map(|(p,v)| format!("{:02X}={:02X}",p,v)).collect::<Vec<_>>().join(",");
        println!("{case} found={found} irq={} writes=[{writes}]",IRQ.load(Ordering::SeqCst));
    }
}
"""
with tempfile.TemporaryDirectory() as td:
    work = Path(td)
    (work / "init.rs").write_text(model + "\n".join(function(name) for name in
                                                    ("controller_wait_write", "controller_read", "keyboard_init")))
    subprocess.run([os.environ.get("RUSTC", "rustc"), "--edition=2021", str(work / "init.rs"),
                    "-o", str(work / "init")], check=True)
    subprocess.run([str(work / "init")], check=True)
