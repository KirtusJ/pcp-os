//! The core of the PCP OS package manager.
//!
//! What this crate is: the rules by which a name becomes exactly one set of bytes. Names, index entries,
//! the check of supplied files against an entry, and the choice among sources. It reads no file, opens no
//! connection and starts no program; a front end (the host's commands, its tool server, a graphical view)
//! brings it bytes and takes its answers. It is not the Store: it knows nothing of reviews, editions or
//! prices, and it works with any source that holds entries in this form.
//!
//! What it is not yet: there is no signature check here, no install transaction and no record in a register.
//! An entry that passes [`verify`] is a statement that the supplied bytes are the bytes the entry names,
//! and nothing about who wrote the entry.
//!
//! `no_std`, heap allocation only, no `unsafe`, no dependency beyond the integrity core.
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use pcpos_core::sha256_hex;
use pcpos_core::value::Value;

/// The version of the index entry form this crate reads.
pub const INDEX_FORM: i64 = 1;
/// The longest owner or package name, in bytes.
pub const MAX_NAME: usize = 64;
/// The longest path of an installed file, in bytes. Chosen so that a path under any reasonable root stays
/// inside the 260 characters a default Windows installation accepts.
pub const MAX_PATH: usize = 200;
/// The longest single component of a path, in bytes.
pub const MAX_COMPONENT: usize = 100;
/// The deepest path, in components.
pub const MAX_DEPTH: usize = 16;
/// The most files one entry may list.
pub const MAX_FILES: usize = 4096;

/// Names that Windows gives to devices, with or without an extension, in any letter case.
const DEVICES: [&str; 24] = ["con", "prn", "aux", "nul", "com0", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt0", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"];

fn is_device(stem: &str) -> bool {
    DEVICES.iter().any(|d| d.eq_ignore_ascii_case(stem))
}

/// Whether `s` may be the name of an owner or of a package.
///
/// Lower-case letters, digits and single hyphens; a letter first, a letter or digit last; at most
/// [`MAX_NAME`] bytes; not a device name. With one letter case only, two names never differ by case alone.
pub fn check_name(s: &str) -> Result<(), String> {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > MAX_NAME {
        return Err(format!("a name has 1 to {MAX_NAME} characters; this one has {}", b.len()));
    }
    if !b[0].is_ascii_lowercase() {
        return Err(format!("a name begins with a lower-case letter: {s:?}"));
    }
    if !(b[b.len() - 1].is_ascii_lowercase() || b[b.len() - 1].is_ascii_digit()) {
        return Err(format!("a name ends with a lower-case letter or a digit: {s:?}"));
    }
    for (i, c) in b.iter().enumerate() {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || (*c == b'-' && b[i - 1] != b'-');
        if !ok {
            return Err(format!("a name is made of lower-case letters, digits and single hyphens: {s:?}"));
        }
    }
    if is_device(s) {
        return Err(format!("{s:?} is the name of a device on Windows and cannot be a package or an owner"));
    }
    Ok(())
}

