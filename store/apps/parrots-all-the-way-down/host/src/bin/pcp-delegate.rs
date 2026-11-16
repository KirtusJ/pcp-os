//! Admit a real worker before spawning it. The adapter never evaluates shell text.
use parrots_all_the_way_down::{admit, cancel, finish, get, State};
use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, Output},
    thread,
    time::Duration,
};

const FILE_VAR: &str = "PCP_RECURSION_DESK";
const PARENT_VAR: &str = "PCP_RECURSION_PARENT";
const HELP: &str = "Parrots all the way Down\npcp-delegate FILE REQUEST LABEL -- PROGRAM [ARG ...]\npcp-delegate --register REGISTER ACTOR LABEL -- PROGRAM [ARG ...]\nNested workers can use - to inherit the current file.";
const REG_VAR: &str = "PCP_DELEGATE_REGISTER";
const REG_PARENT: &str = "PCP_DELEGATE_TASK";
const REG_ACTOR: &str = "PCP_DELEGATE_ACTOR";

fn host_failure(reply: &Output) -> String {
    // Guard refusals are protocol replies on stdout. Invocation and I/O errors use stderr.
    [&reply.stdout, &reply.stderr]
        .into_iter()
        .map(|bytes| {
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
            let suffix = if bytes.len() > 1024 { "…" } else { "" };
            format!("{}{suffix}", text.trim())
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn register_run(args: &[OsString]) -> Result<u8, String> {
    let [file, actor, label, sep, program, rest @ ..] = args else {
        return Err(HELP.into());
    };
    if sep != "--" {
        return Err(HELP.into());
    }
    if env::var_os(FILE_VAR).is_some() || env::var_os(PARENT_VAR).is_some() {
        return Err("a nested worker must use its existing desk".into());
    }
    let inherited = env::var_os(REG_VAR);
    let parent = env::var_os(REG_PARENT);
    let inherited_actor = env::var_os(REG_ACTOR);
    let path = if file == "-" {
        PathBuf::from(inherited.as_ref().ok_or("no inherited register")?)
    } else {
        PathBuf::from(file)
    };
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    match (&inherited, &parent, &inherited_actor) {
        (None, None, None) => (),
        (Some(old), Some(_), Some(who))
            if Path::new(old).canonicalize().map_err(|e| e.to_string())? == path
                && who == actor =>
        {
            ()
        }
        _ => return Err("inconsistent inherited register, task or actor".into()),
    }
    let host = env::var_os("PCPOS_BIN").unwrap_or_else(|| "pcpos".into());
    let mut admission = Command::new(&host);
    admission
        .arg("task-admit")
        .arg(&path)
        .arg("--by")
        .arg(actor)
        .arg("--label")
        .arg(label);
    if let Some(id) = parent {
        admission.arg("--parent").arg(id);
    }
    let reply = admission
        .output()
        .map_err(|e| format!("start PCP host: {e}"))?;
    if !reply.status.success() {
        return Err(format!("admission refused: {}", host_failure(&reply)));
    }
    let text = std::str::from_utf8(&reply.stdout).map_err(|_| "unreadable admission receipt")?;
    let id = text
        .lines()
        .next()
        .and_then(|s| s.strip_prefix("ADMITTED "))
        .filter(|v| {
            v.strip_prefix('T')
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .ok_or("ambiguous admission receipt; inspect the register before retrying")?;
    let spawned = Command::new(program)
        .args(rest)
        .env(REG_VAR, &path)
        .env(REG_PARENT, id)
        .env(REG_ACTOR, actor)
        .spawn();
    let exit = match spawned {
        Ok(mut child) => Ok(child.wait().map_err(|e| {
            format!("cannot establish worker exit for {id}: {e}; admission retained")
        })?),
        Err(e) => Err(format!("could not start worker for {id}: {e}")),
    };
    let release = Command::new(host)
        .arg("task-release")
        .arg(&path)
        .arg("--by")
        .arg(actor)
        .arg("--id")
        .arg(id)
        .output()
        .map_err(|e| format!("inspect register task {id}; release failed: {e}"))?;
    if !release.status.success() {
        return Err(format!(
            "inspect register task {id}; release refused: {}",
            host_failure(&release)
        ));
    }
    let expected = format!("RELEASED {id}");
    if std::str::from_utf8(&release.stdout)
        .ok()
        .and_then(|s| s.lines().next())
        != Some(expected.as_str())
    {
        return Err(format!(
            "ambiguous release receipt for {id}; inspect the register"
        ));
    }
    let status = exit?;
    Ok(status
        .code()
        .and_then(|n| u8::try_from(n).ok())
        .unwrap_or(1))
}

fn inherited_desk(file: &OsStr) -> Result<(PathBuf, Option<u32>), String> {
    let inherited = env::var_os(FILE_VAR);
    let parent = env::var_os(PARENT_VAR);
    let input = if file == "-" {
        inherited.as_deref().ok_or("no inherited desk")?
    } else {
        file
    };
    let path = Path::new(input)
        .canonicalize()
        .map_err(|e| format!("open desk: {e}"))?;
    match (inherited, parent) {
        (None, None) => Ok((path, None)),
        (Some(old), Some(id)) => {
            if Path::new(&old).canonicalize().map_err(|e| e.to_string())? != path {
                return Err("nested dispatch must use the inherited desk".into());
            }
            let parent = id
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|v| *v > 0)
                .ok_or("invalid inherited parent ID")?;
            Ok((path, Some(parent)))
        }
        _ => Err("incomplete parent context".into()),
    }
}

fn stop_child(child: &mut Child) -> Result<(), String> {
    if child.try_wait().map_err(|e| e.to_string())?.is_none() {
        // The child may exit between try_wait and kill; wait still reaps it.
        if let Err(e) = child.kill() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err(e.to_string());
            }
        }
    }
    child.wait().map(|_| ()).map_err(|e| e.to_string())
}

// A nested copy of this adapter owns another process. If we kill the adapter while
// it handles the same cascade, that worker loses its supervisor. Resolve ordinary
// command names as well as explicitly supplied executable paths.
fn is_this_adapter(program: &OsStr) -> bool {
    let Ok(me) = env::current_exe().and_then(|p| p.canonicalize()) else {
        return false;
    };
    let mut name = PathBuf::from(program);
    if cfg!(windows) && name.extension().is_none() {
        name.set_extension("exe");
    }
    if name.components().count() != 1 {
        return name.canonicalize().ok().as_ref() == Some(&me);
    }
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Some(dir) = me.parent() {
            dirs.push(dir.to_owned());
        }
        if let Ok(cwd) = env::current_dir() {
            dirs.push(cwd);
        }
    }
    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path));
    }
    dirs.into_iter()
        .map(|dir| dir.join(&name))
        .find(|p| p.is_file())
        .and_then(|p| p.canonicalize().ok())
        .as_ref()
        == Some(&me)
}

