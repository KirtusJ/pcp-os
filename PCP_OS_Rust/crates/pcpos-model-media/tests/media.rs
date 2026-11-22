use pcpos_model_media::{LoadError, Manifest, ManifestError, ReadAt, VerifiedModel, HEADER_BYTES, MAGIC};
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> [u8;32] { Sha256::digest(bytes).into() }
fn manifest(bytes: &[u8]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(&1024u32.to_le_bytes());
    out.extend_from_slice(&(bytes.len().div_ceil(1024) as u32).to_le_bytes());
    out.extend_from_slice(&digest(bytes));
    for shard in bytes.chunks(1024) { out.extend_from_slice(&digest(shard)); }
    out
}
struct Source { bytes: Vec<u8>, maximum_read: usize, declared: u64, reads: usize, failure: bool, overreport: bool }
impl Source {
    fn new(bytes: &[u8]) -> Self {
        Self { bytes: bytes.to_vec(), maximum_read: usize::MAX, declared: bytes.len() as u64,
               reads: 0, failure: false, overreport: false }
    }
}
impl ReadAt for Source {
    type Error = &'static str;
    fn len(&mut self) -> Result<u64, Self::Error> { Ok(self.declared) }
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, Self::Error> {
        self.reads += 1;
        if self.failure { return Err("device failed"); }
        if self.overreport { return Ok(out.len()+1); }
        let at = offset as usize;
        if at >= self.bytes.len() { return Ok(0); }
        let count = (self.bytes.len()-at).min(out.len()).min(self.maximum_read);
        out[..count].copy_from_slice(&self.bytes[at..at+count]); Ok(count)
    }
}

#[test]
fn partial_reads_and_cross_shard_ranges_preserve_exact_bytes() {
    let bytes: Vec<u8> = (0..2103).map(|i| (i%251) as u8).collect();
    let encoded = manifest(&bytes);
    let description = Manifest::parse(&encoded, digest(&encoded)).unwrap();
    let mut source = Source::new(&bytes); source.maximum_read=7;
    let mut progress = Vec::new();
    let model = VerifiedModel::load(&description, &mut source, |p| { progress.push(p); true }).unwrap();
    assert_eq!(model.len(), 2103);
    assert_eq!(model.digest(), digest(&bytes));
    assert_eq!(progress.last().unwrap().verified_shards, 3);
    assert!(progress.windows(2).all(|p| p[0].bytes_read <= p[1].bytes_read));
    source.bytes.fill(0); // Later device mutation cannot replace verified owned bytes.
    let mut out = [0; 1100]; model.copy_into(1000, &mut out).unwrap();
    assert_eq!(&out, &bytes[1000..2100]);
    let mut unchanged = [0xa5; 4];
    assert!(model.copy_into(2100, &mut unchanged).is_err()); assert_eq!(unchanged, [0xa5;4]);
    assert!(model.copy_into(u64::MAX, &mut unchanged).is_err()); assert_eq!(unchanged, [0xa5;4]);
    assert!(model.copy_into(2103, &mut []).is_ok());
    assert!(model.copy_into(2104, &mut []).is_err());
}

#[test]
fn manifest_is_bound_to_an_independent_digest_before_use() {
    let mut encoded = manifest(&[1; 2048]);
    let pin = digest(&encoded);
    encoded[HEADER_BYTES] ^= 1;
    assert_eq!(Manifest::parse(&encoded, pin).unwrap_err(), ManifestError::Digest);
    assert!(Manifest::parse(&encoded, digest(&encoded)).is_ok()); // New pin is a distinct trust decision.
}

#[test]
fn full_model_and_each_shard_are_both_checked() {
    let bytes = vec![3; 2049];
    let mut encoded = manifest(&bytes);
    encoded[24] ^= 1; // All shard hashes correct, but the complete-model pin is wrong.
    let description = Manifest::parse(&encoded, digest(&encoded)).unwrap();
    assert_eq!(VerifiedModel::load(&description, &mut Source::new(&bytes), |_| true).unwrap_err(), LoadError::ModelDigest);
    let encoded = manifest(&bytes);
    let description = Manifest::parse(&encoded, digest(&encoded)).unwrap();
    let mut source = Source::new(&bytes); source.bytes[1024] ^= 1;
    assert_eq!(VerifiedModel::load(&description, &mut source, |_| true).unwrap_err(), LoadError::ShardDigest { shard: 1 });
}

