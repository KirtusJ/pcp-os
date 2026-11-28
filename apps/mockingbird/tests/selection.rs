#[allow(dead_code)]
#[path = "../src/editor.rs"]
mod editor;
#[allow(dead_code)]
#[path = "../src/find.rs"]
mod find;

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
