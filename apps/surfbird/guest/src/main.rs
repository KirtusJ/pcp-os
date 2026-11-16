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
                    text(&mut frame, 26, y, if index == 0 { "1" } else { "2" }, 0x65dedf);
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
        record(if index == 0 { "link1" } else { "link2" }, link.path());
    }
    frame.rect(26, 310, 586, 1, 0x447078);
    text(
        &mut frame,
        26,
        314,
        "Tab / Shift+Tab  Select link    Enter / 1 / 2  Follow",
        0x96cace,
    );
    text(&mut frame, 26, 337, "Up / Down  Scroll    B  Back    Esc  Exit", 0x96cace);
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

fn browse_pages(stream: &mut impl Stream) -> Result<(), &'static str> {
    let mut display = Display::new(W, H);
    let mut bodies = [[0; CAPACITY]; 2];
    let mut lengths = [0; 2];
    let mut locations = [Location::start(); 2];
    let mut views = [View::default(); 2];
    lengths[0] = fetch(stream, START, &mut bodies[0])?;
    let mut slot = 0;
    let mut previous = false;
    loop {
        let page = parse(&bodies[slot][..lengths[slot]])?;
        let path = locations[slot].address();
        let last_full_view = page.lines().count().saturating_sub(VISIBLE_ROWS);
        present(&mut display, path, &page, views[slot]);
        let index = loop {
            stream.poll()?;
            let Some(key) = display.next_press() else {
                sleep_us(2000);
                continue;
            };
            let old_view = views[slot];
            match key {
                1 => return Ok(()),
                48 if previous => break None,
                2 if page.count > 0 => break Some(0),
                3 if page.count > 1 => break Some(1),
                15 => views[slot].tab(&page, display.held[42] || display.held[54]),
                28 if old_view.focus.is_some() => break old_view.focus,
                72 => views[slot].row = old_view.row.saturating_sub(1),
                80 if old_view.row < last_full_view => views[slot].row += 1,
                _ => {}
            }
            if views[slot] != old_view {
                present(&mut display, path, &page, views[slot]);
            }
        };
        let other = 1 - slot;
        if let Some(i) = index {
            let target = match locations[slot].follow(page.links[i].path()) {
                Ok(target) => target,
                Err(error) => {
                    record("navigation", error);
                    continue;
                }
            };
            views[other] = View::default();
            if target.fragment().is_some()
                && target.request_target() == locations[slot].request_target()
            {
                views[other].row = page.fragment_line(target.fragment(), views[slot].row);
                lengths[other] = lengths[slot];
                bodies[other] = bodies[slot];
            } else {
                lengths[other] = fetch(stream, target.request_target(), &mut bodies[other])?;
                let next = parse(&bodies[other][..lengths[other]])?;
                views[other].row = next.fragment_line(target.fragment(), 0);
            }
            locations[other] = target;
            previous = true;
        } else {
            previous = false;
        }
        slot = other;
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
