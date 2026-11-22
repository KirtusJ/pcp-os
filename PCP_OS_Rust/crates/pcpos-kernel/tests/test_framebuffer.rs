//! Host tests of the framebuffer decision. Build and run from this crate's folder:
//!   rustc --edition=2021 --test tests/test_framebuffer.rs -o /tmp/test-framebuffer && /tmp/test-framebuffer
#[path = "../src/arch/framebuffer.rs"]
mod framebuffer;
use framebuffer::{changed_span, decide, draws_off_screen, grant, kept_pages, kept_parts, may_compare, Window};

fn entry(base: u64, length: u64, kind: u32) -> Vec<u8> {
    let mut e = Vec::new();
    e.extend_from_slice(&20u32.to_le_bytes());
    e.extend_from_slice(&base.to_le_bytes());
    e.extend_from_slice(&length.to_le_bytes());
    e.extend_from_slice(&kind.to_le_bytes());
    e
}
fn map(entries: &[(u64, u64, u32)]) -> Vec<u8> {
    entries.iter().flat_map(|&(b, l, k)| entry(b, l, k)).collect()
}
const MIB: u64 = 1 << 20;
/// The kernel's own ranges in these tests: image and files to 20 MiB, frame memory 20 to 96 MiB.
const KERNEL: [(usize, usize); 2] = [(1 << 20, 20 << 20), (20 << 20, 96 << 20)];
/// A 128 MiB machine as B3 measured it: usable RAM with the firmware's framebuffer set aside inside it.
fn ramfb_128() -> Vec<u8> {
    map(&[(0, 0x9f000, 1), (MIB, 0x06a3f000 - MIB, 1), (0x06a3f000, 0x300000, 2), (0x06d3f000, 0x07f00000 - 0x06d3f000, 1), (0x07f00000, 0x100000, 2)])
}

#[test]
fn ramfb_in_reserved_ram_is_accepted_page_exact() {
    // 800 by 600 at 0x06a3f000: 1,920,000 bytes, 469 pages, the last with 1,024 bytes past the picture.
    let w = decide(0x06a3f000, 3200, 800, 600, 32, Some(&ramfb_128()), None, &KERNEL).unwrap();
    assert_eq!(w, Window { pages: (0x06a3f000, 0x06a3f000 + 469 * 4096), bytes: (0x06a3f000, 0x06a3f000 + 1_920_000) });
    assert_eq!(w.pages.1 - w.bytes.1, 1024);
}

#[test]
fn device_memory_above_the_top_of_low_memory_is_accepted_without_a_descriptor() {
    // Virtio VGA at 0x80000000, 1280 by 800, no entry of the map covering it (B3's control).
    let w = decide(0x8000_0000, 5120, 1280, 800, 32, Some(&ramfb_128()), None, &KERNEL).unwrap();
    assert_eq!(w.pages, (0x8000_0000, 0x8000_0000 + 1000 * 4096));
    // A machine with memory above four gibibytes: the display sits in the gap below them.
    let big = map(&[(0, 0x9f000, 1), (MIB, 0xbff0_0000 - MIB, 1), (0x1_0000_0000, 0x1_0000_0000, 1)]);
    assert!(decide(0xc000_0000, 7680, 1920, 1080, 32, Some(&big), None, &KERNEL).is_ok());
}

#[test]
fn usable_memory_is_never_a_framebuffer() {
    let plain = map(&[(0, 0x9f000, 1), (MIB, 127 * MIB, 1)]);
    assert_eq!(decide(0x06a3f000, 3200, 800, 600, 32, Some(&plain), None, &KERNEL), Err("the loader's map calls part of it usable memory"));
    // One page of the picture reaching into usable memory is enough.
    let short = map(&[(MIB, 0x06a3f000 - MIB, 1), (0x06a3f000, 468 * 4096, 2), (0x06a3f000 + 468 * 4096, MIB, 1)]);
    assert_eq!(decide(0x06a3f000, 3200, 800, 600, 32, Some(&short), None, &KERNEL), Err("the loader's map calls part of it usable memory"));
}

#[test]
fn a_gap_below_the_top_of_memory_is_refused() {
    // Reserved for 468 of 469 pages, then nothing, with usable memory higher up.
    let gap = map(&[(MIB, 0x06a3f000 - MIB, 1), (0x06a3f000, 468 * 4096, 2), (0x0700_0000, MIB, 1)]);
    assert_eq!(decide(0x06a3f000, 3200, 800, 600, 32, Some(&gap), None, &KERNEL),
               Err("it lies below the top of memory and the loader's map does not set all of it aside"));
    // Two reserved entries that together cover it are accepted, in either order.
    let two = map(&[(0x06b0_0000, 0x30_0000, 2), (MIB, 0x06a3f000 - MIB, 1), (0x06a3f000, 0xc1000, 2), (0x0700_0000, MIB, 1)]);
    assert!(decide(0x06a3f000, 3200, 800, 600, 32, Some(&two), None, &KERNEL).is_ok());
}

