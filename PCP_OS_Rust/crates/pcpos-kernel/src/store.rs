//! A bounded durable snapshot of the existing writable namespace. Sector zero
//! is an immutable format marker. Two 129-sector slots each hold a checksummed
//! commit sector followed by up to 64 KiB of serialized file names and bytes.
use crate::{dev::store_disk as disk, regular::Files};
use alloc::{string::String, vec::Vec};
use pcpos_core::sha256;

const MARKER: &[u8] = b"PCPOS USER STORE 2\n";
const COMMIT: &[u8; 8] = b"PCPSLOT1";
const LIMIT: usize = 128 * 512;

pub struct Volume {
    slot: u16,
    generation: u64,
}

pub enum Failure {
    Full,
    Io,
}

fn header_sector(slot: u16) -> u16 {
    1 + slot * 129
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with("dev/")
        && !name.contains('\0')
        && name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn decode(mut bytes: &[u8]) -> Option<Files> {
    let mut files = Files::default();
    while !bytes.is_empty() {
        let name_len = u16::from_le_bytes(bytes.get(..2)?.try_into().ok()?) as usize;
        let data_len = u32::from_le_bytes(bytes.get(2..6)?.try_into().ok()?) as usize;
        let name = core::str::from_utf8(bytes.get(6..6 + name_len)?).ok()?;
        if files.len() == 256 || !valid_name(name) || files.named(name).is_some() {
            return None;
        }
        let end = 6 + name_len + data_len;
        files.insert(String::from(name), bytes.get(6 + name_len..end)?.to_vec());
        bytes = &bytes[end..];
    }
    Some(files)
}

fn read_slot(slot: u16) -> Result<Option<(Volume, Files)>, ()> {
    let base = header_sector(slot);
    let h = disk::read(base)?;
    if &h[..8] != COMMIT || sha256(&h[..52]) != h[52..84] {
        return Ok(None);
    }
    let generation = u64::from_le_bytes(h[8..16].try_into().unwrap());
    let len = u32::from_le_bytes(h[16..20].try_into().unwrap()) as usize;
    if generation == 0 || len > LIMIT {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(len);
    for i in 0..len.div_ceil(512) {
        let sector = disk::read(base + 1 + i as u16)?;
        bytes.extend_from_slice(&sector[..(len - bytes.len()).min(512)]);
    }
    if sha256(&bytes) != h[20..52] {
        return Ok(None);
    }
    Ok(decode(&bytes).map(|files| (Volume { slot, generation }, files)))
}

/// The host presents one dedicated VM data file. A role serial and exact geometry
/// identify the device, and the expected volume ID binds the file contents.
pub fn from_command_line(line: Option<&str>) -> Result<Option<(Volume, Files)>, ()> {
    let line = line.ok_or(())?;
    let mut selectors = line
        .split_ascii_whitespace()
        .filter_map(|arg| arg.strip_prefix("store="));
    let mut ids = line
        .split_ascii_whitespace()
        .filter_map(|arg| arg.strip_prefix("store_id="));
    let (selector, id) = (selectors.next(), ids.next());
    if selector.is_none() && id.is_none() {
        return Ok(None);
    }
    if selector != Some("ide0") || selectors.next().is_some() || ids.next().is_some() {
        return Err(());
    }
    let id = id.ok_or(())?;
    if id.len() != 32
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(());
    }
    let mut marker = [0; 512];
    marker[..MARKER.len()].copy_from_slice(MARKER);
    for i in 0..16 {
        marker[32 + i] = u8::from_str_radix(&id[i * 2..i * 2 + 2], 16).map_err(|_| ())?;
    }
    if marker[32..48] == [0; 16] {
        return Err(());
    }
    disk::identify()?;
    if disk::read(0)? != marker {
        return Err(());
    }
    select_generation().map(Some)
}

fn select_generation() -> Result<(Volume, Files), ()> {
    match (read_slot(0)?, read_slot(1)?) {
        (Some(a), Some(b)) if a.0.generation != b.0.generation => {
            Ok(if a.0.generation > b.0.generation {
                a
            } else {
                b
            })
        }
        (Some(a), None) | (None, Some(a)) => Ok(a),
        _ => Err(()),
    }
}

impl Volume {
    pub fn commit(&mut self, files: &Files) -> Result<(), Failure> {
        if files.len() > 256 {
            return Err(Failure::Full);
        }
        let mut bytes = Vec::new();
        for (name, file) in files.iter() {
            let data = file.bytes();
            if !valid_name(name) || 6 + name.len() + data.len() > LIMIT - bytes.len() {
                return Err(Failure::Full);
            }
            bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&data);
        }
        let next = self.generation.checked_add(1).ok_or(Failure::Full)?;
        let slot = 1 - self.slot;
        let base = header_sector(slot);
        let mut header = [0; 512];
        header[..8].copy_from_slice(COMMIT);
        header[8..16].copy_from_slice(&next.to_le_bytes());
        header[16..20].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        header[20..52].copy_from_slice(&sha256(&bytes));
        let digest = sha256(&header[..52]);
        header[52..84].copy_from_slice(&digest);
        for (i, chunk) in bytes.chunks(512).enumerate() {
            let mut sector = [0; 512];
            sector[..chunk.len()].copy_from_slice(chunk);
            disk::write(base + 1 + i as u16, &sector).map_err(|_| Failure::Io)?;
        }
        disk::flush().map_err(|_| Failure::Io)?;
        disk::write(base, &header).map_err(|_| Failure::Io)?;
        disk::flush().map_err(|_| Failure::Io)?;
        self.slot = slot;
        self.generation = next;
        Ok(())
    }
}
