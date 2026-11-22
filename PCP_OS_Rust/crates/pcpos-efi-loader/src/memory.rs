use crate::Result;
pub const PAGE: u64 = 4096;
pub const LOW: u64 = 1 << 30;
pub const HIGH: u64 = 4 << 30;
pub const END: u64 = 68 << 30;
pub const MODEL: u32 = 0x8000_0001;
pub const MAX: usize = 8;
const RESERVED: u64 = (1 << 63) | 0x40000 | 0x8000 | 0x2000 | 0x1000 | 0x20000 | 0x4000;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span {
    pub start: u64,
    pub size: u64,
}
impl Span {
    pub fn end(self) -> u64 {
        self.start + self.size
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub kind: u32,
    pub span: Span,
    pub attr: u64,
}
impl Region {
    fn ordinary(self) -> bool {
        self.attr & 8 != 0 && self.attr & RESERVED == 0
    }
    pub fn free(self) -> bool {
        self.kind == 7 && self.ordinary()
    }
}
#[derive(Clone, Copy)]
pub struct Map<'a> {
    raw: &'a [u8],
    step: usize,
}
impl<'a> Map<'a> {
    pub fn new(raw: &'a [u8], step: usize, version: u32) -> Result<Self> {
        let bad = "The firmware memory map contains an invalid range.";
        if version != 1 || step < 40 || step % 8 != 0 || raw.is_empty() || raw.len() % step != 0 {
            return Err(bad);
        }
        for b in raw.chunks_exact(step) {
            let start = word(b, 8);
            let pages = word(b, 24);
            if start % PAGE != 0 || pages == 0 || pages > (u64::MAX - start) / PAGE {
                return Err(bad);
            }
        }
        Ok(Self { raw, step })
    }
    pub fn iter(self) -> impl Iterator<Item = Region> + 'a {
        self.raw.chunks_exact(self.step).map(|b| Region {
            kind: u32::from_le_bytes(b[..4].try_into().unwrap()),
            span: Span {
                start: word(b, 8),
                size: word(b, 24) * PAGE,
            },
            attr: word(b, 32),
        })
    }
}
fn word(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}
pub fn high_plan(map: Map, need: u64) -> core::result::Result<([Span; MAX], usize), u64> {
    let mut best = [Span::default(); MAX];
    for d in map.iter().filter(|d| d.free()) {
        let lo = HIGH.max(d.span.start);
        let hi = END.min(d.span.end());
        if hi <= lo {
            continue;
        }
        let mut item = Span {
            start: lo,
            size: hi - lo,
        };
        for slot in &mut best {
            if item.size > slot.size || (item.size == slot.size && item.start < slot.start) {
                core::mem::swap(slot, &mut item);
            }
        }
    }
    let total = best.iter().map(|x| x.size).sum();
    if total < need {
        return Err(total);
    }
    let mut left = need;
    let mut used = 0;
    for s in &mut best {
        if left == 0 {
            break;
        }
        s.size = s.size.min(left);
        left -= s.size;
        used += 1;
    }
    Ok((best, used))
}
pub fn low(map: Map, after: u64, size: u64) -> Result<Span> {
    map.iter()
        .filter(|d| d.free())
        .filter_map(|d| {
            let start = d.span.start.max(after);
            let end = d.span.end().min(LOW);
            (end.saturating_sub(start) >= size).then_some(Span { start, size })
        })
        .min_by_key(|s| s.start)
        .ok_or("Not enough usable low memory for the kernel and its programs.")
}
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct Range {
    pub bytes: u32,
    pub start: u64,
    pub size: u64,
    pub kind: u32,
}
pub fn translate(map: Map, pieces: &[Span], output: &mut [Range]) -> Result<usize> {
    let mut covered = [0; MAX];
    let mut n = 0usize;
    if pieces.len() > MAX {
        return Err("Too many model ranges.");
    }
    for d in map.iter() {
        let mut addr = d.span.start;
        while addr < d.span.end() {
            let mut next = d.span.end();
            let mut kind = if d.free() {
                1
            } else if d.kind == 9 {
                3 // ACPI reclaimable after the kernel finishes using the tables
            } else if d.kind == 10 {
                4
            } else {
                2
            };
            for (i, p) in pieces.iter().enumerate() {
                if addr < p.start {
                    next = next.min(p.start)
                } else if addr < p.end() {
                    if d.kind != 2 || !d.ordinary() {
                        return Err(
                            "A model range lacks suitable loader-owned pages in the final map.",
                        );
                    }
                    next = next.min(p.end());
                    kind = MODEL;
                    covered[i] += next - addr;
                    break;
                }
            }
            let len = next - addr;
            if n > 0
                && output[n - 1].kind == kind
                && output[n - 1].start + output[n - 1].size == addr
            {
                output[n - 1].size += len;
            } else {
                let slot = output
                    .get_mut(n)
                    .ok_or("The final firmware map exceeds the kernel map limit.")?;
                *slot = Range {
                    bytes: 20,
                    start: addr,
                    size: len,
                    kind,
                };
                n += 1;
            }
            addr = next;
        }
    }
    if pieces.iter().zip(covered).any(|(p, n)| p.size != n) {
        return Err("The final firmware map does not cover the model.");
    }
    Ok(n)
}
pub trait Pages {
    fn claim(&mut self, p: Span) -> Result<()>;
    fn free(&mut self, p: Span) -> Result<()>;
}
#[derive(Default)]
pub struct Owned {
    spans: [Span; 16],
    n: usize,
}
impl Owned {
    pub fn claim(&mut self, fw: &mut impl Pages, s: Span) -> Result<()> {
        if self.n == self.spans.len() || s.start % PAGE != 0 || s.size == 0 || s.size % PAGE != 0 {
            return Err("Invalid allocation.");
        }
        fw.claim(s)?;
        self.spans[self.n] = s;
        self.n += 1;
        Ok(())
    }
    pub fn release(&mut self, fw: &mut impl Pages, address: u64) -> Result<()> {
        let i = self.spans[..self.n]
            .iter()
            .position(|s| s.start == address)
            .ok_or("Unknown allocation.")?;
        fw.free(self.spans[i])?;
        self.n -= 1;
        self.spans[i] = self.spans[self.n];
        Ok(())
    }
    pub fn rollback(&mut self, fw: &mut impl Pages) -> bool {
        let mut clean = true;
        while self.n > 0 {
            self.n -= 1;
            clean &= fw.free(self.spans[self.n]).is_ok()
        }
        clean
    }
}

/// Refresh the translated map immediately before each exit attempt. Only a stale
/// key permits another attempt. The caller records when boot services may have
/// been partially shut down so cleanup can be restricted to memory services.
pub fn exit_with_map(
    mut next_key: impl FnMut() -> Result<usize>,
    mut exit: impl FnMut(usize) -> Result<bool>,
) -> Result<()> {
    for _ in 0..8 {
        let key = next_key()?;
        if exit(key)? {
            return Ok(());
        }
    }
    Err("Firmware could not complete the handoff.")
}