#[test]
fn nothing_the_kernel_uses_is_a_framebuffer() {
    let all_reserved = map(&[(MIB, 255 * MIB, 2), (0x1000_0000, MIB, 1)]);
    for address in [0x0010_0000u64, 0x0100_0000, 0x013f_f000, 0x05ff_f000] {
        assert_eq!(decide(address, 3200, 800, 600, 32, Some(&all_reserved), None, &KERNEL), Err("it overlaps memory the kernel uses"), "{address:#x}");
    }
    // Just past the frame memory it is accepted: the rule is overlap, not nearness.
    assert!(decide(96 * MIB, 3200, 800, 600, 32, Some(&all_reserved), None, &KERNEL).is_ok());
    // A last page that would reach into the frame memory from below is refused.
    assert_eq!(decide(20 * MIB - 1_920_000 + 4, 3200, 800, 600, 32, Some(&all_reserved), None, &[(20 << 20, 96 << 20)]), Err("it overlaps memory the kernel uses"));
}

#[test]
fn malformed_descriptions_are_refused() {
    let m = ramfb_128();
    let ok = |a, p, w, h, b| decide(a, p, w, h, b, Some(&m), None, &KERNEL);
    assert_eq!(ok(0x8000_0000, 5120, 1280, 800, 24), Err("it is not 32 bits a pixel"));
    assert_eq!(ok(0x8000_0000, 5116, 1280, 800, 32), Err("its pitch is shorter than its width"));
    assert_eq!(ok(0x8000_0000, 5122, 1280, 800, 32), Err("its pitch is not a multiple of four"));
    assert_eq!(ok(0x8000_0000, 1200, 300, 200, 32), Err("it is smaller than 320 by 200"));
    assert_eq!(ok(0x8000_0002, 5120, 1280, 800, 32), Err("its address is not a multiple of four"));
    assert_eq!(ok(0x000a_0000, 1280, 320, 200, 32), Err("it lies in the first mebibyte"));
    assert_eq!(ok(0xffff_f000, 5120, 1280, 800, 32), Err("it reaches beyond four gibibytes"));
    assert_eq!(ok(0x1_0000_0000, 5120, 1280, 800, 32), Err("it reaches beyond four gibibytes"));
    assert_eq!(ok(u64::MAX - 4095, 5120, 1280, 800, 32), Err("its extent wraps"));
    // Exactly up to four gibibytes is inside.
    assert!(ok(0x1_0000_0000 - 4_096_000, 5120, 1280, 800, 32).is_ok());
}

#[test]
fn malformed_maps_are_refused() {
    let mut cut = ramfb_128();
    cut.truncate(cut.len() - 5);
    assert_eq!(decide(0x8000_0000, 5120, 1280, 800, 32, Some(&cut), None, &KERNEL), Err("the loader's memory map ends inside an entry"));
    let mut small = entry(0, MIB, 1);
    small[0..4].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(decide(0x8000_0000, 5120, 1280, 800, 32, Some(&small), None, &KERNEL), Err("the loader's memory map has an entry of impossible size"));
    let mut long = entry(0, MIB, 1);
    long[0..4].copy_from_slice(&400u32.to_le_bytes());
    assert_eq!(decide(0x8000_0000, 5120, 1280, 800, 32, Some(&long), None, &KERNEL), Err("the loader's memory map has an entry of impossible size"));
    // An empty map says nothing is usable: nothing is below the top of memory, and device memory is accepted.
    assert!(decide(0x8000_0000, 5120, 1280, 800, 32, Some(&[]), None, &KERNEL).is_ok());
}

#[test]
fn without_a_map_only_memory_above_what_was_reported_is_accepted() {
    assert!(decide(0x8000_0000, 5120, 1280, 800, 32, None, Some(128 * MIB), &KERNEL).is_ok());
    assert_eq!(decide(0x06a3f000, 3200, 800, 600, 32, None, Some(128 * MIB), &KERNEL),
               Err("there is no memory map and it lies inside the memory the loader reported"));
    // With neither a map nor a size, the old floor of one gibibyte stands.
    assert!(decide(0x4000_0000, 3200, 800, 600, 32, None, None, &KERNEL).is_ok());
    assert!(decide(0x3000_0000, 3200, 800, 600, 32, None, None, &KERNEL).is_err());
}

