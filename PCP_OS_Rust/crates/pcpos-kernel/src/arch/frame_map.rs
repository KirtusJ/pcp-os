//! Select the largest contiguous, 2 MiB-aligned frame pool below the bootstrap mapping limit.
//! Overlapping reserved descriptors take precedence, regardless of the loader's record order.
extern crate alloc;
use alloc::vec::Vec;

pub(super) const MAX_MAP_BYTES: usize = 64 * 1024;
const LIMIT: usize = 1 << 30;
const PAGE: usize = 2 << 20;

fn consider(start: usize, end: usize, best: &mut (usize, usize)) {
    let start = start.div_ceil(PAGE) * PAGE;
    let end = end / PAGE * PAGE;
    if end.saturating_sub(start) > best.1 - best.0 {
        *best = (start, end);
    }
}

pub(super) fn pool(bytes: &[u8], minimum: usize) -> (usize, usize) {
    if bytes.len() > MAX_MAP_BYTES || minimum >= LIMIT {
        return (0, 0);
    }
    // Each boundary changes the number of available or reserved descriptors covering an address.
    // Clipping before the sweep also excludes every byte below the end of the kernel and modules.
    let mut boundaries = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        if rest.len() < 24 {
            return (0, 0);
        }
        let size = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
        if size < 20 || size > rest.len() - 4 {
            return (0, 0);
        }
        let base = u64::from_le_bytes(rest[4..12].try_into().unwrap());
        let length = u64::from_le_bytes(rest[12..20].try_into().unwrap());
        let kind = u32::from_le_bytes(rest[20..24].try_into().unwrap());
        let Some(end) = base.checked_add(length) else { return (0, 0) };
        let first = base.max(minimum as u64);
        let last = end.min(LIMIT as u64);
        if first < last {
            let (usable, reserved) = if kind == 1 { (1_i32, 0_i32) } else { (0, 1) };
            boundaries.push((first as usize, usable, reserved));
            boundaries.push((last as usize, -usable, -reserved));
        }
        rest = &rest[4 + size..];
    }
    boundaries.sort_unstable();
    let mut best = (0, 0);
    let mut start = None;
    let (mut usable, mut reserved) = (0, 0);
    let mut iter = boundaries.into_iter().peekable();
    while let Some((at, add_usable, add_reserved)) = iter.next() {
        usable += add_usable;
        reserved += add_reserved;
        // Adjacent usable intervals can form one complete page even if neither is page-aligned.
        while let Some((_, u, r)) = iter.next_if(|b| b.0 == at) {
            usable += u;
            reserved += r;
        }
        if usable > 0 && reserved == 0 {
            start.get_or_insert(at);
        } else if let Some(first) = start.take() {
            consider(first, at, &mut best);
        }
    }
    best
}

/// The most ranges of memory above four gibibytes that are used, and where they may lie.
pub(super) const HIGH_RANGES: usize = 8;
pub(super) const HIGH_START: u64 = 4 << 30;
pub(super) const HIGH_END: u64 = HIGH_START + (64 << 30);

