//! Verify the game store listings against the files in a checkout.
//! Run with `cargo run -p pcpos-pkg --example verify-game-store [-- REPOSITORY_ROOT]`.

use pcpos_pkg::{index_path, verify, Entry, Kind};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const OWNER: &str = "kirtusj";
const PACKAGES: &[(&str, &str, &[&str])] = &[
    (
        "games",
        "night-post",
        &["dist/Night_Post.html", "dist/nightpost.elf"],
    ),
    (
        "games",
        "bellweather-harbor",
        &["dist/Bellweather_Harbor.html", "dist/bellweather.elf"],
    ),
    ("games", "gullpost-express", &["dist/gullpost.elf"]),
    (
        "games",
        "gullpost-express-64",
        &["dist/gull64.elf", "evidence/render-0.1.1/REPORT.md"],
    ),
    ("games", "sunspur-rally", &["dist/sunspur.elf"]),
    ("games", "flux-rink", &["dist/fluxrink.elf"]),
    ("engines", "heliograph", &["dist/heliograph.elf"]),
    ("apps", "keyrook", &["dist/keyrook.elf"]),
];

fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn check_package(
    root: &Path,
    category: &str,
    name: &str,
    required: &[&str],
) -> Result<Entry, String> {
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
        let path = format!("store/{category}/{name}/{filename}");
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
    println!("Verified {} ({} files)", entry.id(), supplied.len());
    Ok(entry)
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
    let mut installed = pcpos_pkg::txn::Installed::new();
    for &(category, name, required) in PACKAGES {
        let entry = check_package(&root, category, name, required)?;
        installed.commit(&installed.plan_install(name, &entry, Some(entry.digest()))?)?;
        if installed.installed(OWNER, name) != Some(entry.digest()) {
            return Err(format!("incorrect installed digest for {name}"));
        }
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
