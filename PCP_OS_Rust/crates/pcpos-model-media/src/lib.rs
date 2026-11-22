//! Image-pinned model identity and verified owned or borrowed bytes, independent of any device ABI.
//!
//! The manifest digest comes from a trusted image/launch binding, never from the
//! same untrusted medium as the manifest. No model is returned until all bytes
//! match both shard digests and the complete model digest. Callers cannot mutate
//! returned buffers; later reads therefore do not depend on mutable media.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub const MAGIC: &[u8; 8] = b"PCPMDL01";
pub const HEADER_BYTES: usize = 56;
pub const MAX_SHARDS: u32 = 4096;
pub const MAX_SHARD_BYTES: u32 = 64 * 1024 * 1024;
pub const MAX_MODEL_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const READ_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError { Digest, Magic, Length, Shape, Limit }

/// Canonical v1 record: magic[8], total u64 LE, shard size u32 LE, count u32 LE,
/// whole-model SHA-256[32], then exactly count ordered shard SHA-256 values.
#[derive(Debug)]
pub struct Manifest<'a> {
    bytes: &'a [u8], total: u64, shard_bytes: u32, count: u32, model_digest: [u8; 32],
}

impl<'a> Manifest<'a> {
    pub fn parse(bytes: &'a [u8], trusted_digest: [u8; 32]) -> Result<Self, ManifestError> {
        if bytes.len() < HEADER_BYTES || bytes.len() > HEADER_BYTES + MAX_SHARDS as usize * 32 {
            return Err(ManifestError::Length);
        }
        if <[u8; 32]>::from(Sha256::digest(bytes)) != trusted_digest { return Err(ManifestError::Digest); }
        if &bytes[..8] != MAGIC { return Err(ManifestError::Magic); }
        let total = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let shard_bytes = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let count = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        if total > MAX_MODEL_BYTES || shard_bytes > MAX_SHARD_BYTES || count > MAX_SHARDS {
            return Err(ManifestError::Limit);
        }
        if total == 0 || shard_bytes < 1024 || !shard_bytes.is_power_of_two() || count == 0
            || (total - 1) / shard_bytes as u64 + 1 != count as u64 {
            return Err(ManifestError::Shape);
        }
        if bytes.len() != HEADER_BYTES + count as usize * 32 { return Err(ManifestError::Length); }
        Ok(Self { bytes, total, shard_bytes, count, model_digest: bytes[24..56].try_into().unwrap() })
    }
    pub fn model_bytes(&self) -> u64 { self.total }
    pub fn model_digest(&self) -> [u8; 32] { self.model_digest }
    pub fn shard_bytes(&self) -> u32 { self.shard_bytes }
    pub fn shard_count(&self) -> u32 { self.count }
    fn digest(&self, shard: u32) -> [u8; 32] {
        let first = HEADER_BYTES + shard as usize * 32;
        self.bytes[first..first + 32].try_into().unwrap()
    }
}

/// Read-only logical model bytes; physical sector/shard layout belongs to the adapter.
/// Return 0 at EOF, never report more than buffer.len(). Short reads are supported.
/// Each call must have its own bounded device wait; cancellation cannot interrupt
/// an arbitrary blocking implementation of this trait.
pub trait ReadAt {
    type Error;
    fn len(&mut self) -> Result<u64, Self::Error>;
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoadError<E> {
    Source(E), Length, ShortRead, SourceContract, Allocation,
    ShardDigest { shard: u32 }, ModelDigest, Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub bytes_read: u64,
    pub total_bytes: u64,
    pub verified_shards: u32,
}

/// All returned buffers have been verified and are held privately by this value.
/// No lazy rereads from a replaceable source occur after construction.
#[derive(Debug)]
pub struct VerifiedModel {
    shards: Vec<Vec<u8>>, total: u64, shard_bytes: u32, digest: [u8; 32],
}

impl VerifiedModel {
    /// Verify an entire nominated model. `keep_going` runs before each at-most-64-KiB
    /// read and after each shard. False cancels; all temporary allocations are dropped.
    /// It must not display service Ready until this function returns Ok.
    pub fn load<S: ReadAt>(manifest: &Manifest<'_>, source: &mut S,
                          mut keep_going: impl FnMut(Progress) -> bool) -> Result<Self, LoadError<S::Error>> {
        let mut progress = Progress { bytes_read: 0, total_bytes: manifest.total, verified_shards: 0 };
        if !keep_going(progress) { return Err(LoadError::Cancelled); }
        if source.len().map_err(LoadError::Source)? != manifest.total { return Err(LoadError::Length); }
        let mut shards = Vec::new();
        shards.try_reserve_exact(manifest.count as usize).map_err(|_| LoadError::Allocation)?;
        let mut whole = Sha256::new();
        for ordinal in 0..manifest.count {
            let length = (manifest.total - progress.bytes_read).min(manifest.shard_bytes as u64) as usize;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length).map_err(|_| LoadError::Allocation)?;
            bytes.resize(length, 0);
            let mut have = 0;
            let mut shard_hash = Sha256::new();
            while have < length {
                if !keep_going(progress) { return Err(LoadError::Cancelled); }
                let wanted = (length - have).min(READ_BYTES);
                let got = source.read_at(progress.bytes_read, &mut bytes[have..have + wanted]).map_err(LoadError::Source)?;
                if got == 0 { return Err(LoadError::ShortRead); }
                if got > wanted { return Err(LoadError::SourceContract); }
                shard_hash.update(&bytes[have..have + got]);
                whole.update(&bytes[have..have + got]);
                have += got;
                progress.bytes_read += got as u64;
            }
            if <[u8; 32]>::from(shard_hash.finalize()) != manifest.digest(ordinal) {
                return Err(LoadError::ShardDigest { shard: ordinal });
            }
            shards.push(bytes);
            progress.verified_shards += 1;
            if !keep_going(progress) { return Err(LoadError::Cancelled); }
        }
        if <[u8; 32]>::from(whole.finalize()) != manifest.model_digest { return Err(LoadError::ModelDigest); }
        // Check actual EOF too, rather than trusting a source's advertised length alone.
        let mut extra = [0u8; 1];
        if source.read_at(manifest.total, &mut extra).map_err(LoadError::Source)? != 0 {
            return Err(LoadError::Length);
        }
        Ok(Self { shards, total: manifest.total, shard_bytes: manifest.shard_bytes, digest: manifest.model_digest })
    }
    pub fn len(&self) -> u64 { self.total }
    pub fn is_empty(&self) -> bool { self.total == 0 }
    pub fn digest(&self) -> [u8; 32] { self.digest }
    /// Copy a requested logical range, including ranges crossing a shard boundary.
    /// An invalid range fails before modifying output.
    pub fn copy_into(&self, offset: u64, output: &mut [u8]) -> Result<(), RangeError> {
        copy_from_shards(self.total, self.shard_bytes, offset, output, |i| &self.shards[i])
    }
}

