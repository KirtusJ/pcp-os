//! Single-sequence forward execution. No tokenizer, sampler or native device I/O.
//! All model bytes must be authenticated and immutable for the decoder's lifetime.
use alloc::vec::Vec;
use pcpos_gguf::Tensor;
pub use pcpos_model_tensor::Projection;
use pcpos_model_tensor::{packed::{PackedKernel, Q8Block}, Row};
use crate::{Config, Model};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Shape, Weights, Budget, Allocation, Cancelled, Token, ContextFull, NonFinite }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefillEvent {
    /// Check whether the caller still wants this request to run.
    Poll,
    /// Number of fully processed tokens in this prompt, including the current one.
    /// The decoder commits the entire prompt only when prefill returns successfully.
    TokenProcessed { completed: usize, total: usize },
}
impl From<pcpos_model_tensor::Error> for Error {
    fn from(e: pcpos_model_tensor::Error) -> Self {
        match e {
            pcpos_model_tensor::Error::Cancelled => Self::Cancelled,
            pcpos_model_tensor::Error::NonFinite => Self::NonFinite,
            _ => Self::Weights,
        }
    }
}

/// One model-bound sequence, with f32 KV and scratch storage allocated fallibly once.
/// A failed/cancelled step does not advance the sequence or return partial logits.
/// Retrying overwrites the uncommitted KV position; all previous positions remain intact.
pub struct Decoder<'m, 'i, 'b, 'w> {
    model: &'m Model<'i, 'b>, weights: &'w [u8], context: usize, position: usize,
    math: MatrixMath, lanes: usize,
    keys: Vec<f32>, values: Vec<f32>,
    x: Vec<f32>, norm: Vec<f32>, temp: Vec<f32>, norm_weights: Vec<f32>,
    q: Vec<f32>, k: Vec<f32>, v: Vec<f32>, attention: Vec<f32>,
    gate: Vec<f32>, up: Vec<f32>, scores: Vec<f32>, logits: Vec<f32>, frequencies: Vec<f32>,
}

const PROMPT_TILE: usize = 4;

struct MatrixMath {
    ordered: Projection,
    kernel: Option<PackedKernel>,
    scratch: Vec<Q8Block>,
}

impl<'m, 'i, 'b, 'w> Decoder<'m, 'i, 'b, 'w> {
    /// Minimum decoder payload, including KV. Larger budgets enable prompt batching. Excludes allocator overhead,
    /// model, index/tokenizer, decoder struct, stack, page tables and OS storage.
    pub fn buffer_bytes(model: &Model<'_, '_>, context: usize) -> Result<u64, Error> {
        let kv = model.kv_bytes(context, u64::MAX).map_err(|_| Error::Budget)?;
        let c = model.config();
        let scratch = 3*c.embedding + c.embedding.max(c.head_width) + 2*c.heads*c.head_width
            + 2*c.kv_heads*c.head_width + 2*c.feed_forward + context + c.vocabulary + c.head_width/2;
        kv.checked_add((scratch as u64)*4).ok_or(Error::Budget)
    }

    pub fn new(model: &'m Model<'i, 'b>, weights: &'w [u8], context: usize,
        budget: u64, keep_going: impl FnMut() -> bool) -> Result<Self, Error> {
        Self::with_projection(model, weights, context, budget, Projection::detect(), keep_going)
    }

    /// The worker may use ggml's quantized activation arithmetic on capable CPUs.
    /// The ordered decoder remains available for numerical controls and fallback.
    pub fn for_inference(model: &'m Model<'i, 'b>, weights: &'w [u8], context: usize,
        budget: u64, keep_going: impl FnMut() -> bool) -> Result<Self, Error> {
        Self::allocate(model, weights, context, budget, Projection::detect(), PackedKernel::detect(), keep_going)
    }

