//! Which processors the machine has, read from the firmware's ACPI tables. No I/O here: the caller
//! hands over a way to read physical memory, and gets back the processors' identities or a refusal.
//!
//! The path is the one the ACPI specification gives: the root pointer (RSDP) names the root table
//! (XSDT, or RSDT on older firmware), which lists the other tables; the one signed `APIC` (the
//! MADT) has one entry for each processor's local interrupt controller. Every table carries its own
//! length and a checksum, and both are checked before a byte of it is believed. Anything that does
//! not add up is a refusal, and the kernel then runs on the one processor it started on: a wrong
//! table must never start a processor that is not there.
extern crate alloc;
use alloc::vec::Vec;

/// The most processors this kernel will start. More than this in the table is not an error: the
/// rest are counted and left alone.
pub const MOST: usize = 64;
/// No table is believed to be longer than this.
const LONGEST_TABLE: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Processor {
    /// The identity of its local interrupt controller: what a start-up message is addressed to.
    pub apic: u32,
    /// Whether an identity above 254 was given, which needs the extended (x2APIC) way of addressing.
    pub extended: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// Where the local interrupt controller's registers are, in physical memory.
    pub controller: u64,
    /// The processors firmware says are there and may be used, in the table's order, at most `MOST`.
    pub processors: Vec<Processor>,
    /// How many more usable ones the table listed beyond `MOST`.
    pub beyond: usize,
    /// How many it listed as present but not to be used.
    pub disabled: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The memory could not be read.
    Unreadable,
    /// No root pointer where one was looked for, or its signature or checksum is wrong.
    NoRoot,
    /// A table's signature, length or checksum is wrong.
    BadTable,
    /// The root table lists no processor table.
    NoProcessorTable,
    /// The processor table has an entry that runs past its end or has no length.
    BadEntry,
    /// It names the same processor twice, or none at all.
    BadProcessors,
}

fn sums_to_zero(bytes: &[u8]) -> bool {
    bytes.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) == 0
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// One whole table at `at`, whose signature must be `signature`: its header says how long it is,
/// and the whole of it must sum to zero.
fn table(read: &mut dyn FnMut(u64, usize) -> Option<Vec<u8>>, at: u64, signature: &[u8; 4]) -> Result<Vec<u8>, Refused> {
    let header = read(at, 36).ok_or(Refused::Unreadable)?;
    if header.len() != 36 || &header[..4] != signature {
        return Err(Refused::BadTable);
    }
    let length = u32_at(&header, 4).ok_or(Refused::BadTable)? as usize;
    if !(36..=LONGEST_TABLE).contains(&length) {
        return Err(Refused::BadTable);
    }
    let whole = read(at, length).ok_or(Refused::Unreadable)?;
    if whole.len() != length || !sums_to_zero(&whole) {
        return Err(Refused::BadTable);
    }
    Ok(whole)
}

/// Whether the twenty (or, from revision 2, thirty-six) bytes at the start of `bytes` are a root
/// pointer: its signature and its checksum, and for revision 2 the second checksum over all of it.
pub fn root_holds(bytes: &[u8]) -> bool {
    if bytes.len() < 20 || &bytes[..8] != b"RSD PTR " || !sums_to_zero(&bytes[..20]) {
        return false;
    }
    bytes[15] < 2 || (bytes.len() >= 36 && u32_at(bytes, 20) == Some(36) && sums_to_zero(&bytes[..36]))
}

