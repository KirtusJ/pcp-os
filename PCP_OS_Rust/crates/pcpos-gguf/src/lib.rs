//! Inspect a bounded GGUF v3 prefix without loading tensor data.
//!
//! File offsets stay `u64`, including on a 32-bit host. The caller owns the prefix
//! and supplies the length of the same immutable file. Success validates the index,
//! not the weights, model architecture, publisher, tokenizer or inference engine.
//! No I/O or unsafe code; only the small index allocates, with fallible reservations.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::vec::Vec;

/// Policy limits for this reader, not GGUF format limits.
pub const MAX_PREFIX_BYTES: usize = 32 * 1024 * 1024;
const MAX_METADATA: u64 = 256;
const MAX_TENSORS: u64 = 1024;
const MAX_VALUES: u64 = 2_000_000;
const MAX_ARRAY: u64 = 1_000_000;
const MAX_DEPTH: u8 = 8;
const MAX_STRING: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The file is long enough, but the supplied prefix must grow to at least this size.
    NeedMore { minimum: usize },
    Truncated,
    Limit,
    Overflow,
    Allocation,
    Magic,
    Version,
    ValueType,
    Boolean,
    Utf8,
    Key,
    DuplicateKey,
    Alignment,
    TensorName,
    DuplicateTensor,
    Dimensions,
    TensorType,
    QuantizedRow,
    TensorRange,
    Overlap,
    Padding,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value<'a> {
    U8(u8), I8(i8), U16(u16), I16(i16), U32(u32), I32(i32), F32(f32),
    Bool(bool), String(&'a str), Array(Array<'a>), U64(u64), I64(i64), F64(f64),
}

/// Already validated encoded array; iteration preserves all elements and string bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Array<'a> {
    kind: u32,
    count: u64,
    bytes: &'a [u8],
}

impl<'a> Array<'a> {
    pub fn len(&self) -> u64 { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn value_type(&self) -> u32 { self.kind }
    pub fn iter(&self) -> Values<'a> {
        Values { cursor: Cursor::new(self.bytes, self.bytes.len() as u64),
                 kind: self.kind, left: self.count, budget: MAX_VALUES }
    }
}

pub struct Values<'a> {
    cursor: Cursor<'a>, kind: u32, left: u64, budget: u64,
}

