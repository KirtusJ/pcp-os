//! People on the quay follow the same clock as freight. Watching the days pass
//! never answers a request on the player's behalf.

pub const NAMES: [&str; 3] = ["Nell Reed", "Ada Pike", "Kit Sayer"];
pub const WATCHES: [&str; 4] = ["Dawn", "Day", "Dusk", "Night"];
pub const WEATHER: [&str; 4] = ["Fair", "Sea mist", "Rain", "Fresh wind"];

// Five days of weather, one condition for each three-tide watch.
const FORECAST: [[usize; 4]; 5] = [
    [0, 0, 3, 1],
    [1, 3, 2, 2],
    [2, 1, 0, 0],
    [0, 3, 0, 1],
    [1, 2, 3, 0],
];

pub fn watch(tide: u64) -> usize {
    ((tide % 12) / 3) as usize
}
/// Harbor minutes since midnight. Dawn opens at 06:00 and each tide lasts two
/// harbor hours. Tide length and the partial-tide timer come from the economy.
pub fn minutes(tide: u64, elapsed_ms: u16) -> u16 {
    let whole = (6 * 60 + (tide % 12) * 120) % 1440;
    let fraction =
        u32::from(elapsed_ms).min(crate::freight::TIDE_MS - 1) * 120 / crate::freight::TIDE_MS;
    ((whole + u64::from(fraction)) % 1440) as u16
}

pub fn weather(tide: u64) -> usize {
    FORECAST[((tide / 12) % 5) as usize][watch(tide)]
}
pub fn visitor(tide: u64) -> usize {
    (tide % 3) as usize
}

/// Number of scheduled visits up to and including the present tide.
pub fn visits(tide: u64, neighbor: usize) -> u64 {
    if neighbor >= NAMES.len() {
        return 0;
    }
    tide / 3 + u64::from(neighbor <= visitor(tide))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Favor {
    pub tide: u64,
    pub neighbor: usize,
    pub tier: usize,
    /// Salt, crab, timber, provisions and fittings, in exchange order.
    pub cargo: [u16; 5],
    pub reward: u64,
}

type Terms = ([u16; 5], u64);
const TERMS: [[Terms; 3]; 3] = [
    [
        ([1, 0, 0, 0, 0], 12),
        ([3, 1, 0, 0, 0], 64),
        ([3, 0, 0, 1, 0], 165),
    ],
    [
        ([2, 0, 0, 0, 0], 22),
        ([0, 3, 0, 0, 0], 90),
        ([0, 0, 2, 0, 1], 285),
    ],
    [
        ([1, 0, 0, 0, 0], 11),
        ([2, 0, 1, 0, 0], 90),
        ([0, 0, 0, 1, 1], 330),
    ],
];

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Neighbors {
    /// Lifetime favors the player actually completed, in visitor order.
    pub helped: [u32; 3],
    /// The player answered the current tide's request.
    pub served: bool,
}

impl Neighbors {
    pub const fn new() -> Self {
        Self {
            helped: [0; 3],
            served: false,
        }
    }

    pub fn offer(&self, tide: u64) -> Favor {
        let neighbor = visitor(tide);
        // Retain the displayed, already-paid terms until this visitor departs.
        let completed_before = self.helped[neighbor].saturating_sub(u32::from(self.served));
        let tier = completed_before.min(2) as usize;
        let (cargo, reward) = TERMS[neighbor][tier];
        Favor {
            tide,
            neighbor,
            tier,
            cargo,
            reward,
        }
    }

    pub fn lanterns_unlocked(&self) -> bool {
        self.helped.iter().all(|&n| n != 0)
    }
}

const ANSWER_DELAY_MS: [u64; 3] = [1_200, 1_100, 1_400];
const ANSWER_WINDOW_MS: u64 = 1_000;
const BELL_COOLDOWN_MS: u64 = 5_000;

/// Transient stage cue owned by the host. Supply monotonic milliseconds and
/// clear the cue when suspending the performance or loading another harbor.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BellReply {
    strike_ms: Option<u64>,
    listener: Option<usize>,
}