/// Verified references to bytes the caller already holds immutably. No model-sized
/// copy or allocation is made. The lifetime retains the very bytes that were hashed.
///
/// A native adapter constructing slices over mapped firmware pages must first prove
/// their validity and lifetime, read-only/NX mapping, absence of writable aliases and
/// exclusion from allocators. This safe library neither establishes nor bypasses
/// that unsafe boundary. A replaceable ReadAt source cannot supply this guarantee.
#[derive(Debug)]
pub struct VerifiedSlices<'a> {
    shards: &'a [&'a [u8]], total: u64, shard_bytes: u32, digest: [u8; 32],
}

impl<'a> VerifiedSlices<'a> {
    /// Hash each immutable shard and the entire ordered model. Each slice must be
    /// exactly one manifest shard, including the short last shard. Work and progress
    /// use the same at-most-64-KiB boundaries as owned loading. False cancels.
    pub fn verify(manifest: &Manifest<'_>, shards: &'a [&'a [u8]],
                  mut keep_going: impl FnMut(Progress) -> bool) -> Result<Self, LoadError<core::convert::Infallible>> {
        let mut progress = Progress { bytes_read: 0, total_bytes: manifest.total, verified_shards: 0 };
        if !keep_going(progress) { return Err(LoadError::Cancelled); }
        if shards.len() != manifest.count as usize { return Err(LoadError::Length); }
        // Validate every extent before reading model bytes; no extra or omitted bytes.
        for (ordinal, bytes) in shards.iter().enumerate() {
            let length = (manifest.total - ordinal as u64 * manifest.shard_bytes as u64).min(manifest.shard_bytes as u64);
            if bytes.len() as u64 != length { return Err(LoadError::Length); }
        }
        let mut whole = Sha256::new();
        for (ordinal, bytes) in shards.iter().enumerate() {
            let mut shard_hash = Sha256::new();
            for block in bytes.chunks(READ_BYTES) {
                if !keep_going(progress) { return Err(LoadError::Cancelled); }
                shard_hash.update(block); whole.update(block);
                progress.bytes_read += block.len() as u64;
            }
            if <[u8; 32]>::from(shard_hash.finalize()) != manifest.digest(ordinal as u32) {
                return Err(LoadError::ShardDigest { shard: ordinal as u32 });
            }
            progress.verified_shards += 1;
            if !keep_going(progress) { return Err(LoadError::Cancelled); }
        }
        if <[u8; 32]>::from(whole.finalize()) != manifest.model_digest { return Err(LoadError::ModelDigest); }
        Ok(Self { shards, total: manifest.total, shard_bytes: manifest.shard_bytes, digest: manifest.model_digest })
    }
    pub fn len(&self) -> u64 { self.total }
    pub fn is_empty(&self) -> bool { self.total == 0 }
    pub fn digest(&self) -> [u8; 32] { self.digest }
    /// The same immutable shard that was verified, without copying it.
    pub fn shard(&self, ordinal: usize) -> Option<&'a [u8]> { self.shards.get(ordinal).copied() }
    pub fn copy_into(&self, offset: u64, output: &mut [u8]) -> Result<(), RangeError> {
        copy_from_shards(self.total, self.shard_bytes, offset, output, |i| self.shards[i])
    }
}

fn copy_from_shards<'a>(total: u64, shard_bytes: u32, offset: u64, output: &mut [u8],
                       shard: impl Fn(usize) -> &'a [u8]) -> Result<(), RangeError> {
    if offset.checked_add(output.len() as u64).filter(|&end| end <= total).is_none() { return Err(RangeError); }
    let mut position = offset;
    let mut written = 0;
    while written < output.len() {
        let ordinal = (position / shard_bytes as u64) as usize;
        let within = (position % shard_bytes as u64) as usize;
        let bytes = shard(ordinal);
        let part = (bytes.len() - within).min(output.len() - written);
        output[written..written + part].copy_from_slice(&bytes[within..within + part]);
        position += part as u64;
        written += part;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeError;
