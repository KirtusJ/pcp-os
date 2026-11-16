//! Saved progress for The Second Manifest. Clues may be revisited after a
//! deduction. Only reading both corroborating documents proves a chapter.

use crate::manifest_text::PAIRS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Ending {
    None,
    Print,
    Kitchen,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Manifest {
    pub unlocked: bool,
    /// Number of corroborated chapters, in order.
    pub solved: u8,
    /// Four evidence bits per chapter.
    pub read: [u8; 3],
    pub ending: Ending,
}

impl Manifest {
    pub const fn new() -> Self {
        Self {
            unlocked: false,
            solved: 0,
            read: [0; 3],
            ending: Ending::None,
        }
    }

    pub fn inspect(&mut self, chapter: usize, lead: usize) -> bool {
        if !self.unlocked
            || chapter >= PAIRS.len()
            || chapter > usize::from(self.solved)
            || lead >= 4
        {
            return false;
        }
        self.read[chapter] |= 1 << lead;
        true
    }

    pub fn prove(&mut self, chapter: usize, a: usize, b: usize) -> bool {
        if !self.unlocked
            || chapter != usize::from(self.solved)
            || chapter >= PAIRS.len()
            || a >= 4
            || b >= 4
            || a == b
        {
            return false;
        }
        let evidence = (1 << a) | (1 << b);
        if pair_mask(chapter) != evidence || self.read[chapter] & evidence != evidence {
            return false;
        }
        self.solved += 1;
        true
    }

    pub fn choose(&mut self, choice: u8) -> bool {
        if !self.unlocked || self.solved != 3 || self.ending != Ending::None {
            return false;
        }
        self.ending = match choice {
            1 => Ending::Print,
            2 => Ending::Kitchen,
            _ => return false,
        };
        true
    }

    pub(crate) fn valid(&self) -> bool {
        if !self.unlocked {
            return *self == Self::new();
        }
        if self.solved > 3 || (self.ending != Ending::None && self.solved != 3) {
            return false;
        }
        self.read
            .iter()
            .copied()
            .enumerate()
            .all(|(chapter, mask)| {
                if mask > 15 {
                    return false;
                }
                let solved = usize::from(self.solved);
                if chapter < solved {
                    mask & pair_mask(chapter) == pair_mask(chapter)
                } else {
                    chapter == solved || mask == 0
                }
            })
    }
}

fn pair_mask(chapter: usize) -> u8 {
    let [a, b] = PAIRS[chapter];
    (1 << a) | (1 << b)
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corroboration_requires_two_read_sources_and_later_files_stay_sealed() {
        let mut case = Manifest::new();
        assert!(!case.inspect(0, 0));
        assert!(!case.prove(0, 0, 2));
        case.unlocked = true;
        assert!(!case.inspect(usize::MAX, usize::MAX));
        assert!(!case.prove(0, usize::MAX, 2));
        assert!(!case.choose(2));
        for (chapter, pair) in PAIRS.iter().enumerate() {
            let [a, b] = pair.map(usize::from);
            let decoy = (0..4).find(|id| *id != a && *id != b).unwrap();
            assert!(!case.inspect(chapter + 1, 0));
            assert!(case.inspect(chapter, a));
            assert!(case.inspect(chapter, decoy));
            let before = case;
            assert!(!case.prove(chapter, a, decoy));
            assert!(!case.prove(chapter, a, a));
            assert!(!case.prove(chapter, a, b));
            assert_eq!(case, before);
            assert!(case.inspect(chapter, b));
            assert!(case.prove(chapter, b, a));
            assert!(!case.prove(chapter, a, b));
            assert!(case.inspect(chapter, decoy));
            assert!(case.valid());
        }
        assert!(!case.choose(0));
        assert!(!case.choose(3));
        assert!(case.choose(1));
        assert!(!case.choose(2));
        assert_eq!(case.ending, Ending::Print);
    }

    #[test]
    fn saved_future_leads_false_claims_and_early_endings_are_rejected() {
        let mut case = Manifest::new();
        case.solved = 1;
        assert!(!case.valid());
        case.unlocked = true;
        assert!(!case.valid());
        case.solved = 0;
        case.read[1] = 1;
        assert!(!case.valid());
        case.read = [0; 3];
        case.ending = Ending::Kitchen;
        assert!(!case.valid());
        case.ending = Ending::None;
        case.read[0] = 16;
        assert!(!case.valid());
    }
}
