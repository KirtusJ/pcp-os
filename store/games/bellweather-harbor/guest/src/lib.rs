#![no_std]
mod device;
mod freight;
mod glyph;
mod journal;
mod mail;
mod order;
mod quay;
mod sound;
mod ui;
mod works;
mod yard;

use bellweather::economy::{Bulk, Economy, MAX_UNITS};
use sound::Cue;
use ui::{Book, Desk, Notice};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    use core::fmt::Write;
    struct ConsoleLog;
    impl Write for ConsoleLog {
        fn write_str(&mut self, text: &str) -> core::fmt::Result {
            device::log(text);
            Ok(())
        }
    }
    let _ = writeln!(ConsoleLog, "Bellweather: {info}");
    unsafe { device::_exit(3) }
}
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

fn set_notice(desk: &mut Desk, event: Notice, now: u64) {
    desk.notice = event;
    desk.notice_until = now + 3000;
}

enum Outcome {
    None,
    Changed(Cue),
    Save,
    Sound(Cue),
    Exit,
}

fn handle(key: usize, game: &mut Economy, desk: &mut Desk, now: u64) -> Outcome {
    let i = desk.selected;
    if matches!(
        desk.book,
        Book::Harbor | Book::Yard(_) | Book::Freight(_) | Book::Works(_) | Book::Quay(_)
    ) {
        if let Book::Freight(exchange) = &mut desk.book {
            if key == 1 && exchange.order.is_some() {
                return exchange.handle(key, game, now);
            }
        }
        let next = match (&desk.book, key) {
            (Book::Quay(quay), 1) if quay.in_letters() => None,
            (Book::Quay(_), 16 | 1) => Some(Book::Harbor),
            (Book::Freight(_), 33 | 1) | (Book::Yard(_), 21 | 1) | (Book::Works(_), 17 | 1) => {
                Some(Book::Harbor)
            }
            (_, 33) => Some(Book::Freight(freight::Exchange::new())),
            (_, 21) => Some(Book::Yard(yard::Slipway::open(game))),
            (_, 17) => Some(Book::Works(works::WorksDesk::new())),
            (_, 16) => Some(Book::Quay(quay::QuayDesk::open(game))),
            _ => None,
        };
        if let Some(page) = next {
            desk.book = page;
            return Outcome::None;
        }
    }
    match &mut desk.book {
        Book::Quay(quay) => return quay.handle(key, game),
        Book::Journal(journal) => {
            if journal.close(key) {
                desk.book = Book::Harbor;
                return Outcome::None;
            }
            return journal.handle(key, game);
        }
        Book::Yard(slipway) => return slipway.handle(key, game, now),
        Book::Freight(exchange) => return exchange.handle(key, game, now),
        Book::Works(works) => return works.handle(key, game, now),
        _ => {}
    }
    if let Book::Mail(run) = &mut desk.book {
        match key {
            1 | 25 => desk.book = Book::Harbor,
            57 | 28 => {
                if run.attempt(game, i, now) {
                    device::log("Bellweather: berth postmark collected\n");
                    return Outcome::Changed(Cue::Manager);
                }
            }
            _ => {}
        }
        return Outcome::None;
    }
    if matches!(desk.book, Book::Charter) {
        match key {
            1 | 46 => desk.book = Book::Harbor,
            28 => {
                let award = game.charter();
                if award > 0 {
                    desk.selected = 0;
                    desk.book = Book::Harbor;
                    set_notice(desk, Notice::Chartered(award), now);
                    device::log("Bellweather: new charter signed\n");
                    return Outcome::Changed(Cue::Charter);
                }
            }
            _ => {}
        }
        return Outcome::None;
    }
    match key {
        36 if game.manifest.unlocked => desk.book = Book::Journal(journal::Journal::open(game)),
        1 => return Outcome::Exit,
        2..=6 => desk.selected = key - 2,
        72 | 75 => desk.selected = (i + 4) % 5,
        80 | 77 => desk.selected = (i + 1) % 5,
        45 => {
            desk.bulk = match desk.bulk {
                Bulk::One => Bulk::Ten,
                Bulk::Ten => Bulk::Max,
                Bulk::Max => Bulk::One,
            }
        }
        46 => desk.book = Book::Charter,
        25 => desk.book = Book::Mail(mail::MailRun::open(game, i, now)),
        19 => {
            game.ring();
            desk.bell_reply
                .ring(now, bellweather::neighbors::visitor(game.freight.tide));
            set_notice(desk, Notice::Bell, now);
            return Outcome::Changed(Cue::Bell);
        }
        57 | 28 => {
            if game.dispatch(i) {
                device::log("Bellweather: voyage dispatched\n");
                set_notice(desk, Notice::Dispatch, now);
                return Outcome::Changed(Cue::CastOff);
            }
            set_notice(
                desk,
                if game.shops[i].owned == 0 {
                    Notice::NoFleet
                } else {
                    Notice::Busy
                },
                now,
            );
        }
        48 => {
            let added = game.buy(i, desk.bulk);
            if added > 0 {
                device::log("Bellweather: fleet expanded\n");
                set_notice(desk, Notice::Bought(added), now);
                return Outcome::Changed(Cue::Purchase);
            }
            set_notice(
                desk,
                if game.shops[i].owned == MAX_UNITS {
                    Notice::MaxFleet
                } else {
                    Notice::Unaffordable
                },
                now,
            );
        }
        50 => {
            if game.hire(i) {
                device::log("Bellweather: manager hired\n");
                set_notice(desk, Notice::Hired, now);
                return Outcome::Changed(Cue::Manager);
            }
            let why = if game.shops[i].manager {
                Notice::Staffed
            } else if game.shops[i].owned == 0 {
                Notice::NoFleet
            } else {
                Notice::Unaffordable
            };
            set_notice(desk, why, now);
        }
        _ => {}
    }
    Outcome::None
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let Some(mut console) = device::Console::open() else {
        device::log("Bellweather needs a 640x416 framebuffer and keyboard\n");
        return 2;
    };
    static mut SCREEN: ui::Screen = [0; ui::W * ui::H];
    static mut HARBOR: ui::Harbor = [0; 480 * 192];
    // These buffers have one owner: this single-threaded guest's main loop.
    let screen = unsafe { &mut *(&raw mut SCREEN) };
    let harbor = unsafe { &mut *(&raw mut HARBOR) };
    let (mut game, notice) = match device::load() {
        Some((game, amount)) => {
            device::log("Bellweather: ledger restored\n");
            if amount > 0 {
                device::log("Bellweather: away receipts settled\n");
            }
            (game, Notice::Restored(amount))
        }
        None => (Economy::new(), Notice::Welcome),
    };
    let mut last = device::clock_ms(1);
    let mut last_frame = None;
    let mut last_save = last;
    let mut desk = Desk {
        selected: 0,
        bulk: Bulk::One,
        book: Book::Harbor,
        notice,
        notice_until: last + 6000,
        store_ok: device::save(&game),
        bell_reply: bellweather::neighbors::BellReply::new(),
        answer: None,
    };
    let mut speaker = sound::Speaker::open();
    let mut failed_exit = false;
    device::log("Bellweather: harbor open\n");
    loop {
        let now = device::clock_ms(1);
        let pending_freight = game.freight.reserved.then(|| game.freight.offer().reward);
        let built_before = game.yard.completed;
        let income = game.advance(now.saturating_sub(last));
        if game.yard.completed != built_before {
            let launched = (game.yard.completed ^ built_before).trailing_zeros() as usize;
            device::log("Bellweather: vessel launched\n");
            set_notice(&mut desk, Notice::Launched(launched), now);
            if let Book::Yard(slipway) = &mut desk.book {
                slipway.notice = yard::Notice::Launched(launched);
                slipway.notice_until = now + 6000;
            }
            speaker.play(Cue::Charter);
        }
        if let Some(payment) = pending_freight.filter(|_| !game.freight.reserved) {
            device::log("Bellweather: freight order delivered\n");
            if let Book::Freight(exchange) = &mut desk.book {
                exchange.receipt = freight::Receipt::Delivered(payment);
                exchange.receipt_until = now + 5000;
            }
        }
        last = now;
        if income > 0
            && (now >= desk.notice_until
                || matches!(desk.notice, Notice::Dispatch | Notice::Paid(_)))
        {
            set_notice(&mut desk, Notice::Paid(income), now);
        }
        let mut changed = false;
        while let Some(key) = console.next_key() {
            match handle(key, &mut game, &mut desk, now) {
                Outcome::Exit => {
                    if device::save(&game) {
                        device::log("Bellweather: ledger saved, returning to launcher\n");
                        return 0;
                    }
                    if failed_exit {
                        device::log("Bellweather: left with store unavailable\n");
                        return 0;
                    }
                    failed_exit = true;
                    desk.store_ok = false;
                    set_notice(&mut desk, Notice::SaveFailed, now);
                }
                Outcome::Sound(cue) => speaker.play(cue),
                Outcome::Save => {
                    changed = true;
                    failed_exit = false;
                }
                Outcome::Changed(cue) => {
                    speaker.play(cue);
                    changed = true;
                    failed_exit = false;
                }
                Outcome::None => {}
            }
        }
        if changed || now.saturating_sub(last_save) >= 3000 {
            desk.store_ok = device::save(&game);
            last_save = now;
            if desk.store_ok {
                failed_exit = false;
            }
        }
        if let Some(person) = desk.bell_reply.poll(now) {
            desk.answer = Some((person, now + 2800));
            speaker.play(Cue::Answer(person));
        }
        speaker.pump();
        // The small audio DMA queue is serviced more frequently than the display.
        if last_frame.is_none_or(|frame| now.saturating_sub(frame) >= 40) {
            ui::render(screen, harbor, &game, &mut desk, now);
            if !console.show(screen) {
                device::save(&game);
                device::log("Bellweather: display update failed\n");
                return 2;
            }
            last_frame = Some(now);
        }
        device::sleep_ms(
            speaker
                .frame_ms()
                .saturating_sub(device::clock_ms(1).saturating_sub(now))
                .max(1),
        );
    }
}
