//! Allocation-free admission of dense Qwen2/Qwen3 geometry from a GGUF index.
//! This is not tokenizer admission, weight verification, or an inference service.
//! The caller must authenticate the immutable model independently before using weights.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
pub mod forward;

use core::fmt::{self, Write};
use pcpos_gguf::{Index, Tensor, TensorType, Value};

pub const MAX_LAYERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture { Qwen2, Qwen3 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Architecture, Metadata, UnsupportedMetadata, Geometry,
    TensorSet, TensorShape, TensorEncoding, QuantizationVersion, Context, Budget,
}

/// Validated dimensions; copies are informational and cannot alter an admitted model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub architecture: Architecture,
    pub layers: usize,
    pub embedding: usize,
    pub feed_forward: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub head_width: usize,
    pub vocabulary: usize,
    pub training_context: usize,
    pub rope_base: f32,
    pub rms_epsilon: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Layer<'i, 'b> {
    pub attention_norm: &'i Tensor<'b>,
    pub query: &'i Tensor<'b>,
    pub key: &'i Tensor<'b>,
    pub value: &'i Tensor<'b>,
    pub attention_output: &'i Tensor<'b>,
    pub query_bias: Option<&'i Tensor<'b>>,
    pub key_bias: Option<&'i Tensor<'b>>,
    pub value_bias: Option<&'i Tensor<'b>>,
    pub query_norm: Option<&'i Tensor<'b>>,
    pub key_norm: Option<&'i Tensor<'b>>,
    pub feed_forward_norm: &'i Tensor<'b>,
    pub gate: &'i Tensor<'b>,
    pub up: &'i Tensor<'b>,
    pub down: &'i Tensor<'b>,
}

#[derive(Debug)]
pub struct Model<'i, 'b> {
    config: Config,
    file_bytes: u64,
    embedding: &'i Tensor<'b>,
    output: &'i Tensor<'b>,
    output_norm: &'i Tensor<'b>,
    layers: [Option<Layer<'i, 'b>>; MAX_LAYERS],
}

