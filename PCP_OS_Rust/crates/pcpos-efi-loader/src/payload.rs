use crate::Result;
use pcpos_model_media::Manifest;
use sha2::{Digest, Sha256};

pub trait Media {
    type File: Copy;
    fn open(&mut self, name: &[u8]) -> Result<Self::File>;
    fn read(&mut self, file: Self::File, buf: &mut [u8]) -> Result<usize>;
    fn close(&mut self, file: Self::File) -> Result<()>;
    fn progress(&mut self, done: u64, total: u64);
}
fn file<M: Media, T>(
    m: &mut M,
    name: &[u8],
    body: impl FnOnce(&mut M, M::File) -> Result<T>,
) -> Result<T> {
    let f = m.open(name)?;
    let result = body(m, f).and_then(|v| {
        if m.read(f, &mut [0])? != 0 {
            return Err("A required file extends past its expected length.");
        }
        Ok(v)
    });
    let closed = m.close(f);
    let v = result?;
    closed?;
    Ok(v)
}
fn exact<M: Media>(m: &mut M, f: M::File, mut out: &mut [u8]) -> Result<()> {
    while !out.is_empty() {
        let n = m.read(f, out)?;
        if n == 0 || n > out.len() {
            return Err("A required file could not be read to its expected length.");
        }
        out = &mut out[n..];
    }
    Ok(())
}
pub fn pinned<M: Media>(m: &mut M, name: &[u8], out: &mut [u8], pin: [u8; 32]) -> Result<()> {
    file(m, name, |m, f| exact(m, f, out))?;
    if <[u8; 32]>::from(Sha256::digest(out)) != pin {
        return Err("The file does not match the SHA-256 fixed in this loader.");
    }
    Ok(())
}
fn field(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}
fn shard_len(m: &Manifest, i: u32) -> u64 {
    let width = u64::from(m.shard_bytes());
    width.min(m.model_bytes() - u64::from(i) * width)
}
pub fn inventory(m: &Manifest, mut next: impl FnMut(&mut [u8]) -> Result<usize>) -> Result<()> {
    let mut seen = [0u8; 512];
    let mut entry = [0u8; 1024];
    let mut count = 0;
    loop {
        let n = next(&mut entry)?;
        if n == 0 {
            break;
        }
        let bad = "The boot volume returned an invalid directory entry.";
        if !(82..=entry.len()).contains(&n) {
            return Err(bad);
        }
        let size = usize::try_from(field(&entry, 0)).map_err(|_| bad)?;
        if !(82..=n).contains(&size) || size % 2 != 0 {
            return Err(bad);
        }
        let mut name = [0u8; 15];
        let mut len = 0;
        let mut ended = false;
        for c in entry[80..size].chunks_exact(2) {
            let c = u16::from_le_bytes([c[0], c[1]]);
            if c == 0 {
                ended = true;
                break;
            }
            if let Some(v) = name.get_mut(len) {
                *v = u8::try_from(c).unwrap_or(255).to_ascii_lowercase()
            }
            len += 1;
        }
        if !ended {
            return Err(bad);
        }
        if !name.starts_with(b"shard-") {
            continue;
        }
        if len != 15 || &name[11..] != b".bin" || !name[6..11].iter().all(u8::is_ascii_digit) {
            return Err("A shard in PCPOS has an unexpected name.");
        }
        let i = name[6..11]
            .iter()
            .fold(0u32, |v, &c| v * 10 + u32::from(c - b'0'));
        if i >= m.shard_count() {
            return Err("The volume has an extra or duplicate model shard.");
        }
        let bit = 1 << (i % 8);
        let byte = &mut seen[i as usize / 8];
        if *byte & bit != 0 {
            return Err("The volume has an extra or duplicate model shard.");
        }
        if field(&entry, 72) & 16 != 0 || field(&entry, 8) != shard_len(m, i) {
            return Err("A model shard has the wrong length or is not a file.");
        }
        *byte |= bit;
        count += 1;
    }
    if count != m.shard_count() {
        return Err("The volume is missing a model shard.");
    }
    Ok(())
}
pub fn load_model<M: Media>(m: &mut M, manifest: &Manifest, dest: &mut [&mut [u8]]) -> Result<()> {
    let total = manifest.model_bytes();
    if dest.is_empty()
        || dest.iter().any(|p| p.is_empty() || p.len() % 4096 != 0)
        || dest
            .iter()
            .try_fold(0u64, |a, p| a.checked_add(p.len() as u64))
            != Some(total.next_multiple_of(4096))
    {
        return Err("The model does not fit the reserved memory.");
    }
    let mut whole = Sha256::new();
    let (mut piece, mut offset, mut done) = (0, 0, 0);
    for i in 0..manifest.shard_count() {
        let mut name = *b"shard-00000.bin";
        let mut num = i;
        for digit in name[6..11].iter_mut().rev() {
            *digit = b'0' + (num % 10) as u8;
            num /= 10;
        }
        let hash = file(m, &name, |m, f| {
            let mut shard = Sha256::new();
            let mut left = shard_len(manifest, i) as usize;
            while left > 0 {
                let p = dest
                    .get_mut(piece)
                    .ok_or("The model does not fit the reserved memory.")?;
                let take = left.min(1 << 20).min(p.len() - offset);
                let buf = &mut p[offset..offset + take];
                exact(m, f, buf)?;
                shard.update(&*buf);
                whole.update(&*buf);
                left -= take;
                done += take as u64;
                offset += take;
                if offset == p.len() {
                    piece += 1;
                    offset = 0
                }
            }
            Ok(<[u8; 32]>::from(shard.finalize()))
        })?;
        if manifest.shard_digest(i) != Some(hash) {
            return Err("A model shard does not match its ordered manifest digest.");
        }
        m.progress(done, total);
    }
    if done != total || <[u8; 32]>::from(whole.finalize()) != manifest.model_digest() {
        return Err("The complete model does not match the pinned manifest.");
    }
    if offset != 0 {
        dest[piece][offset..].fill(0)
    }
    Ok(())
}
