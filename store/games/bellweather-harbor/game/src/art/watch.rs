//! Light, weather and the people on duty follow the saved harbor tide.
use super::*;
use crate::neighbors;

const TIDE: u32 = 30_000;
const DAY: u32 = TIDE * 12;

#[derive(Clone, Copy)]
struct Light {
    at: u32,
    zenith: Color,
    horizon: Color,
    black: Color,
    white: Color,
    lamps: u32,
}

// Outer colors of the day's palette. Interpolation between successive rows
// stays continuous at the last midnight tide and the following dawn.
const LIGHTS: [Light; 10] = [
    Light {
        at: 0,
        zenith: 0x89919d,
        horizon: 0xffc787,
        black: 0x060a12,
        white: 0xffe8d2,
        lamps: 60,
    },
    Light {
        at: 44_000,
        zenith: 0x8eafbc,
        horizon: 0xffdfa2,
        black: 0x030909,
        white: 0xfff9e6,
        lamps: 4,
    },
    Light {
        at: 90_000,
        zenith: 0x75aabc,
        horizon: 0xd9ebd4,
        black: 0x02100e,
        white: 0xf1fff7,
        lamps: 0,
    },
    Light {
        at: 144_000,
        zenith: 0x679bb5,
        horizon: 0xd5e4d7,
        black: 0x040d12,
        white: 0xf2fff9,
        lamps: 0,
    },
    Light {
        at: 180_000,
        zenith: 0x8ea4ab,
        horizon: 0xffd5a0,
        black: 0x0d0c11,
        white: 0xffecd2,
        lamps: 12,
    },
    Light {
        at: 223_000,
        zenith: 0x746786,
        horizon: 0xf7a579,
        black: 0x110f1d,
        white: 0xe4c2b0,
        lamps: 106,
    },
    Light {
        at: 263_000,
        zenith: 0x324260,
        horizon: 0xb78287,
        black: 0x071321,
        white: 0x8598aa,
        lamps: 219,
    },
    Light {
        at: 292_000,
        zenith: 0x17293d,
        horizon: 0x597184,
        black: 0x071320,
        white: 0x658296,
        lamps: 256,
    },
    Light {
        at: 330_000,
        zenith: 0x1d3048,
        horizon: 0x72838f,
        black: 0x081323,
        white: 0x708899,
        lamps: 250,
    },
    Light {
        at: DAY,
        zenith: 0x89919d,
        horizon: 0xffc787,
        black: 0x060a12,
        white: 0xffe8d2,
        lamps: 60,
    },
];

fn mix(a: Color, b: Color, factor: u32) -> Color {
    let s = 256 - factor;
    let rb = (((a & 0xff00ff) * s + (b & 0xff00ff) * factor) >> 8) & 0xff00ff;
    let g = (((a & 0x00ff00) * s + (b & 0x00ff00) * factor) >> 8) & 0x00ff00;
    rb | g
}

fn light_at(clock: u32) -> Light {
    let pair = LIGHTS
        .windows(2)
        .find(|p| clock < p[1].at)
        .unwrap_or(&LIGHTS[8..]);
    let (a, b) = (pair[0], pair[1]);
    let f = (clock - a.at) * 256 / (b.at - a.at);
    Light {
        at: clock,
        zenith: mix(a.zenith, b.zenith, f),
        horizon: mix(a.horizon, b.horizon, f),
        black: mix(a.black, b.black, f),
        white: mix(a.white, b.white, f),
        lamps: (a.lamps * (256 - f) + b.lamps * f) >> 8,
    }
}

struct Grade {
    r: [u32; 256],
    g: [u32; 256],
    b: [u32; 256],
}
impl Grade {
    fn new(low: Color, high: Color) -> Self {
        let mut g = Self {
            r: [0; 256],
            g: [0; 256],
            b: [0; 256],
        };
        for i in 0..256 {
            let v = mix(low, high, i as u32);
            g.r[i] = v & 0xff0000;
            g.g[i] = v & 0xff00;
            g.b[i] = v & 255;
        }
        g
    }
    #[inline(always)]
    fn color(&self, p: Color) -> Color {
        self.r[((p >> 16) & 255) as usize]
            | self.g[((p >> 8) & 255) as usize]
            | self.b[(p & 255) as usize]
    }
}

fn weather_strength(tide: u64, elapsed: u32) -> [u32; 4] {
    let mut result = [0; 4];
    let kind = neighbors::weather(tide);
    // Use the prior clearing's weather for a gentle 14-second handover.
    let before = neighbors::weather(tide.saturating_sub(1));
    let f = (elapsed.min(14_000) * 256) / 14_000;
    result[kind] += f;
    result[before] += 256 - f;
    result
}

