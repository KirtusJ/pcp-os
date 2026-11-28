#[allow(dead_code)]
#[path = "../src/editor.rs"]
mod editor;
#[allow(dead_code)]
#[path = "../src/find.rs"]
mod find;

fn sheet(text: &str) -> editor::Editor {
    let mut e = editor::Editor::new();
    e.bytes[..text.len()].copy_from_slice(text.as_bytes());
    e.len = text.len();
    e
}

#[test]
fn a_full_row_wraps_only_when_the_next_character_needs_a_cell() {
    for (text, expected) in [
        ("abcd", vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]),
        (
            "abcd\nx",
            vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (1, 0), (1, 1)],
        ),
        (
            "abcdx",
            vec![(0, 0), (0, 1), (0, 2), (0, 3), (1, 0), (1, 1)],
        ),
        (
            "é\t\n\nx",
            vec![(0, 0), (0, 1), (0, 4), (1, 0), (2, 0), (2, 1)],
        ),
    ] {
        let e = sheet(text);
        let actual: Vec<_> = e.cells(4).map(|p| (p.row, p.col)).collect();
        assert_eq!(actual, expected, "{text:?}");
        let bytes: Vec<_> = e.cells(4).map(|p| p.byte).collect();
        assert_eq!(
            bytes,
            text.char_indices()
                .map(|(i, _)| i)
                .chain([text.len()])
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn vertical_selection_keeps_its_column_and_horizontal_movement_sets_a_new_one() {
    let mut e = sheet("abcde\nx\nABCDE");
    e.move_to(e.len, false);
    e.move_row(false, 12, true);
    assert_eq!((e.anchor, e.caret), (13, 7));
    e.move_row(false, 12, true);
    assert_eq!((e.anchor, e.caret), (13, 5));
    e.move_row(true, 12, false);
    e.horizontal(true, false);
    e.move_row(true, 12, false);
    assert_eq!(e.caret, 8);
    e.line_edge(true, 12, false);
    e.move_row(false, 12, false);
    e.move_row(false, 12, false);
    assert_eq!(e.caret, 5);
}

#[test]
fn find_keeps_the_active_end_when_a_query_misses_or_is_cleared() {
    let mut e = editor::Editor::new();
    let mut find = find::Find::new();
    e.bytes[..4].copy_from_slice(b"abcd");
    e.len = 4;
    for (anchor, caret) in [(3, 1), (1, 3)] {
        e.move_to(anchor, false);
        e.move_to(caret, true);
        find.begin(&e);
        find.insert(&mut e, b"z");
        assert_eq!((e.selection(), e.caret), (1..3, caret));
        find.select_query = true;
        find.insert(&mut e, b"d");
        assert_eq!(e.selection(), 3..4);
        find.backspace(&mut e);
        assert_eq!((e.selection(), e.caret), (1..3, caret));
    }
}
