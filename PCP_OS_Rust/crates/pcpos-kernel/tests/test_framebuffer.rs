//! Host tests of the framebuffer decision. Build and run from this crate's folder:
//!   rustc --edition=2021 --test tests/test_framebuffer.rs -o /tmp/test-framebuffer && /tmp/test-framebuffer
#[path = "../src/arch/framebuffer.rs"]
mod framebuffer;
use framebuffer::{decide, Window};

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