/// The usable memory above four gibibytes, as up to eight ranges of whole 2 MiB pages in ascending
/// order, each free of every reserved descriptor, by the same sweep and the same rules as the pool
/// below one gibibyte. A map that is malformed anywhere yields none. Ranges beyond the eighth, and
/// memory beyond the ceiling, are left unused. `kept_out` is memory the loader filled and named (a
/// model's bytes): it counts as reserved here, so no frame of it is ever handed out or written.
pub(super) fn high(bytes: &[u8], kept_out: &[(u64, u64)]) -> ([(usize, usize); HIGH_RANGES], usize) {
    let mut found = ([(0usize, 0usize); HIGH_RANGES], 0usize);
    if bytes.len() > MAX_MAP_BYTES {
        return found;
    }
    let mut boundaries = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        if rest.len() < 24 {
            return found;
        }
        let size = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
        if size < 20 || size > rest.len() - 4 {
            return found;
        }
        let base = u64::from_le_bytes(rest[4..12].try_into().unwrap());
        let length = u64::from_le_bytes(rest[12..20].try_into().unwrap());
        let kind = u32::from_le_bytes(rest[20..24].try_into().unwrap());
        let Some(end) = base.checked_add(length) else { return found };
        let (first, last) = (base.max(HIGH_START), end.min(HIGH_END));
        if first < last {
            let (usable, reserved) = if kind == 1 { (1_i32, 0_i32) } else { (0, 1) };
            boundaries.push((first as usize, usable, reserved));
            boundaries.push((last as usize, -usable, -reserved));
        }
        rest = &rest[4 + size..];
    }
    for &(base, length) in kept_out {
        let Some(end) = base.checked_add(length) else { return found };
        // Whole 4 KiB pages: the last page of a model may be part filled, and all of it is kept out.
        let (first, last) = (base.max(HIGH_START), (end.div_ceil(4096) * 4096).min(HIGH_END));
        if first < last {
            boundaries.push((first as usize, 0, 1));
            boundaries.push((last as usize, 0, -1));
        }
    }
    boundaries.sort_unstable();
    let mut start = None;
    let (mut usable, mut reserved) = (0, 0);
    let mut iter = boundaries.into_iter().peekable();
    while let Some((at, add_usable, add_reserved)) = iter.next() {
        usable += add_usable;
        reserved += add_reserved;
        while let Some((_, u, r)) = iter.next_if(|b| b.0 == at) {
            usable += u;
            reserved += r;
        }
        if usable > 0 && reserved == 0 {
            start.get_or_insert(at);
        } else if let Some(first) = start.take() {
            let (from, to) = (first.div_ceil(PAGE) * PAGE, at / PAGE * PAGE);
            if from < to && found.1 < HIGH_RANGES {
                found.0[found.1] = (from, to);
                found.1 += 1;
            }
        }
    }
    found
}

/// The most pieces a loader may leave a model in.
pub(super) const MODEL_EXTENTS: usize = 8;

/// Read `address:bytes[,address:bytes...]`, each number in hexadecimal with `0x` before it: where the
/// loader left a model's bytes, piece after piece in the model's own order. None if it is anything else.
pub(super) fn model_extents(word: &str) -> Option<([(u64, u64); MODEL_EXTENTS], usize)> {
    let mut found = ([(0u64, 0u64); MODEL_EXTENTS], 0usize);
    let hex = |text: &str| -> Option<u64> {
        let digits = text.strip_prefix("0x")?;
        if digits.is_empty() || digits.len() > 16 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u64::from_str_radix(digits, 16).ok()
    };
    for piece in word.split(',') {
        let (address, bytes) = piece.split_once(':')?;
        if found.1 == MODEL_EXTENTS {
            return None;
        }
        found.0[found.1] = (hex(address)?, hex(bytes)?);
        found.1 += 1;
    }
    Some(found)
}

