#![no_std]

mod islands;
pub mod mail;
mod postcard;
use mail::Mailbox;
mod paint;
mod route;
mod scenery;
mod screen;
pub use route::{clean_album, Route, ROUTES};

pub const W: usize = 512;
pub const H: usize = 288;
pub type Pixels = [u32; W * H];

pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;
pub const ACTION: u8 = 16;
pub const PAUSE: u8 = 32;
pub const BRAKE: u8 = 64;
pub const MAIL: u8 = 128;
pub const STEP_MS: u32 = 25;
pub const ROUTE: i32 = 2280;
pub const NAMES: [&str; 3] = ["PINE KEY", "MOTH BANK", "GULL ROCK"];
pub const POSTS: [(i32, i32); 3] = [(555, 201), (1280, 170), (2050, 207)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Title,
    Flying,
    Paused,
    Arrived,
    Mail,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Quiet,
    Launch,
    Drop,
    Delivered,
    Missed,
    Thunder,
    Complete,
    Paper,
}
#[derive(Clone, Copy)]
pub struct Letter {
    pub x: i32,
    pub y: i32,
    pub age: i32,
}
#[derive(Clone, Copy)]
pub enum Notice {
    None,
    Landed(usize),
    Miss,
    Gust,
    Again,
}

pub struct Game {
    pub scene: Scene,
    pub tick: u32,
    pub flight_ticks: u32,
    pub route: Route,
    pub album: u32,
    pub centered: u8,
    pub mailbox: Mailbox,
    before_mail: Scene,
    pub braking: bool,
    pub distance: i32,
    pub x: i32,
    pub y: i32,
    vx: i32,
    vy: i32,
    pub delivered: u8,
    pub drops: u16,
    pub laps: u16,
    pub letter: Option<Letter>,
    pub notice: Notice,
    pub notice_time: u8,
    pub gust: u8,
    pub celebration: u8,
    previous: u8,
}

impl Game {
    pub const fn new() -> Self {
        Self {
            scene: Scene::Title,
            tick: 0,
            flight_ticks: 0,
            route: Route::Coast,
            album: 0,
            centered: 0,
            mailbox: Mailbox::new(),
            before_mail: Scene::Title,
            braking: false,
            distance: 0,
            x: 118,
            y: 120,
            vx: 0,
            vy: 0,
            delivered: 0,
            drops: 0,
            laps: 1,
            letter: None,
            notice: Notice::None,
            notice_time: 0,
            gust: 0,
            celebration: 0,
            previous: 0,
        }
    }

    pub fn draw(&self, pixels: &mut Pixels) {
        screen::draw(self, pixels);
    }

    pub fn count(&self) -> u32 {
        self.delivered.count_ones()
    }

    // A roof is visible briefly on either side of the viewport. Wrap its position
    // into that interval so a letter can cross the route seam with its bird.
    pub fn post_x(&self, id: usize) -> i32 {
        self.screen_x(self.route.position(id, self.flight_ticks).0)
    }
    pub fn post_y(&self, id: usize) -> i32 {
        self.route.position(id, self.flight_ticks).1
    }
    pub fn screen_x(&self, world_x: i32) -> i32 {
        (world_x - self.distance + 200).rem_euclid(self.route.length()) - 200
    }
    pub fn scroll_speed(&self) -> i32 {
        if self.braking {
            1
        } else {
            self.route.speed()
        }
    }
    pub fn restore_album(&mut self, value: u32) {
        self.album = clean_album(value);
    }
    pub fn drop_origin(&self) -> Letter {
        Letter {
            x: self.x + 5,
            y: self.y + 6,
            age: 0,
        }
    }
    pub fn return_to_chart(&mut self) {
        let (route, album, mailbox) = (self.route, self.album, self.mailbox);
        *self = Self::new();
        self.route = route;
        self.album = album;
        self.mailbox = mailbox;
    }

    pub fn clouds(&self) -> [(i32, i32); 3] {
        let bob = triangle((self.flight_ticks / 4) as i32, 34) / 3;
        let clouds = match self.route {
            Route::Coast => [(850, 87 + bob), (1550, 119 - bob), (1900, 64 + bob)],
            Route::Flotilla => [(760, 137 - bob), (1540, 88 + bob), (1970, 150 - bob)],
            Route::Aurora => [(820, 106 + bob), (1610, 72 - bob), (2040, 133 + bob)],
        };
        clouds.map(|(x, y)| (self.screen_x(x), y))
    }

    fn notice(&mut self, notice: Notice, ticks: u8) {
        self.notice = notice;
        self.notice_time = ticks;
    }

    fn takeoff(&mut self) {
        self.return_to_chart();
        self.scene = Scene::Flying;
        self.previous = ACTION;
    }