fn stroll(c: &mut Canvas, clock: u32, watch: usize) {
    let t = clock / 65;
    let x = 15 + triangle(t / 3, 53);
    let y = 164 - x * 2 / 5;
    if watch < 2 {
        // Box trolley, axle and brass wheel hubs on the diagonal plank walk.
        let cx = x + 8;
        let cy = y - 2;
        c.line((x + 2, y - 5), (cx, cy - 3), 0x926b4e);
        c.rect(cx - 2, cy - 4, 10, 3, 0xb18556);
        c.rect(
            cx - 1,
            cy - 7,
            7,
            4,
            if watch == 0 { 0xc3b28b } else { 0xa67851 },
        );
        c.rect(cx - 1, cy - 7, 7, 1, 0xd3b17d);
        for off in [0, 6] {
            c.disc(cx + off, cy, 2, TAR);
            c.pixel(cx + off, cy, 0xaa9873);
        }
    }
    worker(
        c,
        x,
        y,
        match watch {
            0 => 0xb57656,
            1 => 0x5a827e,
            2 => 0x9d7050,
            _ => 0x60767b,
        },
        false,
    );
    c.pixel(x + (t / 3 % 2) as i32, y + 1, 0x5c564c);
    c.rect(
        x - 1,
        y - 7,
        4,
        1,
        if watch == 2 { 0xd4a765 } else { 0x827652 },
    );
    if watch >= 2 {
        c.line((x + 1, y - 3), (x + 4, y - 4), 0xcd9e75);
        c.rect(x + 3, y - 4, 2, 4, 0xbf9d66);
        c.pixel(x + 4, y - 3, 0xffe0a6);
    }
    // A passenger at the short landing and a watcher on the far pier.
    let px = 74 + triangle(t / 18, 8);
    worker(
        c,
        px,
        134 - (px - 74) / 3,
        if watch == 0 { 0xa67e63 } else { 0x62796f },
        false,
    );
    if watch != 3 {
        worker(c, 437 + triangle(t / 20, 5), 110, 0xb99065, false);
    }
}

#[derive(Clone, Copy)]
struct Orb {
    x: i32,
    y: i32,
    r: i32,
    edge: Color,
    body: Color,
    opacity: u32,
    moon: bool,
}
impl Orb {
    fn at(&self, x: i32, y: i32, bg: Color) -> Color {
        let dx = x - self.x;
        let dy = y - self.y;
        if dx.abs() > self.r || dy.abs() > self.r {
            return bg;
        }
        let rr = dx * dx + dy * dy;
        if rr > self.r * self.r {
            return bg;
        }
        if self.moon && (dx - 5) * (dx - 5) + (dy + 4) * (dy + 4) < (self.r - 1) * (self.r - 1) {
            return mix(bg, 0x8094a4, self.opacity / 12);
        }
        let col = if rr > (self.r - 2) * (self.r - 2) {
            self.edge
        } else {
            self.body
        };
        mix(bg, col, self.opacity)
    }
}

fn sky_cloud(p: Color) -> Option<(u32, u32)> {
    match p {
        0xc5a49e | 0xaaa3a3 | 0xc7aaa2 | 0xaaa2a2 => Some((0, 68)),
        0xe0b0a2 | 0xe1b19e => Some((1, 114)),
        0xe1b2a1 | 0xedbfab | 0xbba9a6 | 0xf1c1a5 | 0xb7a6a5 => Some((1, 149)),
        0xf9c991 | 0xfacb90 | 0xf2c098 | 0xf8c897 => Some((1, 116)),
        _ => None,
    }
}

fn lamplight(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, target: Color, amount: u32) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && px < ART_W as i32 && py >= 0 && py < ART_H as i32 {
                let loc = py as usize * ART_W + px as usize;
                c.0[loc] = mix(c.0[loc], target, amount);
            }
        }
    }
}

