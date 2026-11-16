use crate::Sound;
pub const RATE: usize = 48000;
fn notes(s: Sound) -> &'static [(u16, u16)] {
    match s {
        Sound::Launch => &[(440, 60), (660, 70), (880, 100)],
        Sound::Drop => &[(740, 24), (560, 38)],
        Sound::Delivered => &[(587, 60), (740, 60), (988, 120)],
        Sound::Missed => &[(247, 45), (196, 58)],
        Sound::Thunder => &[(82, 85), (65, 100)],
        Sound::Complete => &[(523, 92), (659, 92), (784, 92), (1047, 200)],
        Sound::Paper => &[(1175, 16), (880, 20)],
        Sound::Quiet => &[],
    }
}
pub fn length(s: Sound) -> usize {
    notes(s).iter().map(|&(_, ms)| ms as usize * 48).sum()
}
pub fn sample(s: Sound, mut at: usize) -> i16 {
    for &(hz, ms) in notes(s) {
        let len = ms as usize * 48;
        if at < len {
            let phase = (at * hz as usize * 256 / RATE) % 256;
            let wave = if phase < 128 {
                phase as i32 - 64
            } else {
                191 - phase as i32
            };
            let env = at.min(220).min((len - at) * 220 / (len / 3).max(1));
            return (wave * env as i32 / 5) as i16;
        }
        at -= len;
    }
    0
}