    pub fn step(&mut self, keys: u8) -> Sound {
        let pressed = keys & !self.previous;
        self.previous = keys;
        if pressed & MAIL != 0 {
            if self.scene == Scene::Mail {
                self.scene = self.before_mail;
            } else {
                self.before_mail = self.scene;
                self.scene = Scene::Mail;
                if !Mailbox::received(self.mailbox.card, self.album) {
                    self.mailbox.card = 0;
                }
                self.mailbox.side = mail::Side::Picture;
            }
            return Sound::Paper;
        }
        if self.scene == Scene::Mail {
            if pressed & ACTION != 0 {
                self.mailbox.flip();
                return Sound::Paper;
            }
            if pressed & (LEFT | RIGHT) != 0 {
                self.mailbox.browse(pressed & RIGHT != 0, self.album);
                return Sound::Paper;
            }
            return Sound::Quiet;
        }
        if self.scene != Scene::Paused {
            self.tick = self.tick.wrapping_add(1);
        }
        match self.scene {
            Scene::Title | Scene::Arrived => {
                if pressed & (LEFT | RIGHT) != 0 {
                    let choice = self.route.turn(pressed & RIGHT != 0);
                    self.return_to_chart();
                    self.route = choice;
                    self.previous = keys;
                    return Sound::Quiet;
                }
                if pressed & ACTION != 0 {
                    self.takeoff();
                    return Sound::Launch;
                }
                return Sound::Quiet;
            }
            Scene::Paused => {
                if pressed & ACTION != 0 {
                    self.return_to_chart();
                    self.previous = keys;
                    return Sound::Quiet;
                }
                if pressed & PAUSE != 0 {
                    self.scene = Scene::Flying;
                }
                return Sound::Quiet;
            }
            Scene::Flying if pressed & PAUSE != 0 => {
                self.scene = Scene::Paused;
                return Sound::Quiet;
            }
            _ => {}
        }
        self.flight_ticks = self.flight_ticks.saturating_add(1);
        self.notice_time = self.notice_time.saturating_sub(1);
        self.celebration = self.celebration.saturating_sub(1);
        self.gust = self.gust.saturating_sub(1);
        self.braking = keys & BRAKE != 0;
        let prior_distance = self.distance;
        self.distance = (self.distance + self.scroll_speed()) % self.route.length();
        if self.distance < prior_distance {
            self.laps = self.laps.saturating_add(1);
            self.notice(Notice::Again, 90);
        }

        let steer =
            |positive, negative| i32::from(keys & positive != 0) - i32::from(keys & negative != 0);
        self.vx = approach(self.vx, steer(RIGHT, LEFT) * 3);
        self.vy = approach(self.vy, steer(DOWN, UP) * 3);
        self.x = (self.x + self.vx).clamp(32, 322);
        self.y = (self.y + self.vy + i32::from(self.gust > 31) * 2).clamp(49, 229);
        let mut sound = Sound::Quiet;
        if self.gust == 0
            && self
                .clouds()
                .iter()
                .any(|&(cx, cy)| (cx - self.x).abs() < 34 && (cy - self.y).abs() < 14)
        {
            self.gust = 58;
            self.notice(Notice::Gust, 44);
            sound = Sound::Thunder;
        }

        if let Some(mut letter) = self.letter {
            let prior_y = letter.y;
            letter.advance(self.scroll_speed());
            let target = (0..3).find(|&i| {
                self.delivered & (1 << i) == 0
                    && prior_y <= self.route.position(i, self.flight_ticks - 1).1
                    && letter.y >= self.post_y(i)
                    && (letter.x - self.post_x(i)).abs() <= self.route.radius()
            });
            if let Some(id) = target {
                self.delivered |= 1 << id;
                let stamp = self.route.bit(id);
                self.album |= stamp;
                self.mailbox.card = 1 + self.route as usize * 3 + id;
                self.mailbox.side = mail::Side::Picture;
                if (letter.x - self.post_x(id)).abs() <= 6 {
                    self.centered |= 1 << id;
                    self.album |= stamp << 9;
                }
                self.letter = None;
                self.celebration = 38;
                self.notice(Notice::Landed(id), 70);
                if self.count() == 3 {
                    self.scene = Scene::Arrived;
                    return Sound::Complete;
                }
                return Sound::Delivered;
            } else if letter.y > 247 || letter.x < -8 {
                self.letter = None;
                self.notice(Notice::Miss, 40);
                sound = Sound::Missed;
            } else {
                self.letter = Some(letter);
            }
        }
        if pressed & ACTION != 0 && self.letter.is_none() {
            self.letter = Some(self.drop_origin());
            self.drops = self.drops.saturating_add(1);
            sound = Sound::Drop;
        }
        sound
    }
}

impl Letter {
    pub fn advance(&mut self, scroll_speed: i32) {
        self.age += 1;
        // The envelope gains one world pixel per tick while the camera advances.
        self.x += 1 - scroll_speed;
        self.y += (2 + self.age / 5).min(8);
    }
}

fn approach(value: i32, target: i32) -> i32 {
    value + (target - value).signum()
}
pub(crate) fn triangle(n: i32, period: i32) -> i32 {
    let p = n.rem_euclid(period);
    if p < period / 2 {
        p
    } else {
        period - p
    }
}
pub mod audio;
