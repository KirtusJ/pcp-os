//! Which of the loader's files is a worker that runs only beside another program.
//!
//! The loader's line can name one program by digest as the one a worker is for (`worker_for=`) and
//! the worker by its digest (`worker_image=`). When the first is started, the kernel starts the
//! worker beside it with the console and the channel between them and nothing else. Such a worker
//! is not something a person starts: alone it has nobody to talk to. So it is not put in the list
//! of programs. It remains among the files the loader handed over, with its name, size and digest,
//! where Details shows it.
//!
//! No I/O here, so that the rule is tested on a host.

fn option<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace().find_map(|w| w.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

/// Whether the file with this digest is a worker that will be started beside another loaded
/// program, and so is not listed as a program itself. All of these must hold:
/// the loader's line was taken whole (`line` is `None` where it was cut, and then nothing it says
/// is relied on: the file stays listed); the line names this digest as the worker; it names a
/// different digest as the program the worker is for; and a file with that digest was loaded.
/// Where the program it is for is absent, the worker is an ordinary file and stays listed: hiding
/// it would leave something the loader handed over that no screen of programs accounts for.
pub fn runs_only_beside(line: Option<&str>, digest: &str, loaded: &[&str]) -> bool {
    let line = match line {
        Some(line) => line,
        None => return false,
    };
    match (option(line, "worker_for"), option(line, "worker_image")) {
        (Some(front), Some(worker)) => worker == digest && front != digest && loaded.iter().any(|d| *d == front),
        _ => false,
    }
}