    pub fn projection_name(&self) -> &'static str {
        if self.math.kernel.is_some() { "AVX2 Q8_K" } else { self.math.ordered.name() }
    }

    /// Construct a decoder with an explicit reference or locally detected ordered projection.
    pub fn with_projection(model: &'m Model<'i, 'b>, weights: &'w [u8], context: usize,
        budget: u64, projection: Projection, keep_going: impl FnMut() -> bool) -> Result<Self, Error> {
        Self::allocate(model, weights, context, budget, projection, None, keep_going)
    }

    fn allocate(model: &'m Model<'i, 'b>, weights: &'w [u8], context: usize, budget: u64,
        ordered: Projection, kernel: Option<PackedKernel>, mut keep_going: impl FnMut() -> bool) -> Result<Self, Error> {
        if weights.len() as u64 != model.file_bytes { return Err(Error::Weights); }
        let base = Self::buffer_bytes(model, context)?;
        let c = model.config();
        let q = c.heads*c.head_width;
        let k = c.kv_heads*c.head_width;
        let remaining = budget.checked_sub(base).ok_or(Error::Budget)?;
        let blocks_per_lane = c.embedding.max(c.feed_forward).max(q) / 256;
        let floats_per_lane = 3*c.embedding + 2*q + 2*k + 2*c.feed_forward;
        let affordable = |&lanes: &usize| {
            let extra = (lanes - 1) * floats_per_lane * 4
                + lanes * blocks_per_lane * core::mem::size_of::<Q8Block>();
            extra as u64 <= remaining
        };
        // A small approved budget can still run the ordered decoder.
        let width = kernel.and_then(|_| (1..=PROMPT_TILE.min(context)).rev().find(affordable));
        let lanes = width.unwrap_or(1);
        let blocks = width.unwrap_or(0) * blocks_per_lane;
        let kernel = width.and(kernel);
        let kv = c.layers.checked_mul(context).and_then(|v| v.checked_mul(k)).ok_or(Error::Budget)?;
        let mut scratch = Vec::new();
        scratch.try_reserve_exact(blocks).map_err(|_| Error::Allocation)?;
        scratch.resize(blocks, Q8Block::ZERO);
        let mut new = |n| zeros(n, &mut keep_going);
        let mut result = Self {
            model, weights, context, position:0, lanes, math: MatrixMath { ordered, kernel, scratch },
            keys:new(kv)?, values:new(kv)?, x:new(lanes*c.embedding)?, norm:new(lanes*c.embedding)?, temp:new(lanes*c.embedding)?,
            norm_weights:new(c.embedding.max(c.head_width))?, q:new(lanes*q)?, k:new(lanes*k)?, v:new(lanes*k)?,
            attention:new(lanes*q)?, gate:new(lanes*c.feed_forward)?, up:new(lanes*c.feed_forward)?,
            scores:new(context)?, logits:new(c.vocabulary)?,
            frequencies:new(c.head_width/2)?,
        };
        for (i,f) in result.frequencies.iter_mut().enumerate() {
            *f = libm::powf(c.rope_base, -((2*i) as f32) / c.head_width as f32);
        }
        Ok(result)
    }

    /// Opt in to four-lane f64 accumulation. This changes summation order and
    /// needs model-level numerical/quality qualification; `new` keeps the
    /// sequential reference's order. Allocation and cancellation bounds are unchanged.
    pub fn new_unrolled(model: &'m Model<'i, 'b>, weights: &'w [u8], context: usize,
        budget: u64, keep_going: impl FnMut() -> bool) -> Result<Self, Error> {
        Self::with_projection(model, weights, context, budget, Projection::Unrolled, keep_going)
    }

    pub fn position(&self) -> usize { self.position }

    /// Ordered arithmetic available for unsupported formats and reference comparisons.
    pub fn projection(&self) -> Projection { self.math.ordered }

    /// Start a new logical sequence. Old positions become inaccessible immediately.
    /// This does not promise secure erasure of the worker's private memory.
    pub fn reset(&mut self) { self.position = 0; }

    /// Process one token and expose complete logits only on success. The callback is
    /// polled at initialization chunks, row/block boundaries and attention positions.
    /// Row validation itself scans at most 65536 weights; there is no wall-clock Stop claim.
    pub fn step(&mut self, token: u32, mut keep_going: impl FnMut() -> bool) -> Result<&[f32], Error> {
        self.advance(&[token], true, &mut keep_going)?;
        Ok(&self.logits)
    }

    /// Process a nonempty prompt, projecting vocabulary logits only for its last
    /// token. Numerical layer/KV operations are identical to repeated `step`.
    /// On any failure the logical position returns to its entry value; prior
    /// context remains untouched and the entire same prompt can be retried.
    pub fn prefill(&mut self, tokens: &[u32], mut keep_going: impl FnMut() -> bool) -> Result<&[f32], Error> {
        self.prefill_with_progress(tokens, |event| match event {
            PrefillEvent::Poll => keep_going(),
            PrefillEvent::TokenProcessed { .. } => true,
        })
    }

    /// As in `prefill`, with one progress notification after each completed token.
    /// Returning false at either kind of event rolls back the whole call, including
    /// when cancellation arrives after the last projection. Invalid prompts emit no events.
    pub fn prefill_with_progress(
        &mut self,
        tokens: &[u32],
        mut observe: impl FnMut(PrefillEvent) -> bool,
    ) -> Result<&[f32], Error> {
        if tokens.is_empty() { return Err(Error::Token); }
        if tokens.len() > self.context - self.position { return Err(Error::ContextFull); }
        if tokens.iter().any(|&id| id as usize >= self.model.config().vocabulary) { return Err(Error::Token); }
        let start = self.position;
        // Report the first token promptly. Subsequent groups share each weight row;
        // only fully computed tokens are reported, in order, even after a partial tile.
        let mut completed = 0;
        while completed < tokens.len() {
            let count = if completed == 0 { 1 } else { self.lanes.min(tokens.len()-completed) };
            let end = completed + count;
            if let Err(error) = self.advance(&tokens[completed..end], end == tokens.len(), || observe(PrefillEvent::Poll)) {
                self.position = start;
                return Err(error);
            }
            while completed < end {
                completed += 1;
                if !observe(PrefillEvent::TokenProcessed { completed, total: tokens.len() }) {
                    self.position = start;
                    return Err(Error::Cancelled);
                }
            }
        }
        Ok(&self.logits)
    }

    fn advance(&mut self, tokens: &[u32], project: bool, mut go: impl FnMut() -> bool) -> Result<(), Error> {
        let c = self.model.config();
        let lanes = tokens.len();
        if lanes == 0 || lanes > self.lanes { return Err(Error::Shape); }
        if tokens.iter().any(|&id| id as usize >= c.vocabulary) { return Err(Error::Token); }
        if lanes > self.context - self.position { return Err(Error::ContextFull); }
        check(&mut go)?;
        let emb = c.embedding;
        let qw = c.heads*c.head_width;
        let kw = c.kv_heads*c.head_width;
        let x = &mut self.x[..lanes*emb];
        let norm = &mut self.norm[..lanes*emb];
        let temp = &mut self.temp[..lanes*emb];
        let query = &mut self.q[..lanes*qw];
        let key = &mut self.k[..lanes*kw];
        let value = &mut self.v[..lanes*kw];
        let attn = &mut self.attention[..lanes*qw];
        let gate = &mut self.gate[..lanes*c.feed_forward];
        let up = &mut self.up[..lanes*c.feed_forward];
        let mut matrix = |t: &Tensor<'_>, a: &[f32], out: &mut [f32], go: &mut _| {
            self.math.multiply(self.weights, t, a, out, lanes, go)
        };
        for (&token, dst) in tokens.iter().zip(x.chunks_exact_mut(emb)) {
            row(self.weights, self.model.embedding(), token as usize)?.decode(dst)?;
        }
        for index in 0..c.layers {
            let l = self.model.layer(index).ok_or(Error::Shape)?;
            for (src, dst) in x.chunks_exact(emb).zip(norm.chunks_exact_mut(emb)) {
                normalize(self.weights, l.attention_norm, src, dst, &mut self.norm_weights, c.rms_epsilon)?;
            }
            matrix(l.query, norm, query, &mut go)?;
            matrix(l.key, norm, key, &mut go)?;
            matrix(l.value, norm, value, &mut go)?;
            for lane in 0..lanes {
                let pos = self.position + lane;
                let q = &mut query[lane*qw..(lane+1)*qw];
                let k = &mut key[lane*kw..(lane+1)*kw];
                let v = &mut value[lane*kw..(lane+1)*kw];
                for (bias, dst) in [(l.query_bias, &mut *q), (l.key_bias, &mut *k), (l.value_bias, &mut *v)] {
                    if let Some(bias) = bias { add_bias(self.weights, bias, dst)?; }
                }
                if let (Some(qn), Some(kn)) = (l.query_norm, l.key_norm) {
                    normalize_heads(self.weights, qn, q, &mut self.norm_weights, c.head_width, c.rms_epsilon)?;
                    normalize_heads(self.weights, kn, k, &mut self.norm_weights, c.head_width, c.rms_epsilon)?;
                }
                rotate(q, &self.frequencies, pos, c.head_width)?;
                rotate(k, &self.frequencies, pos, c.head_width)?;
                let at = (index*self.context + pos)*kw;
                self.keys[at..at+kw].copy_from_slice(k);
                self.values[at..at+kw].copy_from_slice(v);
                attention(c, self.context, pos, index, q, &self.keys, &self.values, &mut self.scores,
                    &mut attn[lane*qw..(lane+1)*qw], &mut go)?;
            }
            matrix(l.attention_output, attn, temp, &mut go)?;
            add(x,temp)?;
            for (src, dst) in x.chunks_exact(emb).zip(norm.chunks_exact_mut(emb)) {
                normalize(self.weights, l.feed_forward_norm, src, dst, &mut self.norm_weights, c.rms_epsilon)?;
            }
            matrix(l.gate, norm, gate, &mut go)?;
            matrix(l.up, norm, up, &mut go)?;
            for (i,(g,u)) in gate.iter_mut().zip(up.iter()).enumerate() {
                if i%256==0 { check(&mut go)?; }
                let sigmoid = if *g >= 0.0 {1.0/(1.0+libm::expf(-*g))}
                    else {let e=libm::expf(*g); e/(1.0+e)};
                *g=finite(*g*sigmoid*u)?;
            }
            matrix(l.down, gate, temp, &mut go)?;
            add(x,temp)?;
        }
        if project {
            normalize(self.weights, self.model.output_norm(), &x[(lanes-1)*emb..], &mut norm[..emb],
                &mut self.norm_weights, c.rms_epsilon)?;
            self.math.multiply(self.weights, self.model.output(), &norm[..emb], &mut self.logits, 1, &mut go)?;
        }
        check(&mut go)?;
        self.position += lanes;
        Ok(())
    }
}