fn lights(c: &mut Canvas, t: u32, level: u32, ferry: Option<(i32, i32)>) {
    if level == 0 {
        return;
    }
    let flash = triangle(t / 65, 30) as u32;
    lamplight(c, 32, 37, 7, 5, 0xffdf8e, level * 4 / 5);
    lamplight(c, 34, 37, 2, 4, 0xffefaa, level);
    lamplight(c, 3, 72, 2, 3, 0xe4a768, level * 3 / 4);
    lamplight(c, 10, 72, 2, 3, 0xe4ac76, level * 3 / 4);
    lamplight(c, 448, 120, 3, 4, 0xffd68c, level);
    for x in [451, 460, 469] {
        lamplight(c, x, 87, 3, 3, 0xf2c082, level * 3 / 4);
    }
    for (x, y) in [
        (211, 98),
        (217, 95),
        (229, 99),
        (244, 95),
        (255, 96),
        (267, 96),
        (284, 94),
        (294, 96),
        (305, 93),
        (323, 96),
        (329, 93),
    ] {
        lamplight(c, x, y, 2, 1, 0xefba80, level * 3 / 4);
    }
    if let Some((x, y)) = ferry {
        for cabin in [0, 6, 12, 18, 24, 29] {
            lamplight(c, x + cabin, y, 3, 2, 0xf5bf87, level * 3 / 4);
        }
    }
    // At the lamp carrier's feet the planks catch a little of its warm light.
    if neighbors::watch(u64::from(t / TIDE)) >= 2 {
        let x = 15 + triangle((t / 65) / 3, 53);
        let y = 164 - x * 2 / 5;
        lamplight(c, x + 3, y - 3, 2, 2, 0xffd58c, level);
        lamplight(c, x + 2, y + 2, 4, 1, 0xbc9267, level / 3);
    }
    // The tug's wheelhouse bobs by one pixel. The middle three rows overlap at all phases.
    for x in [370, 375, 380, 385] {
        lamplight(c, x, 138, 3, 3, 0xffbf72, level * 4 / 5);
    }
    for (x, y, n) in [(36, 110, 7), (377, 174, 5), (452, 180, 3), (286, 105, 3)] {
        for idx in 0..n {
            let h = noise((idx * 37 + x) as u32);
            let offset = triangle(t / (290 + idx as u32 * 19), 3) - 1;
            let width = 2 + ((h >> 8) % 5) as i32;
            let factor = level * (110 + flash) / (185 + idx as u32 * 22);
            lamplight(
                c,
                x + offset - width / 2,
                y + idx * 2,
                width,
                1,
                0xd9a262,
                factor,
            );
        }
    }
}

fn sky_marks(c: &mut Canvas, t: u32, lamps: u32, wet: u32) {
    if lamps > 76 {
        for k in 0..60 {
            let h = noise(k + 446);
            let x = (h % 474) as i32;
            let y = 3 + ((h >> 10) % 47) as i32;
            // Exclude the crane, tower and gallery on the far edges of the sky.
            if (x > 351 && x < 443 && y > 28) || (x < 55 && y > 26) {
                continue;
            }
            let f = (lamps - 76) * (256 - wet / 2) / 256;
            let glimmer = triangle(t / (70 + (h % 97)), 12) as u32;
            lamplight(c, x, y, 1, 1, 0xe9deba, (f * (180 + glimmer)) / 190);
        }
    }
}