/// A version: three whole numbers, as Cargo writes the core of a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// Read `major.minor.patch`. No leading zeros, no pre-release or build suffix: those are refused, not ignored.
    pub fn parse(s: &str) -> Result<Version, String> {
        let mut parts = [0u32; 3];
        let mut n = 0;
        for piece in s.split('.') {
            if n == 3 {
                return Err(format!("a version is three numbers separated by dots: {s:?}"));
            }
            let plain = !piece.is_empty() && piece.len() <= 9 && piece.bytes().all(|c| c.is_ascii_digit()) && (piece == "0" || !piece.starts_with('0'));
            if !plain {
                return Err(format!("each part of a version is a whole number without a leading zero, below one thousand million: {s:?}"));
            }
            parts[n] = piece.parse::<u32>().map_err(|_| format!("not a version: {s:?}"))?;
            n += 1;
        }
        if n != 3 {
            return Err(format!("a version is three numbers separated by dots: {s:?}"));
        }
        Ok(Version(parts[0], parts[1], parts[2]))
    }

    /// The version as it is written.
    pub fn text(&self) -> String {
        format!("{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Whether `p` may be the path of an installed file, on every system this is meant to run on.
///
/// Relative, components separated by `/`. A component is made of ASCII letters, digits, dot, hyphen and
/// underscore; it is not `.` or `..`, does not end in a dot, and its part before the first dot is not a
/// device name. So there is no colon (no alternate stream), no space, no backslash and no way out of the root.
pub fn check_path(p: &str) -> Result<(), String> {
    if p.is_empty() || p.len() > MAX_PATH {
        return Err(format!("a path has 1 to {MAX_PATH} characters; this one has {}", p.len()));
    }
    let mut depth = 0;
    for comp in p.split('/') {
        depth += 1;
        if comp.is_empty() || comp.len() > MAX_COMPONENT {
            return Err(format!("each part of a path has 1 to {MAX_COMPONENT} characters, and a path neither begins nor ends with a slash: {p:?}"));
        }
        if !comp.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-' || c == b'_') {
            return Err(format!("a path is made of ASCII letters, digits, dot, hyphen and underscore, with / between its parts: {p:?}"));
        }
        if comp == "." || comp == ".." || comp.ends_with('.') {
            return Err(format!("a part of a path is not . or .. and does not end in a dot: {p:?}"));
        }
        if is_device(comp.split('.').next().unwrap_or("")) {
            return Err(format!("{comp:?} in {p:?} is the name of a device on Windows"));
        }
    }
    if depth > MAX_DEPTH {
        return Err(format!("a path has at most {MAX_DEPTH} parts: {p:?}"));
    }
    Ok(())
}

/// Where the entries of one package are kept in an index, by a rule a client can compute from the name
/// alone, so that it fetches only what it asks for. The layout under the owner is the one Cargo's index uses.
pub fn index_path(owner: &str, name: &str) -> Result<String, String> {
    check_name(owner)?;
    check_name(name)?;
    Ok(match name.len() {
        1 => format!("{owner}/1/{name}"),
        2 => format!("{owner}/2/{name}"),
        3 => format!("{owner}/3/{}/{name}", &name[..1]),
        _ => format!("{owner}/{}/{}/{name}", &name[..2], &name[2..4]),
    })
}

/// What kind of thing an entry installs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A module of the method: text. It declares no capability and nothing in it is run.
    Module,
    /// A Store application: a guest program the host may run with the capabilities the entry declares.
    Application,
    /// A plugin for an assistant. Another program installs it; the entry records where its bytes come from.
    Plugin,
}

/// One file an entry names. It is made only by reading an entry and cannot be changed afterwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    path: String,
    sha256: String,
    size: u64,
}

impl File {
    /// Its path under the place of installation.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// The SHA-256 of its bytes, 64 lower-case hexadecimal digits.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    /// Its length in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// One version of one package, as an index states it.
///
/// An entry is made only by [`Entry::parse`], which checks all of it, and it cannot be changed afterwards:
/// its fields are read through the methods below. So what [`verify`] accepts is always what the digest names.
///
/// Two things are kept apart. The *artifact* is what is installed: owner, name, version, kind, commit, files
/// and capabilities. Its digest is the pin, and it never changes. The *status* is whether the index has
/// withdrawn the entry from new installations; it can change later and is no part of the pin, so an
/// installation that pinned an entry before it was withdrawn still finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    owner: String,
    name: String,
    version: Version,
    kind: Kind,
    commit: String,
    files: Vec<File>,
    capabilities: Vec<String>,
    withdrawn: bool,
    digest: String,
}

fn hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