fn zeros(n:usize,go:&mut impl FnMut()->bool)->Result<Vec<f32>,Error> {
    check(go)?;
    let mut v=Vec::new();v.try_reserve_exact(n).map_err(|_|Error::Allocation)?;
    const ZERO:[f32;1024]=[0.0;1024];
    while v.len()<n {check(go)?;let count=(n-v.len()).min(ZERO.len());v.extend_from_slice(&ZERO[..count]);}
    Ok(v)
}
fn check(go:&mut impl FnMut()->bool)->Result<(),Error>{if go(){Ok(())}else{Err(Error::Cancelled)}}
fn finite(v:f32)->Result<f32,Error>{if v.is_finite(){Ok(v)}else{Err(Error::NonFinite)}}

fn row<'a>(weights:&'a[u8],t:&Tensor<'_>,r:usize)->Result<Row<'a>,Error>{
    let dims=t.dimensions();
    let columns=usize::try_from(dims[0]).map_err(|_|Error::Shape)?;
    let rows=if dims.len()==1{1}else{usize::try_from(dims[1]).map_err(|_|Error::Shape)?};
    if r>=rows{return Err(Error::Shape);}
    let bytes=usize::try_from(t.bytes/(rows as u64)).map_err(|_|Error::Shape)?;
    let start=usize::try_from(t.offset).ok().and_then(|n|r.checked_mul(bytes).and_then(|off|n.checked_add(off))).ok_or(Error::Weights)?;
    let end=start.checked_add(bytes).ok_or(Error::Weights)?;
    Ok(Row::new(t.kind,columns,weights.get(start..end).ok_or(Error::Weights)?)?)
}
impl MatrixMath {
    fn multiply(&mut self, weights: &[u8], t: &Tensor<'_>, input: &[f32], output: &mut [f32],
        lanes: usize, go: &mut impl FnMut()->bool) -> Result<(),Error> {
        let (columns, rows) = (input.len()/lanes, output.len()/lanes);
        if t.dimensions()!=[columns as u64,rows as u64] { return Err(Error::Shape); }
        if let Some(kernel) = self.kernel {
            if matches!(t.kind, pcpos_gguf::TensorType::Q4K | pcpos_gguf::TensorType::Q6K) {
                let start = usize::try_from(t.offset).map_err(|_| Error::Weights)?;
                let size = usize::try_from(t.bytes).map_err(|_| Error::Weights)?;
                let end = start.checked_add(size).ok_or(Error::Weights)?;
                let data = weights.get(start..end).ok_or(Error::Weights)?;
                let used = match lanes {
                    1 => kernel.project::<1>(t.kind, data, input, output, &mut self.scratch, go),
                    2 => kernel.project::<2>(t.kind, data, input, output, &mut self.scratch, go),
                    3 => kernel.project::<3>(t.kind, data, input, output, &mut self.scratch, go),
                    4 => kernel.project::<4>(t.kind, data, input, output, &mut self.scratch, go),
                    _ => return Err(Error::Shape),
                }?;
                if used { return Ok(()); }
            }
        }
        for (src, dst) in input.chunks_exact(columns).zip(output.chunks_exact_mut(rows)) {
            for (i,out) in dst.iter_mut().enumerate() {
                check(go)?;
                *out = self.ordered.dot(&row(weights,t,i)?,src,&mut *go)?;
            }
        }
        Ok(())
    }
}
fn add(a:&mut[f32],b:&[f32])->Result<(),Error>{
    if a.len()!=b.len(){return Err(Error::Shape);}
    for (a,b) in a.iter_mut().zip(b){*a=finite(*a+*b)?;} Ok(())
}
fn add_bias(weights:&[u8],t:&Tensor<'_>,output:&mut[f32])->Result<(),Error>{
    // Biases are small float vectors. No full-row scratch or allocation is needed.
    let bytes=t.kind.block_layout().1 as usize;
    let start=usize::try_from(t.offset).map_err(|_|Error::Weights)?;
    for (i,out) in output.iter_mut().enumerate(){
        let off=start+i*bytes; let mut value=[0.0];
        Row::new(t.kind,1,weights.get(off..off+bytes).ok_or(Error::Weights)?)?.decode(&mut value)?;
        *out=finite(*out+value[0])?;
    }Ok(())
}
fn rms(input:&[f32],epsilon:f32)->Result<f32,Error>{
    let mut sum=0.0f64;for &x in input {if !x.is_finite(){return Err(Error::NonFinite);}sum+=(x as f64)*(x as f64);}
    finite((1.0/libm::sqrt(sum/input.len() as f64+epsilon as f64)) as f32)
}
fn normalize(weights:&[u8],t:&Tensor<'_>,input:&[f32],output:&mut[f32],scratch:&mut[f32],epsilon:f32)->Result<(),Error>{
    if input.len()!=output.len()||scratch.len()<input.len(){return Err(Error::Shape);}
    let scale=rms(input,epsilon)?;let scratch=&mut scratch[..input.len()];row(weights,t,0)?.decode(scratch)?;
    for ((out,x),w) in output.iter_mut().zip(input).zip(scratch){*out=finite(*x*scale*(*w))?;}Ok(())
}
fn normalize_heads(weights:&[u8],t:&Tensor<'_>,values:&mut[f32],scratch:&mut[f32],width:usize,epsilon:f32)->Result<(),Error>{
    let scratch=&mut scratch[..width];row(weights,t,0)?.decode(scratch)?;
    for head in values.chunks_exact_mut(width){let scale=rms(head,epsilon)?;
        for (v,w) in head.iter_mut().zip(scratch.iter()){*v=finite(*v*scale*(*w))?;}}
    Ok(())
}
fn rotate(values:&mut[f32],frequencies:&[f32],position:usize,width:usize)->Result<(),Error>{
    for head in values.chunks_exact_mut(width){
        for (i,freq) in frequencies.iter().enumerate(){
            let(s,c)=libm::sincosf(position as f32*freq);
            let(a,b)=(head[i],head[i+width/2]);
            head[i]=finite(a*c-b*s)?;head[i+width/2]=finite(a*s+b*c)?;
        }
    }Ok(())
}
#[allow(clippy::too_many_arguments)]
fn attention(c:Config,context:usize,pos:usize,layer:usize,q:&[f32],keys:&[f32],values:&[f32],scores:&mut[f32],output:&mut[f32],go:&mut impl FnMut()->bool)->Result<(),Error>{
    let width=c.head_width;let kv_width=c.kv_heads*width;let scale=1.0/libm::sqrt(width as f64);
    for head in 0..c.heads {
        let kv_head=head/(c.heads/c.kv_heads); let query=&q[head*width..(head+1)*width];
        let mut max=f32::NEG_INFINITY;
        for (time,score) in scores[..=pos].iter_mut().enumerate(){
            check(go)?;let base=(layer*context+time)*kv_width+kv_head*width;
            let mut sum=0.0f64;for (a,b) in query.iter().zip(&keys[base..base+width]){sum+=(*a as f64)*(*b as f64);}
            *score=finite((sum*scale) as f32)?;max=max.max(*score);
        }
        let mut sum=0.0f64;
        for (i,score) in scores[..=pos].iter_mut().enumerate(){if i%256==0{check(go)?;}*score=libm::expf(*score-max);sum+=*score as f64;}
        if !sum.is_finite()||sum<=0.0{return Err(Error::NonFinite);}
        for column in 0..width {
            let mut value=0.0f64;
            for (time,score) in scores[..=pos].iter().enumerate(){
                if time%256==0{check(go)?;}
                let base=(layer*context+time)*kv_width+kv_head*width;
                value+=(*score as f64)*values[base+column] as f64;
            }
            output[head*width+column]=finite((value/sum) as f32)?;
        }
    }Ok(())
}
