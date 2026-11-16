//! Greedy inference for the pinned float32 TinyStories checkpoint.
const DIM: usize = 64;
const KV: usize = 32;
const HIDDEN: usize = 172;
const LAYERS: usize = 5;
const HEAD: usize = 8;
const CHECKPOINT_POSITIONS: usize = 512;
pub const VOCAB: usize = 512;

#[repr(align(4))]
struct Aligned([u8; 1_056_540]);
static CHECKPOINT: Aligned = Aligned(*include_bytes!(env!("PCPOS_WORKER_CHECKPOINT")));

pub struct Model {
    embedding: &'static [f32],
    attention_norm: &'static [f32],
    query: &'static [f32],
    key: &'static [f32],
    value: &'static [f32],
    project: &'static [f32],
    ffn_norm: &'static [f32],
    up: &'static [f32],
    down: &'static [f32],
    gate: &'static [f32],
    final_norm: &'static [f32],
    cosine: &'static [f32],
    sine: &'static [f32],
}

type Cache<const N: usize> = [[[f32; KV]; N]; LAYERS];

pub struct Context<const N: usize = 4> {
    keys: Cache<N>,
    values: Cache<N>,
}

impl<const N: usize> Context<N> {
    pub const fn new() -> Self {
        Self {
            keys: [[[0.0; KV]; N]; LAYERS],
            values: [[[0.0; KV]; N]; LAYERS],
        }
    }
}

fn layer(block: &[f32], n: usize) -> &[f32] {
    let stride = block.len() / LAYERS;
    &block[n * stride..(n + 1) * stride]
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0.0, |sum, (x, y)| sum + x * y)
}

fn mat(output: &mut [f32], weights: &[f32], input: &[f32]) {
    for (v, row) in output.iter_mut().zip(weights.chunks_exact(input.len())) {
        *v = dot(row, input);
    }
}

fn sqrt(value: f32) -> f32 {
    let result: f32;
    // SSE2 is part of the x86-64 userspace ABI. The kernel saves XMM state.
    unsafe {
        core::arch::asm!("sqrtss {0}, {1}", lateout(xmm_reg) result, in(xmm_reg) value, options(pure, nomem, nostack));
    }
    result
}

fn norm(output: &mut [f32; DIM], weights: &[f32], x: &[f32; DIM]) {
    let scale = 1.0 / sqrt(dot(x, x) / DIM as f32 + 1.0e-5);
    for i in 0..DIM {
        output[i] = weights[i] * (scale * x[i]);
    }
}

fn exp(x: f32) -> f32 {
    // Softmax and SiLU only see finite checkpoint activations. Reduce to
    // [-ln(2)/2, ln(2)/2] and rescale the Taylor approximation by a power of two.
    if x < -87.0 {
        return 0.0;
    }
    if x > 88.0 {
        return f32::INFINITY;
    }
    let scaled = x * core::f32::consts::LOG2_E;
    let n = (scaled + if scaled >= 0.0 { 0.5 } else { -0.5 }) as i32;
    let r = x - n as f32 * core::f32::consts::LN_2;
    let mut p = 1.0;
    for k in (1..=7).rev() {
        p = 1.0 + r * p / k as f32;
    }
    f32::from_bits(((n + 127) as u32) << 23) * p
}

impl Model {
    pub fn load() -> Self {
        let bytes = &CHECKPOINT.0;
        // The builder checks the entire checkpoint digest and header. The byte
        // array is four-byte aligned, little endian, and its header is 28 bytes.
        let mut data = unsafe {
            core::slice::from_raw_parts(
                bytes.as_ptr().add(28).cast::<f32>(),
                (bytes.len() - 28) / 4,
            )
        };
        let mut take = |count| {
            let (field, rest) = data.split_at(count);
            data = rest;
            field
        };
        let model = Self {
            embedding: take(VOCAB * DIM),
            attention_norm: take(LAYERS * DIM),
            query: take(LAYERS * DIM * DIM),
            key: take(LAYERS * KV * DIM),
            value: take(LAYERS * KV * DIM),
            project: take(LAYERS * DIM * DIM),
            ffn_norm: take(LAYERS * DIM),
            up: take(LAYERS * HIDDEN * DIM),
            down: take(LAYERS * DIM * HIDDEN),
            gate: take(LAYERS * HIDDEN * DIM),
            final_norm: take(DIM),
            cosine: take(512 * HEAD / 2),
            sine: take(512 * HEAD / 2),
        };
        assert!(data.is_empty());
        model
    }

