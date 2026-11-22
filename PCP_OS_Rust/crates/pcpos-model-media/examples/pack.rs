//! Host helper: copy a publisher-pinned model into 64 MiB shards and write the manifest last.
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::PathBuf};
use pcpos_model_media::{Manifest, MAGIC, MAX_MODEL_BYTES, MAX_SHARD_BYTES};
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args=std::env::args_os().skip(1);
    let source=PathBuf::from(args.next().ok_or("usage: pack MODEL EXPECTED_SHA256 NEW_DIRECTORY")?);
    let expected=args.next().ok_or("missing publisher SHA-256")?.into_string().map_err(|_| "non-UTF-8 digest")?;
    if expected.len()!=64 || !expected.bytes().all(|c| c.is_ascii_hexdigit()) { return Err("invalid SHA-256".into()); }
    let expected=expected.to_ascii_lowercase();
    let destination=PathBuf::from(args.next().ok_or("missing new output directory")?);
    if args.next().is_some() { return Err("unexpected argument".into()); }
    let mut input=File::open(source)?;
    let total=input.metadata()?.len();
    if total==0 || total>MAX_MODEL_BYTES { return Err("model size exceeds policy or is empty".into()); }
    let count=total.div_ceil(MAX_SHARD_BYTES as u64) as u32;
    fs::create_dir(&destination)?; // Existing destinations are never overwritten.
    let mut whole=Sha256::new();
    let mut digests=Vec::new();
    let mut copied=0u64;
    let mut buffer=[0u8;64*1024];
    for ordinal in 0..count {
        let mut shard=OpenOptions::new().write(true).create_new(true)
            .open(destination.join(format!("shard-{ordinal:05}.bin")))?;
        let length=(total-copied).min(MAX_SHARD_BYTES as u64);
        let mut written=0u64;
        let mut hash=Sha256::new();
        while written<length {
            let wanted=(length-written).min(buffer.len() as u64) as usize;
            input.read_exact(&mut buffer[..wanted])?;
            shard.write_all(&buffer[..wanted])?;
            whole.update(&buffer[..wanted]); hash.update(&buffer[..wanted]);
            written+=wanted as u64; copied+=wanted as u64;
        }
        shard.sync_all()?;
        digests.push(<[u8;32]>::from(hash.finalize()));
    }
    let mut extra=[0u8;1];
    if input.read(&mut extra)? != 0 { return Err("source grew during packing; no manifest published".into()); }
    let model_digest: [u8;32]=whole.finalize().into();
    if hex(&model_digest)!=expected { return Err("publisher digest mismatch; no manifest published".into()); }
    let mut encoded=MAGIC.to_vec();
    encoded.extend_from_slice(&total.to_le_bytes());
    encoded.extend_from_slice(&MAX_SHARD_BYTES.to_le_bytes());
    encoded.extend_from_slice(&count.to_le_bytes());
    encoded.extend_from_slice(&model_digest);
    for digest in digests { encoded.extend_from_slice(&digest); }
    let manifest_digest: [u8;32]=Sha256::digest(&encoded).into();
    Manifest::parse(&encoded,manifest_digest).map_err(|e| format!("manifest self-check: {e:?}"))?;
    let mut manifest=OpenOptions::new().write(true).create_new(true).open(destination.join("model.manifest"))?;
    manifest.write_all(&encoded)?; manifest.sync_all()?;
    println!("model_bytes={total} shards={count} model_sha256={} manifest_sha256={}",hex(&model_digest),hex(&manifest_digest));
    Ok(())
}