#[test]
fn a_program_is_granted_only_pages_that_lie_wholly_inside_the_picture() {
    // The measured RAMFB picture at 128 MiB: 800 by 600, 1,920,000 bytes from 0x6ab2000, which is 468
    // whole pages and three quarters of another.
    let bytes = (0x06ab_2000, 0x06ab_2000 + 1_920_000);
    assert!(grant(bytes, bytes.0, 468));
    assert!(grant(bytes, bytes.0, 1));
    assert!(grant(bytes, bytes.0 + 467 * 4096, 1));
    // The page the picture ends in, alone or as the last of a request, is not granted.
    assert!(!grant(bytes, bytes.0, 469));
    assert!(!grant(bytes, bytes.0 + 468 * 4096, 1));
    // Nor is the page before it, anything unaligned, nothing at all, or a size that wraps.
    assert!(!grant(bytes, bytes.0 - 4096, 1));
    assert!(!grant(bytes, bytes.0 + 4, 1));
    assert!(!grant(bytes, bytes.0, 0));
    assert!(!grant(bytes, bytes.0, usize::MAX / 4096 + 1));
    assert!(!grant(bytes, usize::MAX & !4095, 2));
    // A picture that does not begin on a page: the page it begins in is not granted, the next whole one is.
    let odd = (0x8000_0100, 0x8000_0100 + 3 * 4096);
    assert!(!grant(odd, 0x8000_0000, 1));
    assert!(grant(odd, 0x8000_1000, 2));
    assert!(!grant(odd, 0x8000_1000, 3));
    // A picture that fills whole pages is granted whole.
    assert!(grant((0x8000_0000, 0x8000_0000 + 4_096_000), 0x8000_0000, 1000));
}

#[test]
fn a_one_page_display_is_drawn_off_the_screen_whether_or_not_its_memory_maps_whole() {
    // The owner's laptop: 1920 by 1080 by 4 is 8,294,400 bytes, exactly 2,025 pages, so the picture
    // can be granted whole. The rule this replaces answered "no" here and handed over the screen.
    let laptop = (0x8000_0000usize, 0x8000_0000 + 1920 * 1080 * 4);
    assert_eq!(1920 * 1080 * 4 % 4096, 0);
    let whole = grant(laptop, laptop.0, 2025);
    assert!(whole);
    assert!(draws_off_screen(1, whole));
    // 800 by 600 ends partway through a page: its last page cannot be granted. Drawn off the screen, as before.
    let small = (0x06ab_2000usize, 0x06ab_2000 + 800 * 600 * 4);
    assert_ne!(800 * 600 * 4 % 4096, 0);
    let whole = grant(small, small.0, (800 * 600 * 4 + 4095) / 4096);
    assert!(!whole);
    assert!(draws_off_screen(1, whole));
}

#[test]
fn a_display_with_a_second_page_is_drawn_on_the_page_not_shown() {
    // The control: with two pages a program draws on the one that is not shown, whatever maps whole.
    assert!(!draws_off_screen(2, true));
    assert!(!draws_off_screen(2, false));
    assert!(!draws_off_screen(0, true));
}

