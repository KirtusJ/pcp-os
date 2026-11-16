//! Bookmark and loose pins stay in this desk. The economy preserves read evidence,
//! filed cases and the one signed conclusion in both editions of the game.
use crate::{device, Cue, Outcome};
use bellweather::{
    economy::Economy,
    manifest_text::{CASES, PAIRS},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Scene,
    Lead,
    Finding,
    Decision,
    Confirm,
    Ending,
    Wire,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Note {
    None,
    Wrong,
    NeedTwo,
    PinsFull,
    Hint,
}
pub struct Journal {
    pub case: usize,
    pub lead: usize,
    pub pins: [u8; 3],
    pub page: Page,
    pub note: Note,
    pub choice: usize,
    pub offset: usize,
    pub shown_lines: usize,
    pub wire_tide: u64,
    pub wire_guess: Option<usize>,
    pub wire_replay_ms: Option<u16>,
}
impl Journal {
    pub fn open(g: &Economy) -> Self {
        let complete = g.manifest.solved as usize == CASES.len();
        let ending = g.manifest.ending as usize;
        Self {
            case: (g.manifest.solved as usize).min(CASES.len() - 1),
            lead: 0,
            pins: [0; 3],
            page: if ending > 0 {
                Page::Ending
            } else if complete {
                Page::Decision
            } else {
                Page::Scene
            },
            choice: ending.saturating_sub(1),
            note: Note::None,
            offset: 0,
            shown_lines: 0,
            wire_tide: g.freight.tide,
            wire_guess: None,
            wire_replay_ms: None,
        }
    }
    pub fn close(&self, key: usize) -> bool {
        key == 36
            || key == 1
                && !matches!(
                    self.page,
                    Page::Confirm | Page::Lead | Page::Decision | Page::Wire
                )
    }
    fn show(&mut self, page: Page) {
        self.page = page;
        self.offset = 0;
        self.shown_lines = 0;
        self.note = Note::None;
    }
    pub fn proved(g: &Economy, case: usize) -> u8 {
        if case < g.manifest.solved as usize {
            let [a, b] = PAIRS[case];
            (1 << a) | (1 << b)
        } else {
            0
        }
    }
    pub fn tune(&mut self, tide: u64) {
        if self.wire_tide != tide {
            self.wire_tide = tide;
            self.wire_guess = None;
            self.wire_replay_ms = None;
        }
    }
    pub(super) fn handle(&mut self, key: usize, g: &mut Economy) -> Outcome {
        if self.page == Page::Wire {
            self.tune(g.freight.tide);
            match key {
                1 | 14 | 18 => self.show(Page::Ending),
                2..=6 => self.wire_guess = Some(key - 2),
                57 => {
                    self.wire_replay_ms = Some(g.freight.elapsed_ms);
                    return Outcome::Sound(Cue::Wire(bellweather::wire::wire_good(g.freight.tide)));
                }
                _ => {}
            }
            return Outcome::None;
        }
        if self.page == Page::Ending && key == 18 && g.manifest.ending as u8 > 0 {
            self.show(Page::Wire);
            self.wire_guess = None;
            self.wire_replay_ms = None;
            self.wire_tide = g.freight.tide;
            return Outcome::None;
        }
        let last = CASES.len() - 1;
        if matches!(key, 72 | 80 | 73 | 81) {
            if key == 72 || key == 73 {
                self.offset = self.offset.saturating_sub(4);
            } else {
                self.offset = (self.offset + 4).min(self.shown_lines);
            }
            return Outcome::None;
        }
        if self.page == Page::Confirm {
            if matches!(key, 1 | 14) {
                self.show(Page::Decision);
            }
            if key == 28 && g.choose_ending((self.choice + 1) as u8) {
                self.show(Page::Ending);
                device::log("Bellweather: manifest conclusion signed\n");
                return Outcome::Changed(Cue::Charter);
            }
            return Outcome::None;
        }
        if matches!(key, 75 | 77) && self.page != Page::Decision {
            let last_open = (g.manifest.solved as usize).min(last);
            self.case = if key == 75 {
                self.case.saturating_sub(1)
            } else {
                (self.case + 1).min(last_open)
            };
            self.show(Page::Scene);
            return Outcome::None;
        }
        if self.page == Page::Decision {
            match key {
                2 | 3 => self.choice = key - 2,
                75 | 77 => self.choice = 1 - self.choice,
                28 => self.show(Page::Confirm),
                1 | 14 => {
                    self.case = last;
                    self.show(Page::Finding);
                }
                _ => {}
            }
            return Outcome::None;
        }
        if self.page == Page::Ending {
            if matches!(key, 2..=4) {
                self.case = key - 2;
                self.show(Page::Scene);
            }
            return Outcome::None;
        }
        if self.page == Page::Finding && matches!(key, 28 | 49) {
            if self.case < last {
                self.case += 1;
                self.show(Page::Scene);
            } else if g.manifest.solved as usize == CASES.len() {
                self.show(if g.manifest.ending as u8 == 0 {
                    Page::Decision
                } else {
                    Page::Ending
                });
            }
            return Outcome::None;
        }
        match key {
            2..=5 => {
                let l = key - 2;
                let first = g.manifest.read[self.case] & (1 << l) == 0;
                if g.inspect_lead(self.case, l) {
                    self.lead = l;
                    self.show(Page::Lead);
                    if first {
                        return Outcome::Save;
                    }
                }
            }
            57 | 25 if self.page == Page::Lead && self.case == g.manifest.solved as usize => {
                let pin = 1 << self.lead;
                let set = &mut self.pins[self.case];
                self.note = Note::None;
                if *set & pin != 0 {
                    *set &= !pin;
                } else if set.count_ones() < 2 {
                    *set |= pin;
                } else {
                    self.note = Note::PinsFull;
                }
            }
            28 => {
                if self.case < g.manifest.solved as usize {
                    self.show(Page::Finding);
                } else {
                    let pins = self.pins[self.case];
                    if pins.count_ones() == 2 {
                        let a = pins.trailing_zeros() as usize;
                        let b = (pins & !(1 << a)).trailing_zeros() as usize;
                        if g.prove_pair(self.case, a, b) {
                            self.show(Page::Finding);
                            device::log("Bellweather: manifest case filed\n");
                            return Outcome::Changed(Cue::Manager);
                        }
                        self.note = Note::Wrong;
                    } else {
                        self.note = Note::NeedTwo;
                    }
                }
            }
            35 => {
                if self.note == Note::Hint {
                    self.note = Note::None;
                } else {
                    self.note = Note::Hint;
                }
                self.offset = 0;
            }
            1 | 14 => self.show(Page::Scene),
            _ => {}
        }
        Outcome::None
    }
}