impl Entry {
    /// Read an entry. Every field is required, no other field is accepted, and everything is checked: an
    /// entry that parses names files that can exist beside each other on every system.
    pub fn parse(bytes: &[u8]) -> Result<Entry, String> {
        let v = Value::parse(bytes).map_err(|e| format!("the entry is not a record value: {e:?}"))?;
        let Value::Map(map) = &v else {
            return Err("the entry is not a map".to_string());
        };
        const KEYS: [&str; 9] = ["capabilities", "commit", "files", "kind", "name", "owner", "pcp_index", "version", "withdrawn"];
        if let Some(k) = map.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(format!("the entry has a field this form does not know: {k:?}"));
        }
        let text = |k: &str| v.get(k).and_then(Value::as_str).ok_or(format!("the entry needs {k}, as text"));
        if v.get("pcp_index").and_then(Value::as_int) != Some(INDEX_FORM) {
            return Err(format!("the entry needs pcp_index = {INDEX_FORM}; another form is not read as this one"));
        }
        let (owner, name) = (text("owner")?, text("name")?);
        check_name(owner)?;
        check_name(name)?;
        let version = Version::parse(text("version")?)?;
        let kind = match text("kind")? {
            "module" => Kind::Module,
            "application" => Kind::Application,
            "plugin" => Kind::Plugin,
            other => return Err(format!("kind is module, application or plugin, not {other:?}")),
        };
        let commit = text("commit")?;
        if !hex(commit, 40) {
            return Err("commit is 40 lower-case hexadecimal digits: a branch or a tag can move, a commit cannot".to_string());
        }
        let withdrawn = match v.get("withdrawn") {
            Some(Value::Bool(b)) => *b,
            _ => return Err("the entry needs withdrawn, true or false".to_string()),
        };
        let Some(Value::List(caps)) = v.get("capabilities") else {
            return Err("the entry needs capabilities, a list (empty if it asks for nothing)".to_string());
        };
        let mut capabilities = Vec::new();
        for c in caps {
            let c = c.as_str().ok_or("a capability is text")?;
            let ok = !c.is_empty() && c.len() <= MAX_NAME && c.split('.').all(|p| check_name(p).is_ok());
            if !ok || capabilities.iter().any(|x: &String| x == c) {
                return Err(format!("a capability is names joined by dots, each given once: {c:?}"));
            }
            capabilities.push(c.to_string());
        }
        if kind != Kind::Application && !capabilities.is_empty() {
            return Err("only an application asks for capabilities: a module is text and a plugin is installed by another program".to_string());
        }
        let Some(Value::List(list)) = v.get("files") else {
            return Err("the entry needs files, a list".to_string());
        };
        if list.is_empty() || list.len() > MAX_FILES {
            return Err(format!("an entry names 1 to {MAX_FILES} files; this one names {}", list.len()));
        }
        let mut files: Vec<File> = Vec::new();
        let mut folded: BTreeMap<String, String> = BTreeMap::new();
        for f in list {
            let Value::Map(m) = f else {
                return Err("a file is a map with path, sha256 and size".to_string());
            };
            if m.len() != 3 {
                return Err("a file has exactly path, sha256 and size".to_string());
            }
            let path = f.get("path").and_then(Value::as_str).ok_or("a file needs path, as text")?;
            check_path(path)?;
            let sha = f.get("sha256").and_then(Value::as_str).filter(|s| hex(s, 64)).ok_or(format!("{path}: sha256 is 64 lower-case hexadecimal digits"))?;
            let size = f.get("size").and_then(Value::as_int).filter(|n| *n >= 0).ok_or(format!("{path}: size is a whole number of bytes, zero or more"))?;
            // Two paths that a case-insensitive file system would make one.
            let lower = path.to_ascii_lowercase();
            if let Some(first) = folded.insert(lower, path.to_string()) {
                return Err(format!("{first:?} and {path:?} would be one file where letter case is not kept apart"));
            }
            files.push(File { path: path.to_string(), sha256: sha.to_string(), size: size as u64 });
        }
        // A path that is also a folder of another path.
        for lower in folded.keys() {
            let mut at = 0;
            while let Some(i) = lower[at..].find('/') {
                at += i;
                if let Some(file) = folded.get(&lower[..at]) {
                    return Err(format!("{file:?} is a file and also a folder of {:?}", folded[lower]));
                }
                at += 1;
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        // The pin is the digest of the artifact: the entry in canonical form without its status.
        let mut artifact = map.clone();
        artifact.remove("withdrawn");
        let digest = sha256_hex(&Value::Map(artifact).canonical_json());
        Ok(Entry { owner: owner.to_string(), name: name.to_string(), version, kind, commit: commit.to_string(), files, capabilities, withdrawn, digest })
    }

    /// The SHA-256 of the artifact in canonical form: every field but the status. This is what an
    /// installation pins. It fixes every file's digest, the commit, the kind and the capabilities at once,
    /// and it is the same before and after the index withdraws the entry.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Who publishes it.
    pub fn owner(&self) -> &str {
        &self.owner
    }
    /// Its name under that owner.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Its version.
    pub fn version(&self) -> Version {
        self.version
    }
    /// What kind of thing it is.
    pub fn kind(&self) -> Kind {
        self.kind
    }
    /// The commit its files are taken from: 40 lower-case hexadecimal digits.
    pub fn commit(&self) -> &str {
        &self.commit
    }
    /// Every file it installs, in order of path.
    pub fn files(&self) -> &[File] {
        &self.files
    }
    /// What it asks to be allowed to do. A request, not a grant.
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }
    /// Whether the index has withdrawn it from new installations.
    pub fn withdrawn(&self) -> bool {
        self.withdrawn
    }

    /// `owner/name` and the version, as a person writes it.
    pub fn id(&self) -> String {
        format!("{}/{} {}", self.owner, self.name, self.version.text())
    }
}

