// Host-side Rust tests for the no_std kernel desktop painter. No kernel boot.
// Run: rustc --edition=2021 --test tests/desktop_raster.rs -o /tmp/pcpos-desktop-tests
//      /tmp/pcpos-desktop-tests
extern crate alloc;

#[path = "../src/desktop.rs"]
mod desktop;

use desktop::{pixel, rasterize, Geometry};

fn geometry(w: u32, h: u32) -> Geometry {
    Geometry::checked(w, h, w * 4, (w as usize) * (h as usize) * 4).unwrap()
}

#[test]
fn exact_reported_modes_are_accepted() {
    for (w, h) in [(320, 200), (640, 480), (800, 600), (1024, 768), (1920, 1080), (2560, 1440)] {
        assert_eq!(geometry(w, h).width, w);
        assert_eq!(geometry(w, h).height, h);
    }
}

#[test]
fn padding_is_not_mistaken_for_visible_pixels() {
    let g = Geometry::checked(320, 200, 320 * 4 + 64, (320 * 4 + 64) * 200).unwrap();
    let mut count = 0;
    assert!(rasterize(g, |y, pixels| {
        assert_eq!(y, count);
        assert_eq!(pixels.len(), 320);
        count += 1;
    }));
    assert_eq!(count, 200);
}

#[test]
fn bad_geometry_is_refused_before_writing() {
    let bad = [
        (0, 480, 3200, 1_536_000usize),
        (319, 200, 1276, 255_200),
        (320, 199, 1280, 254_720),
        (16385, 200, 65540, 13_108_000),
        (320, 16385, 1280, 20_972_800),
        (320, 200, 1276, 255_200),
        (320, 200, 1282, 256_400),
        (320, 200, 1280, 255_999),
    ];
    for (w, h, pitch, mapped) in bad {
        assert!(Geometry::checked(w, h, pitch, mapped).is_none(),
            "invalid geometry accepted: {w}x{h}/{pitch}/{mapped}");
    }
}

#[test]
fn raster_rows_cover_active_mode_once_and_in_order() {
    for (w, h) in [(320, 200), (800, 600)] {
        let g = geometry(w, h);
        let mut rows = 0;
        let mut sampled = 0u64;
        assert!(rasterize(g, |y, pixels| {
            assert_eq!(y, rows);
            assert_eq!(pixels.len(), w as usize);
            assert_eq!(pixels[0], pixel(g, 0, y));
            assert_eq!(pixels[(w - 1) as usize], pixel(g, w - 1, y));
            sampled += pixels.len() as u64;
            rows += 1;
        }));
        assert_eq!(rows, h);
        assert_eq!(sampled, w as u64 * h as u64);
    }
}

#[test]
fn colour_is_deterministic_nonflat_and_24_bit() {
    let g = geometry(1024, 768);
    let top = pixel(g, 0, 0);
    let bottom = pixel(g, 0, g.height - 1);
    let mid = pixel(g, g.width / 2, g.height / 2);
    assert_ne!(top, bottom);
    assert_ne!(top, mid);
    assert!(top <= 0x00ff_ffff && bottom <= 0x00ff_ffff && mid <= 0x00ff_ffff);
    assert_eq!(pixel(g, 123, 456), pixel(g, 123, 456));
}

#[test]
fn model_and_network_are_not_inputs_to_rasterization() {
    // No callbacks request authority, model inference, files or network.
    let g = geometry(320, 200);
    let mut row_count = 0;
    assert!(rasterize(g, |_, _| row_count += 1));
    assert_eq!(row_count, 200);
}
