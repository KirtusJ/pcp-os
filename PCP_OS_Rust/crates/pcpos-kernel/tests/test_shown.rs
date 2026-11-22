//! Host tests of the name a person reads for a program. From this crate's folder:
//!   rustc --edition=2021 --test tests/test_shown.rs -o /tmp/test-shown && /tmp/test-shown
#[path = "../src/session/shown.rs"]
mod shown_names;
use shown_names::{shown, NAMES};

#[test]
fn the_two_products_are_shown_with_their_spelling() {
    assert_eq!(shown("tailorbird"), "Tailorbird");
    assert_eq!(shown("surfbird"), "Surfbird");
}

#[test]
fn any_other_name_is_shown_as_the_loader_gave_it() {
    for name in ["model-worker", "surf.elf", "rdesk.elf", "demo", "", "a b"] {
        assert_eq!(shown(name), name);
    }
}

#[test]
fn only_the_exact_name_is_respelled() {
    for name in ["TAILORBIRD", "Tailorbird", "tailorbird.elf", "tailorbird ", " tailorbird", "tailorbird-old", "tailorbir", "surfbirds", "Surfbird", "SURFBIRD"] {
        assert_eq!(shown(name), name, "{name:?}");
    }
}

#[test]
fn a_shown_name_is_as_long_as_the_file_name_and_differs_only_in_case() {
    // The list cuts names to a width; a longer shown name could be cut where the file's was not.
    for (file, product) in NAMES {
        assert_eq!(file.len(), product.len());
        assert!(file.eq_ignore_ascii_case(product));
        assert_ne!(file, product);
    }
}

#[test]
fn showing_twice_is_showing_once() {
    for (file, _) in NAMES {
        assert_eq!(shown(shown(file)), shown(file));
    }
}