/// The processors, starting from the root pointer at `root`.
pub fn find(root: u64, read: &mut dyn FnMut(u64, usize) -> Option<Vec<u8>>) -> Result<Found, Refused> {
    let pointer = read(root, 36).ok_or(Refused::Unreadable)?;
    if !root_holds(&pointer) {
        return Err(Refused::NoRoot);
    }
    // Revision 2 and later name the 64-bit root table; it is used when it is there.
    let wide = pointer[15] >= 2 && u64_at(&pointer, 24).is_some_and(|at| at != 0);
    let (listing, width) = if wide {
        (table(read, u64_at(&pointer, 24).ok_or(Refused::NoRoot)?, b"XSDT")?, 8)
    } else {
        (table(read, u32_at(&pointer, 16).ok_or(Refused::NoRoot)? as u64, b"RSDT")?, 4)
    };
    if (listing.len() - 36) % width != 0 {
        return Err(Refused::BadTable);
    }
    let mut processors_table = None;
    for entry in listing[36..].chunks_exact(width) {
        let at = if width == 8 { u64_at(entry, 0) } else { u32_at(entry, 0).map(u64::from) }.ok_or(Refused::BadTable)?;
        // Only the signature is read here; a table that is not the one wanted is not judged.
        let signature = read(at, 4).ok_or(Refused::Unreadable)?;
        if signature.as_slice() == b"APIC" {
            processors_table = Some(table(read, at, b"APIC")?);
            break;
        }
    }
    let madt = processors_table.ok_or(Refused::NoProcessorTable)?;
    if madt.len() < 44 {
        return Err(Refused::BadTable);
    }
    let mut found = Found { controller: u32_at(&madt, 36).ok_or(Refused::BadTable)? as u64, processors: Vec::new(), beyond: 0, disabled: 0 };
    let mut at = 44;
    while at < madt.len() {
        let (kind, length) = match (madt.get(at), madt.get(at + 1)) {
            (Some(kind), Some(length)) => (*kind, *length as usize),
            _ => return Err(Refused::BadEntry),
        };
        if length < 2 || at + length > madt.len() {
            return Err(Refused::BadEntry);
        }
        let entry = &madt[at..at + length];
        // (identity, flags) of a processor entry of either form; other entries are passed over.
        let processor = match (kind, length) {
            (0, 8) => Some((entry[3] as u32, u32_at(entry, 4).ok_or(Refused::BadEntry)?, false)),
            (9, 16) => Some((u32_at(entry, 4).ok_or(Refused::BadEntry)?, u32_at(entry, 8).ok_or(Refused::BadEntry)?, true)),
            (0, _) | (9, _) => return Err(Refused::BadEntry),
            // Where the controller's registers really are, when that does not fit in 32 bits.
            (5, 12) => {
                found.controller = u64_at(entry, 4).ok_or(Refused::BadEntry)?;
                None
            }
            (5, _) => return Err(Refused::BadEntry),
            _ => None,
        };
        if let Some((apic, flags, extended)) = processor {
            // Bit 0: enabled. (Bit 1, "may be brought online later", is not used: a processor that
            // is not enabled now is not started.) The identity 255, in the short form, means no one.
            if flags & 1 == 0 || (!extended && apic == 0xFF) {
                found.disabled += 1;
            } else if found.processors.iter().any(|p| p.apic == apic) {
                return Err(Refused::BadProcessors);
            } else if found.processors.len() == MOST {
                found.beyond += 1;
            } else {
                found.processors.push(Processor { apic, extended: extended && apic > 254 });
            }
        }
        at += length;
    }
    if found.processors.is_empty() || found.controller == 0 || found.controller % 4096 != 0 {
        return Err(Refused::BadProcessors);
    }
    Ok(found)
}

/// What a loader that started from UEFI says about the root pointer on its line: the one word
/// `acpi_rsdp=0x` and exactly sixteen lowercase hexadecimal digits, the root pointer's own physical
/// address, never nothing. `Ok(None)` is no such word. Two such words, or one in any other form, is
/// an error, and the kernel then looks nowhere else: a loader that tried to say and said it wrongly
/// is not second-guessed.
pub fn named_root(line: &str) -> Result<Option<u64>, ()> {
    let mut words = line.split_ascii_whitespace().filter_map(|w| w.strip_prefix("acpi_rsdp="));
    let Some(word) = words.next() else { return Ok(None) };
    if words.next().is_some() {
        return Err(());
    }
    let digits = word.strip_prefix("0x").ok_or(())?;
    if digits.len() != 16 || !digits.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(());
    }
    match u64::from_str_radix(digits, 16) {
        Ok(0) | Err(_) => Err(()),
        Ok(at) => Ok(Some(at)),
    }
}