fn sky_and_grade(buf: &mut Frame, clock: u32, light: Light, weather: [u32; 4]) {
    let mist = weather[1];
    let rain = weather[2];
    let cloudiness = (mist * 122 + rain * 158) >> 8;
    let overcast = mix(0x89969f, 0x485966, light.lamps * 2 / 3);
    let zenith = mix(light.zenith, overcast, cloudiness);
    let horizon = mix(
        light.horizon,
        mix(0xcdc6ab, 0x75828c, light.lamps),
        cloudiness,
    );
    let dark = mix(light.black, 0x839191, (mist * 22 + rain * 10) >> 8);
    let bright = mix(
        light.white,
        mix(0xb9cabd, 0x7d95a0, light.lamps / 2),
        (mist * 62 + rain * 70) >> 8,
    );
    let grade = Grade::new(dark, bright);

    // Orb trajectories use the saved day, including their rise/set. Only sky pixels
    // are replaced, so real boats, the crane and gulls always cross in front of them.
    let sun_time = clock.min(272_000) / 100;
    let sx = 117 + (sun_time * 218 / 2720) as i32;
    let sy = 99 - (sun_time * (2720 - sun_time) * 73 / (1360 * 1360)) as i32;
    let sun = Orb {
        x: sx,
        y: sy,
        r: if clock < 35_000 || clock > 217_000 {
            19
        } else {
            16
        },
        edge: 0xffcd85,
        body: 0xffe6a6,
        opacity: if clock > 272_000 { 0 } else { 256 - cloudiness },
        moon: false,
    };
    let lunar_clock = if clock >= 220_000 {
        clock - 220_000
    } else {
        clock + DAY - 220_000
    };
    let moon_t = lunar_clock.min(166_000) / 100;
    let moon = Orb {
        x: 48 + (moon_t * 164 / 1660) as i32,
        y: 104 - (moon_t * (1660 - moon_t) * 76 / (830 * 830)) as i32,
        r: 10,
        edge: 0xd2d5be,
        body: 0xf0e6c2,
        opacity: if lunar_clock > 166_000 {
            0
        } else {
            light.lamps * (256 - cloudiness) / 256
        },
        moon: true,
    };

    for (y, row) in buf.chunks_exact_mut(ART_W).enumerate() {
        if y > 101 {
            for p in row {
                *p = grade.color(*p);
            }
            continue;
        }
        let band = (y / 3 * 3) as u32;
        let old = if band < 54 {
            between(0x89949a, 0xe4ab91, band, 54)
        } else {
            between(0xe4ab91, 0xffce87, band - 54, 52)
        };
        let sky = mix(zenith, horizon, (band * 256) / 105);
        let shadow = mix(sky, mix(0x8b97a3, 0x4b536a, light.lamps), 135);
        let rim = mix(sky, mix(0xffedc6, 0xb9a9ad, light.lamps), 147);
        let orb_row = (y as i32 - sun.y).abs() <= sun.r && sun.opacity > 0;
        let moon_row = (y as i32 - moon.y).abs() <= moon.r && moon.opacity > 0;
        for (x, p) in row.iter_mut().enumerate() {
            let orig = *p;
            if orig == old || matches!(orig, 0xffcf85 | 0xffdf99 | 0xffe3a2) {
                let mut color = sky;
                if orb_row {
                    color = sun.at(x as i32, y as i32, color);
                }
                if moon_row {
                    color = moon.at(x as i32, y as i32, color);
                }
                *p = color;
            } else if let Some((lit, weight)) = sky_cloud(orig) {
                *p = mix(sky, if lit == 0 { shadow } else { rim }, weight);
            } else {
                *p = grade.color(orig)
            }
        }
    }
}

fn weather_marks(c: &mut Canvas, t: u32, weather: [u32; 4]) {
    let mist = weather[1];
    if mist > 8 {
        for i in 0..21 {
            let seed = noise(738 + i);
            let offset = triangle(t / (195 + seed % 160), 11) - 5;
            let x = (seed % 430) as i32 + offset;
            let y = 72 + ((seed >> 9) % 45) as i32;
            let w = 9 + ((seed >> 17) % 35) as i32;
            lamplight(c, x, y, w, 1, 0xc1c4ae, mist / 9);
            lamplight(c, x + 5, y + 1, w / 2, 1, 0xc1c4ae, mist / 13);
        }
    }
    let rain = weather[2];
    if rain > 0 {
        for i in 0..65 {
            let seed = noise(i + 121);
            let y = ((seed % 192 + t / 25) % 192) as i32;
            let x = (((seed >> 10) % 510 + 510 - (t / 63) % 510) % 510) as i32 - 12;
            lamplight(c, x, y, 1, 1, 0xb3c5c3, rain / 4);
            lamplight(c, x - 1, y + 1, 1, 1, 0xb3c5c3, rain / 3);
        }
    }
    let wind = weather[3];
    if wind > 0 {
        for i in 0..19 {
            let seed = noise(i + 531);
            let x = (((seed >> 10) % 500 + t / 100) % 500) as i32 - 10;
            let y = 151 + (seed % 39) as i32;
            lamplight(c, x, y, 6, 1, 0xa4c6b5, wind / 4);
        }
    }
}

