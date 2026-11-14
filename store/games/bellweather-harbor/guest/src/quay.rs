use crate::{device, Cue, Outcome};
use bellweather::{economy::Economy, neighbors::Favor};

#[derive(Clone, Copy)]
pub enum Reply {
    None,
    Declined,
    Helped,
    Short,
    Changed,
}

pub struct QuayDesk {
    pub person: usize,
    pub offer: Favor,
    pub letter: Option<usize>,
    pub reply: Reply,
}

impl QuayDesk {
    pub fn open(game: &Economy) -> Self {
        let offer = game.current_favor();
        Self {
            person: offer.neighbor,
            offer,
            letter: None,
            reply: Reply::None,
        }
    }

    pub fn in_letters(&self) -> bool {
        self.letter.is_some()
    }

    fn visit(&mut self, game: &Economy, person: usize) {
        self.person = person;
        self.offer = game.current_favor();
        self.letter = None;
        self.reply = Reply::None;
    }

    pub(super) fn handle(&mut self, key: usize, game: &mut Economy) -> Outcome {
        let current = game.current_favor();
        let letters = game.neighbors.helped[self.person].min(3) as usize;
        if let Some(i) = self.letter {
            match key {
                1 | 14 | 38 => self.letter = None,
                75 if i > 0 => self.letter = Some(i - 1),
                77 if i + 1 < letters => self.letter = Some(i + 1),
                2..=4 if key - 2 < letters => self.letter = Some(key - 2),
                _ => {}
            }
            return Outcome::None;
        }
        match key {
            2..=4 => self.visit(game, key - 2),
            75 => self.visit(game, (self.person + 2) % 3),
            77 => self.visit(game, (self.person + 1) % 3),
            47 => self.visit(game, current.neighbor),
            38 if letters > 0 => self.letter = Some(letters - 1),
            32 if self.offer == current
                && self.person == current.neighbor
                && !game.neighbors.served =>
            {
                self.reply = Reply::Declined;
            }
            28 if self.person == self.offer.neighbor => {
                if current.tide != self.offer.tide {
                    self.reply = Reply::Changed;
                } else if game.neighbors.served || matches!(self.reply, Reply::Declined) {
                    return Outcome::None;
                } else if game.help_neighbor(self.offer) {
                    device::log("Bellweather: quay favor fulfilled\n");
                    self.reply = Reply::Helped;
                    return Outcome::Changed(Cue::Manager);
                } else if self.offer != current {
                    self.reply = Reply::Changed;
                } else {
                    self.reply = Reply::Short;
                }
            }
            _ => {}
        }
        Outcome::None
    }
}
