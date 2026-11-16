use pcpos_pkg::{index_path, Entry, Kind};
use std::{fs, path::Path};

fn collect_games(root: &Path, dir: &Path, games: &mut Vec<(String, String)>) {
    for item in fs::read_dir(dir).unwrap() {
        let item = item.unwrap();
        let path = item.path();
        let kind = item.file_type().unwrap();
        if kind.is_dir() {
            collect_games(root, &path, games);
            continue;
        }
        assert!(
            kind.is_file(),
            "unexpected index object at {}",
            path.display()
        );
        let entry = Entry::parse(&fs::read(&path).unwrap())
            .unwrap_or_else(|error| panic!("invalid index entry {}: {error}", path.display()));
        let shard = index_path(entry.owner(), entry.name()).unwrap();
        let link = format!("store/index/{shard}");
        assert_eq!(path, root.join(&link), "incorrect index location");
        if !entry.withdrawn()
            && entry.kind() == Kind::Application
            && entry
                .files()
                .iter()
                .any(|f| f.path().starts_with("store/games/"))
        {
            games.push((format!("{}/{}", entry.owner(), entry.name()), link));
        }
    }
}

#[test]
fn front_page_lists_each_active_game_with_its_own_index_entry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut expected = Vec::new();
    collect_games(&root, &root.join("store/index"), &mut expected);

    let mut lines = include_str!("../../../../README.md").lines();
    assert!(
        lines.any(|line| line == "## Games"),
        "missing Games section"
    );
    let mut games = lines.take_while(|line| !line.starts_with("## "));
    assert!(
        games.any(|line| line.starts_with("| ---")),
        "missing Games table"
    );
    let mut actual = Vec::new();
    for row in games.take_while(|line| line.starts_with('|')) {
        let (_, code) = row.split_once('`').expect("game row needs a package ID");
        let (id, _) = code.split_once('`').expect("unclosed package ID");
        let (_, url) = row
            .split_once("[Package](")
            .expect("game row needs a package link");
        let (link, _) = url.split_once(')').expect("unclosed package link");
        actual.push((id.to_owned(), link.to_owned()));
    }
    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "README game IDs and links must match the active game entries"
    );
}