impl<'a> Iterator for Values<'a> {
    type Item = Result<Value<'a>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 { return None; }
        self.left -= 1;
        let value = self.cursor.value(self.kind, 0, &mut self.budget);
        if value.is_err() { self.left = 0; }
        Some(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorType { F32, F16, Q4_0, Q4_1, Q5_0, Q5_1, Q8_0, Q8_1, Q2K, Q3K, Q4K, Q5K, Q6K, Q8K, Bf16 }

impl TensorType {
    fn from_code(code: u32) -> Result<Self, Error> {
        Ok(match code {
            0 => Self::F32, 1 => Self::F16, 2 => Self::Q4_0, 3 => Self::Q4_1,
            6 => Self::Q5_0, 7 => Self::Q5_1, 8 => Self::Q8_0, 9 => Self::Q8_1,
            10 => Self::Q2K, 11 => Self::Q3K, 12 => Self::Q4K, 13 => Self::Q5K,
            14 => Self::Q6K, 15 => Self::Q8K, 30 => Self::Bf16,
            _ => return Err(Error::TensorType),
        })
    }
    /// Elements per stored block and bytes per block, including scales.
    pub fn block_layout(self) -> (u64, u64) {
        match self {
            Self::F32 => (1, 4), Self::F16 | Self::Bf16 => (1, 2),
            Self::Q4_0 => (32, 18), Self::Q4_1 => (32, 20),
            Self::Q5_0 => (32, 22), Self::Q5_1 => (32, 24),
            Self::Q8_0 => (32, 34), Self::Q8_1 => (32, 36),
            Self::Q2K => (256, 84), Self::Q3K => (256, 110),
            Self::Q4K => (256, 144), Self::Q5K => (256, 176),
            Self::Q6K => (256, 210), Self::Q8K => (256, 292),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metadata<'a> { pub key: &'a str, pub value: Value<'a> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tensor<'a> {
    pub name: &'a str,
    dimensions: [u64; 4],
    rank: usize,
    pub kind: TensorType,
    /// Absolute file position, never a host pointer or a slice index.
    pub offset: u64,
    pub bytes: u64,
}

impl Tensor<'_> {
    pub fn dimensions(&self) -> &[u64] { &self.dimensions[..self.rank] }
}

#[derive(Debug)]
pub struct Index<'a> {
    metadata: Vec<Metadata<'a>>,
    tensors: Vec<Tensor<'a>>,
    pub data_offset: u64,
    pub alignment: u32,
    pub file_bytes: u64,
}

impl<'a> Index<'a> {
    pub fn metadata(&self) -> &[Metadata<'a>] { &self.metadata }
    pub fn tensors(&self) -> &[Tensor<'a>] { &self.tensors }
    pub fn get(&self, key: &str) -> Option<Value<'a>> {
        self.metadata.iter().find(|entry| entry.key == key).map(|entry| entry.value)
    }

    /// Parse little-endian GGUF v3. Unsupported encodings fail explicitly.
    /// `prefix` may contain bytes after the index, but no read goes beyond the policy cap.
    pub fn parse(prefix: &'a [u8], file_bytes: u64) -> Result<Self, Error> {
        if prefix.len() as u64 > file_bytes { return Err(Error::Truncated); }
        let mut c = Cursor::new(prefix, file_bytes);
        if c.take(4)? != b"GGUF" { return Err(Error::Magic); }
        if c.u32()? != 3 { return Err(Error::Version); }
        let nt = c.u64()?;
        let nk = c.u64()?;
        if nt > MAX_TENSORS || nk > MAX_METADATA { return Err(Error::Limit); }
        let mut metadata: Vec<Metadata<'a>> = Vec::new();
        metadata.try_reserve_exact(nk as usize).map_err(|_| Error::Allocation)?;
        let mut alignment = 32u32;
        let mut budget = MAX_VALUES;
        for _ in 0..nk {
            let key = c.string(65535)?;
            if !valid_key(key) { return Err(Error::Key); }
            if metadata.iter().any(|entry| entry.key == key) { return Err(Error::DuplicateKey); }
            let kind = c.u32()?;
            let value = c.value(kind, 0, &mut budget)?;
            if key == "general.alignment" {
                alignment = match value { Value::U32(n) => n, _ => return Err(Error::Alignment) };
                if !alignment.is_power_of_two() || alignment > 4096 { return Err(Error::Alignment); }
            }
            metadata.push(Metadata { key, value });
        }
        let mut tensors: Vec<Tensor<'a>> = Vec::new();
        tensors.try_reserve_exact(nt as usize).map_err(|_| Error::Allocation)?;
        for _ in 0..nt {
            let name = c.string(64)?;
            if name.is_empty() || name.bytes().any(|b| b == 0) { return Err(Error::TensorName); }
            if tensors.iter().any(|t| t.name == name) { return Err(Error::DuplicateTensor); }
            let rank = c.u32()? as usize;
            if !(1..=4).contains(&rank) { return Err(Error::Dimensions); }
            let mut dimensions = [0; 4];
            let mut elements = 1u64;
            for d in &mut dimensions[..rank] {
                *d = c.u64()?;
                if *d == 0 { return Err(Error::Dimensions); }
                elements = elements.checked_mul(*d).ok_or(Error::Overflow)?;
            }
            let kind = TensorType::from_code(c.u32()?)?;
            let (block, block_bytes) = kind.block_layout();
            if dimensions[0] % block != 0 { return Err(Error::QuantizedRow); }
            let bytes = (elements / block).checked_mul(block_bytes).ok_or(Error::Overflow)?;
            let offset = c.u64()?;
            if offset % alignment as u64 != 0 { return Err(Error::Alignment); }
            let end = offset.checked_add(bytes).ok_or(Error::Overflow)?;
            // There is no requirement that the tensor directory be sorted by file position.
            if tensors.iter().any(|t| offset < t.offset + t.bytes && t.offset < end) {
                return Err(Error::Overlap);
            }
            tensors.push(Tensor { name, dimensions, rank, kind, offset, bytes });
        }
        let padding = (alignment as usize - c.pos % alignment as usize) % alignment as usize;
        if c.take(padding as u64)?.iter().any(|&b| b != 0) { return Err(Error::Padding); }
        let data_offset = c.pos as u64;
        for tensor in &mut tensors {
            tensor.offset = tensor.offset.checked_add(data_offset).ok_or(Error::Overflow)?;
            let end = tensor.offset.checked_add(tensor.bytes).ok_or(Error::Overflow)?;
            if end > file_bytes { return Err(Error::TensorRange); }
        }
        Ok(Self { metadata, tensors, data_offset, alignment, file_bytes })
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.split('.').all(|part| {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

struct Cursor<'a> { bytes: &'a [u8], file_bytes: u64, pos: usize }
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], file_bytes: u64) -> Self { Self { bytes, file_bytes, pos: 0 } }
    fn take(&mut self, count: u64) -> Result<&'a [u8], Error> {
        let count = usize::try_from(count).map_err(|_| Error::Limit)?;
        let end = self.pos.checked_add(count).ok_or(Error::Overflow)?;
        if end > MAX_PREFIX_BYTES { return Err(Error::Limit); }
        if end as u64 > self.file_bytes { return Err(Error::Truncated); }
        if end > self.bytes.len() { return Err(Error::NeedMore { minimum: end }); }
        let result = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(result)
    }
    fn u8(&mut self) -> Result<u8, Error> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, Error> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
    fn string(&mut self, max: u64) -> Result<&'a str, Error> {
        let n = self.u64()?;
        if n > max { return Err(Error::Limit); }
        core::str::from_utf8(self.take(n)?).map_err(|_| Error::Utf8)
    }
    fn value(&mut self, kind: u32, depth: u8, budget: &mut u64) -> Result<Value<'a>, Error> {
        *budget = budget.checked_sub(1).ok_or(Error::Limit)?;
        Ok(match kind {
            0 => Value::U8(self.u8()?), 1 => Value::I8(self.u8()? as i8),
            2 => Value::U16(self.u16()?), 3 => Value::I16(self.u16()? as i16),
            4 => Value::U32(self.u32()?), 5 => Value::I32(self.u32()? as i32),
            6 => Value::F32(f32::from_bits(self.u32()?)),
            7 => Value::Bool(match self.u8()? { 0 => false, 1 => true, _ => return Err(Error::Boolean) }),
            8 => Value::String(self.string(MAX_STRING)?),
            9 => {
                if depth >= MAX_DEPTH { return Err(Error::Limit); }
                let kind = self.u32()?;
                if kind > 12 { return Err(Error::ValueType); }
                let count = self.u64()?;
                if count > MAX_ARRAY || count > *budget { return Err(Error::Limit); }
                let start = self.pos;
                for _ in 0..count { self.value(kind, depth + 1, budget)?; }
                Value::Array(Array { kind, count, bytes: &self.bytes[start..self.pos] })
            }
            10 => Value::U64(self.u64()?), 11 => Value::I64(self.u64()? as i64),
            12 => Value::F64(f64::from_bits(self.u64()?)),
            _ => return Err(Error::ValueType),
        })
    }
}
