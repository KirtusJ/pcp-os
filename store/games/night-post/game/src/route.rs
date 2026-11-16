//! Each run has three addresses. Keep their world-space motion here so the chart,
//! artwork, aiming sight and delivery checks all refer to the same destinations.
use crate::{triangle, POSTS, ROUTE};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Route {
    Coast,
    Flotilla,
    Aurora,
}
pub const ROUTES: [Route; 3] = [Route::Coast, Route::Flotilla, Route::Aurora];

impl Route {
    pub fn turn(self, right: bool) -> Self {
        ROUTES[(self as usize + if right { 1 } else { 2 }) % 3]
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Coast => "LIGHTHOUSE COAST",
            Self::Flotilla => "LANTERN FLOTILLA",
            Self::Aurora => "AURORA EXPRESS",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Coast => "THREE ISLAND LIGHTHOUSES",
            Self::Flotilla => "THREE SHIPS ON THE TIDE",
            Self::Aurora => "THREE HIGH-ALTITUDE AIRSHIPS",
        }
    }
    pub fn names(self) -> [&'static str; 3] {
        match self {
            Self::Coast => crate::NAMES,
            Self::Flotilla => ["MINNOW", "PAPER MOON", "LAST FERRY"],
            Self::Aurora => ["JUNIPER", "ALCYONE", "NORTH STAR"],
        }
    }
    pub fn length(self) -> i32 {
        match self {
            Self::Coast => ROUTE,
            Self::Flotilla => 2480,
            Self::Aurora => 2660,
        }
    }
    pub fn speed(self) -> i32 {
        if self == Self::Aurora {
            3
        } else {
            2
        }
    }
    pub fn berths(self) -> [(i32, i32); 3] {
        match self {
            Self::Coast => POSTS,
            Self::Flotilla => [(555, 208), (1350, 196), (2110, 211)],
            Self::Aurora => [(600, 144), (1410, 114), (2275, 165)],
        }
    }
    pub fn position(self, id: usize, t: u32) -> (i32, i32) {
        let (x, y) = self.berths()[id];
        let t = (t % 720000) as i32 + id as i32 * 60;
        match self {
            Self::Coast => (x, y),
            Self::Flotilla => (
                x + triangle(t, 160) / 2 - 20,
                y + triangle(t / 2, 32) / 3 - 2,
            ),
            Self::Aurora => (
                x + triangle(t, 120) / 2 - 15,
                y + triangle(t / 2, 80) / 2 - 10,
            ),
        }
    }
    pub fn radius(self) -> i32 {
        match self {
            Self::Coast => 24,
            Self::Flotilla => 21,
            Self::Aurora => 18,
        }
    }
    pub fn bit(self, id: usize) -> u32 {
        1 << (self as u32 * 3 + id as u32)
    }
}

// Lower nine bits are received letters; upper nine are a centered delivery.
pub fn clean_album(value: u32) -> u32 {
    let stamps = value & 511;
    stamps | (((value >> 9) & stamps) << 9)
}
