//! Conservative, test-only inspection of a single synthetic QEMU guest journal.
//! The marker files are not authenticated ACR events or a production store.
use std::{fs, io, path::Path};

pub(crate) enum Decision {
    Ready(usize),
    CompleteOrHeld,
    Attention(String),
}

pub(crate) fn inspect_journal(directory: &Path, request: &str) -> io::Result<Decision> {
    // Once a fault/ambiguity is recorded, a process restart does not silently
    // reactivate the request merely because the prior container exited.
    if fs::symlink_metadata(directory.join("attention.txt")).is_ok() {
        return Ok(Decision::CompleteOrHeld);
    }

    // Future attempts and out-of-band output from an unrecorded run must not
    // be ignored. First check the *entire* bounded namespace, not just the
    // first empty attempt.
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let raw = entry.file_name();
        let Some(name) = raw.to_str() else {
            return Ok(Decision::Attention("INVALID_JOURNAL: non-UTF8 file name".into()));
        };
        if !["start-", "end-", "qemu-"].iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        let recognised = (1..=3).any(|n| {
            name == format!("start-{n}.txt")
                || name == format!("end-{n}.txt")
                || name == format!("qemu-{n}.serial")
                || name == format!("qemu-{n}.stderr")
        });
        if !recognised || !entry.file_type()?.is_file() {
            return Ok(Decision::Attention(format!(
                "INVALID_JOURNAL: unexpected or non-regular reserved member {name}"
            )));
        }
        if entry.metadata()?.len() > 1024 * 1024 {
            return Ok(Decision::Attention(format!(
                "INVALID_JOURNAL: oversized reserved member {name}"
            )));
        }
    }

    let mut next = None;
    for n in 1..=3 {
        let start = directory.join(format!("start-{n}.txt"));
        let end = directory.join(format!("end-{n}.txt"));
        let start_exists = start.exists();
        let end_exists = end.exists();
        if !start_exists && !end_exists {
            if directory.join(format!("qemu-{n}.serial")).exists()
                || directory.join(format!("qemu-{n}.stderr")).exists()
            {
                return Ok(Decision::Attention(format!(
                    "INVALID_JOURNAL: orphan QEMU evidence without attempt {n}"
                )));
            }
            if next.is_none() {
                next = Some(n);
            }
            continue;
        }
        if next.is_some() {
            return Ok(Decision::Attention(format!(
                "INVALID_JOURNAL: noncontiguous attempt {n} after earlier empty slot"
            )));
        }
        if end_exists && !start_exists {
            return Ok(Decision::Attention("INVALID_JOURNAL: END exists without START".into()));
        }
        if start_exists && !end_exists {
            return Ok(Decision::Attention(format!(
                "INDETERMINATE: run {n} START without END; no automatic retry"
            )));
        }
        if fs::read_to_string(&start)? != format!("REQUEST={request};ATTEMPT={n};START\n")
            || fs::read_to_string(&end)? != format!("REQUEST={request};ATTEMPT={n};QEMU_EXIT=33;END\n")
        {
            return Ok(Decision::Attention("INVALID_JOURNAL: original entries incompatible".into()));
        }
    }
    Ok(match next {
        Some(n) => Decision::Ready(n),
        None => Decision::CompleteOrHeld,
    })
}

#[cfg(test)]
mod tests {
    use super::{inspect_journal, Decision};
    use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    struct Arena(PathBuf);
    impl Arena {
        fn new() -> Self {
            let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("d12-journal-{}-{seed}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn put(&self, name: &str, data: &str) {
            fs::write(self.0.join(name), data).unwrap();
        }
        fn check(&self) -> Decision { inspect_journal(&self.0, "unit-request").unwrap() }
    }
    impl Drop for Arena {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    fn completed(a: &Arena, n: usize) {
        a.put(&format!("start-{n}.txt"), &format!("REQUEST=unit-request;ATTEMPT={n};START\n"));
        a.put(&format!("end-{n}.txt"), &format!("REQUEST=unit-request;ATTEMPT={n};QEMU_EXIT=33;END\n"));
    }
    fn must_reject(d: Decision, needle: &str) {
        match d {
            Decision::Attention(reason) => assert!(reason.contains(needle), "{reason}"),
            _ => panic!("adversarial journal incorrectly allowed progress"),
        }
    }

    #[test]
    fn consecutive_completed_attempts_and_end_of_budget() {
        let a = Arena::new();
        assert!(matches!(a.check(), Decision::Ready(1)));
        completed(&a, 1);
        assert!(matches!(a.check(), Decision::Ready(2)));
        completed(&a, 2);
        assert!(matches!(a.check(), Decision::Ready(3)));
        completed(&a, 3);
        assert!(matches!(a.check(), Decision::CompleteOrHeld));
    }

    #[test]
    fn gap_and_future_marker_do_not_allow_duplicate_launch() {
        let a = Arena::new();
        completed(&a, 3);
        must_reject(a.check(), "noncontiguous");
        let b = Arena::new();
        completed(&b, 1);
        completed(&b, 3);
        must_reject(b.check(), "noncontiguous");
        let c = Arena::new();
        c.put("start-4.txt", "future");
        must_reject(c.check(), "unexpected");
    }

    #[test]
    fn partial_missing_and_invalid_orphaned_evidence_are_not_completed() {
        let a = Arena::new();
        a.put("start-1.txt", "REQUEST=unit-request;ATTEMPT=1;START\n");
        must_reject(a.check(), "INDETERMINATE");
        let b = Arena::new();
        b.put("end-1.txt", "END");
        must_reject(b.check(), "END exists without START");
        let c = Arena::new();
        c.put("qemu-2.serial", "orphan output");
        must_reject(c.check(), "orphan");
        let d = Arena::new();
        completed(&d, 1);
        d.put("end-1.txt", "REQUEST=someone-else;ATTEMPT=1;QEMU_EXIT=33;END\n");
        must_reject(d.check(), "incompatible");
    }

    #[test]
    fn attention_fences_rerun() {
        let a = Arena::new();
        a.put("attention.txt", "UNCONFIRMED prior result");
        assert!(matches!(a.check(), Decision::CompleteOrHeld));
    }
}