impl BellReply {
    pub const fn new() -> Self {
        Self {
            strike_ms: None,
            listener: None,
        }
    }

    /// Schedule the person who heard the bell. Further rings still earn their
    /// ordinary tip and leave this cue and its original cool-down alone.
    pub fn ring(&mut self, now_ms: u64, person: usize) -> bool {
        if person >= NAMES.len() {
            return false;
        }
        if let Some(age) = self.strike_ms.and_then(|t| now_ms.checked_sub(t)) {
            if age < BELL_COOLDOWN_MS {
                return false;
            }
        }
        self.strike_ms = Some(now_ms);
        self.listener = Some(person);
        true
    }

    /// A late poll drops the cue. A returned ID is consumed immediately, so
    /// the host can play the entire corresponding phrase exactly once.
    pub fn poll(&mut self, now_ms: u64) -> Option<usize> {
        let person = self.listener?;
        let Some(age) = now_ms.checked_sub(self.strike_ms?) else {
            self.clear();
            return None;
        };
        let delay = ANSWER_DELAY_MS[person];
        if age < delay {
            return None;
        }
        self.listener = None;
        (age < delay + ANSWER_WINDOW_MS).then_some(person)
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_wraps_at_midnight_and_tracks_whose_visit_this_is() {
        assert_eq!(minutes(0, 0), 360);
        assert_eq!(minutes(8, 29_999), 1439);
        assert_eq!(minutes(9, 0), 0);
        assert_eq!(minutes(11, 29_999), 359);
        assert_eq!(watch(6), 2);
        for t in 0..120 {
            assert_eq!(weather(t), weather(t + 60));
            assert!(weather(t) < WEATHER.len());
            let total: u64 = (0..3).map(|n| visits(t, n)).sum();
            assert_eq!(total, t + 1);
        }
        assert_eq!(visits(u64::MAX, 4), 0);
        assert!(minutes(u64::MAX, u16::MAX) < 1440);
        assert_eq!(
            (visitor(0), visitor(1), visitor(2), visitor(3)),
            (0, 1, 2, 0)
        );
    }

    #[test]
    fn coalescing_rings_keep_the_original_listener_and_strike_time() {
        let mut bell = BellReply::new();
        for person in 0..3 {
            let now = person as u64 * BELL_COOLDOWN_MS + 100;
            assert!(bell.ring(now, person));
            assert!(!bell.ring(now + 500, (person + 1) % 3));
            let due = now + ANSWER_DELAY_MS[person];
            assert_eq!(bell.poll(due - 1), None);
            assert_eq!(bell.poll(due), Some(person));
            assert_eq!(bell.poll(due), None);
            assert!(!bell.ring(now + 4_999, person));
        }
    }

    #[test]
    fn late_loaded_or_invalid_cues_do_not_play_on_return() {
        let mut bell = BellReply::new();
        assert!(!bell.ring(0, usize::MAX));
        assert_eq!(bell.poll(100), None);
        bell.ring(100, 0);
        assert_eq!(bell.poll(2_299), Some(0));
        bell.ring(5_100, 0);
        assert_eq!(bell.poll(7_300), None);
        assert_eq!(bell.poll(7_301), None);
        bell.ring(10_100, 1);
        bell.clear();
        assert_eq!(bell.poll(11_200), None);
        assert!(bell.ring(11_200, 2));
        assert_eq!(bell.poll(u64::MAX), None);
    }

    #[test]
    fn clock_restarts_and_values_near_the_integer_limit_stay_silent() {
        let mut bell = BellReply::new();
        assert!(bell.ring(u64::MAX - 1, 1));
        assert_eq!(bell.poll(u64::MAX), None);
        assert_eq!(bell.poll(0), None);
        assert!(bell.ring(0, 2));
        assert_eq!(bell.poll(1_400), Some(2));
        assert!(bell.ring(u64::MAX, 0));
        assert!(bell.ring(1, 1));
        assert_eq!(bell.poll(1_101), Some(1));
    }
}
