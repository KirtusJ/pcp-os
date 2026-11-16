// Isolated, bounded Linux OCI probe. Not PCP OS, a model or a production service.
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    process::{self, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

unsafe extern "C" {
    fn setsid() -> i32;
}

fn touch(path: &str, content: &[u8]) -> io::Result<()> {
    fs::write(path, content)
}

fn heartbeat(path: &str, duration: Duration, crash_marker: Option<&str>) -> io::Result<()> {
    let start = Instant::now();
    while start.elapsed() < duration && !Path::new("/state/release").exists() {
        if crash_marker.is_some_and(|marker| Path::new(marker).exists()) {
            // Explicit controller fault injection, never a production control.
            process::abort();
        }
        if path == "/state/grandchild.tick" && Path::new("/state/grandchild-quiet").exists() {
            // Negative oracle: a live worker can be silent without having exited.
            thread::sleep(Duration::from_millis(75));
            continue;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(b"x")?;
        file.sync_data()?;
        thread::sleep(Duration::from_millis(75));
    }
    Ok(())
}

fn run(mode: &str) -> io::Result<i32> {
    match mode {
        "grandchild" => {
            // A session/group escape from the parent is deliberately attempted.
            // The OCI runtime boundary, not the process group, is under test.
            let result = unsafe { setsid() };
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
            touch("/state/grandchild-setsid", b"true")?;
            heartbeat("/state/grandchild.tick", Duration::from_secs(40), None)?;
            Ok(0)
        }
        "parent" => {
            let _child = Command::new("/worker")
                .arg("grandchild")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            touch("/state/parent-start", b"true")?;
            heartbeat("/state/parent.tick", Duration::from_secs(40), Some("/state/parent-crash"))?;
            Ok(0)
        }
        "restart" => {
            if !Path::new("/state/restart-first").exists() {
                touch("/state/restart-first", b"first boot; deliberate nonzero exit")?;
                // The Docker restart manager only considers a container
                // successfully started after it has remained up for 10 seconds.
                heartbeat("/state/restart-first.tick", Duration::from_millis(11_500), None)?;
                Ok(23)
            } else {
                touch("/state/restart-second", b"second process after runtime restart")?;
                heartbeat("/state/restart-second.tick", Duration::from_secs(40), None)?;
                Ok(0)
            }
        }
        _ => Ok(2),
    }
}

fn main() {
    let code = match env::args().nth(1).as_deref() {
        Some(mode) => run(mode).unwrap_or_else(|err| {
            eprintln!("test worker error: {err}");
            3
        }),
        None => 2,
    };
    process::exit(code);
}
