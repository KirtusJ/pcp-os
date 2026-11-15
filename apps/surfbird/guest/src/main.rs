#![no_std]
#![no_main]

use pcp_game_guest::{exit, ink::Frame, log, sleep_us, time_us, Display};
#[cfg(feature = "local-media")]
mod local_media;
#[cfg(feature = "tcp-ip")]
mod tcp;
#[cfg(feature = "https-probe")]
mod https;
#[cfg(feature = "https-probe")]
mod watch_view;
const ORIGIN_HOST: &str = "fixture.test";
use surfbird_cable::{body_length, parse, Page, CAPACITY, START};

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

fn label(frame: &mut Frame, x: i32, y: i32, value: &str, color: u32) {
    let mut bytes = [0; 96];
    for (dst, src) in bytes.iter_mut().zip(value.bytes()) {
        *dst = src.to_ascii_uppercase();
    }
    let upper = core::str::from_utf8(&bytes[..value.len()]).unwrap();
    frame.text(x, y, upper, 1, color);
}

fn present(display: &mut Display, path: &str, page: &Page<'_>) {
    let pixels = unsafe { &mut *(&raw mut PIXELS) };
    pixels.fill(0x102a33);
    let mut f = Frame {
        px: pixels,
        pitch: W,
    };
    f.text(25, 26, "SURFBIRD", 3, 0x70e7e3);
    label(&mut f, 26, 65, path, 0xa5b9bc);
    f.rect(26, 87, 586, 1, 0x447078);
    label(&mut f, 26, 109, page.title, 0xf2f6f7);
    label(&mut f, 26, 136, page.text, 0xcbdadd);
    record("path", path);
    record("title", page.title);
    record("text", page.text);
    for (i, l) in page.links[..page.count].iter().enumerate() {
        let y = 185 + 32 * i as i32;
        f.text(26, y, if i == 0 { "1" } else { "2" }, 2, 0x65dedf);
        label(&mut f, 51, y + 4, l.label, 0xe4edef);
        record(if i == 0 { "link1" } else { "link2" }, l.path);
    }
    f.text(
        26,
        315,
        "1 / 2  FOLLOW     B  BACK     ESC  EXIT",
        1,
        0x96cace,
    );
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
    let mut paths = [[0; 96]; 2];
    let mut path_len = [START.len(), 0];
    paths[0][..START.len()].copy_from_slice(START.as_bytes());
    lengths[0] = fetch(stream, START, &mut bodies[0])?;
    let mut slot = 0;
    let mut previous = false;
    loop {
        let page = parse(&bodies[slot][..lengths[slot]])?;
        let path = core::str::from_utf8(&paths[slot][..path_len[slot]]).unwrap();
        present(&mut display, path, &page);
        // Keep the displayed document borrowed while waiting for the next selection.
        let index = loop {
            stream.poll()?;
            display.poll();
            if display.pressed[1] {
                return Ok(());
            }
            if previous && display.pressed[48] {
                break None;
            }
            if page.count > 0 && display.pressed[2] {
                break Some(0);
            }
            if page.count > 1 && display.pressed[3] {
                break Some(1);
            }
            sleep_us(2000);
        };
        let other = 1 - slot;
        if let Some(i) = index {
            let target = page.links[i].path.as_bytes();
            path_len[other] = target.len();
            paths[other][..target.len()].copy_from_slice(target);
            let target = core::str::from_utf8(&paths[other][..path_len[other]]).unwrap();
            lengths[other] = fetch(stream, target, &mut bodies[other])?;
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
