use super::Result;
use keyrook_core::{run_episode, Episode, Memory};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use std::{fs, path::Path};

pub const FILE_NAME: &str = "keyrook.redb";
const VERSION: TableDefinition<u8, u8> = TableDefinition::new("keyrook_schema");
// depth, strategy, verified, work, nodes, candidates
pub type Outcome = (u8, u8, bool, u64, u32, u32);
pub const EPISODES: TableDefinition<(u8, u32), Outcome> = TableDefinition::new("episodes");
type Counters = (u32, u32, u64);
pub const STATS: TableDefinition<(u8, u8), Counters> = TableDefinition::new("strategy_stats");

pub fn memory(table: &impl ReadableTable<(u8, u8), Counters>, bits: u8) -> Result<Memory> {
    let mut m = Memory::default();
    for policy in 0..2u8 {
        if let Some(row) = table.get((bits, policy))? {
            let i = usize::from(policy);
            (m.runs[i], m.wins[i], m.work[i]) = row.value();
        }
    }
    Ok(m)
}

// Policy selection, seed uniqueness and both writes belong to the same transaction.
fn record(
    tx: &WriteTransaction,
    bits: u8,
    seed: u32,
    depth: u8,
    policy: Option<u8>,
) -> Result<Episode> {
    let mut episodes = tx.open_table(EPISODES)?;
    if episodes.get((bits, seed))?.is_some() {
        return Err("seed already trained at this difficulty".into());
    }
    let mut stats = tx.open_table(STATS)?;
    let mut m = memory(&stats, bits)?;
    let policy = policy.unwrap_or_else(|| m.choose());
    let out = run_episode(bits, seed, depth, policy);
    episodes.insert(
        (bits, seed),
        (
            depth,
            policy,
            out.verified,
            out.work,
            out.branches,
            out.candidates,
        ),
    )?;
    m.observe(&out);
    let i = usize::from(policy);
    stats.insert((bits, policy), (m.runs[i], m.wins[i], m.work[i]))?;
    Ok(out)
}

pub fn train(db: &Database, bits: u8, seed: u32, depth: u8) -> Result<Episode> {
    let tx = db.begin_write()?;
    let out = record(&tx, bits, seed, depth, None)?;
    tx.commit()?;
    Ok(out)
}

pub fn stats(db: &Database, bits: u8) -> Result<Memory> {
    memory(&db.begin_read()?.open_table(STATS)?, bits)
}

fn migrate_directory(tx: &WriteTransaction, dir: &Path, bits: u8) -> Result<()> {
    let legacy = dir.join(format!("v1-{bits}"));
    if !legacy.try_exists()? {
        return Ok(());
    }
    for entry in fs::read_dir(legacy)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(seed) = name.to_str().and_then(|s| s.strip_prefix("episode-")) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            return Err("invalid legacy episode directory".into());
        }
        let seed = seed.parse()?;
        let decision = fs::read_to_string(entry.path().join("decision"))?;
        let (depth, policy) = decision
            .trim()
            .split_once(' ')
            .ok_or("invalid legacy decision")?;
        let policy = policy.parse()?;
        if policy > 1 {
            return Err("invalid legacy strategy".into());
        }
        record(tx, bits, seed, super::depth(depth)?, Some(policy))?;
    }
    Ok(())
}

pub fn open(dir: &Path) -> Result<Database> {
    fs::create_dir_all(dir)?;
    let db = Database::create(dir.join(FILE_NAME))?;
    let tx = db.begin_write()?;
    let version = tx.open_table(VERSION)?.get(0)?.map(|v| v.value());
    match version {
        Some(1) => tx.abort()?,
        Some(_) => return Err("unsupported Keyrook session version".into()),
        None => {
            tx.open_table(EPISODES)?;
            tx.open_table(STATS)?;
            for bits in [4, 6, 8] {
                migrate_directory(&tx, dir, bits)?;
            }
            tx.open_table(VERSION)?.insert(0, 1)?;
            tx.commit()?;
        }
    }
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_uncommitted_training_episode_cannot_change_the_next_policy() {
        let dir = std::env::temp_dir().join(format!("keyrook-rollback-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let db = open(&dir).unwrap();
        train(&db, 4, 42, 2).unwrap();
        let tx = db.begin_write().unwrap();
        assert_eq!(record(&tx, 4, 7, 1, None).unwrap().strategy, 1);
        drop(tx);
        drop(db);
        let db = open(&dir).unwrap();
        assert_eq!(stats(&db, 4).unwrap().runs, [1, 0]);
        assert_eq!(train(&db, 4, 7, 1).unwrap().strategy, 1);
        assert!(train(&db, 4, 7, 1).is_err());
        drop(db);
        fs::remove_file(dir.join(FILE_NAME)).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