#[test]
fn shuffled_shards_do_not_produce_a_verified_model() {
    let bytes: Vec<u8> = [vec![1;1024], vec![2;1024]].concat();
    let encoded = manifest(&bytes);
    let description = Manifest::parse(&encoded, digest(&encoded)).unwrap();
    let mut source = Source::new(&bytes); source.bytes.rotate_left(1024);
    assert_eq!(VerifiedModel::load(&description, &mut source, |_| true).unwrap_err(), LoadError::ShardDigest { shard: 0 });
}

#[test]
fn wrong_size_short_read_extra_bytes_and_device_errors_remain_distinct() {
    let encoded = manifest(&[1; 2048]);
    let description = Manifest::parse(&encoded, digest(&encoded)).unwrap();
    let mut source = Source::new(&[1;2048]); source.declared=2047;
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| true).unwrap_err(),LoadError::Length);
    let mut source = Source::new(&[1;2047]); source.declared=2048;
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| true).unwrap_err(),LoadError::ShortRead);
    let mut source = Source::new(&[1;2049]); source.declared=2048;
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| true).unwrap_err(),LoadError::Length);
    let mut source = Source::new(&[1;2048]); source.failure=true;
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| true).unwrap_err(),LoadError::Source("device failed"));
    let mut source = Source::new(&[1;2048]); source.overreport=true;
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| true).unwrap_err(),LoadError::SourceContract);
}

#[test]
fn cancellation_before_or_during_loading_never_returns_a_model() {
    let encoded = manifest(&[1; 2048]);
    let description = Manifest::parse(&encoded, digest(&encoded)).unwrap();
    let mut source = Source::new(&[1;2048]);
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| false).unwrap_err(),LoadError::Cancelled);
    assert_eq!(source.reads,0);
    assert_eq!(VerifiedModel::load(&description,&mut source,|p| p.verified_shards<1).unwrap_err(),LoadError::Cancelled);
    assert_eq!(source.reads,1);
    source.reads=0; source.maximum_read=100;
    assert_eq!(VerifiedModel::load(&description,&mut source,|p| p.bytes_read<100).unwrap_err(),LoadError::Cancelled);
    assert_eq!(source.reads,1);
}

#[test]
fn canonical_shape_rejects_forged_counts_lengths_and_trailing_data() {
    let original=manifest(&[1;2048]);
    for (at, replacement, expected) in [
        (8, 0u64.to_le_bytes().to_vec(), ManifestError::Shape),
        (8, u64::MAX.to_le_bytes().to_vec(), ManifestError::Limit),
        (16, 0u32.to_le_bytes().to_vec(), ManifestError::Shape),
        (16, 1025u32.to_le_bytes().to_vec(), ManifestError::Shape),
        (16, u32::MAX.to_le_bytes().to_vec(), ManifestError::Limit),
        (20, 0u32.to_le_bytes().to_vec(), ManifestError::Shape),
        (20, 3u32.to_le_bytes().to_vec(), ManifestError::Shape),
        (20, u32::MAX.to_le_bytes().to_vec(), ManifestError::Limit),
    ] {
        let mut encoded=original.clone(); encoded[at..at+replacement.len()].copy_from_slice(&replacement);
        assert_eq!(Manifest::parse(&encoded,digest(&encoded)).unwrap_err(),expected);
    }
    let mut encoded=original.clone(); encoded.push(0);
    assert_eq!(Manifest::parse(&encoded,digest(&encoded)).unwrap_err(),ManifestError::Length);
    for n in 0..original.len() {
        assert!(Manifest::parse(&original[..n],digest(&original[..n])).is_err());
    }
}

#[test]
fn model_length_above_four_gibibytes_is_not_truncated() {
    let total=5_027_783_488u64; let shard=64*1024*1024u32;
    let count=total.div_ceil(shard as u64) as u32;
    let mut encoded=MAGIC.to_vec(); encoded.extend_from_slice(&total.to_le_bytes());
    encoded.extend_from_slice(&shard.to_le_bytes()); encoded.extend_from_slice(&count.to_le_bytes());
    encoded.resize(HEADER_BYTES+count as usize*32,0);
    let description=Manifest::parse(&encoded,digest(&encoded)).unwrap();
    assert_eq!(description.model_bytes(),total); assert_eq!(description.shard_count(),75);
    let mut source=Source::new(&[]); source.declared=total as u32 as u64;
    assert_eq!(VerifiedModel::load(&description,&mut source,|_| true).unwrap_err(),LoadError::Length);
    assert_eq!(source.reads,0);
}