/// Whether `length` bytes at `at` lie wholly outside every range the loader's memory map (in the
/// Multiboot form: a size, then base, length and kind) gives as free for use. Firmware's tables are
/// never in memory it hands out as free; an address that is must not be believed, and is not read.
/// A map that cannot be followed to its end answers no.
pub fn outside_free_memory(map: &[u8], at: u64, length: usize) -> bool {
    let Some(end) = at.checked_add(length as u64) else { return false };
    let mut entry = 0usize;
    let mut seen = false;
    while entry < map.len() {
        let (Some(size), Some(base), Some(extent), Some(kind)) = (u32_at(map, entry), u64_at(map, entry + 4), u64_at(map, entry + 12), u32_at(map, entry + 20)) else {
            return false;
        };
        if size < 20 {
            return false;
        }
        let Some(top) = base.checked_add(extent) else { return false };
        if kind == 1 && at < top && base < end {
            return false;
        }
        seen = true;
        entry += size as usize + 4;
    }
    seen && entry == map.len()
}

/// Whether `length` bytes at `at` lie wholly inside what the loader's memory map gives as the
/// firmware's own tables: ACPI memory that may be reclaimed (kind 3) or must be kept (kind 4). The
/// bytes may run across ranges of those kinds that touch. This is the rule for a root pointer the
/// loader named, which a UEFI machine's firmware keeps in such memory; a map that cannot be followed
/// to its end answers no.
pub fn inside_table_memory(map: &[u8], at: u64, length: usize) -> bool {
    let Some(end) = at.checked_add(length as u64) else { return false };
    if length == 0 {
        return false;
    }
    let mut reached = at;
    // Each pass must carry `reached` forward through one more range, so there are at most as many passes as ranges.
    for _ in 0..=map.len() / 24 {
        let mut entry = 0usize;
        let mut next = reached;
        while entry < map.len() {
            let (Some(size), Some(base), Some(extent), Some(kind)) = (u32_at(map, entry), u64_at(map, entry + 4), u64_at(map, entry + 12), u32_at(map, entry + 20)) else {
                return false;
            };
            let Some(top) = base.checked_add(extent) else { return false };
            if size < 20 {
                return false;
            }
            if (kind == 3 || kind == 4) && base <= reached && reached < top {
                next = next.max(top);
            }
            entry += size as usize + 4;
        }
        if entry != map.len() || next == reached {
            return false;
        }
        reached = next;
        if reached >= end {
            return true;
        }
    }
    false
}