/// Check supplied files against an entry: every file the entry names is supplied once, with the length and
/// the SHA-256 it names, and nothing else is supplied. Modes, dates and line endings play no part; a file
/// whose line endings were changed has other bytes and fails here, which is the point.
pub fn verify(entry: &Entry, supplied: &[(&str, &[u8])]) -> Result<(), String> {
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    for (path, bytes) in supplied {
        let Some(want) = entry.files.iter().find(|f| f.path == *path) else {
            return Err(format!("{path:?} was supplied and {} does not name it; nothing extra is installed", entry.id()));
        };
        if seen.insert(path, ()).is_some() {
            return Err(format!("{path:?} was supplied twice"));
        }
        if bytes.len() as u64 != want.size {
            return Err(format!("{path}: {} bytes were supplied and the entry names {}", bytes.len(), want.size));
        }
        let got = sha256_hex(bytes);
        if got != want.sha256 {
            return Err(format!("{path}: the bytes supplied have SHA-256 {got} and the entry names {}", want.sha256));
        }
    }
    if let Some(missing) = entry.files.iter().find(|f| !seen.contains_key(f.path.as_str())) {
        return Err(format!("{:?} is named by {} and was not supplied", missing.path, entry.id()));
    }
    Ok(())
}

/// A place entries come from, with its rank among the places a machine trusts.
#[derive(Clone, Debug)]
pub struct Source {
    /// What the machine calls it.
    pub label: String,
    /// Its rank: the lower number is asked first. No two sources share a rank.
    pub rank: u32,
    /// The entries it holds.
    pub entries: Vec<Entry>,
}

/// What is asked of the sources.
#[derive(Clone, Copy, Debug)]
pub enum Want<'a> {
    /// The highest version that is not withdrawn.
    Newest,
    /// This version, if it is not withdrawn.
    Exact(Version),
    /// The artifact with this digest, withdrawn or not: what an existing installation names.
    Pinned(&'a str),
}