/// Whether memory the loader named as holding a model may be kept for one: and if not, why not.
/// Every piece begins on a 4 KiB page; every piece but the last is whole pages (so the model's bytes
/// follow one another when the pieces are mapped page after page); none is empty; all lie between
/// four gibibytes and the ceiling; no two share a page; and every byte of every page of them is
/// memory the loader's map calls usable and nothing in the map calls reserved. A map that is
/// malformed anywhere allows nothing.
pub(super) fn model_allowed(bytes: &[u8], extents: &[(u64, u64)]) -> Result<u64, &'static str> {
    if extents.is_empty() || extents.len() > MODEL_EXTENTS {
        return Err("no memory is named, or more pieces than are taken");
    }
    let mut total = 0u64;
    let mut pages = [(0u64, 0u64); MODEL_EXTENTS];
    for (i, &(base, length)) in extents.iter().enumerate() {
        let end = base.checked_add(length).ok_or("a piece runs past the end of all addresses")?;
        if length == 0 {
            return Err("a piece is empty");
        }
        if base % 4096 != 0 || (i + 1 < extents.len() && length % 4096 != 0) {
            return Err("a piece does not begin on a page, or one before the last is not whole pages");
        }
        let whole = end.div_ceil(4096) * 4096;
        if base < HIGH_START || whole > HIGH_END {
            return Err("a piece lies outside the memory between four gibibytes and the ceiling");
        }
        if pages[..i].iter().any(|&(a, b)| base < b && a < whole) {
            return Err("two pieces share memory");
        }
        pages[i] = (base, whole);
        total = total.checked_add(length).ok_or("the pieces are longer than can be counted")?;
    }
    // The memory the map calls usable and nothing calls reserved, to the byte (no rounding).
    if bytes.len() > MAX_MAP_BYTES {
        return Err("the loader's memory map is too long to read");
    }
    let mut boundaries = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        if rest.len() < 24 {
            return Err("the loader's memory map is malformed");
        }
        let size = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
        if size < 20 || size > rest.len() - 4 {
            return Err("the loader's memory map is malformed");
        }
        let base = u64::from_le_bytes(rest[4..12].try_into().unwrap());
        let length = u64::from_le_bytes(rest[12..20].try_into().unwrap());
        let kind = u32::from_le_bytes(rest[20..24].try_into().unwrap());
        let end = base.checked_add(length).ok_or("the loader's memory map is malformed")?;
        if base < end {
            let (usable, reserved) = if kind == 1 { (1_i32, 0_i32) } else { (0, 1) };
            boundaries.push((base, usable, reserved));
            boundaries.push((end, -usable, -reserved));
        }
        rest = &rest[4 + size..];
    }
    boundaries.sort_unstable();
    let mut good: Vec<(u64, u64)> = Vec::new();
    let mut start = None;
    let (mut usable, mut reserved) = (0, 0);
    let mut iter = boundaries.into_iter().peekable();
    while let Some((at, add_usable, add_reserved)) = iter.next() {
        usable += add_usable;
        reserved += add_reserved;
        while let Some((_, u, r)) = iter.next_if(|b| b.0 == at) {
            usable += u;
            reserved += r;
        }
        if usable > 0 && reserved == 0 {
            start.get_or_insert(at);
        } else if let Some(first) = start.take() {
            // Adjacent usable descriptors join: a piece may lie across the line between two.
            match good.last_mut() {
                Some(last) if last.1 == first => last.1 = at,
                _ => good.push((first, at)),
            }
        }
    }
    for &(base, whole) in &pages[..extents.len()] {
        if !good.iter().any(|&(a, b)| a <= base && whole <= b) {
            return Err("a piece is not wholly memory the loader's map calls usable");
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(base: u64, end: u64, kind: u32) -> Vec<u8> {
        [20u32.to_le_bytes().as_slice(), &base.to_le_bytes(), &(end - base).to_le_bytes(),
         &kind.to_le_bytes()].concat()
    }

    #[test]
    fn fragmented_map_and_reserved_holes_in_any_order() {
        let mb = 1 << 20;
        let low = entry(mb, 8 * mb, 1);
        let high = entry(40 * mb, 90 * mb, 1);
        let hole = entry(63 * mb, 65 * mb, 2);
        let minimum = 41 * mb as usize;
        let want = (66 * mb as usize, 90 * mb as usize);
        assert_eq!(pool(&[low.clone(), high.clone(), hole.clone()].concat(), minimum), want);
        assert_eq!(pool(&[hole, high, low].concat(), minimum), want);
    }

    #[test]
    fn adjacent_short_entries_join_and_unknown_memory_stays_reserved() {
        let p = PAGE as u64;
        let entries = [entry(p, p + 7, 1), entry(p + 7, 3 * p, 1), entry(2 * p, 2 * p + 1, 88)];
        assert_eq!(pool(&entries.concat(), PAGE), (PAGE, 2 * PAGE));
        assert_eq!(pool(&entry(0, 2 * LIMIT as u64, 1), LIMIT - PAGE), (LIMIT - PAGE, LIMIT));
    }

    #[test]
    fn memory_above_four_gibibytes_is_whole_pages_between_reserved_holes() {
        let g = 1u64 << 30;
        let p = PAGE as u64;
        // Below four gibibytes nothing is taken, whatever the map says is usable there.
        assert_eq!(high(&entry(0, 4 * g, 1), &[]).1, 0);
        // One range, cut to whole pages; a reserved hole in it makes two; order of records does not matter.
        let big = entry(4 * g + 5, 6 * g - 5, 1);
        let hole = entry(5 * g, 5 * g + 1, 2);
        let (ranges, count) = high(&[hole.clone(), big.clone()].concat(), &[]);
        assert_eq!((count, ranges[0], ranges[1]), (2, ((4 * g + p) as usize, (5 * g) as usize), ((5 * g + p) as usize, (6 * g - p) as usize)));
        assert_eq!(high(&[big, hole].concat(), &[]), (ranges, count));
        // A usable range that begins below four gibibytes keeps the whole of its part above (the cut
        // to the window comes before the cut to whole pages), and loses all of its part below.
        let (crossing, n) = high(&entry(3 * g + 5, 6 * g, 1), &[]);
        assert_eq!((n, crossing[0]), (1, ((4 * g) as usize, (6 * g) as usize)));
        // A reserved record that only touches a usable one at its end removes nothing; one that
        // overlaps it by a byte removes that page and nothing is invented beyond what is usable.
        assert_eq!(high(&[entry(4 * g, 5 * g, 1), entry(5 * g, 6 * g, 2)].concat(), &[]).0[0], ((4 * g) as usize, (5 * g) as usize));
        assert_eq!(high(&[entry(4 * g, 5 * g, 1), entry(5 * g - 1, 6 * g, 2)].concat(), &[]).0[0], ((4 * g) as usize, (5 * g - p) as usize));
        assert_eq!(high(&entry(4 * g, 4 * g + p - 1, 1), &[]).1, 0);
        // Memory past the ceiling is left; more than eight ranges keep the first eight; a bad map gives none.
        let (far, n) = high(&entry(60 * g, 80 * g, 1), &[]);
        assert_eq!((n, far[0]), (1, ((60 * g) as usize, HIGH_END as usize)));
        let many: Vec<u8> = (0..10u64).flat_map(|i| entry(4 * g + i * 4 * p, 4 * g + i * 4 * p + 2 * p, 1)).collect();
        assert_eq!(high(&many, &[]).1, HIGH_RANGES);
        assert_eq!(high(&[entry(4 * g, 8 * g, 1), vec![1]].concat(), &[]).1, 0);
    }

    #[test]
    fn malformed_tail_or_overflow_invalidates_the_whole_map() {
        let good = entry(0, LIMIT as u64, 1);
        for tail in [vec![1], vec![0; 24], vec![255; 24]] {
            assert_eq!(pool(&[good.clone(), tail].concat(), PAGE), (0, 0));
        }
        let mut wrap = entry(0, 100, 1);
        wrap[4..12].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(pool(&[good, wrap].concat(), PAGE), (0, 0));
    }

    #[test]
    fn named_model_memory_is_read_exactly_or_not_at_all() {
        let (found, count) = model_extents("0x100000000:0x12c00000,0x180000000:0x1234").unwrap();
        assert_eq!((count, found[0], found[1]), (2, (0x1_0000_0000, 0x12c0_0000), (0x1_8000_0000, 0x1234)));
        for bad in ["", "0x100000000", "100000000:0x1000", "0x100000000:1000", "0x:0x1000", "0x100000000:0x1000,", "0x1g:0x1", "0x10000000000000000:0x1",
                    "0x1:0x1:0x1", " 0x100000000:0x1000", "0x100000000:0x1000 "] {
            assert!(model_extents(bad).is_none(), "{bad:?}");
        }
        let nine = ["0x100000000:0x1000"; 9].join(",");
        assert!(model_extents(&nine).is_none());
        assert_eq!(model_extents(&["0x100000000:0x1000"; 8].join(",")).unwrap().1, 8);
    }

    #[test]
    fn model_memory_is_kept_only_where_the_map_calls_every_page_of_it_usable() {
        let g = 1u64 << 30;
        let map = [entry(0, 3 * g, 1), entry(4 * g, 6 * g, 1), entry(6 * g, 8 * g, 1), entry(5 * g, 5 * g + 4096, 2)].concat();
        // Two pieces, the second ending partway through a page; the total is the bytes named.
        assert_eq!(model_allowed(&map, &[(4 * g, 8192), (4 * g + 16384, 5000)]), Ok(13192));
        // Across the line between two usable descriptors that touch.
        assert_eq!(model_allowed(&map, &[(6 * g - 4096, 8192)]), Ok(8192));
        let refused = |extents: &[(u64, u64)]| model_allowed(&map, extents).is_err();
        assert!(refused(&[])); // nothing named
        assert!(refused(&[(4 * g, 0)])); // empty
        assert!(refused(&[(4 * g + 1, 4096)])); // not on a page
        assert!(refused(&[(4 * g, 5000), (4 * g + 8192, 4096)])); // a piece before the last not whole pages
        assert!(refused(&[(2 * g, 4096)])); // below four gibibytes, though usable
        assert!(refused(&[(4 * g - 4096, 8192)])); // begins below
        assert!(refused(&[(67 * g, 2 * g)])); // past the ceiling
        assert!(refused(&[(u64::MAX - 4095, 8192)])); // wraps
        assert!(refused(&[(4 * g, 8192), (4 * g + 4096, 4096)])); // two share a page
        assert!(refused(&[(4 * g, 4097), (4 * g + 4096, 1)][1..2].iter().chain(&[(4 * g, 4097)]).copied().collect::<Vec<_>>().as_slice())); // the last page of one is another's
        assert!(refused(&[(5 * g - 4096, 8192)])); // runs into a reserved page
        assert!(refused(&[(5 * g, 1)])); // is a reserved page
        assert!(refused(&[(8 * g - 4096, 8192)])); // runs past the end of memory
        assert!(refused(&[(9 * g, 4096)])); // no memory there
        assert!(refused(&[(4 * g, 4096); 9])); // more pieces than are taken
        // A map that is malformed anywhere allows nothing.
        assert!(model_allowed(&[map.clone(), vec![1]].concat(), &[(4 * g, 4096)]).is_err());
        assert!(model_allowed(&[], &[(4 * g, 4096)]).is_err());
    }

    #[test]
    fn memory_kept_for_a_model_is_never_in_the_pool() {
        let (g, p) = (1u64 << 30, 2u64 << 20);
        let map = entry(4 * g, 8 * g, 1);
        let whole = high(&map, &[]);
        assert_eq!((whole.1, whole.0[0]), (1, ((4 * g) as usize, (8 * g) as usize)));
        // A model in the middle: the pool is what lies before and after it, in whole 2 MiB pages.
        let (ranges, count) = high(&map, &[(5 * g, g + 1)]);
        assert_eq!(count, 2);
        assert_eq!(ranges[0], ((4 * g) as usize, (5 * g) as usize));
        assert_eq!(ranges[1], ((6 * g + p) as usize, (8 * g) as usize));
        // Two pieces, and one that covers everything.
        assert_eq!(high(&map, &[(4 * g, g), (7 * g, g)]).0[0], ((5 * g) as usize, (7 * g) as usize));
        assert_eq!(high(&map, &[(4 * g, 4 * g)]).1, 0);
        // A piece that would wrap allows no pool at all.
        assert_eq!(high(&map, &[(u64::MAX, 2)]).1, 0);
    }
}
