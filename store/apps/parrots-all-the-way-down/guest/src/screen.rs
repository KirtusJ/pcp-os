use pcp_game_guest::{digits, ink::Frame};
use recursion_desk_core::{Node, State};
use crate::record::{MAX_DEPTH, MAX_LIVE, MAX_TOTAL};

pub const W: usize = 800;
pub const H: usize = 480;
const INK: u32 = 0x193c31;
const LEAF: u32 = 0x216647;
const LIME: u32 = 0xbce99b;
const PAPER: u32 = 0xf8f3df;
const LIGHT: u32 = 0xfffcf0;
const RULE: u32 = 0xbec9af;
const ORANGE: u32 = 0xf5a067;

// The shared pixel font supplies uppercase letters, digits and punctuation.
// These lowercase glyphs retain the title and the operator's label casing.
const LOWER: [[u8; 7]; 26] = [
    [0,0,14,1,15,17,15],[16,16,30,17,17,17,30],[0,0,14,17,16,17,14],
    [1,1,15,17,17,17,15],[0,0,14,17,31,16,14],[6,8,8,28,8,8,8],
    [0,15,17,17,15,1,14],[16,16,30,17,17,17,17],[4,0,12,4,4,4,14],
    [2,0,6,2,2,18,12],[16,16,18,20,24,20,18],[12,4,4,4,4,4,14],
    [0,0,26,21,21,21,21],[0,0,30,17,17,17,17],[0,0,14,17,17,17,14],
    [0,30,17,17,30,16,16],[0,15,17,17,15,1,1],[0,0,22,25,16,16,16],
    [0,0,15,16,14,1,30],[8,8,28,8,8,9,6],[0,0,17,17,17,19,13],
    [0,0,17,17,17,10,4],[0,0,17,17,21,21,10],[0,0,17,10,4,10,17],
    [0,17,17,17,15,1,14],[0,0,31,2,4,8,31],
];
fn text(f: &mut Frame, x: i32, y: i32, value: &str, z: i32, color: u32) {
    for (i, b) in value.bytes().enumerate() {
        let x = x + i as i32 * 6 * z;
        if b.is_ascii_lowercase() {
            for (row, bits) in LOWER[(b-b'a') as usize].iter().enumerate() {
                for col in 0..5 {
                    if bits & (16 >> col) != 0 { f.rect(x+col*z, y+row as i32*z, z, z, color); }
                }
            }
        } else if let Ok(s) = core::str::from_utf8(core::slice::from_ref(&b)) {
            f.text(x, y, s, z, color);
        }
    }
}
fn number(f: &mut Frame, x: i32, y: i32, value: u32, z: i32, color: u32) {
    f.text(x, y, digits(value, &mut [0;10]), z, color);
}

fn parrot(f: &mut Frame) {
    f.rect(22, 14, 55, 51, LEAF);
    f.rect(35, 26, 14, 24, LIME);
    f.rect(44, 22, 13, 17, LIME);
    f.rect(51, 24, 12, 7, LIME);
    f.rect(55, 27, 3, 3, INK);
    f.rect(59, 32, 8, 6, ORANGE);
    f.rect(59, 38, 4, 4, ORANGE);
    f.rect(37, 36, 7, 12, LEAF);
    f.rect(32, 46, 7, 9, LIME);
    f.rect(29, 53, 6, 5, LIME);
    f.rect(45, 49, 3, 5, ORANGE);
    f.rect(41, 54, 16, 2, ORANGE);
}
fn depth(nodes: &[Node], mut parent: Option<u32>) -> u32 {
    let mut level = 0;
    while let Some(id) = parent {
        parent = nodes[(id-1) as usize].parent;
        level += 1;
    }
    level
}
pub fn state_name(state: State) -> &'static str {
    match state { State::Active => "ACTIVE", State::Finished => "FINISHED", State::Cancelled => "CANCELLED" }
}

