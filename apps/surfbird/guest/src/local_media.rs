//! A bounded local video and audio profile. The AC'97 playback cursor drives the picture.
use super::{record, H, PIXELS, W};
use pcp_game_guest::{call, digits, ink::Frame, ioctl, load, log, open, sleep_us, time_us, Display};

const WIDTH: usize = 48;
const HEIGHT: usize = 24;
const COUNT: usize = 32;
const HEADER: &[u8] = b"P6\n48 24\n255\n";
const STRIDE: usize = HEADER.len() + WIDTH * HEIGHT * 3;
const AUDIO_PER_FRAME: usize = 4 * 4800; // 100 ms of signed 16-bit stereo at 48 kHz.
const AUDIO_LEN: usize = AUDIO_PER_FRAME * COUNT;
static mut VIDEO: [u8; STRIDE * COUNT] = [0; STRIDE * COUNT];
static mut WAVE: [u8; 44 + AUDIO_LEN] = [0; 44 + AUDIO_LEN];

fn exists(path: &[u8]) -> bool {
    let fd = open(path, 0);
    if fd >= 0 {
        call(3, fd as usize, 0, 0, 0, 0, 0);
    }
    fd >= 0
}

fn valid_wave(w: &[u8]) -> bool {
    w.len() == 44 + AUDIO_LEN
        && &w[..4] == b"RIFF"
        && w[4..8] == ((w.len() - 8) as u32).to_le_bytes()
        && &w[8..16] == b"WAVEfmt "
        && w[16..20] == 16u32.to_le_bytes()
        && w[20..22] == 1u16.to_le_bytes()
        && w[22..24] == 2u16.to_le_bytes()
        && w[24..28] == 48000u32.to_le_bytes()
        && w[28..32] == 192000u32.to_le_bytes()
        && w[32..34] == 4u16.to_le_bytes()
        && w[34..36] == 16u16.to_le_bytes()
        && &w[36..40] == b"data"
        && w[40..44] == (AUDIO_LEN as u32).to_le_bytes()
}

fn show(display: &mut Display, video: &[u8], index: usize) {
    let src = &video[index * STRIDE + HEADER.len()..(index + 1) * STRIDE];
    let pixels = unsafe { &mut *(&raw mut PIXELS) };
    // Center the image in the display at 12x.
    for y in 0..HEIGHT {
        let first = (36 + y * 12) * W + 32;
        for x in 0..WIDTH {
            let p = (y * WIDTH + x) * 3;
            let rgb = u32::from_be_bytes([0, src[p], src[p + 1], src[p + 2]]);
            pixels[first + 12 * x..first + 12 * (x + 1)].fill(rgb);
        }
        for dy in 1..12 {
            pixels.copy_within(first..first + WIDTH * 12, first + dy * W);
        }
    }
    display.show(pixels);
    record("media-frame", digits(index as u32, &mut [0; 10]));
}

fn play(display: &mut Display, video: &[u8], pcm: &[u8]) -> Result<(), &'static str> {
    let fd = open(b"/dev/audio\0", 1);
    if fd < 0 {
        return Err("audio unavailable");
    }
    let mut queued = 0usize;
    let mut last = None;
    let mut info = [0u32; 5];
    let deadline = time_us() + 12_000_000;
    unsafe { (*(&raw mut PIXELS)).fill(0x0c1218) };
    loop {
        if time_us() >= deadline {
            return Err("audio stopped advancing");
        }
        if ioctl(fd, 0x5011, info.as_mut_ptr() as usize) != 0 || info[..3] != [48000, 2, 16] {
            return Err("unsupported audio device");
        }
        // Waiting counts whole DMA pieces (512 stereo frames) plus the partial piece.
        let played = queued.saturating_sub(info[4] as usize);
        if played == pcm.len() {
            break;
        }
        let index = played / AUDIO_PER_FRAME;
        let count = (pcm.len() - queued).min(info[3] as usize);
        if count > 0 {
            let n = call(1, fd as usize, pcm[queued..].as_ptr() as usize, count, 0, 0, 0);
            // The device can become full between the availability check and write.
            let n = if n == -11 { 0 } else { usize::try_from(n).map_err(|_| "audio write failed")? };
            if n > count || n % 4 != 0 {
                return Err("invalid audio write");
            }
            queued += n;
        }
        // A restarted AC'97 ring can temporarily report a larger backlog. Keep the
        // last picture while the playback estimate catches up, without rewinding.
        if last.is_none_or(|previous| index > previous) {
            show(display, video, index);
            last = Some(index);
        }
        display.poll();
        if display.pressed[1] {
            log(b"SURFBIRD media stopped\n");
            return Ok(());
        }
        // PCP sleeps to the next timer interrupt. Stay responsive to the short
        // hardware queue while playing, especially when the VM is descheduled.
        call(24, 0, 0, 0, 0, 0, 0);
    }
    log(b"SURFBIRD media drained\n");
    Ok(())
}

pub fn menu() -> Result<bool, &'static str> {
    if !exists(b"clip.ppm\0") && !exists(b"clip.wav\0") {
        return Ok(false);
    }
    let video = unsafe { &mut *(&raw mut VIDEO) };
    let wave = unsafe { &mut *(&raw mut WAVE) };
    if !load(b"clip.ppm\0", video)
        || !video.chunks_exact(STRIDE).all(|frame| frame.starts_with(HEADER))
    {
        return Err("invalid local picture sequence");
    }
    if !load(b"clip.wav\0", wave) || !valid_wave(wave) {
        return Err("invalid local stereo track");
    }
    let mut display = Display::new(W, H);
    let pixels = unsafe { &mut *(&raw mut PIXELS) };
    pixels.fill(0x102a33);
    let mut frame = Frame { px: pixels, pitch: W };
    frame.text(36, 45, "SURFBIRD", 3, 0x70e7e3);
    frame.text(36, 139, "LOCAL VIDEO AND STEREO AUDIO", 2, 0xf2f6f7);
    frame.text(36, 260, "SPACE  PLAY     ESC  EXIT", 1, 0xcbdadd);
    display.show(pixels);
    log(b"SURFBIRD media ready\n");
    loop {
        display.poll();
        if display.pressed[1] {
            return Ok(true);
        }
        if display.pressed[57] {
            play(&mut display, video, &wave[44..])?;
            return Ok(true);
        }
        sleep_us(2000);
    }
}
