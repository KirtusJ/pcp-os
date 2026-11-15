#![no_std]
#![no_main]
extern crate alloc;

mod heap;
mod record;
mod screen;

use alloc::{format, string::String};
use pcp_game_guest::{call, exit, log, open, sleep_us, Display};
use recursion_desk_core::State;
use record::Journal;
use screen::{H, W};

static mut PIXELS: [u32; W*H] = [0; W*H];

struct Editor { parent: Option<u32>, label: String }
struct App {
    journal: Journal,
    selected: usize,
    editor: Option<Editor>,
    message: String,
    stored: &'static str,
}

impl App {
    fn id(&self) -> Option<u32> {
        self.journal.desk.nodes().get(self.selected).map(|n| n.id)
    }

    fn applied(&mut self, action: &str, result: Result<u32,String>) {
        self.message = match result {
            Ok(id) => {
                self.selected = (id-1) as usize;
                self.stored = self.journal.write();
                format!("{action} / request {id}")
            }
            Err(reason) => format!("Refused / {reason}"),
        };
        log(b"PARROTS ACTION ");
        log(self.message.as_bytes());
        log(b"\n");
    }

    fn key(&mut self, code: u8, shifted: bool) -> bool {
        if self.journal.locked() {
            if code == 1 { exit(0); }
            if matches!(code, 49 | 46 | 33 | 45 | 28) {
                self.applied("", Err("queue recovery required".into()));
                return true;
            }
            return false;
        }
        if let Some(editor) = &mut self.editor {
            match code {
                1 => { self.editor = None; }
                14 => { editor.label.pop(); }
                28 => {
                    let e = self.editor.take().unwrap();
                    let result = self.journal.add(e.parent, e.label.trim());
                    self.applied("Opened", result);
                }
                _ => {
                    let Some(ch) = letter(code, shifted) else { return false };
                    if editor.label.len() >= record::MAX_LABEL { return false; }
                    editor.label.push(char::from(ch));
                }
            }
        } else {
            match code {
                1 => exit(0),
                49 => self.editor = Some(Editor { parent: None, label: String::new() }), // N
                46 if self.id().is_some() => self.editor = Some(Editor { parent: self.id(), label: String::new() }), // C
                33 | 45 if self.id().is_some() => { // F or X
                    let cancel = code == 45;
                    let result = self.journal.close(self.id().unwrap(),cancel);
                    self.applied(if cancel { "Cancelled branch" } else { "Finished" }, result);
                }
                46 | 33 | 45 => self.applied("", Err("select a request first".into())),
                72 => self.selected = self.selected.saturating_sub(1),
                80 => self.selected = (self.selected+1).min(self.journal.desk.nodes().len().saturating_sub(1)),
                _ => return false,
            }
        }
        true
    }

    fn report(&self) {
        let nodes = self.journal.desk.nodes();
        let active = nodes.iter().filter(|n| n.state==State::Active).count();
        let state = nodes.get(self.selected).map(|n| screen::state_name(n.state)).unwrap_or("NONE");
        log(format!("PARROTS QUEUE total={} active={active} selected={} state={state} store={}\n",
                    nodes.len(),self.id().unwrap_or(0),self.stored).as_bytes());
    }
}

fn letter(code: u8, shifted: bool) -> Option<u8> {
    let ascii = match code {
        2..=11 => b"1234567890"[(code-2) as usize],
        12 => b'-', 52 => b'.', 53 => b'/', 57 => b' ',
        16..=25 => b"qwertyuiop"[(code-16) as usize],
        30..=38 => b"asdfghjkl"[(code-30) as usize],
        44..=50 => b"zxcvbnm"[(code-44) as usize],
        _ => return None,
    };
    Some(if shifted { ascii.to_ascii_uppercase() } else { ascii })
}

pcp_game_guest::entry!(parrots_main);
#[no_mangle]
pub extern "C" fn parrots_main() -> ! {
    let mut display = Display::new(W,H);
    // Read ordered events for text input. Display::poll reports a set of keys
    // and discards the order of two letters pressed between frames.
    let keyboard = open(b"/dev/kbd\0", 0x800);
    if keyboard < 0 { log(b"PARROTS keyboard unavailable\n"); exit(2); }
    let (journal, stored) = Journal::read();
    let mut app = App { journal, selected:0, editor:None, message:stored.into(), stored };
    let px = unsafe { &mut *(&raw mut PIXELS) };
    let mut held = [false;128];
    let mut dirty = true;
    let mut first = true;
    loop {
        if dirty {
            let edit = app.editor.as_ref().map(|e| (e.parent.is_some(),e.label.as_str()));
            screen::draw(px,app.journal.desk.nodes(),app.selected,&app.message,app.stored,edit,app.journal.locked());
            display.show(px);
            if first { log(b"PARROTS READY\n"); first=false; }
            if app.editor.is_none() { app.report(); }
            dirty=false;
        }
        loop {
            let mut e = [0u8;2];
            if call(0,keyboard as usize,e.as_mut_ptr() as usize,2,0,0,0)!=2 { break; }
            let k = (e[0]&127) as usize;
            let pressed = e[1]&1==0;
            let fresh = pressed && !held[k];
            held[k]=pressed;
            if fresh && app.key(k as u8,held[42] || held[54]) { dirty=true; }
        }
        sleep_us(10_000);
    }
}
