//! Durable admission and closure of bounded recursive task trees.
use recursion_desk_core::Desk;
pub use recursion_desk_core::{Node, State};
use redb::{
    Database, DatabaseError, ReadableDatabase, ReadableTable, TableDefinition, TableHandle,
};
use std::{
    error::Error,
    path::Path,
    thread,
    time::{Duration, Instant},
};

type Stored<T> = Result<T, Box<dyn Error>>;
type Limits = (u16, u16, u32);
type Event<'a> = (u8, u32, &'a str, u32);
const CONFIG: TableDefinition<u8, Limits> = TableDefinition::new("parrots.config.v1");
const EVENTS: TableDefinition<u64, Event<'static>> = TableDefinition::new("parrots.events.v1");
const REQUESTS: TableDefinition<&str, u64> = TableDefinition::new("parrots.requests.v1");
const OPEN: u8 = 1;
const FINISH: u8 = 2;
const CANCEL: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub node: u32,
    pub revision: u64,
    pub replayed: bool,
}

#[derive(Debug)]
pub struct Snapshot {
    pub max_depth: u16,
    pub max_live: u16,
    pub max_total: u32,
    pub revision: u64,
    pub nodes: Vec<Node>,
}

fn database(path: &Path, create: bool) -> Stored<Database> {
    let start = Instant::now();
    loop {
        let result = if create {
            Database::create(path)
        } else {
            Database::open(path)
        };
        match result {
            Ok(db) => return Ok(db),
            Err(DatabaseError::DatabaseAlreadyOpen) if start.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn initialize(path: &Path, limits: Limits) -> Stored<()> {
    Desk::new(limits.0, limits.1, limits.2)?;
    let db = database(path, true)?;
    let tx = db.begin_write()?;
    let existing: Vec<_> = tx.list_tables()?.map(|t| t.name().to_owned()).collect();
    if !existing.is_empty() && !existing.iter().any(|n| n == CONFIG.name()) {
        return Err("file contains a different redb database".into());
    }
    let old = tx.open_table(CONFIG)?.get(1)?.map(|r| r.value());
    match old {
        Some(old) if old == limits => tx.abort()?,
        Some(_) => return Err("desk already exists with different limits".into()),
        None => {
            tx.open_table(CONFIG)?.insert(1, limits)?;
            tx.open_table(EVENTS)?;
            tx.open_table(REQUESTS)?;
            tx.commit()?;
        }
    }
    Ok(())
}

pub fn init(path: &Path, max_depth: u16, max_live: u16, max_total: u32) -> Result<(), String> {
    initialize(path, (max_depth, max_live, max_total)).map_err(|e| e.to_string())
}

fn apply(desk: &mut Desk, op: u8, arg: u32, label: &str) -> Result<u32, String> {
    match op {
        OPEN => desk.open(if arg == 0 { None } else { Some(arg) }, label),
        FINISH => {
            desk.finish(arg)?;
            Ok(arg)
        }
        CANCEL => {
            desk.cancel(arg)?;
            Ok(arg)
        }
        _ => Err("unrecognized event".into()),
    }
}

fn replay(limits: Limits, events: &impl ReadableTable<u64, Event<'static>>) -> Stored<(Desk, u64)> {
    let mut desk = Desk::new(limits.0, limits.1, limits.2)?;
    let mut head = 0;
    for item in events.iter()? {
        let (revision, event) = item?;
        head += 1;
        let (op, arg, label, node) = event.value();
        if revision.value() != head || apply(&mut desk, op, arg, label)? != node {
            return Err("invalid desk event sequence".into());
        }
    }
    Ok((desk, head))
}

fn write(path: &Path, request: &str, op: u8, arg: u32, label: &str) -> Stored<Receipt> {
    if request.trim().is_empty() || request.len() > 256 {
        return Err("request ID must contain text and fit in 256 UTF-8 bytes".into());
    }
    let db = database(path, false)?;
    let tx = db.begin_write()?;
    let limits = tx
        .open_table(CONFIG)?
        .get(1)?
        .ok_or("uninitialized desk")?
        .value();
    let receipt;
    {
        let mut events = tx.open_table(EVENTS)?;
        let mut requests = tx.open_table(REQUESTS)?;
        let prior = requests.get(request)?.map(|r| r.value());
        if let Some(revision) = prior {
            let row = events
                .get(revision)?
                .ok_or("request refers to a missing event")?;
            let (old_op, old_arg, old_label, node) = row.value();
            if (old_op, old_arg, old_label) != (op, arg, label) {
                return Err("request ID already belongs to a different operation".into());
            }
            return Ok(Receipt {
                node,
                revision,
                replayed: true,
            });
        }
        let (mut desk, head) = replay(limits, &events)?;
        let node = apply(&mut desk, op, arg, label)?;
        let revision = head + 1;
        events.insert(revision, (op, arg, label, node))?;
        requests.insert(request, revision)?;
        receipt = Receipt {
            node,
            revision,
            replayed: false,
        };
    }
    tx.commit()?;
    Ok(receipt)
}

pub fn admit(
    path: &Path,
    request: &str,
    parent: Option<u32>,
    label: &str,
) -> Result<Receipt, String> {
    if parent == Some(0) {
        return Err("parent node IDs start at one".into());
    }
    write(path, request, OPEN, parent.unwrap_or(0), label).map_err(|e| e.to_string())
}

pub fn finish(path: &Path, request: &str, node: u32) -> Result<Receipt, String> {
    write(path, request, FINISH, node, "").map_err(|e| e.to_string())
}

pub fn cancel(path: &Path, request: &str, node: u32) -> Result<Receipt, String> {
    write(path, request, CANCEL, node, "").map_err(|e| e.to_string())
}

fn snapshot(path: &Path) -> Stored<Snapshot> {
    let db = database(path, false)?;
    let tx = db.begin_read()?;
    let (max_depth, max_live, max_total) = tx
        .open_table(CONFIG)?
        .get(1)?
        .ok_or("uninitialized desk")?
        .value();
    let (desk, revision) = replay((max_depth, max_live, max_total), &tx.open_table(EVENTS)?)?;
    Ok(Snapshot {
        max_depth,
        max_live,
        max_total,
        revision,
        nodes: desk.nodes().to_vec(),
    })
}

pub fn status(path: &Path) -> Result<Snapshot, String> {
    snapshot(path).map_err(|e| e.to_string())
}

pub fn get(path: &Path, node: u32) -> Result<Node, String> {
    let i = node.checked_sub(1).ok_or("unknown node")? as usize;
    status(path)?
        .nodes
        .get(i)
        .cloned()
        .ok_or_else(|| "unknown node".into())
}