fn pixels(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn a_row_that_is_already_shown_is_not_written() {
    let row = pixels(&[1, 2, 3, 4, 5]);
    assert_eq!(changed_span(&row, &row), None);
    assert_eq!(changed_span(&[], &[]), None);
}

#[test]
fn a_changed_row_is_written_from_its_first_changed_pixel_to_its_last() {
    let kept = pixels(&[1, 2, 3, 4, 5, 6]);
    assert_eq!(changed_span(&kept, &pixels(&[1, 2, 9, 4, 5, 6])), Some((2, 3)), "one pixel");
    assert_eq!(changed_span(&kept, &pixels(&[1, 9, 3, 4, 9, 6])), Some((1, 5)), "two apart: one span over both");
    assert_eq!(changed_span(&kept, &pixels(&[9, 2, 3, 4, 5, 6])), Some((0, 1)), "the first");
    assert_eq!(changed_span(&kept, &pixels(&[1, 2, 3, 4, 5, 9])), Some((5, 6)), "the last");
    assert_eq!(changed_span(&kept, &pixels(&[9, 9, 9, 9, 9, 9])), Some((0, 6)), "all");
}

#[test]
fn one_changed_byte_of_a_pixel_is_that_whole_pixel() {
    let kept = pixels(&[0x00112233, 0x00445566]);
    for byte in 0..8 {
        let mut new = kept.clone();
        new[byte] ^= 0x80;
        assert_eq!(changed_span(&kept, &new), Some((byte / 4, byte / 4 + 1)), "byte {byte}");
    }
}

#[test]
fn rows_that_cannot_be_compared_are_written_whole() {
    assert_eq!(changed_span(&pixels(&[1, 2]), &pixels(&[1, 2, 3])), Some((0, 3)), "a different length");
    assert_eq!(changed_span(&[], &pixels(&[1, 2])), Some((0, 2)), "nothing kept");
    assert_eq!(changed_span(&[1, 2, 3], &[1, 2, 3]), Some((0, 0)), "not whole pixels: never called equal; the kernel only ever passes whole rows");
}

#[test]
fn a_kept_picture_takes_the_pages_its_pixels_need() {
    assert_eq!(kept_pages(1920, 1080), Some(2025), "the laptop: exactly 2,025");
    assert_eq!(kept_pages(800, 600), Some(469), "1,920,000 bytes: 468 and a part");
    assert_eq!(kept_pages(1, 1), Some(1));
    assert_eq!(kept_pages(0, 600), Some(0));
    assert_eq!(kept_pages(u32::MAX, u32::MAX), None, "more bytes than can be counted");
}

#[test]
fn a_row_falls_across_its_pages_with_nothing_lost_or_repeated() {
    // A row of 1920 pixels is 7,680 bytes. Row 0 starts a page; row 1 starts 3,584 bytes into the second.
    assert_eq!(kept_parts(0, 7680).collect::<Vec<_>>(), [(0, 0, 4096, 0), (1, 0, 3584, 4096)]);
    assert_eq!(kept_parts(7680, 7680).collect::<Vec<_>>(), [(1, 3584, 512, 0), (2, 0, 4096, 512), (3, 0, 3072, 4608)]);
    assert_eq!(kept_parts(4095, 2).collect::<Vec<_>>(), [(0, 4095, 1, 0), (1, 0, 1, 1)], "across a boundary");
    assert_eq!(kept_parts(4096, 4096).collect::<Vec<_>>(), [(1, 0, 4096, 0)], "exactly one page");
    assert_eq!(kept_parts(123, 0).count(), 0, "nothing asked, nothing given");
    // Every byte of every row of a 1920 by 1080 picture is in exactly one part, in order.
    let mut next = 0usize;
    for y in 0..1080usize {
        let mut done = 0;
        for (page, at, n, from) in kept_parts(y * 7680, 7680) {
            assert_eq!((page * 4096 + at, from), (next, done));
            assert!(n > 0 && at + n <= 4096);
            next += n;
            done += n;
        }
        assert_eq!(done, 7680);
    }
    assert_eq!(next, 2025 * 4096);
}

#[test]
fn a_kept_picture_is_compared_only_while_it_is_what_the_display_shows() {
    assert!(may_compare(7, 7, 0), "recorded at 7, nothing has drawn since, nobody has the screen itself");
    assert!(!may_compare(7, 8, 0), "something drew after it was recorded");
    assert!(!may_compare(0, 0, 0), "zero is never the count: a copy marked zero was never recorded whole");
    assert!(!may_compare(0, 7, 0), "not recorded whole");
}

#[test]
fn nothing_is_compared_while_a_program_has_the_screen_itself_mapped() {
    // The kernel session's case: a program that maps the display directly writes it unseen.
    assert!(!may_compare(7, 7, 1), "one such program alive");
    assert!(!may_compare(7, 7, 3));
    assert!(may_compare(7, 7, 0), "and again once it has ended, if the count had not moved (it does move: see dev::display_direct_map)");
}

#[test]
fn a_picture_that_fails_partway_leaves_no_copy_trusted() {
    // C3's case, as the kernel orders it: the count moves on BEFORE the first row is written, and the
    // presenting program's own copy is marked zero until it has finished and recorded every row.
    let (mut display_at, other_kept_at) = (5u64, 5u64);
    assert!(may_compare(other_kept_at, display_at, 0), "another program's copy is good before");
    display_at += 1; // shadow_present, before its first device write
    let presenting_kept_at = 0u64;
    // ... some rows reach the display, then a row fails and the call returns ...
    assert!(!may_compare(other_kept_at, display_at, 0), "the other copy is stale though the picture was never finished");
    assert!(!may_compare(presenting_kept_at, display_at, 0), "and the presenting program's own copy is not good either");
}
