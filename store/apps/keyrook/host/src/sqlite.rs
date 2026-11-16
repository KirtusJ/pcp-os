use super::{session, Result};
use redb::{Database, ReadableDatabase, ReadableTable};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs,
    io,
    path::{Path, PathBuf},
    ptr,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

type Callback =
    Option<unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int>;

// Windows supplies WinSQLite as part of the OS. Unix builds link the system SQLite library.
#[cfg_attr(windows, link(name = "winsqlite3", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "sqlite3"))]
extern "C" {
    fn sqlite3_open_v2(
        path: *const c_char,
        db: *mut *mut c_void,
        flags: c_int,
        vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_errmsg(db: *mut c_void) -> *const c_char;
    fn sqlite3_exec(
        db: *mut c_void,
        sql: *const c_char,
        cb: Callback,
        arg: *mut c_void,
        error: *mut *mut c_char,
    ) -> c_int;
}

struct Connection(*mut c_void);

impl Connection {
    fn open(path: &Path) -> Result<Self> {
        let path = CString::new(path.to_str().ok_or("SQLite export path must be UTF-8")?)?;
        let mut handle = ptr::null_mut();
        // SQLite takes a UTF-8 filename and initializes this handle, even on most failures.
        let rc = unsafe { sqlite3_open_v2(path.as_ptr(), &mut handle, 2, ptr::null()) }; // SQLITE_OPEN_READWRITE
        if handle.is_null() {
            return Err("SQLite could not allocate a connection".into());
        }
        let db = Self(handle);
        db.check(rc)?;
        Ok(db)
    }

    fn check(&self, code: c_int) -> Result<()> {
        if code != 0 {
            // SQLite owns the error text until the next API call on this open connection.
            let message = unsafe { CStr::from_ptr(sqlite3_errmsg(self.0)) }.to_string_lossy();
            return Err(format!("SQLite ({code}): {message}").into());
        }
        Ok(())
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let sql = CString::new(sql)?;
        // All export statements use fixed SQL and typed numeric episode fields.
        let code =
            unsafe { sqlite3_exec(self.0, sql.as_ptr(), None, ptr::null_mut(), ptr::null_mut()) };
        self.check(code)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // sqlite3_exec has already finalized its statements. Closing aborts an unfinished transaction.
        unsafe {
            sqlite3_close(self.0);
        }
    }
}

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn reserve_staging_file(destination: &Path) -> Result<PathBuf> {
    // A new name for every attempt avoids reusing the journal of an
    // interrupted SQLite transaction.
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    destination
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("SQLite export destination must have a UTF-8 filename")?;
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    for attempt in 0..64 {
        let stage = parent.join(format!(
            ".keyrook-stage-{}-{nanos}-{sequence}-{attempt}.sqlite",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
        {
            Ok(file) => {
                drop(file);
                return Ok(stage);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err("could not reserve a unique SQLite export staging file".into())
}

// The staged SQLite file is always a sibling of its destination, so a
// same-volume rename is possible on Windows even when the target filesystem
// (such as the PCP D: workspace) does not support hard links.
#[cfg(windows)]
fn publish_no_replace(stage: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(source: *const u16, target: *const u16, flags: u32) -> i32;
    }

    let stage = fs::canonicalize(stage)?;
    let destination = stage.parent().ok_or("missing staging directory")?
        .join(destination.file_name().ok_or("missing export filename")?);
    let source: Vec<u16> = stage.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    // MOVEFILE_WRITE_THROUGH=0x8; deliberately exclude REPLACE_EXISTING=0x1
    // and COPY_ALLOWED=0x2. See Microsoft Learn, MoveFileExW. The absent
    // flags make an existing target an error and prevent copy/delete fallback.
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

// On Unix a hard-link creation is atomic and fails if the final destination
// already exists. The caller unlinks the staging name on ordinary completion.
// Windows uses the no-replace move above and does not require hard-link support.
#[cfg(not(windows))]
fn publish_no_replace(stage: &Path, destination: &Path) -> Result<()> {
    fs::hard_link(stage, destination)?;
    Ok(())
}

pub fn export(db: &Database, destination: &Path) -> Result<()> {
    let snapshot = db.begin_read()?;
    let episodes = snapshot.open_table(session::EPISODES)?;
    let stats = snapshot.open_table(session::STATS)?;
    // The final destination never exists until SQLite COMMIT and close.
    // A process kill can leave an orphan staging DB and its journal. Retry
    // uses a fresh name rather than opening a possibly hot old journal.
    let staging = reserve_staging_file(destination)?;
    let result = (|| -> Result<()> {
        {
            let sql = Connection::open(&staging)?;
            sql.execute("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; BEGIN IMMEDIATE;
                PRAGMA user_version=1;
                CREATE TABLE episodes(bits INTEGER NOT NULL, seed INTEGER NOT NULL, depth INTEGER NOT NULL,
                    strategy INTEGER NOT NULL, verified INTEGER NOT NULL, work INTEGER NOT NULL,
                    nodes INTEGER NOT NULL, candidates INTEGER NOT NULL, PRIMARY KEY(bits,seed));
                CREATE TABLE stats(bits INTEGER NOT NULL, strategy INTEGER NOT NULL, runs INTEGER NOT NULL,
                    wins INTEGER NOT NULL, work INTEGER NOT NULL, next INTEGER NOT NULL, PRIMARY KEY(bits,strategy));")?;
            for row in episodes.iter()? {
                let (key, value) = row?;
                let (bits, seed) = key.value();
                let (depth, policy, verified, work, nodes, candidates) = value.value();
                sql.execute(&format!(
                    "INSERT INTO episodes VALUES({bits},{seed},{depth},{policy},{},{work},{nodes},{candidates})",
                    u8::from(verified)
                ))?;
            }
            for bits in [4, 6, 8] {
                let m = session::memory(&stats, bits)?;
                for i in 0..2 {
                    sql.execute(&format!(
                        "INSERT INTO stats VALUES({bits},{i},{},{},{},{})",
                        m.runs[i], m.wins[i], m.work[i], m.choose()
                    ))?;
                }
            }
            sql.execute("COMMIT")?;
        } // Connection dropped; no SQLite journal is needed for recovery.
        // Open with WRITE permission so FlushFileBuffers works on Windows.
        // Close the handle before renaming: Windows requires no open writer
        // handles at a move boundary.
        {
            let writable = fs::OpenOptions::new().write(true).open(&staging)?;
            writable.sync_all()?;
        }
        publish_no_replace(&staging, destination)?;
        Ok(())
    })();
    // Best-effort local cleanup. A sudden kill leaves an orphan, not a
    // falsely published destination; the unique next path enables retry.
    let _ = fs::remove_file(&staging);
    result
}


#[cfg(test)]
mod publication_tests {
    use super::*;

    #[test]
    fn existing_destination_is_not_replaced_by_a_staged_file() {
        let name = format!(
            "keyrook-no-replace-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let dir = std::env::temp_dir().join(name);
        fs::create_dir(&dir).unwrap();
        let staging = dir.join("stage.sqlite");
        let destination = dir.join("final.sqlite");
        fs::write(&staging, b"new snapshot").unwrap();
        fs::write(&destination, b"original existing file").unwrap();
        assert!(publish_no_replace(&staging, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original existing file");
        assert_eq!(fs::read(&staging).unwrap(), b"new snapshot");
        fs::remove_file(&destination).unwrap();
        publish_no_replace(&staging, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new snapshot");
        // On Unix the staging path is a second link; on Windows it was moved.
        if staging.exists() {
            fs::remove_file(&staging).unwrap();
        }
        fs::remove_file(&destination).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
