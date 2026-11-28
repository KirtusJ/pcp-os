#[path = "../src/theme.rs"]
mod theme;

use theme::Theme;

fn luminance(color: u32) -> f64 {
    let channel = |shift: u32| {
        let s = f64::from((color >> shift) & 255) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

#[test]
fn both_palettes_keep_text_and_selection_legible() {
    let light = Theme::default();
    let dark = light.other();
    assert!(matches!(
        (light, dark, dark.other()),
        (Theme::Light, Theme::Dark, Theme::Light)
    ));
    assert_eq!(light.switch_hint(), "F6 Dark theme");
    assert_eq!(dark.switch_hint(), "F6 Light theme");
    for theme in [light, dark] {
        let p = theme.palette();
        for (ink, bg) in [
            (p.text, p.sheet),
            (p.title, p.band),
            (p.hint, p.band),
            (p.selected_text, p.selection),
        ] {
            let (a, b) = (luminance(ink), luminance(bg));
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
        }
        assert_ne!(p.selection, p.sheet);
        assert_ne!(p.rule, p.band);
        assert_ne!(p.field_edge, p.sheet);
    }
}