fn lantern_boats(c: &mut Canvas, clock: u32) {
    let progress = clock - 180_000;
    // Boats enter from the near landing and gather in the sheltered foreground.
    let fade = (progress.min(90_000 - progress).min(7000) * 256) / 7000;
    for n in 0..6 {
        let base_x = 81 + n * 33 + (progress * 41 / 90_000) as i32;
        let x = base_x + triangle(clock / (190 + n as u32 * 11), 4) - 2;
        let y = 161 + (n % 3) * 7 + triangle(clock / (246 + n as u32 * 23), 1);
        let side = if n % 2 == 0 { 0xb36e4a } else { 0x986b54 };
        let shell = if n % 3 == 0 {
            0xfbd693
        } else if n % 3 == 1 {
            0xf9af6b
        } else {
            0xe9c083
        };
        lamplight(c, x - 6, y, 14, 1, 0xe5b87a, fade);
        lamplight(c, x - 5, y + 1, 12, 1, 0x344950, fade);
        lamplight(c, x - 3, y + 2, 8, 1, side, fade);
        // Wire handle, folded-paper shade, and candle showing through its ribs.
        lamplight(c, x - 1, y - 8, 3, 1, 0x8e724e, fade);
        lamplight(c, x - 2, y - 7, 6, 1, 0xad8056, fade);
        lamplight(c, x - 3, y - 6, 8, 4, shell, fade);
        lamplight(c, x - 2, y - 2, 6, 1, 0xe6aa68, fade);
        lamplight(c, x - 2, y - 1, 6, 1, 0x9b6b48, fade);
        lamplight(c, x, y - 6, 2, 4, 0xffe4a7, fade);
        lamplight(c, x + 4, y - 5, 1, 3, 0xb78957, fade);
        lamplight(c, x - 3, y + 4, 8, 1, 0xffcd89, fade * 2 / 3);
        for k in 0..4 {
            let off = triangle(clock / (220 + k * 31 + n as u32 * 17), 2) - 1;
            lamplight(
                c,
                x + off - 1,
                y + 4 + k as i32 * 2,
                5 - (k % 3) as i32,
                1,
                0xe5ad69,
                fade / (2 + k),
            );
        }
    }
}

/// Center of the visitor's figure, in the same native pixels as the harbor scene.
/// All three meet at the mooring bollard on the near side of the plank pier.
pub fn quay_contact(_tide: u64) -> (usize, usize) {
    (38, 146)
}