    fn rotate(&self, vector: &mut [f32], pos: usize) {
        for (i, pair) in vector.chunks_exact_mut(2).enumerate() {
            let index = pos * (HEAD / 2) + i % (HEAD / 2);
            let (c, s) = (self.cosine[index], self.sine[index]);
            let (a, b) = (pair[0], pair[1]);
            pair[0] = a * c - b * s;
            pair[1] = a * s + b * c;
        }
    }

    fn forward<const N: usize>(
        &self, ctx: &mut Context<N>, token: usize, pos: usize, mut logits: Option<&mut [f32; VOCAB]>,
    ) -> usize {
        let mut x = [0.0; DIM];
        x.copy_from_slice(&self.embedding[token * DIM..(token + 1) * DIM]);
        let (mut xb, mut tmp, mut q) = ([0.0; DIM], [0.0; DIM], [0.0; DIM]);
        let (mut a, mut b) = ([0.0; HIDDEN], [0.0; HIDDEN]);
        for l in 0..LAYERS {
            norm(&mut xb, layer(self.attention_norm, l), &x);
            mat(&mut q, layer(self.query, l), &xb);
            mat(&mut ctx.keys[l][pos], layer(self.key, l), &xb);
            mat(&mut ctx.values[l][pos], layer(self.value, l), &xb);
            self.rotate(&mut q, pos);
            self.rotate(&mut ctx.keys[l][pos], pos);
            for (h, query) in q.chunks_exact(HEAD).enumerate() {
                let start = h / (DIM / KV) * HEAD;
                let mut scores = [0.0; N];
                let scores = &mut scores[..=pos];
                for (t, score) in scores.iter_mut().enumerate() {
                    *score = dot(query, &ctx.keys[l][t][start..start + HEAD]) / sqrt(HEAD as f32);
                }
                let largest = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                for score in &mut *scores {
                    *score = exp(*score - largest);
                }
                let sum: f32 = scores.iter().sum();
                let head = &mut xb[h * HEAD..(h + 1) * HEAD];
                head.fill(0.0);
                for (t, score) in scores.iter().enumerate() {
                    let weight = score / sum;
                    for i in 0..HEAD {
                        head[i] += weight * ctx.values[l][t][start + i];
                    }
                }
            }
            mat(&mut tmp, layer(self.project, l), &xb);
            for i in 0..DIM {
                x[i] += tmp[i];
            }
            norm(&mut xb, layer(self.ffn_norm, l), &x);
            mat(&mut a, layer(self.up, l), &xb);
            mat(&mut b, layer(self.gate, l), &xb);
            for i in 0..HIDDEN {
                a[i] = (a[i] * (1.0 / (1.0 + exp(-a[i])))) * b[i];
            }
            mat(&mut tmp, layer(self.down, l), &a);
            for i in 0..DIM {
                x[i] += tmp[i];
            }
        }
        norm(&mut xb, self.final_norm, &x);
        let (mut best, mut largest) = (0, f32::NEG_INFINITY);
        for (i, row) in self.embedding.chunks_exact(DIM).enumerate() {
            let logit = dot(row, &xb);
            if let Some(values) = &mut logits { values[i] = logit; }
            if logit > largest {
                best = i;
                largest = logit;
            }
        }
        best
    }

    /// Consume the full prompt, then generate three greedy predictions. When requested,
    /// capture the vocabulary scores that produced the first predicted token.
    pub fn complete<const N: usize>(
        &self,
        ctx: &mut Context<N>,
        tokens: &[usize],
        prompt_logits: Option<&mut [f32; VOCAB]>,
    ) -> Option<[usize; 3]> {
        let (&last, prefix) = tokens.split_last()?;
        if tokens.len().checked_add(2)? > N.min(CHECKPOINT_POSITIONS)
            || tokens.iter().any(|&id| id >= VOCAB)
        {
            return None;
        }
        for (position, &token) in prefix.iter().enumerate() {
            self.forward(ctx, token, position, None);
        }
        let first = self.forward(ctx, last, prefix.len(), prompt_logits);
        let second = self.forward(ctx, first, tokens.len(), None);
        let third = self.forward(ctx, second, tokens.len() + 1, None);
        Some([first, second, third])
    }

    pub fn continue_token<const N: usize>(
        &self, ctx: &mut Context<N>, token: usize, at: usize,
    ) -> usize {
        self.forward(ctx, token, at, None)
    }

    pub fn predict(&self, ctx: &mut Context, prompt: usize) -> [usize; 3] {
        self.complete(ctx, &[1, 300 + prompt], None).expect("valid indexed prompt")
    }
}