/// Choose one entry for `owner/name`.
///
/// The first source by rank that holds any version of the name is the only one consulted for it: a source
/// further down cannot supply a newer version of a name that a source above already holds. Within that
/// source, each version names one artifact. If any record for that artifact is withdrawn, a pinned
/// lookup returns a withdrawn entry independently of the order in which the source lists its records.
pub fn resolve<'a>(sources: &'a [Source], owner: &str, name: &str, want: Want) -> Result<(&'a str, &'a Entry), String> {
    check_name(owner)?;
    check_name(name)?;
    let mut order: Vec<&Source> = sources.iter().collect();
    order.sort_by_key(|s| s.rank);
    if let Some(w) = order.windows(2).find(|w| w[0].rank == w[1].rank) {
        return Err(format!("sources {:?} and {:?} have the same rank {}; which is asked first must be stated", w[0].label, w[1].label, w[0].rank));
    }
    let Some(holder) = order.iter().find(|s| s.entries.iter().any(|e| e.owner == owner && e.name == name)) else {
        return Err(format!("no source holds {owner}/{name} ({} asked)", order.len()));
    };
    // One artifact for each version. The same artifact may be stated more than once, as it is when an index
    // keeps the record of a withdrawal beside the first record: then it is withdrawn if any record says so,
    // since a withdrawal is not taken back by an older record. Two different artifacts under one version
    // are refused: a published version does not change.
    let mut held: BTreeMap<Version, &Entry> = BTreeMap::new();
    for e in holder.entries.iter().filter(|e| e.owner == owner && e.name == name) {
        let representative = held.entry(e.version).or_insert(e);
        if representative.digest != e.digest {
            return Err(format!("source {:?} holds {} as two different artifacts; a published version does not change", holder.label, e.id()));
        }
        if e.withdrawn {
            *representative = e;
        }
    }
    let found = match want {
        Want::Newest => held.values().rev().copied().find(|e| !e.withdrawn).ok_or_else(|| format!("every version of {owner}/{name} in source {:?} is withdrawn", holder.label))?,
        Want::Exact(v) => {
            let e = held.get(&v).copied().ok_or_else(|| format!("source {:?} holds {owner}/{name} and not version {}", holder.label, v.text()))?;
            if e.withdrawn {
                return Err(format!("{} is withdrawn from new installations; an installation that names it by digest still verifies", e.id()));
            }
            e
        }
        Want::Pinned(d) => held.values().copied().find(|e| e.digest == d).ok_or_else(|| format!("source {:?} is the one that holds {owner}/{name}, and it has no entry with digest {d}", holder.label))?,
    };
    Ok((holder.label.as_str(), found))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn entry_json(name: &str, version: &str, kind: &str, caps: &str, files: &[(&str, &[u8])], withdrawn: bool) -> String {
        let files: Vec<String> = files.iter().map(|(p, b)| format!(r#"{{"path":"{p}","sha256":"{}","size":{}}}"#, sha256_hex(b), b.len())).collect();
        format!(r#"{{"capabilities":[{caps}],"commit":"{COMMIT}","files":[{}],"kind":"{kind}","name":"{name}","owner":"kirtusj","pcp_index":1,"version":"{version}","withdrawn":{withdrawn}}}"#, files.join(","))
    }

    fn entry(name: &str, version: &str, files: &[(&str, &[u8])], withdrawn: bool) -> Entry {
        Entry::parse(entry_json(name, version, "module", "", files, withdrawn).as_bytes()).unwrap()
    }

    #[test]
    fn a_name_is_one_spelling_on_every_system() {
        for good in ["a", "pcp", "pcp-os", "science-order", "x1", "a-b-c"] {
            assert!(check_name(good).is_ok(), "{good}");
        }
        for (bad, why) in [("", "1 to"), ("Pcp", "lower-case letter"), ("1pcp", "begins"), ("pcp-", "ends"), ("pcp--os", "single hyphens"), ("pcp_os", "single hyphens"), ("pcp os", "single hyphens"), ("pcp.", "ends"), ("pcp:x", "single hyphens"), ("con", "device"), ("lpt1", "device"), ("nul", "device"), ("p\u{e9}", "ends")] {
            let e = check_name(bad).expect_err(bad);
            assert!(e.contains(why), "{bad}: {e}");
        }
        assert!(check_name(&"a".repeat(MAX_NAME)).is_ok() && check_name(&"a".repeat(MAX_NAME + 1)).is_err());
    }

    #[test]
    fn a_version_is_three_plain_numbers() {
        assert_eq!(Version::parse("1.0.22").unwrap(), Version(1, 0, 22));
        assert_eq!(Version::parse("0.0.0").unwrap().text(), "0.0.0");
        assert!(Version::parse("1.10.0").unwrap() > Version::parse("1.9.9").unwrap(), "numbers, not text");
        for bad in ["", "1", "1.0", "1.0.0.0", "1.0.x", "01.0.0", "1.0.0-beta", "1.0.0+build", " 1.0.0", "1..0", "1.0.", "1.0.9999999999", "-1.0.0"] {
            assert!(Version::parse(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn a_path_can_exist_on_every_system_and_stays_under_its_root() {
        for good in ["Specification.md", "src/lib.rs", "a/b/c/d.txt", "data_1/file-2.tar.gz", "README", ".hidden"] {
            assert!(check_path(good).is_ok(), "{good}");
        }
        // Director G's fixtures on Windows: a trailing dot names the same object, a colon opens another stream.
        for (bad, why) in [("evidence.", "end in a dot"), ("evidence:extra", "made of"), ("../x", "not . or .."), ("a/../b", "not . or .."), ("./a", "not . or .."), ("/abs", "neither begins"), ("a/", "neither begins"), ("a//b", "neither begins"), ("a\\b", "made of"), ("a b", "made of"), ("C:/x", "made of"), ("con", "device"), ("a/NUL.txt", "device"), ("x/Com1", "device"), ("a/aux.tar.gz", "device"), ("caf\u{e9}", "made of"), ("a\0b", "made of")] {
            let e = check_path(bad).expect_err(bad);
            assert!(e.contains(why), "{bad:?}: {e}");
        }
        assert!(check_path(&"a".repeat(MAX_COMPONENT)).is_ok() && check_path(&"a".repeat(MAX_COMPONENT + 1)).is_err());
        let deep = vec!["d"; MAX_DEPTH].join("/");
        assert!(check_path(&deep).is_ok() && check_path(&format!("{deep}/d")).is_err());
        assert!(check_path(&format!("{}/{}", "a".repeat(100), "b".repeat(100))).is_err(), "longer than MAX_PATH");
    }

    #[test]
    fn an_index_path_is_computed_from_the_name_alone() {
        assert_eq!(index_path("kirtusj", "a").unwrap(), "kirtusj/1/a");
        assert_eq!(index_path("kirtusj", "ab").unwrap(), "kirtusj/2/ab");
        assert_eq!(index_path("kirtusj", "abc").unwrap(), "kirtusj/3/a/abc");
        assert_eq!(index_path("kirtusj", "pcp-os").unwrap(), "kirtusj/pc/p-/pcp-os");
        assert!(index_path("Kirtusj", "pcp").is_err() && index_path("kirtusj", "../x").is_err());
        // Whatever the name, the path it gives passes the path rule.
        for name in ["a", "ab", "abc", "abcd", "science-order", "x1-2"] {
            assert!(check_path(&index_path("kirtusj", name).unwrap()).is_ok(), "{name}");
        }
    }

    #[test]
    fn an_entry_is_read_whole_or_refused() {
        let files: [(&str, &[u8]); 2] = [("Specification.md", b"# A module\n"), ("data/table.csv", b"a,b\r\n1,2\r\n")];
        let json = entry_json("optics", "1.2.3", "module", "", &files, false);
        let e = Entry::parse(json.as_bytes()).unwrap();
        assert_eq!((e.owner(), e.name(), e.version(), e.kind(), e.withdrawn()), ("kirtusj", "optics", Version(1, 2, 3), Kind::Module, false));
        assert_eq!(e.id(), "kirtusj/optics 1.2.3");
        assert_eq!((e.files().len(), e.files()[0].path(), e.files()[0].size(), e.commit()), (2, "Specification.md", 11, COMMIT));
        assert_eq!(e.digest().len(), 64);
        // The digest is of the canonical form: the same entry written with other spacing has the same digest.
        let spaced = json.replace(",\"", ", \"").replace("{\"", "{ \"");
        assert_eq!(Entry::parse(spaced.as_bytes()).unwrap().digest(), e.digest());
        // Any change to what is installed changes it.
        let other = entry_json("optics", "1.2.3", "module", "", &[("Specification.md", b"# A module!\n"), files[1]], false);
        assert_ne!(Entry::parse(other.as_bytes()).unwrap().digest(), e.digest());

        let refused = |json: String, why: &str| {
            let err = Entry::parse(json.as_bytes()).expect_err(why);
            assert!(err.contains(why), "{why}: {err}");
        };
        refused(json.replace(r#""pcp_index":1"#, r#""pcp_index":2"#), "pcp_index = 1");
        refused(json.replace(r#""withdrawn":false"#, r#""withdrawn":false,"post_install":"run.sh""#), "does not know");
        refused(json.replace(r#","withdrawn":false"#, ""), "withdrawn");
        refused(json.replace(COMMIT, "main"), "a branch or a tag can move");
        refused(json.replace(COMMIT, &COMMIT.to_uppercase()), "lower-case");
        refused(json.replace("1.2.3", "1.2"), "three numbers");
        refused(json.replace(r#""kind":"module""#, r#""kind":"driver""#), "module, application or plugin");
        refused(json.replace(r#""owner":"kirtusj""#, r#""owner":"KirtusJ""#), "lower-case letter");
        refused(entry_json("optics", "1.0.0", "module", r#""register.read""#, &files, false), "only an application");
        refused(entry_json("optics", "1.0.0", "application", r#""register.read","register.read""#, &files, false), "each given once");
        refused(entry_json("optics", "1.0.0", "application", r#""Register.Write""#, &files, false), "names joined by dots");
        refused(entry_json("optics", "1.0.0", "module", "", &[], false), "1 to");
        refused(entry_json("optics", "1.0.0", "module", "", &[("a.txt", b"1"), ("A.TXT", b"2")], false), "letter case");
        refused(entry_json("optics", "1.0.0", "module", "", &[("a.txt", b"1"), ("a.txt", b"1")], false), "letter case");
        refused(entry_json("optics", "1.0.0", "module", "", &[("data", b"1"), ("data/x", b"2")], false), "also a folder");
        refused(entry_json("optics", "1.0.0", "module", "", &[("Data", b"1"), ("data/x", b"2")], false), "also a folder");
        refused(entry_json("optics", "1.0.0", "module", "", &[("evidence.", b"1")], false), "end in a dot");
        refused(json.replace(&sha256_hex(b"# A module\n"), "abc"), "64 lower-case");
        refused(json.replace(r#""size":11"#, r#""size":-1"#), "zero or more");
        refused("[]".to_string(), "not a map");
        refused("{".to_string(), "not a record value");
        // An application may ask; what it asks is recorded, in the order given.
        let app = Entry::parse(entry_json("grim", "0.1.0", "application", r#""input.value","output.value""#, &files, false).as_bytes()).unwrap();
        assert_eq!((app.kind(), app.capabilities().len()), (Kind::Application, 2));
    }

    #[test]
    fn supplied_files_are_the_named_bytes_or_nothing_is_verified() {
        let a: &[u8] = b"# A module\n";
        let b: &[u8] = b"a,b\r\n1,2\r\n";
        let e = entry("optics", "1.0.0", &[("Specification.md", a), ("data/table.csv", b)], false);
        assert!(verify(&e, &[("Specification.md", a), ("data/table.csv", b)]).is_ok());
        assert!(verify(&e, &[("data/table.csv", b), ("Specification.md", a)]).is_ok(), "the order supplied does not matter");
        type Supplied<'a> = &'a [(&'a str, &'a [u8])];
        let cases: [(Supplied, &str); 7] = [
            (&[("Specification.md", a)], "was not supplied"),
            (&[("Specification.md", a), ("data/table.csv", b), ("extra.sh", b"x")], "nothing extra"),
            (&[("Specification.md", a), ("Specification.md", a), ("data/table.csv", b)], "supplied twice"),
            (&[("Specification.md", b"# A module!\n"), ("data/table.csv", b)], "bytes were supplied and the entry names"),
            (&[("Specification.md", b"# A modulE\n"), ("data/table.csv", b)], "have SHA-256"),
            // The same table after a checkout that normalized its line endings: other bytes, refused.
            (&[("Specification.md", a), ("data/table.csv", b"a,b\n1,2\n")], "bytes were supplied"),
            (&[("specification.md", a), ("data/table.csv", b)], "does not name it"),
        ];
        for (supplied, why) in cases {
            let err = verify(&e, supplied).expect_err(why);
            assert!(err.contains(why), "{why}: {err}");
        }
        assert!(verify(&e, &[]).is_err());
    }

    #[test]
    fn one_source_answers_for_a_name_and_a_version_never_changes() {
        let f: [(&str, &[u8]); 1] = [("Specification.md", b"one")];
        let g: [(&str, &[u8]); 1] = [("Specification.md", b"two")];
        let private = Source { label: "private".to_string(), rank: 1, entries: vec![entry("optics", "1.0.0", &f, false), entry("optics", "1.1.0", &f, true), entry("optics", "1.0.5", &g, false)] };
        let public = Source { label: "public".to_string(), rank: 2, entries: vec![entry("optics", "9.9.9", &g, false), entry("acoustics", "0.3.0", &f, false)] };
        let both = [public.clone(), private.clone()];
        // The newest that is not withdrawn, from the first source that holds the name; the later source's 9.9.9 is not seen.
        let (from, e) = resolve(&both, "kirtusj", "optics", Want::Newest).unwrap();
        assert_eq!((from, e.version()), ("private", Version(1, 0, 5)));
        // A name the first source does not hold is taken from the next.
        assert_eq!(resolve(&both, "kirtusj", "acoustics", Want::Newest).unwrap().0, "public");
        assert_eq!(resolve(&both, "kirtusj", "optics", Want::Exact(Version(1, 0, 0))).unwrap().1.version(), Version(1, 0, 0));
        // Withdrawn: not for a new installation, still found by the digest an installation names.
        let pin = private.entries[1].digest().to_string();
        assert!(resolve(&both, "kirtusj", "optics", Want::Exact(Version(1, 1, 0))).unwrap_err().contains("withdrawn"));
        assert_eq!(resolve(&both, "kirtusj", "optics", Want::Pinned(&pin)).unwrap().1.version(), Version(1, 1, 0));
        // A digest that only the later source has is not taken for a name the first source holds.
        let below = public.entries[0].digest().to_string();
        assert!(resolve(&both, "kirtusj", "optics", Want::Pinned(&below)).unwrap_err().contains("is the one that holds"));
        assert!(resolve(&both, "kirtusj", "optics", Want::Exact(Version(9, 9, 9))).unwrap_err().contains("not version 9.9.9"));
        assert!(resolve(&both, "kirtusj", "nothing", Want::Newest).unwrap_err().contains("no source holds"));
        assert!(resolve(&both, "kirtusj", "Optics", Want::Newest).is_err());
        // Two sources of one rank: refused, since the answer would depend on the order they were listed in.
        let tie = [private.clone(), Source { rank: 1, ..public.clone() }];
        assert!(resolve(&tie, "kirtusj", "optics", Want::Newest).unwrap_err().contains("same rank"));
        // Two artifacts under one version are refused.
        let changed = [Source { label: "bad".to_string(), rank: 1, entries: vec![entry("optics", "1.0.0", &f, false), entry("optics", "1.0.0", &g, false)] }];
        assert!(resolve(&changed, "kirtusj", "optics", Want::Newest).unwrap_err().contains("two different artifacts"));
        // The same artifact stated twice is one artifact.
        let twice = [Source { label: "ok".to_string(), rank: 1, entries: vec![entry("optics", "1.0.0", &f, false), entry("optics", "1.0.0", &f, false)] }];
        assert_eq!(resolve(&twice, "kirtusj", "optics", Want::Newest).unwrap().1.version(), Version(1, 0, 0));
        // Every version withdrawn: nothing new to install.
        let gone = [Source { label: "old".to_string(), rank: 1, entries: vec![entry("optics", "1.0.0", &f, true)] }];
        assert!(resolve(&gone, "kirtusj", "optics", Want::Newest).unwrap_err().contains("every version"));
        assert!(resolve(&[], "kirtusj", "optics", Want::Newest).unwrap_err().contains("no source holds"));
    }

    #[test]
    fn a_pin_names_the_artifact_and_survives_its_withdrawal() {
        // Director G's second finding. An installation pins an entry; later the index withdraws it.
        let f: [(&str, &[u8]); 1] = [("Specification.md", b"one")];
        let before = entry("optics", "1.0.0", &f, false);
        let after = entry("optics", "1.0.0", &f, true);
        let pin = before.digest().to_string();
        assert_eq!(after.digest(), pin, "the status is no part of the pin");
        assert!(!before.withdrawn() && after.withdrawn());
        // The index now holds only the withdrawn record: the old pin still resolves, a new choice is refused.
        let only = [Source { label: "s".to_string(), rank: 1, entries: vec![after.clone()] }];
        assert!(resolve(&only, "kirtusj", "optics", Want::Pinned(&pin)).unwrap().1.withdrawn());
        assert!(resolve(&only, "kirtusj", "optics", Want::Newest).unwrap_err().contains("every version"));
        assert!(resolve(&only, "kirtusj", "optics", Want::Exact(Version(1, 0, 0))).unwrap_err().contains("withdrawn"));
        // The index keeps both records, in either order: the same answers, and no complaint of a duplicate.
        for entries in [vec![before.clone(), after.clone()], vec![after.clone(), before.clone()],
                        vec![before.clone(), after.clone(), before.clone()]] {
            let both = [Source { label: "s".to_string(), rank: 1, entries }];
            let (_, resolved) = resolve(&both, "kirtusj", "optics", Want::Pinned(&pin)).unwrap();
            assert_eq!(resolved.digest(), pin);
            assert!(resolved.withdrawn(), "the returned artifact must carry the source's effective withdrawal");
            assert!(resolve(&both, "kirtusj", "optics", Want::Newest).unwrap_err().contains("every version"));
            assert!(resolve(&both, "kirtusj", "optics", Want::Exact(Version(1, 0, 0))).unwrap_err().contains("withdrawn"));
        }
        // Changed bytes under the same version are another artifact: another pin, and refused beside the first.
        let changed = entry("optics", "1.0.0", &[("Specification.md", b"two")], true);
        assert_ne!(changed.digest(), pin);
        for entries in [vec![before.clone(), changed.clone()], vec![changed.clone(), before.clone()]] {
            let mixed = [Source { label: "s".to_string(), rank: 1, entries }];
            for want in [Want::Pinned(&pin), Want::Pinned(changed.digest()), Want::Newest, Want::Exact(Version(1, 0, 0))] {
                assert!(resolve(&mixed, "kirtusj", "optics", want).unwrap_err().contains("two different artifacts"));
            }
        }
        assert!(verify(&before, &[("Specification.md", b"two")]).is_err());
        // Every other field is part of the pin.
        let base = entry_json("optics", "1.0.0", "application", r#""input.value""#, &f, false);
        let d = |json: String| Entry::parse(json.as_bytes()).unwrap().digest().to_string();
        let pin = d(base.clone());
        for other in [base.replace("1.0.0", "1.0.1"), base.replace(r#""name":"optics""#, r#""name":"optica""#), base.replace(r#""owner":"kirtusj""#, r#""owner":"other""#), base.replace(COMMIT, &COMMIT.replace('0', "1")), base.replace("input.value", "output.value"), base.replace(r#""kind":"application""#, r#""kind":"plugin""#).replace(r#""input.value""#, "")] {
            assert_ne!(d(other.clone()), pin, "{other}");
        }
        assert_eq!(d(base.replace(r#""withdrawn":false"#, r#""withdrawn":true"#)), pin);
    }

    #[test]
    fn any_text_is_answered_or_refused_and_never_a_panic() {
        // Fixed seed: 20000 strings made of the pieces the rules treat specially.
        let pieces = ["a", "B", "1", "-", "--", ".", "..", "/", "//", "\\", ":", " ", "con", "NUL", "lpt1", "\u{e9}", "\0", "x.y", "_", "{", "\"", "9999999999", "0", ","];
        let (mut seed, mut ok, mut refused) = (20261114u64, 0u32, 0u32);
        for _ in 0..20000 {
            let mut text = String::new();
            for _ in 0..(seed >> 60) as usize + 1 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push_str(pieces[(seed >> 33) as usize % pieces.len()]);
            }
            for r in [check_name(&text).is_ok(), check_path(&text).is_ok(), Version::parse(&text).is_ok(), Entry::parse(text.as_bytes()).is_ok()] {
                if r { ok += 1 } else { refused += 1 }
            }
            // What the name rule accepts, the path rule accepts as a single component.
            if check_name(&text).is_ok() {
                assert!(check_path(&text).is_ok(), "{text:?}");
            }
        }
        assert_eq!(ok + refused, 80000);
        assert!(ok > 0 && refused > 0, "{ok} accepted, {refused} refused");
    }
}