fn visitor_on_quay(c: &mut Canvas, tide: u64, elapsed: u32, night: u32) {
    let (x, y) = quay_contact(tide);
    let (x, y) = (x as i32, y as i32);
    let id = neighbors::visitor(tide);
    // A cooler palette after dusk preserves the silhouette and each signature color.
    let dye = |col| mix(col, mix(col, 0x283c56, 168), night);
    let outline = dye(0x293f41);
    let cloth = match id {
        0 => 0xe3ab53,
        1 => 0xc7c39d,
        _ => 0x537a7d,
    };
    let shadow = match id {
        0 => 0xa7763f,
        1 => 0x8c947c,
        _ => 0x385965,
    };
    let skin = match id {
        0 => 0xb77b59,
        1 => 0xd39770,
        _ => 0xddab86,
    };
    let hair = match id {
        0 => 0x684d42,
        1 => 0x8a5342,
        _ => 0x785846,
    };
    // Fixed feet and the soft contact shadow hold the person on the boards.
    c.rect(x - 5, y + 9, 11, 1, dye(0x796b54));
    c.rect(x - 3, y + 3, 3, 6, outline);
    c.rect(x + 1, y + 3, 3, 6, outline);
    c.rect(x - 4, y + 8, 4, 1, dye(0x554f43));
    c.rect(x + 1, y + 8, 4, 1, dye(0x554f43));
    c.shape(
        &[
            (x - 2, y - 3),
            (x + 2, y - 3),
            (x + 4, y + 1),
            (x + 4, y + 5),
            (x - 4, y + 5),
            (x - 4, y),
        ],
        outline,
    );
    c.shape(
        &[
            (x - 2, y - 2),
            (x + 2, y - 2),
            (x + 3, y + 1),
            (x + 2, y + 4),
            (x - 3, y + 4),
            (x - 3, y),
        ],
        dye(cloth),
    );
    c.rect(x - 3, y, 1, 4, dye(shadow));
    c.rect(x + 1, y + 3, 2, 2, dye(shadow));
    c.rect(x - 2, y - 8, 5, 4, dye(hair));
    c.rect(x - 1, y - 7, 4, 4, dye(skin));
    c.pixel(x + 3, y - 6, dye(skin));
    c.pixel(x + 1, y - 6, outline);
    c.rect(x, y - 3, 2, 1, dye(skin));
    let motion = triangle(elapsed / 470, 1);
    match id {
        0 => {
            // Nell's broad-brimmed oilskin hat and lit brass hurricane lamp.
            c.rect(x - 2, y - 10, 4, 2, dye(cloth));
            c.rect(x - 3, y - 9, 6, 2, dye(cloth));
            c.rect(x - 4, y - 8, 9, 1, dye(0xf4c575));
            c.rect(x - 2, y - 7, 2, 1, dye(shadow));
            c.line((x - 1, y - 2), (x, y), dye(0xf6c675));
            c.pixel(x, y + 2, dye(0x9b714b));
            c.line((x + 2, y), (x + 5, y + motion), dye(cloth));
            c.pixel(x + 5, y + motion, dye(skin));
            let hand = y + motion;
            c.line((x + 5, hand + 1), (x + 7, hand + 1), dye(0xb99050));
            c.rect(x + 5, hand + 2, 4, 1, dye(0xaa8246));
            c.rect(x + 5, hand + 3, 4, 3, dye(0xc99857));
            c.rect(x + 6, hand + 3, 2, 3, mix(0xffd47b, 0xffe0a3, night));
            c.rect(x + 5, hand + 6, 4, 1, dye(0x946c40));
        }
        1 => {
            // Ada's red neckerchief, spectacles, pinned hair and enamel lunch tin.
            c.disc(x - 3, y - 7, 2, dye(hair));
            c.rect(x - 1, y - 9, 4, 2, dye(hair));
            c.line((x - 1, y - 7), (x + 2, y - 8), dye(0xad714d));
            c.rect(x - 1, y - 6, 2, 1, dye(0xe4ca97));
            c.pixel(x + 2, y - 6, dye(0xe4ca97));
            c.line((x - 2, y - 3), (x + 1, y - 2), dye(0xbe5a40));
            c.rect(x, y - 2, 3, 1, dye(0xd77b55));
            c.line((x, y - 1), (x - 2, y + 2), dye(0xa34e3e));
            let hand = y + motion;
            c.rect(x + 3, y, 2, 2, dye(cloth));
            c.pixel(x + 4, hand + 2, dye(skin));
            c.rect(x + 4, hand + 3, 3, 1, outline);
            c.rect(x + 3, hand + 4, 6, 4, dye(0xb7c4af));
            c.rect(x + 3, hand + 4, 6, 1, dye(0xe7d6a9));
            c.rect(x + 3, hand + 7, 6, 1, dye(0xad6b52));
        }
        _ => {
            // Kit's navy peak, small feather, mailing strap and blue dispatch bag.
            c.rect(x - 3, y - 9, 6, 2, dye(0x345767));
            c.rect(x - 2, y - 10, 4, 1, dye(0x426a77));
            c.rect(x - 3, y - 8, 8, 1, dye(0x233f52));
            c.rect(x + 2, y - 8, 3, 1, dye(0x698e8a));
            c.line((x - 3, y - 8), (x - 4, y - 11), dye(0x84aca4));
            c.line((x - 2, y - 2), (x + 3, y + 3), dye(0xbd9666));
            c.rect(x + 2, y + 2 + motion, 6, 4, dye(0x324f65));
            c.rect(x + 2, y + 2 + motion, 6, 2, dye(0x557f94));
            c.rect(x + 3, y + 3 + motion, 3, 1, dye(0xb3b5a1));
            c.pixel(x + 5, y + 4 + motion, dye(0xc6a672));
            c.line((x - 3, y), (x - 5, y + 2), dye(cloth));
            c.pixel(x - 5, y + 3, dye(skin));
        }
    }
}

/// Finish the panorama using the same saved tide and weather as the harbor accounts.
pub fn draw_harbor_watch(buf: &mut [u32; ART_W * ART_H], tide: u64, elapsed: u32, lanterns: bool) {
    let elapsed = elapsed.min(TIDE - 1);
    let clock = (tide % 12) as u32 * TIDE + elapsed;
    let watch = neighbors::watch(tide);
    let weather = weather_strength(tide, elapsed);
    let light = light_at(clock);
    // These blue panes are unique to the harbor ferry. Find the moving cabin on
    // its two possible waterlines before grading; no berth state needs duplicating.
    let mut ferry = None;
    if light.lamps > 0 {
        for y in [128usize, 129] {
            if let Some(first) = buf[y * ART_W + 245..y * ART_W + 305]
                .iter()
                .position(|p| *p == 0x426764)
            {
                ferry = Some((245 + first as i32, y as i32));
                break;
            }
        }
    }
    stroll(&mut Canvas(buf), clock, watch);
    sky_and_grade(buf, clock, light, weather);
    let mut c = Canvas(buf);
    sky_marks(&mut c, clock, light.lamps, weather[1].max(weather[2]));
    lights(&mut c, clock, light.lamps, ferry);
    weather_marks(&mut c, clock, weather);
    visitor_on_quay(&mut c, tide, elapsed, light.lamps);
    if lanterns && watch == 2 {
        lantern_boats(&mut c, clock);
    }
}
