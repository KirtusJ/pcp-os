#![no_std]
#![no_main]

use pcp_game_guest::{exit, ink::Frame, log, sleep_us, time_us, Display};
#[cfg(feature = "https-probe")]
mod https;
#[cfg(feature = "local-media")]
mod local_media;
#[cfg(feature = "tcp-ip")]
mod tcp;
#[cfg(feature = "https-probe")]
mod watch_view;
mod page_text;
const ORIGIN_HOST: &str = "fixture.test";
use page_text::text;
use surfbird_cable::history::{History, Visit};
use surfbird_cable::layout::{Source, View, VISIBLE_ROWS};
use surfbird_cable::{body_length, location::Location, parse, Page, CAPACITY, START};

const W: usize = 640;
const H: usize = 360;
static mut PIXELS: [u32; W * H] = [0; W * H];

trait Stream {
    fn poll(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn write(&mut self, data: &[u8]) -> Result<usize, &'static str>;
    fn read(&mut self, data: &mut [u8]) -> Result<usize, &'static str>;
}

#[cfg(not(feature = "tcp-ip"))]
impl Stream for usize {
    fn write(&mut self, data: &[u8]) -> Result<usize, &'static str> {
        let n = pcp_game_guest::call(1, *self, data.as_ptr() as usize, data.len(), 0, 0, 0);
        if n == -11 {
            Ok(0)
        } else {
            usize::try_from(n).map_err(|_| "write failed")
        }
    }
    fn read(&mut self, data: &mut [u8]) -> Result<usize, &'static str> {
        let n = pcp_game_guest::call(0, *self, data.as_mut_ptr() as usize, data.len(), 0, 0, 0);
        if n == -11 {
            Ok(0)
        } else {
            usize::try_from(n).map_err(|_| "read failed")
        }
    }
}

fn send(stream: &mut impl Stream, mut bytes: &[u8], deadline: u64) -> Result<(), &'static str> {
    while !bytes.is_empty() {
        if time_us() >= deadline {
            return Err("send timeout");
        }
        stream.poll()?;
        let n = stream.write(bytes)?;
        if n == 0 {
            sleep_us(1000);
            continue;
        }
        if n > bytes.len() {
            return Err("write failed");
        }
        bytes = &bytes[n..];
    }
    Ok(())
}
fn fetch(
    stream: &mut impl Stream,
    path: &str,
    wire: &mut [u8; CAPACITY],
) -> Result<usize, &'static str> {
    let deadline = time_us() + 8_000_000;
    let mut request_len = 0;
    for bytes in [
        b"GET ".as_slice(),
        path.as_bytes(),
        b" HTTP/1.1\r\nHost: ",
        ORIGIN_HOST.as_bytes(),
        b"\r\nAccept: text/html\r\n\r\n",
    ] {
        let end = request_len + bytes.len();
        wire.get_mut(request_len..end)
            .ok_or("request too large")?
            .copy_from_slice(bytes);
        request_len = end;
    }
    send(stream, &wire[..request_len], deadline)?;
    let mut used = 0;
    let mut body = None;
    loop {
        if time_us() >= deadline {
            return Err("receive timeout");
        }
        if used == wire.len() {
            return Err("response too large");
        }
        let room = &mut wire[used..];
        stream.poll()?;
        let n = stream.read(room)?;
        if n == 0 {
            sleep_us(1000);
            continue;
        }
        if n > room.len() {
            return Err("read failed");
        }
        used += n;
        if body.is_none() {
            if let Some(at) = wire[..used].windows(4).position(|s| s == b"\r\n\r\n") {
                let start = at + 4;
                let length = body_length(&wire[..at])?;
                if length > wire.len() - start {
                    return Err("response too large");
                }
                body = Some((start, start + length));
            }
        }
        if let Some((start, end)) = body {
            if used > end {
                return Err("unexpected trailing bytes");
            }
            if used == end {
                wire.copy_within(start..end, 0);
                return Ok(end - start);
            }
        }
    }
}

fn record(key: &str, value: &str) {
    // Test captures and human display are both derived from the parsed response.
    log(b"SURFBIRD ");
    log(key.as_bytes());
    log(b" ");
    log(value.as_bytes());
    log(b"\n");
}

