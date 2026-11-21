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
    fn malformed_tail_or_overflow_invalidates_the_whole_map() {
        let good = entry(0, LIMIT as u64, 1);
        for tail in [vec![1], vec![0; 24], vec![255; 24]] {
            assert_eq!(pool(&[good.clone(), tail].concat(), PAGE), (0, 0));
        }
        let mut wrap = entry(0, 100, 1);
        wrap[4..12].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(pool(&[good, wrap].concat(), PAGE), (0, 0));
    }
}
