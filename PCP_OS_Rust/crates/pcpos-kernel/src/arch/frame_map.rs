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
/// memory beyond the ceiling, are left unused. Memory a loader filled with a model is in the map as
/// its own kind (`MODEL_RAM`), which is not usable: no frame of it is ever handed out or written.
pub(super) fn high(bytes: &[u8]) -> ([(usize, usize); HIGH_RANGES], usize) {
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

/// The kind a loader gives, in the memory map it hands over, to memory it allocated itself and
/// filled with a model's bytes: ordinary memory, owned by that loader, and nothing else's. The value
/// is shared with the loader (B3) and is this system's own; no firmware reports it.
pub(super) const MODEL_RAM: u32 = 0x8000_0001;

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

/// Whether memory the loader named as holding a model may be given as one: and if not, why not.
/// Every piece begins on a 4 KiB page; every piece but the last is whole pages (so the model's bytes
/// follow one another when the pieces are mapped page after page); none is empty; all lie between
/// four gibibytes and the ceiling; no two share a page; and every byte of every page of them is
/// memory the loader's map gives the kind `MODEL_RAM`, which nothing else in the map calls reserved.
/// Naming memory on the loader's line does not make it the loader's: only the map does. Memory the
/// map calls usable, or reserved in the ordinary way, is never taken for a model. A map that is
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
        // Checked against the window first: the ceiling is a whole page, so rounding a piece that
        // ends at or below it cannot pass it, or the end of all addresses.
        if base < HIGH_START || end > HIGH_END {
            return Err("a piece lies outside the memory between four gibibytes and the ceiling");
        }
        let whole = end.div_ceil(4096) * 4096;
        if pages[..i].iter().any(|&(a, b)| base < b && a < whole) {
            return Err("two pieces share memory");
        }
        pages[i] = (base, whole);
        total = total.checked_add(length).ok_or("the pieces are longer than can be counted")?;
    }
    // The memory the map gives the model's kind and nothing else calls reserved, to the byte.
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
        if base < end && kind != 1 {
            // Usable memory under a model's own kind takes nothing from it (the allocator gives the
            // other kind precedence); any other kind over it does.
            let (model, other) = if kind == MODEL_RAM { (1_i32, 0_i32) } else { (0, 1) };
            boundaries.push((base, model, other));
            boundaries.push((end, -model, -other));
        }
        rest = &rest[4 + size..];
    }
    boundaries.sort_unstable();
    let mut good: Vec<(u64, u64)> = Vec::new();
    let mut start = None;
    let (mut model, mut other) = (0, 0);
    let mut iter = boundaries.into_iter().peekable();
    while let Some((at, add_model, add_other)) = iter.next() {
        model += add_model;
        other += add_other;
        while let Some((_, m, o)) = iter.next_if(|b| b.0 == at) {
            model += m;
            other += o;
        }
        if model > 0 && other == 0 {
            start.get_or_insert(at);
        } else if let Some(first) = start.take() {
            // Adjacent descriptors of the kind join: a piece may lie across the line between two.
            match good.last_mut() {
                Some(last) if last.1 == first => last.1 = at,
                _ => good.push((first, at)),
            }
        }
    }
    for &(base, whole) in &pages[..extents.len()] {
        if !good.iter().any(|&(a, b)| a <= base && whole <= b) {
            return Err("a piece is not wholly memory the loader's map marks as holding a model");
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
        assert_eq!(high(&entry(0, 4 * g, 1)).1, 0);
        // One range, cut to whole pages; a reserved hole in it makes two; order of records does not matter.
        let big = entry(4 * g + 5, 6 * g - 5, 1);
        let hole = entry(5 * g, 5 * g + 1, 2);
        let (ranges, count) = high(&[hole.clone(), big.clone()].concat());
        assert_eq!((count, ranges[0], ranges[1]), (2, ((4 * g + p) as usize, (5 * g) as usize), ((5 * g + p) as usize, (6 * g - p) as usize)));
        assert_eq!(high(&[big, hole].concat()), (ranges, count));
        // A usable range that begins below four gibibytes keeps the whole of its part above (the cut
        // to the window comes before the cut to whole pages), and loses all of its part below.
        let (crossing, n) = high(&entry(3 * g + 5, 6 * g, 1));
        assert_eq!((n, crossing[0]), (1, ((4 * g) as usize, (6 * g) as usize)));
        // A reserved record that only touches a usable one at its end removes nothing; one that
        // overlaps it by a byte removes that page and nothing is invented beyond what is usable.
        assert_eq!(high(&[entry(4 * g, 5 * g, 1), entry(5 * g, 6 * g, 2)].concat()).0[0], ((4 * g) as usize, (5 * g) as usize));
        assert_eq!(high(&[entry(4 * g, 5 * g, 1), entry(5 * g - 1, 6 * g, 2)].concat()).0[0], ((4 * g) as usize, (5 * g - p) as usize));
        assert_eq!(high(&entry(4 * g, 4 * g + p - 1, 1)).1, 0);
        // Memory past the ceiling is left; more than eight ranges keep the first eight; a bad map gives none.
        let (far, n) = high(&entry(60 * g, 80 * g, 1));
        assert_eq!((n, far[0]), (1, ((60 * g) as usize, HIGH_END as usize)));
        let many: Vec<u8> = (0..10u64).flat_map(|i| entry(4 * g + i * 4 * p, 4 * g + i * 4 * p + 2 * p, 1)).collect();
        assert_eq!(high(&many).1, HIGH_RANGES);
        assert_eq!(high(&[entry(4 * g, 8 * g, 1), vec![1]].concat()).1, 0);
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
    fn model_memory_is_given_only_where_the_map_marks_every_page_of_it_as_a_models() {
        let g = 1u64 << 30;
        let m = MODEL_RAM;
        // Usable memory with the loader's own kind over part of it, in two touching descriptors, and
        // one page inside that the map also calls reserved.
        let map = [entry(0, 3 * g, 1), entry(4 * g, 8 * g, 1), entry(5 * g, 6 * g, m), entry(6 * g, 7 * g, m), entry(6 * g + 8192, 6 * g + 12288, 2)].concat();
        // Two pieces, the second ending partway through a page; the total is the bytes named.
        assert_eq!(model_allowed(&map, &[(5 * g, 8192), (5 * g + 16384, 5000)]), Ok(13192));
        // Across the line between two descriptors of the kind that touch.
        assert_eq!(model_allowed(&map, &[(6 * g - 4096, 8192)]), Ok(8192));
        // The same whichever order the map's records come in.
        let turned = [entry(6 * g + 8192, 6 * g + 12288, 2), entry(6 * g, 7 * g, m), entry(5 * g, 6 * g, m), entry(4 * g, 8 * g, 1), entry(0, 3 * g, 1)].concat();
        assert_eq!(model_allowed(&turned, &[(5 * g, 8192), (5 * g + 16384, 5000)]), Ok(13192));
        let refused = |extents: &[(u64, u64)]| model_allowed(&map, extents).is_err();
        assert!(refused(&[])); // nothing named
        assert!(refused(&[(5 * g, 0)])); // empty
        assert!(refused(&[(5 * g + 1, 4096)])); // not on a page
        assert!(refused(&[(5 * g, 5000), (5 * g + 8192, 4096)])); // a piece before the last not whole pages
        assert!(refused(&[(5 * g, 8192), (5 * g + 4096, 4096)])); // two share a page
        assert!(refused(&[(5 * g + 4096, 1), (5 * g, 4097)])); // the last page of one is another's
        assert!(refused(&[(5 * g, 4096); 9])); // more pieces than are taken
        // Memory the map calls usable is not a model's, however it is named.
        assert!(refused(&[(4 * g, 4096)]));
        assert!(refused(&[(5 * g - 4096, 8192)])); // begins in usable memory
        assert!(refused(&[(7 * g - 4096, 8192)])); // ends in usable memory
        // Nor memory the map calls reserved in the ordinary way, alone or over the model's kind.
        assert!(model_allowed(&[entry(4 * g, 5 * g, 2)].concat(), &[(4 * g, 4096)]).is_err());
        assert!(refused(&[(6 * g + 8192, 4096)]));
        assert!(refused(&[(6 * g + 4096, 8192)])); // runs into the reserved page
        // Nor anything outside the window, even marked.
        let low = [entry(2 * g, 3 * g, m), entry(67 * g, 70 * g, m)].concat();
        assert!(model_allowed(&low, &[(2 * g, 4096)]).is_err()); // below four gibibytes
        assert!(model_allowed(&low, &[(4 * g - 4096, 8192)]).is_err());
        assert!(model_allowed(&low, &[(68 * g - 4096, 8192)]).is_err()); // past the ceiling
        assert!(model_allowed(&low, &[(68 * g - 4096, 4096)]).is_ok()); // the last page below it
        assert!(refused(&[(9 * g, 4096)])); // no memory there
        // Numbers at the end of all addresses are refused, not computed with (C3 found the rounding
        // of such an end overflowed and stopped the kernel).
        for bad in [(u64::MAX - 4095, 4095), (u64::MAX - 4095, 4096), (u64::MAX, 1), (u64::MAX - 8191, 8191), (0xffff_ffff_ffff_f000, 0xfff), (68 * g, 1), (68 * g - 4096, 4097)] {
            assert!(model_allowed(&[entry(4 * g, u64::MAX, m)].concat(), &[bad]).is_err(), "{bad:?}");
        }
        // A map that is malformed anywhere allows nothing.
        assert!(model_allowed(&[map.clone(), vec![1]].concat(), &[(5 * g, 4096)]).is_err());
        assert!(model_allowed(&[], &[(5 * g, 4096)]).is_err());
    }

    #[test]
    fn memory_marked_as_a_models_is_never_in_the_pool() {
        let (g, p) = (1u64 << 30, 2u64 << 20);
        let whole = high(&entry(4 * g, 8 * g, 1));
        assert_eq!((whole.1, whole.0[0]), (1, ((4 * g) as usize, (8 * g) as usize)));
        // A model in the middle, its record before or after the usable one: the pool is what lies
        // before and after it, in whole 2 MiB pages.
        for map in [[entry(4 * g, 8 * g, 1), entry(5 * g, 6 * g + 1, MODEL_RAM)].concat(), [entry(5 * g, 6 * g + 1, MODEL_RAM), entry(4 * g, 8 * g, 1)].concat()] {
            let (ranges, count) = high(&map);
            assert_eq!(count, 2);
            assert_eq!(ranges[0], ((4 * g) as usize, (5 * g) as usize));
            assert_eq!(ranges[1], ((6 * g + p) as usize, (8 * g) as usize));
        }
        // Marked memory with no usable record under it is not usable either.
        assert_eq!(high(&entry(4 * g, 8 * g, MODEL_RAM)).1, 0);
    }
}