/// A page below one mebibyte that the loader's memory map (in the Multiboot form) gives as free, for
/// another processor to be started from: such a processor begins in a mode that reaches no higher.
/// Pages from 0x8000 up are looked at first, the usual place; the first page of memory and everything
/// from 0xA0000, where firmware and devices live, are never taken.
pub fn low_free_page(map: &[u8]) -> Option<u64> {
    let free = |page: u64| {
        let mut entry = 0usize;
        let mut inside = false;
        while entry < map.len() {
            let (Some(size), Some(base), Some(extent), Some(kind)) = (u32_at(map, entry), u64_at(map, entry + 4), u64_at(map, entry + 12), u32_at(map, entry + 20)) else {
                return false;
            };
            let Some(top) = base.checked_add(extent) else { return false };
            if size < 20 {
                return false;
            }
            if page < top && base < page + 4096 {
                // Touched by this range: it must be free memory and hold the whole page, and no other range may touch it.
                if kind != 1 || base > page || top < page + 4096 || inside {
                    return false;
                }
                inside = true;
            }
            entry += size as usize + 4;
        }
        inside && entry == map.len()
    };
    (8..0xA0).chain(1..8).map(|n| n * 4096).find(|page| free(*page))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// A made-up machine's memory: tables at chosen addresses.
    struct Memory(Vec<(u64, Vec<u8>)>);

    impl Memory {
        fn read(&self) -> impl FnMut(u64, usize) -> Option<Vec<u8>> + '_ {
            move |at, length| {
                self.0.iter().find_map(|(base, bytes)| {
                    let from = at.checked_sub(*base)? as usize;
                    bytes.get(from..from.checked_add(length)?).map(|b| b.to_vec())
                })
            }
        }
    }

    fn sealed(mut bytes: Vec<u8>, checksum_at: usize, over: usize) -> Vec<u8> {
        bytes[checksum_at] = 0;
        let sum = bytes[..over].iter().fold(0u8, |s, b| s.wrapping_add(*b));
        bytes[checksum_at] = 0u8.wrapping_sub(sum);
        bytes
    }

    fn sdt(signature: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut t = vec![0u8; 36];
        t[..4].copy_from_slice(signature);
        t[4..8].copy_from_slice(&((36 + body.len()) as u32).to_le_bytes());
        t.extend_from_slice(body);
        let length = t.len();
        sealed(t, 9, length)
    }

    fn local(apic: u8, flags: u32) -> Vec<u8> {
        let mut e = vec![0, 8, apic, apic];
        e.extend_from_slice(&flags.to_le_bytes());
        e
    }

    fn wide(apic: u32, flags: u32) -> Vec<u8> {
        let mut e = vec![9, 16, 0, 0];
        e.extend_from_slice(&apic.to_le_bytes());
        e.extend_from_slice(&flags.to_le_bytes());
        e.extend_from_slice(&[0; 4]);
        e
    }

    fn madt(entries: &[Vec<u8>]) -> Vec<u8> {
        let mut body = 0xFEE0_0000u32.to_le_bytes().to_vec();
        body.extend_from_slice(&1u32.to_le_bytes());
        for e in entries {
            body.extend_from_slice(e);
        }
        sdt(b"APIC", &body)
    }

    const ROOT: u64 = 0xE_0000;
    const LIST: u64 = 0x7FE_0000;
    const OTHER: u64 = 0x7FE_1000;
    const MADT: u64 = 0x7FE_2000;

    fn machine(revision: u8, madt_bytes: Vec<u8>) -> Memory {
        let mut pointer = vec![0u8; 36];
        pointer[..8].copy_from_slice(b"RSD PTR ");
        pointer[15] = revision;
        let other = sdt(b"FACP", &[7; 40]);
        let listing = if revision >= 2 {
            pointer[20..24].copy_from_slice(&36u32.to_le_bytes());
            pointer[24..32].copy_from_slice(&LIST.to_le_bytes());
            sdt(b"XSDT", &[OTHER.to_le_bytes(), MADT.to_le_bytes()].concat())
        } else {
            pointer[16..20].copy_from_slice(&(LIST as u32).to_le_bytes());
            sdt(b"RSDT", &[(OTHER as u32).to_le_bytes(), (MADT as u32).to_le_bytes()].concat())
        };
        let mut pointer = sealed(pointer, 8, 20);
        if revision >= 2 {
            pointer = sealed(pointer, 32, 36);
        }
        Memory(vec![(ROOT, pointer), (LIST, listing), (OTHER, other), (MADT, madt_bytes)])
    }

    fn ids(found: &Found) -> Vec<u32> {
        found.processors.iter().map(|p| p.apic).collect()
    }

    #[test]
    fn the_processors_are_found_through_either_root_table() {
        for revision in [0u8, 2] {
            for count in [1u8, 2, 4, 8] {
                let m = machine(revision, madt(&(0..count).map(|n| local(n, 1)).collect::<Vec<_>>()));
                let found = find(ROOT, &mut m.read()).unwrap();
                assert_eq!(ids(&found), (0..count as u32).collect::<Vec<_>>(), "revision {revision}");
                assert_eq!((found.controller, found.beyond, found.disabled), (0xFEE0_0000, 0, 0));
            }
        }
    }

    #[test]
    fn a_processor_that_is_not_enabled_is_counted_and_not_listed() {
        // Enabled, not enabled, "may come online later" only, the identity that means no one, enabled.
        let m = machine(2, madt(&[local(0, 1), local(1, 0), local(2, 2), local(0xFF, 1), local(3, 1)]));
        let found = find(ROOT, &mut m.read()).unwrap();
        assert_eq!((ids(&found), found.disabled), (vec![0, 3], 3));
    }

    #[test]
    fn other_entries_are_passed_over_and_the_wide_forms_are_read() {
        let mut moved = vec![5, 12, 0, 0];
        moved.extend_from_slice(&0x1_FEE0_0000u64.to_le_bytes());
        // An interrupt-controller entry of another kind (type 1, 12 bytes) lies between processors.
        let m = machine(2, madt(&[local(0, 1), vec![1, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], wide(300, 1), wide(4, 1), moved, wide(9, 0)]));
        let found = find(ROOT, &mut m.read()).unwrap();
        assert_eq!(found.processors, vec![Processor { apic: 0, extended: false }, Processor { apic: 300, extended: true }, Processor { apic: 4, extended: false }]);
        assert_eq!((found.controller, found.disabled), (0x1_FEE0_0000, 1));
    }

    #[test]
    fn more_processors_than_this_kernel_starts_are_counted_and_left() {
        let m = machine(2, madt(&(0..70u8).map(|n| local(n, 1)).collect::<Vec<_>>()));
        let found = find(ROOT, &mut m.read()).unwrap();
        assert_eq!((found.processors.len(), found.beyond), (MOST, 6));
    }

    #[test]
    fn what_does_not_add_up_is_refused() {
        let good = madt(&[local(0, 1), local(1, 1)]);
        assert!(find(ROOT, &mut machine(2, good.clone()).read()).is_ok());
        // One byte of the processor table changed: its checksum no longer holds.
        let mut changed = good.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert_eq!(find(ROOT, &mut machine(2, changed).read()), Err(Refused::BadTable));
        // An entry that says it is longer than the table, one with no length, a processor entry of the wrong size.
        for entry in [vec![0u8, 60, 0, 0, 1, 0, 0, 0], vec![3, 0], vec![0, 6, 0, 0, 1, 0]] {
            let m = machine(2, madt(&[local(0, 1), entry]));
            assert_eq!(find(ROOT, &mut m.read()), Err(Refused::BadEntry));
        }
        // The same processor twice; none enabled; no processors at all.
        assert_eq!(find(ROOT, &mut machine(2, madt(&[local(0, 1), local(0, 1)])).read()), Err(Refused::BadProcessors));
        assert_eq!(find(ROOT, &mut machine(2, madt(&[local(0, 0)])).read()), Err(Refused::BadProcessors));
        assert_eq!(find(ROOT, &mut machine(2, madt(&[])).read()), Err(Refused::BadProcessors));
        // A table shorter than its own fixed part.
        assert_eq!(find(ROOT, &mut machine(2, sdt(b"APIC", &[0; 4])).read()), Err(Refused::BadTable));
        // The root table lists no processor table.
        let mut none = machine(2, good.clone());
        none.0[3].1 = sdt(b"HPET", &[0; 20]);
        assert_eq!(find(ROOT, &mut none.read()), Err(Refused::NoProcessorTable));
        // The root pointer: a changed signature, a changed byte under each checksum, nothing there.
        for (at, what) in [(0usize, "signature"), (10, "first checksum"), (30, "second checksum")] {
            let mut m = machine(2, good.clone());
            m.0[0].1[at] ^= 1;
            assert_eq!(find(ROOT, &mut m.read()), Err(Refused::NoRoot), "{what}");
        }
        assert_eq!(find(0x1234, &mut machine(2, good.clone()).read()), Err(Refused::Unreadable));
        // The root table: a changed byte, and a length that is not a whole number of entries.
        let mut m = machine(2, good.clone());
        m.0[1].1[40] ^= 1;
        assert_eq!(find(ROOT, &mut m.read()), Err(Refused::BadTable));
        let mut m = machine(2, good.clone());
        m.0[1].1 = sdt(b"XSDT", &[OTHER.to_le_bytes().as_slice(), &[1, 2, 3]].concat());
        assert_eq!(find(ROOT, &mut m.read()), Err(Refused::BadTable));
        // A table that says it is longer than any table is.
        let mut m = machine(2, good.clone());
        m.0[3].1[4..8].copy_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
        assert_eq!(find(ROOT, &mut m.read()), Err(Refused::BadTable));
        // A table that cannot be read to its stated length.
        let mut m = machine(2, good);
        m.0[3].1.truncate(40);
        assert_eq!(find(ROOT, &mut m.read()), Err(Refused::Unreadable));
    }

    #[test]
    fn a_controller_address_that_is_nothing_or_not_on_a_page_is_refused() {
        for address in [0u32, 0xFEE0_0010] {
            let mut body = address.to_le_bytes().to_vec();
            body.extend_from_slice(&1u32.to_le_bytes());
            body.extend_from_slice(&local(0, 1));
            assert_eq!(find(ROOT, &mut machine(2, sdt(b"APIC", &body)).read()), Err(Refused::BadProcessors));
        }
    }

    #[test]
    fn the_loaders_word_is_taken_in_one_form_only() {
        assert_eq!(named_root("then=off quiet"), Ok(None));
        assert_eq!(named_root("then=off acpi_rsdp=0x000000007ffe0014"), Ok(Some(0x7ffe_0014)));
        assert_eq!(named_root("acpi_rsdp=0x00000001fffffff0"), Ok(Some(0x1_ffff_fff0)));
        for wrong in ["acpi_rsdp=0x7ffe0014", "acpi_rsdp=0x000000007FFE0014", "acpi_rsdp=000000007ffe0014", "acpi_rsdp=0x0000000000000000", "acpi_rsdp=0x000000007ffe00140",
                      "acpi_rsdp=0x000000007ffe001g", "acpi_rsdp=", "acpi_rsdp=0x000000007ffe0014 acpi_rsdp=0x000000007ffe0014", "acpi_rsdp=0x+00000007ffe0014"] {
            assert_eq!(named_root(wrong), Err(()), "{wrong}");
        }
    }

    #[test]
    fn nothing_is_read_from_memory_the_map_gives_as_free() {
        let entry = |base: u64, extent: u64, kind: u32| {
            let mut e = 20u32.to_le_bytes().to_vec();
            e.extend_from_slice(&base.to_le_bytes());
            e.extend_from_slice(&extent.to_le_bytes());
            e.extend_from_slice(&kind.to_le_bytes());
            e
        };
        let map = [entry(0, 0x9fc00, 1), entry(0xf0000, 0x10000, 2), entry(0x10_0000, 0x7ef_0000, 1), entry(0x7ff_0000, 0x1_0000, 3)].concat();
        assert!(outside_free_memory(&map, 0xf5000, 36));
        assert!(outside_free_memory(&map, 0x7ff_0000, 0x1_0000));
        assert!(outside_free_memory(&map, 0xa0000, 16)); // a hole the map does not mention
        assert!(!outside_free_memory(&map, 0x10_0000, 36));
        assert!(!outside_free_memory(&map, 0x7fe_fff0, 36)); // begins in free memory
        assert!(!outside_free_memory(&map, 0x9fbf0, 36)); // ends outside it, begins inside
        assert!(!outside_free_memory(&map, u64::MAX - 4, 36));
        assert!(!outside_free_memory(&[], 0xf5000, 36));
        assert!(!outside_free_memory(&map[..map.len() - 3], 0xf5000, 36));
        let mut short = map.clone();
        short[0] = 8;
        assert!(!outside_free_memory(&short, 0xf5000, 36));
    }

    #[test]
    fn a_named_root_is_believed_only_in_the_firmwares_own_table_memory() {
        let entry = |base: u64, extent: u64, kind: u32| {
            let mut e = 20u32.to_le_bytes().to_vec();
            e.extend_from_slice(&base.to_le_bytes());
            e.extend_from_slice(&extent.to_le_bytes());
            e.extend_from_slice(&kind.to_le_bytes());
            e
        };
        // Listed out of order on purpose: reclaimable tables, kept tables touching them, reserved, free.
        let map = [entry(0x7ff0_0000, 0x1000, 4), entry(0x10_0000, 0x7fd0_0000, 1), entry(0x7fe0_0000, 0x10_0000, 3), entry(0x7ff0_1000, 0x1000, 2), entry(0xf0000, 0x10000, 2)].concat();
        assert!(inside_table_memory(&map, 0x7fe0_0000, 36));
        assert!(inside_table_memory(&map, 0x7fef_fff0, 36)); // runs from the one kind into the other
        assert!(inside_table_memory(&map, 0x7fe0_0000, 0x10_1000)); // all of both
        assert!(!inside_table_memory(&map, 0x7ff0_0ff0, 36)); // runs on into reserved memory
        assert!(!inside_table_memory(&map, 0x7fdf_fff0, 36)); // begins in free memory
        assert!(!inside_table_memory(&map, 0xf5000, 36)); // reserved: where a BIOS keeps them, not a UEFI machine
        assert!(!inside_table_memory(&map, 0xa0000, 16)); // a hole the map does not mention
        assert!(!inside_table_memory(&map, 0x7fe0_0000, 0));
        assert!(!inside_table_memory(&map, u64::MAX - 4, 36));
        assert!(!inside_table_memory(&[], 0x7fe0_0000, 36));
        assert!(!inside_table_memory(&map[..map.len() - 3], 0x7fe0_0000, 36));
    }

    #[test]
    fn a_page_to_start_from_is_free_memory_below_one_mebibyte_or_none() {
        let entry = |base: u64, extent: u64, kind: u32| {
            let mut e = 20u32.to_le_bytes().to_vec();
            e.extend_from_slice(&base.to_le_bytes());
            e.extend_from_slice(&extent.to_le_bytes());
            e.extend_from_slice(&kind.to_le_bytes());
            e
        };
        assert_eq!(low_free_page(&[entry(0, 0x9fc00, 1), entry(0x9fc00, 0x400, 2), entry(0x10_0000, 0x700_0000, 1)].concat()), Some(0x8000));
        // The usual place is taken: the next whole free page is used.
        assert_eq!(low_free_page(&[entry(0, 0x8800, 1), entry(0x8800, 0x1800, 2), entry(0xa000, 0x9_0000, 1)].concat()), Some(0xa000));
        // Only below 0x8000 is anything free; the first page is never taken.
        assert_eq!(low_free_page(&[entry(0, 0x3000, 1), entry(0x3000, 0x9d000, 2)].concat()), Some(0x1000));
        assert_eq!(low_free_page(&[entry(0, 0x1000, 1), entry(0x1000, 0x9f000, 2)].concat()), None);
        // Free memory only above one mebibyte, a page two ranges both claim, and a map cut short.
        assert_eq!(low_free_page(&entry(0x10_0000, 0x700_0000, 1)), None);
        assert_eq!(low_free_page(&[entry(0x8000, 0x1000, 1), entry(0x8000, 0x1000, 2)].concat()), None);
        let whole = [entry(0, 0x9fc00, 1), entry(0x10_0000, 0x700_0000, 1)].concat();
        assert_eq!(low_free_page(&whole[..whole.len() - 2]), None);
        assert_eq!(low_free_page(&[]), None);
    }
}
