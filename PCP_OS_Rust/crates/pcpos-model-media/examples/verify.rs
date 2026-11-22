//! Host reference for the same verified loader the native worker will call.
use std::{fs::{self, File}, io::{Read, Seek, SeekFrom}, path::PathBuf};
use pcpos_model_media::{Manifest, ReadAt, VerifiedModel, HEADER_BYTES, MAX_SHARDS};

struct Source(File);
impl ReadAt for Source {
    type Error = std::io::Error;
    fn len(&mut self) -> Result<u64, Self::Error> { Ok(self.0.metadata()?.len()) }
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.read(buffer)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args=std::env::args_os().skip(1);
    let model=PathBuf::from(args.next().ok_or("usage: verify MODEL MANIFEST TRUSTED_MANIFEST_SHA256")?);
    let path=PathBuf::from(args.next().ok_or("missing manifest path")?);
    let pin=args.next().ok_or("missing trusted manifest SHA-256")?.into_string().map_err(|_| "non-UTF-8 digest")?;
    if args.next().is_some() { return Err("unexpected argument".into()); }
    if pin.len()!=64 || !pin.bytes().all(|c| c.is_ascii_hexdigit()) { return Err("invalid SHA-256".into()); }
    let mut digest=[0u8;32];
    for (i, value) in digest.iter_mut().enumerate() { *value=u8::from_str_radix(&pin[i*2..i*2+2],16)?; }
    // Bound the read itself, including a file that grows after metadata inspection.
    let maximum=HEADER_BYTES+MAX_SHARDS as usize*32;
    if fs::metadata(&path)?.len()>maximum as u64 { return Err("manifest exceeds policy".into()); }
    let mut encoded=Vec::new();
    File::open(path)?.take(maximum as u64+1).read_to_end(&mut encoded)?;
    let manifest=Manifest::parse(&encoded,digest).map_err(|e| format!("manifest: {e:?}"))?;
    let mut source=Source(File::open(model)?);
    let began=std::time::Instant::now();
    let verified=VerifiedModel::load(&manifest,&mut source,|_| true).map_err(|e| format!("load: {e:?}"))?;
    let mut magic=[0u8;4];
    let prefix=&mut magic[..verified.len().min(4) as usize];
    verified.copy_into(0,prefix).map_err(|_| "cannot read model prefix")?;
    println!("verified_bytes={} shards={} first_bytes={prefix:02x?} elapsed_seconds={:.3}",
             verified.len(),manifest.shard_count(),began.elapsed().as_secs_f64());
    Ok(())
}
