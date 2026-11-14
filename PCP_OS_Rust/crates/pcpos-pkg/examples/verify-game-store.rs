//! Verify the game store listings against the files in a checkout.
//! Run with `cargo run -p pcpos-pkg --example verify-game-store [-- REPOSITORY_ROOT]`.

use pcpos_pkg::{index_path, verify, Entry, Kind};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const OWNER: &str = "kirtusj";
const GAMES: [(&str, &[&str]); 3] = [
    ("night-post", &["Night_Post.html", "nightpost.elf"]),
    (
        "bellweather-harbor",
        &["Bellweather_Harbor.html", "bellweather.elf"],
    ),
    ("gullpost-express", &["gullpost.elf"]),
];

fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn check_game(root: &Path, name: &str, required: &[&str]) -> Result<(), String> {
    let shard = index_path(OWNER, name)?;
    let listing = root.join("store/index").join(&shard);
    let entry = Entry::parse(&read(&listing)?)
        .map_err(|error| format!("invalid listing {}: {error}", listing.display()))?;
    let actual_shard = index_path(entry.owner(), entry.name())?;
    if actual_shard != shard {
        return Err(format!(
            "{} contains {} (belongs in store/index/{actual_shard}); expected {OWNER}/{name}",
            listing.display(),
            entry.id()
        ));
    }
    if entry.kind() != Kind::Application {
        return Err(format!(
            "{} declares {:?}; expected an application",
            entry.id(),
            entry.kind()
        ));
    }
    for filename in required {
        let path = format!("store/games/{name}/dist/{filename}");
        if !entry.files().iter().any(|file| file.path() == path) {
            return Err(format!("{} omits required artifact {path}", entry.id()));
        }
    }

    let mut contents = Vec::with_capacity(entry.files().len());
    for file in entry.files() {
        contents.push((file.path(), read(&root.join(file.path()))?));
    }
    let supplied: Vec<_> = contents
        .iter()
        .map(|(path, bytes)| (*path, bytes.as_slice()))
        .collect();
    verify(&entry, &supplied).map_err(|error| format!("{}: {error}", listing.display()))?;
    let unit = if supplied.len() == 1 { "file" } else { "files" };
    println!("Verified {} ({} {unit})", entry.id(), supplied.len());
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."));
    if args.next().is_some() {
        return Err("usage: verify-game-store [REPOSITORY_ROOT]".into());
    }
    for (name, required) in GAMES {
        check_game(&root, name, required)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
