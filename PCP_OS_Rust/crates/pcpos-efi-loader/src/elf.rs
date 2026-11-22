use crate::Result;
pub const PAGE: u64 = 4096;
const INVALID: &str = "Invalid kernel ELF load geometry.";
fn bytes(b: &[u8], pos: usize, len: usize) -> Result<&[u8]> {
    b.get(pos..pos.checked_add(len).ok_or(INVALID)?)
        .ok_or(INVALID)
}
fn u16le(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(b, p, 2)?.try_into().unwrap()))
}
fn u32le(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(b, p, 4)?.try_into().unwrap()))
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Segment {
    pub start: u64,
    pub memory: usize,
    pub offset: usize,
    pub file: usize,
}
pub struct Elf {
    pub entry: u32,
    pub base: u64,
    pub end: u64,
    segments: [Segment; 32],
    count: usize,
}
impl Elf {
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.get(..7) != Some(b"\x7fELF\x01\x01\x01")
            || u16le(b, 16)? != 2
            || u16le(b, 18)? != 3
            || u32le(b, 20)? != 1
            || u16le(b, 42)? != 32
        {
            return Err(INVALID);
        }
        let entry = u32le(b, 24)?;
        let directory = u32le(b, 28)? as usize;
        let count = u16le(b, 44)? as usize;
        if count > 64 {
            return Err(INVALID);
        }
        let mut elf = Self {
            entry,
            base: 0,
            end: 0,
            segments: [Segment::default(); 32],
            count: 0,
        };
        let mut executable = false;
        for p in bytes(b, directory, count * 32)?.chunks_exact(32) {
            if u32le(p, 0)? != 1 || u32le(p, 20)? == 0 {
                continue;
            }
            let start = u32le(p, 12)? as u64;
            let memory = u32le(p, 20)? as usize;
            let offset = u32le(p, 4)? as usize;
            let file = u32le(p, 16)? as usize;
            if u32le(p, 8)? as u64 != start
                || start < 1 << 20
                || start + memory as u64 >= 1 << 30
                || file > memory
                || elf.count == elf.segments.len()
            {
                return Err(INVALID);
            }
            bytes(b, offset, file)?;
            executable |=
                u32le(p, 24)? & 1 != 0 && (start..start + file as u64).contains(&(entry as u64));
            elf.segments[elf.count] = Segment {
                start,
                memory,
                offset,
                file,
            };
            elf.count += 1;
        }
        let set = &mut elf.segments[..elf.count];
        set.sort_unstable_by_key(|s| s.start);
        if !executable
            || set.is_empty()
            || set[0].start % PAGE != 0
            || set
                .windows(2)
                .any(|w| w[0].start + w[0].memory as u64 > w[1].start)
        {
            return Err(INVALID);
        }
        elf.base = set[0].start;
        let last = set[set.len() - 1];
        elf.end = (last.start + last.memory as u64).next_multiple_of(PAGE);
        Ok(elf)
    }
    pub fn segments(&self) -> &[Segment] {
        &self.segments[..self.count]
    }
}
pub fn measurement(b: &[u8]) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let start = u32le(b, 32)? as usize;
    let width = u16le(b, 46)? as usize;
    let count = u16le(b, 48)? as usize;
    let index = u16le(b, 50)? as usize;
    if width != 40 || index >= count {
        return Err(INVALID);
    }
    let sections = bytes(b, start, width * count)?;
    let section = &sections[index * width..][..width];
    let names = bytes(
        b,
        u32le(section, 16)? as usize,
        u32le(section, 20)? as usize,
    )?;
    let mut found = [None; 2];
    for s in sections.chunks_exact(width) {
        let name = names.get(u32le(s, 0)? as usize..).ok_or(INVALID)?;
        let slot = if name.starts_with(b".text\0") {
            0
        } else if name.starts_with(b".rodata\0") {
            1
        } else {
            continue;
        };
        let data = bytes(b, u32le(s, 16)? as usize, u32le(s, 20)? as usize)?;
        if found[slot].replace((u32le(s, 12)? as u64, data)).is_some() {
            return Err(INVALID);
        }
    }
    let (text, a) = found[0].ok_or(INVALID)?;
    let (ro, b) = found[1].ok_or(INVALID)?;
    if text % PAGE != 0 || text + a.len() as u64 != ro || (ro + b.len() as u64) % PAGE != 0 {
        return Err(INVALID);
    }
    let mut hash = Sha256::new();
    hash.update(a);
    hash.update(b);
    Ok(hash.finalize().into())
}