fn supervise(
    path: &Path,
    node: u32,
    child: &mut Child,
    nested_adapter: bool,
) -> Result<u8, String> {
    let close = format!("dispatch-end/{node}");
    loop {
        let state = get(path, node)?.state;
        if state != State::Active {
            if nested_adapter {
                // The committed cancellation is visible to every descendant. Each
                // nested adapter reaps its own worker before returning to its caller.
                child
                    .wait()
                    .map_err(|e| format!("wait for nested adapter: {e}"))?;
            }
            return Err(format!("node {node} closed externally"));
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                finish(path, &close, node)?;
                return Ok(0);
            }
            cancel(path, &close, node)?;
            return Ok(status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .filter(|v| *v != 0)
                .unwrap_or(1));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn run(args: &[OsString]) -> Result<u8, String> {
    if args.is_empty() || args == [OsString::from("--help")] {
        println!("{HELP}");
        return Ok(0);
    }
    if args[0] == "--register" {
        return register_run(&args[1..]);
    }
    if [REG_VAR, REG_PARENT, REG_ACTOR]
        .iter()
        .any(|key| env::var_os(key).is_some())
    {
        return Err("a nested register worker must dispatch through --register".into());
    }
    let [file, request, label, sep, program, rest @ ..] = args else {
        return Err(HELP.into());
    };
    if sep != "--" {
        return Err(HELP.into());
    }
    let request = request
        .to_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or("invalid request ID")?;
    let label = label.to_str().ok_or("invalid label")?;
    let (path, parent) = inherited_desk(file)?;
    let receipt = admit(&path, &format!("dispatch/{request}"), parent, label)?;
    if receipt.replayed {
        return Err(format!(
            "request already admitted as node {}; inspect its state before retrying",
            receipt.node
        ));
    }
    let node = receipt.node;
    let mut cmd = Command::new(program);
    cmd.args(rest)
        .env(FILE_VAR, &path)
        .env(PARENT_VAR, node.to_string());
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            cancel(&path, &format!("dispatch-abort/{node}"), node)?;
            return Err(format!("could not start worker for node {node}: {e}"));
        }
    };
    match supervise(&path, node, &mut child, is_this_adapter(program)) {
        Ok(code) => Ok(code),
        Err(cause) => {
            stop_child(&mut child).map_err(|e| format!("{cause}; unable to reap worker: {e}"))?;
            if get(&path, node)?.state == State::Active {
                cancel(&path, &format!("dispatch-abort/{node}"), node)?;
            }
            Err(cause)
        }
    }
}

fn main() -> ExitCode {
    match run(&env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