impl<'i, 'b> Model<'i, 'b> {
    /// Admit only the explicitly implemented dense geometry. Unknown architecture-specific
    /// metadata and extra tensors are refused: e.g. sliding windows, RoPE scaling, experts,
    /// adapters and multimodal projections must not silently become a plain dense model.
    /// Quantized values/scales and vocabulary contents are not read here.
    pub fn admit(index: &'i Index<'b>) -> Result<Self, Error> {
        let (architecture, prefix) = match index.get("general.architecture") {
            Some(Value::String("qwen2")) => (Architecture::Qwen2, "qwen2"),
            Some(Value::String("qwen3")) => (Architecture::Qwen3, "qwen3"),
            _ => return Err(Error::Architecture),
        };
        if let Some(value) = index.get("general.type") {
            if value != Value::String("model") { return Err(Error::Architecture); }
        }
        // A quantization scheme's name can survive a layout revision. Bind every
        // quantized tensor to the exact GGML layout version our row decoders test.
        let quantized = index.tensors().iter().any(|t| {
            !matches!(t.kind, TensorType::F32 | TensorType::F16 | TensorType::Bf16)
        });
        if quantized && index.get("general.quantization_version") != Some(Value::U32(2)) {
            return Err(Error::QuantizationVersion);
        }
        const KEYS: &[&str] = &[
            "block_count", "context_length", "embedding_length", "feed_forward_length",
            "attention.head_count", "attention.head_count_kv", "attention.key_length",
            "attention.value_length", "attention.layer_norm_rms_epsilon", "rope.freq_base",
            "rope.dimension_count",
        ];
        for entry in index.metadata() {
            if let Some(suffix) = entry.key.strip_prefix(prefix).and_then(|s| s.strip_prefix('.')) {
                if !KEYS.contains(&suffix) { return Err(Error::UnsupportedMetadata); }
            } else if entry.key.starts_with("qwen") || entry.key.starts_with("split.") {
                return Err(Error::UnsupportedMetadata);
            }
        }
        let get = |suffix: &str| {
            let mut key = Name::new();
            write!(&mut key, "{prefix}.{suffix}").map_err(|_| Error::Metadata)?;
            Ok(index.get(key.as_str()))
        };
        let integer = |key: &str, max: usize| -> Result<usize, Error> {
            match get(key)? {
                Some(Value::U32(n)) if n > 0 && (n as u64) <= max as u64 => Ok(n as usize),
                _ => Err(Error::Metadata),
            }
        };
        let layers = integer("block_count", MAX_LAYERS)?;
        let embedding = integer("embedding_length", 8192)?;
        let feed_forward = integer("feed_forward_length", 65536)?;
        let heads = integer("attention.head_count", 128)?;
        let kv_heads = integer("attention.head_count_kv", 128)?;
        let training_context = integer("context_length", 262144)?;
        if embedding % heads != 0 || heads % kv_heads != 0 { return Err(Error::Geometry); }
        let head_width = match get("attention.key_length")? {
            None => embedding / heads,
            Some(Value::U32(n)) => n as usize,
            _ => return Err(Error::Metadata),
        };
        if head_width == 0 || head_width > 256 || head_width % 2 != 0
            || (architecture == Architecture::Qwen2 && head_width != embedding / heads) {
            return Err(Error::Geometry);
        }
        for key in ["attention.value_length", "rope.dimension_count"] {
            if let Some(value) = get(key)? {
                if value != Value::U32(head_width as u32) { return Err(Error::Geometry); }
            }
        }
        let finite = |key: &str, min: f32, max: f32| -> Result<f32, Error> {
            match get(key)? {
                Some(Value::F32(n)) if n.is_finite() && n >= min && n <= max => Ok(n),
                _ => Err(Error::Metadata),
            }
        };
        let rope_base = finite("rope.freq_base", 1.0, 100_000_000.0)?;
        let rms_epsilon = finite("attention.layer_norm_rms_epsilon", 1e-12, 1.0)?;
        let vocabulary = match index.get("tokenizer.ggml.tokens") {
            Some(Value::Array(a)) if a.value_type() == 8 && (1..=262144).contains(&a.len()) => a.len() as usize,
            _ => return Err(Error::Metadata),
        };
        let config = Config { architecture, layers, embedding, feed_forward, heads, kv_heads,
            head_width, vocabulary, training_context, rope_base, rms_epsilon };
        let mut found = 0;
        let mut tensor = |name: &str, shape: &[usize], optional: bool| -> Result<Option<&'i Tensor<'b>>, Error> {
            let Some(t) = index.tensors().iter().find(|t| t.name == name) else {
                return if optional { Ok(None) } else { Err(Error::TensorSet) };
            };
            if t.dimensions().len() != shape.len()
                || t.dimensions().iter().zip(shape).any(|(&a, &b)| a != b as u64) {
                return Err(Error::TensorShape);
            }
            use TensorType::*;
            // Norms and biases use floating storage; matrices also admit our checked row decoders.
            if !matches!(t.kind, F32 | F16 | Bf16)
                && (shape.len() == 1 || !matches!(t.kind, Q5_0 | Q8_0 | Q4K | Q6K)) {
                return Err(Error::TensorEncoding);
            }
            found += 1;
            Ok(Some(t))
        };
        let emb = [embedding, vocabulary];
        let embedding_tensor = tensor("token_embd.weight", &emb, false)?.ok_or(Error::TensorSet)?;
        let output = tensor("output.weight", &emb, true)?.unwrap_or(embedding_tensor);
        let output_norm = tensor("output_norm.weight", &[embedding], false)?.ok_or(Error::TensorSet)?;
        let mut admitted = [None; MAX_LAYERS];
        for (i, slot) in admitted[..layers].iter_mut().enumerate() {
            let mut named = |suffix: &str, shape: &[usize]| -> Result<&'i Tensor<'b>, Error> {
                let mut name = Name::new();
                write!(&mut name, "blk.{i}.{suffix}").map_err(|_| Error::TensorSet)?;
                tensor(name.as_str(), shape, false)?.ok_or(Error::TensorSet)
            };
            let q = heads * head_width;
            let kv = kv_heads * head_width;
            let (query_bias, key_bias, value_bias, query_norm, key_norm) = match architecture {
                Architecture::Qwen2 => (Some(named("attn_q.bias", &[q])?),
                    Some(named("attn_k.bias", &[kv])?), Some(named("attn_v.bias", &[kv])?), None, None),
                Architecture::Qwen3 => (None, None, None,
                    Some(named("attn_q_norm.weight", &[head_width])?),
                    Some(named("attn_k_norm.weight", &[head_width])?)),
            };
            *slot = Some(Layer {
                attention_norm: named("attn_norm.weight", &[embedding])?,
                query: named("attn_q.weight", &[embedding, q])?,
                key: named("attn_k.weight", &[embedding, kv])?,
                value: named("attn_v.weight", &[embedding, kv])?,
                attention_output: named("attn_output.weight", &[q, embedding])?,
                query_bias, key_bias, value_bias, query_norm, key_norm,
                feed_forward_norm: named("ffn_norm.weight", &[embedding])?,
                gate: named("ffn_gate.weight", &[embedding, feed_forward])?,
                up: named("ffn_up.weight", &[embedding, feed_forward])?,
                down: named("ffn_down.weight", &[feed_forward, embedding])?,
            });
        }
        if found != index.tensors().len() { return Err(Error::TensorSet); }
        Ok(Self { config, file_bytes: index.file_bytes, embedding: embedding_tensor, output, output_norm, layers: admitted })
    }

    pub fn config(&self) -> Config { self.config }
    pub fn embedding(&self) -> &'i Tensor<'b> { self.embedding }
    pub fn output(&self) -> &'i Tensor<'b> { self.output }
    pub fn output_norm(&self) -> &'i Tensor<'b> { self.output_norm }
    pub fn layer(&self, i: usize) -> Option<Layer<'i, 'b>> { self.layers.get(i).copied().flatten() }

    /// Exact f32 K+V storage for the requested context; no allocation is performed.
    /// This excludes weights, tokenizer, scratch, index, page tables and OS memory.
    /// Caller supplies a byte budget independently of untrusted model metadata.
    pub fn kv_bytes(&self, context: usize, budget: u64) -> Result<u64, Error> {
        if context == 0 || context > self.config.training_context { return Err(Error::Context); }
        let c = self.config;
        let bytes = [2, 4, c.layers as u64, c.kv_heads as u64, c.head_width as u64, context as u64]
            .into_iter().try_fold(1u64, |a,b| a.checked_mul(b)).ok_or(Error::Budget)?;
        if bytes > budget { return Err(Error::Budget); }
        Ok(bytes)
    }
}

struct Name { bytes: [u8; 96], len: usize }
impl Name {
    fn new() -> Self { Self { bytes: [0; 96], len: 0 } }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len]).unwrap() }
}
impl Write for Name {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        let out = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        out.copy_from_slice(text.as_bytes()); self.len = end; Ok(())
    }
}