fn present(display: &mut Display, path: &str, page: &Page<'_>, view: View) {
    let pixels = unsafe { &mut *(&raw mut PIXELS) };
    pixels.fill(0x102a33);
    let mut frame = Frame {
        px: pixels,
        pitch: W,
    };
    text(&mut frame, 26, 10, "Surfbird", 0x70e7e3);
    for (line, chunk) in path.as_bytes().chunks(58).enumerate() {
        text(
            &mut frame,
            26,
            39 + 21 * line,
            core::str::from_utf8(chunk).unwrap(),
            0xa5b9bc,
        );
    }
    frame.rect(26, 92, 586, 1, 0x447078);
    let mut outline_top = None;
    let mut outline_bottom = 0;
    for (i, line) in page.lines().skip(view.row).take(VISIBLE_ROWS).enumerate() {
        let y = 104 + i * 22;
        let (x, color) = match line.source {
            Source::Title => (26, 0xf2f6f7),
            Source::Paragraph(_) => {
                record("text", line.text);
                (26, 0xcbdadd)
            }
            Source::Link(index) => {
                if view.focus == Some(index) {
                    outline_top.get_or_insert(y as i32 - 2);
                    outline_bottom = y as i32 + 21;
                }
                if line.first {
                    let digit = &"123456789"[index..index + 1];
                    text(&mut frame, 26, y, digit, 0x65dedf);
                }
                (56, 0xe4edef)
            }
        };
        text(&mut frame, x, y, line.text, color);
    }
    if let Some(top) = outline_top {
        let color = 0x70e7e3;
        frame.rect(20, top, 596, 2, color);
        frame.rect(20, outline_bottom - 1, 596, 2, color);
        frame.rect(20, top, 2, outline_bottom - top, color);
        frame.rect(614, top, 2, outline_bottom - top, color);
    }
    record("path", path);
    record("title", page.title);
    for (index, link) in page.links[..page.count].iter().enumerate() {
        let mut key = *b"link1";
        key[4] += index as u8;
        record(core::str::from_utf8(&key).unwrap(), link.path());
    }
    frame.rect(26, 310, 586, 1, 0x447078);
    text(
        &mut frame,
        26,
        314,
        "Tab / Shift+Tab  Select link    Enter / 1-9  Follow",
        0x96cace,
    );
    text(&mut frame, 26, 337, "Up/Down  Scroll    B  Back    F  Forward    Esc  Exit", 0x96cace);
    display.show(pixels);
    log(b"SURFBIRD ready\n");
}

#[cfg(not(feature = "tcp-ip"))]
fn browse() -> Result<(), &'static str> {
    let fd = pcp_game_guest::open(b"/dev/game-link\0", 2 | 0x800);
    if fd < 0 {
        return Err("cable grant unavailable");
    }
    browse_pages(&mut (fd as usize))
}

#[cfg(feature = "tcp-ip")]
use tcp::browse;

enum Navigation {
    Back,
    Forward,
    Follow(usize),
}

fn browse_pages(stream: &mut impl Stream) -> Result<(), &'static str> {
    let mut display = Display::new(W, H);
    let mut first = Visit::new(Location::start());
    first.length = fetch(stream, START, &mut first.body)?;
    let mut history = History::new(first);
    loop {
        let can_back = history.can_back();
        let can_forward = history.can_forward();
        let current = history.current();
        let page = parse(&current.body[..current.length])?;
        let path = current.location.address();
        let last_full_view = page.lines().count().saturating_sub(VISIBLE_ROWS);
        present(&mut display, path, &page, current.view);
        let navigation = loop {
            stream.poll()?;
            let Some(key) = display.next_press() else {
                sleep_us(2000);
                continue;
            };
            let old_view = current.view;
            match key {
                1 => return Ok(()),
                48 if can_back => break Navigation::Back,
                33 if can_forward => break Navigation::Forward,
                2..=10 if key - 2 < page.count => break Navigation::Follow(key - 2),
                15 => current.view.tab(&page, display.held[42] || display.held[54]),
                28 => {
                    if let Some(index) = old_view.focus {
                        break Navigation::Follow(index);
                    }
                }
                72 => current.view.row = old_view.row.saturating_sub(1),
                80 if old_view.row < last_full_view => current.view.row += 1,
                _ => {}
            }
            if current.view != old_view {
                present(&mut display, path, &page, current.view);
            }
        };
        let index = match navigation {
            Navigation::Back => {
                history.back();
                continue;
            }
            Navigation::Forward => {
                history.forward();
                continue;
            }
            Navigation::Follow(index) => index,
        };
        let target = match current.location.follow(page.links[index].path()) {
            Ok(target) => target,
            Err(error) => {
                record("navigation", error);
                continue;
            }
        };
        let mut next = Visit::new(target);
        if target.fragment().is_some()
            && target.request_target() == current.location.request_target()
        {
            next.view.row = page.fragment_line(target.fragment(), current.view.row);
            next.body[..current.length].copy_from_slice(&current.body[..current.length]);
            next.length = current.length;
        } else {
            next.length = fetch(stream, target.request_target(), &mut next.body)?;
            let destination = parse(&next.body[..next.length])?;
            next.view.row = destination.fragment_line(target.fragment(), 0);
        }
        history.push(next);
    }
}

pcp_game_guest::entry!(surf_main);
#[no_mangle]
pub extern "C" fn surf_main() -> ! {
    #[cfg(feature = "local-media")]
    match local_media::menu() {
        Ok(true) => exit(0),
        Ok(false) => {}
        Err(error) => {
            record("error", error);
            exit(1);
        }
    }
    match browse() {
        Ok(()) => exit(0),
        Err(e) => {
            record("error", e);
            exit(1);
        }
    }
}