pub fn draw(px: &mut [u32], nodes: &[Node], selected: usize, message: &str, saved: &str,
            editor: Option<(bool, &str)>, locked: bool) {
    let f = &mut Frame { px, pitch: W };
    f.px.fill(PAPER);
    f.rect(0, 0, 800, 78, INK);
    parrot(f);
    text(f, 91, 18, "Parrots all the way Down", 3, LIGHT);
    f.text(92, 54, "RECURSION HELPDESK", 1, LIME);
    f.text(618, 23, "GUEST WORKSPACE", 1, LIME);
    text(f, 618, 43, saved, 1, LIGHT);

    f.text(23, 98, "REQUEST QUEUE", 2, INK);
    f.rect(23, 123, 499, 1, RULE);
    f.text(26, 134, "ID", 1, LEAF);
    f.text(72, 134, "REQUEST", 1, LEAF);
    f.text(342, 134, "PARENT", 1, LEAF);
    f.text(431, 134, "STATE", 1, LEAF);
    let first = selected.saturating_sub(6);
    for (row, n) in nodes.iter().enumerate().skip(first).take(7) {
        let y = 153 + (row-first) as i32 * 29;
        let focus = row == selected;
        let c = if focus { LIGHT } else { INK };
        f.rect(22, y, 501, 27, if focus { LEAF } else { LIGHT });
        number(f, 27, y+7, n.id, 1, c);
        let indent = depth(nodes,n.parent) as i32 * 12;
        if indent>0 { f.rect(73+indent-10, y+6, 1, 8, if focus { LIME } else { RULE }); f.rect(73+indent-10,y+13,7,1,RULE); }
        text(f, 73+indent, y+7, &n.label, 1, c);
        match n.parent {
            Some(id) => number(f, 342, y+7, id, 1, c),
            None => f.text(342, y+7, "ROOT", 1, c),
        }
        f.text(431, y+7, state_name(n.state), 1, c);
    }
    if locked {
        text(f, 40, 177, "Queue recovery required", 2, INK);
        text(f, 40, 205, "This session cannot change the saved queue.", 1, LEAF);
        text(f, 40, 227, "Keep a copy of the disk. Restore a known-good", 1, LEAF);
        text(f, 40, 242, "parrots.queue or correct read access, then reopen.", 1, LEAF);
    } else if nodes.is_empty() {
        text(f, 47, 179, "The queue is empty.", 2, INK);
        text(f, 47, 210, "Press N to enter your first request.", 1, LEAF);
    }
    f.rect(542, 94, 237, 258, LIGHT);
    f.rect(542, 94, 237, 4, LEAF);
    f.text(560, 111, "DESK LIMITS", 2, INK);
    f.text(560, 149, "ACTIVE", 1, LEAF);
    number(f, 560, 167, nodes.iter().filter(|n| n.state==State::Active).count() as u32, 3, INK);
    f.text(586, 173, "/", 2, LEAF);
    number(f, 609, 167, u32::from(MAX_LIVE), 3, INK);
    f.text(667, 149, "TOTAL", 1, LEAF);
    number(f, 667, 167, nodes.len() as u32, 2, INK);
    f.text(696, 167, "/", 2, LEAF);
    number(f, 715, 167, MAX_TOTAL, 2, INK);
    f.rect(560, 205, 201, 1, RULE);
    f.text(560, 219, "MAX CHILD DEPTH", 1, LEAF);
    number(f, 702, 216, u32::from(MAX_DEPTH), 2, INK);
    if let Some(n) = nodes.get(selected) {
        f.text(560, 262, "SELECTED REQUEST", 1, LEAF);
        f.text(560, 284, "PARENT", 1, INK);
        match n.parent { Some(id) => number(f, 629, 283, id, 1, INK), None => f.text(629,283,"ROOT",1,INK) }
        f.text(560, 309, "DEPTH", 1, INK);
        number(f, 629, 307, depth(nodes,n.parent), 1, INK);
    }
    f.rect(22, 364, 757, 49, if editor.is_some() { INK } else { 0xe2e9cf });
    if let Some((child, label)) = editor {
        f.text(34, 371, if child { "CHILD REQUEST LABEL" } else { "ROOT REQUEST LABEL" }, 1, LIME);
        text(f, 34, 388, label, 2, LIGHT);
        f.rect(35 + label.len() as i32*12, 389, 2, 12, ORANGE);
    } else {
        f.text(34, 372, "LAST ACTION", 1, LEAF);
        text(f, 34, 390, message, 1, INK);
    }
    f.rect(0, 426, 800, 54, INK);
    if locked {
        f.text(24, 443, "ESC  EXIT FOR RECOVERY", 1, LIME);
    } else if editor.is_some() {
        f.text(24, 439, "ENTER  SUBMIT LABEL", 1, LIME);
        f.text(238, 439, "BACKSPACE  EDIT", 1, LIGHT);
        f.text(459, 439, "ESC  GO BACK", 1, LIGHT);
        text(f, 24, 457, "Up to 30 letters, digits, spaces, dashes, dots or slashes.", 1, LIGHT);
    } else {
        f.text(24, 438, "N  NEW ROOT", 1, LIME);
        f.text(185, 438, "C  ADD CHILD", 1, LIME);
        f.text(353, 438, "F  FINISH", 1, LIGHT);
        f.text(507, 438, "X  CANCEL BRANCH", 1, LIGHT);
        f.text(24, 458, "UP / DOWN  SELECT REQUEST", 1, LIGHT);
        f.text(353, 458, "ESC  EXIT", 1, LIGHT);
    }
}
