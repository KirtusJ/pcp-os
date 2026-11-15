//! Polled PIO for an explicitly selected QEMU primary-master IDE store.
//! All accesses are confined to the marked volume's first 259 sectors.
use super::{in16, in8, out16, out8};

const BASE: u16 = 0x1f0;
const CONTROL: u16 = 0x3f6;
pub const SECTORS: u16 = 259;

fn wait(want_data: bool, check_command: bool) -> Result<(), ()> {
    for _ in 0..1_000_000 {
        let status = in8(BASE + 7);
        if status == 0 || status == 0xff {
            return Err(());
        }
        if status & 0x80 == 0 {
            // ERR can remain latched until the next command is issued.
            if check_command && status & 0x21 != 0 { return Err(()); }
            if (status & 8 != 0) == want_data { return Ok(()); }
        }
        core::hint::spin_loop();
    }
    Err(())
}

fn issue(lba: u16, command: u8) -> Result<(), ()> {
    if lba >= SECTORS { return Err(()); }
    out8(CONTROL, 2); // nIEN: this single-user controller is polled.
    out8(BASE + 6, 0xe0);
    for _ in 0..4 { in8(CONTROL); }
    wait(false, false)?;
    out8(BASE + 2, 1);
    out8(BASE + 3, lba as u8);
    out8(BASE + 4, (lba >> 8) as u8);
    out8(BASE + 5, 0);
    out8(BASE + 7, command);
    for _ in 0..4 { in8(CONTROL); }
    Ok(())
}

pub fn read(lba: u16) -> Result<[u8; 512], ()> {
    issue(lba, 0x20)?;
    wait(true, true)?;
    let mut bytes = [0; 512];
    for pair in bytes.chunks_exact_mut(2) { pair.copy_from_slice(&in16(BASE).to_le_bytes()); }
    wait(false, true)?;
    Ok(bytes)
}

pub fn write(lba: u16, bytes: &[u8; 512]) -> Result<(), ()> {
    issue(lba, 0x30)?;
    wait(true, true)?;
    for pair in bytes.chunks_exact(2) { out16(BASE, u16::from_le_bytes([pair[0], pair[1]])); }
    wait(false, true)
}

pub fn flush() -> Result<(), ()> {
    issue(0, 0xe7)?;
    wait(false, true)
}
