//! Keyrook's allocation-free recursive cipher arena, shared by native and hosted workers.
#![no_std]

/// Two applications of a four-round 16-bit Feistel network. Each stage has a
/// separate small key so forward/backward intersection is practical.
fn stage(x: u16, key: u8, inverse: bool) -> u16 {
    let [mut l, mut r] = x.to_be_bytes();
    for i in 0..4 {
        let round = if inverse { 3-i } else { i };
        let k = key.rotate_left(round) ^ (round as u8).wrapping_mul(73);
        let f = |b: u8| (b ^ k).wrapping_mul(157).rotate_left(3).wrapping_add(k);
        if inverse { (l,r) = (r ^ f(l),l); } else { (l,r) = (r,l ^ f(r)); }
    }
    u16::from_be_bytes([l,r])
}

pub struct Episode {
    pub verified: bool,
    pub work: u64,
    pub branches: u32,
    pub strategy: u8,
    pub candidates: u32,
    pub key: Option<(u8,u8)>,
}

#[derive(Clone,Copy)]
pub struct Challenge { pub pairs: [(u16,u16);3], pub bits: u8 }

fn worker(task: &Challenge, table: &[(u16,u8)], start:u16, end:u16, depth:u8, result:&mut Episode) {
    result.branches += 1;
    if depth > 0 {
        let mid = start + (end-start)/2;
        worker(task,table,start,mid,depth-1,result);
        worker(task,table,mid,end,depth-1,result);
        return;
    }
    let (x,y) = task.pairs[0];
    for b in start..end {
        let b = b as u8;
        let mut confirm = |a:u8, work:&mut u64| {
            for &(p,c) in &task.pairs[1..] {
                *work += 2;
                if stage(stage(p,a,false),b,false) != c { return; }
            }
            result.candidates += 1;
            result.key = Some((a,b));
        };
        if result.strategy == 0 {
            for a in 0..(1u16<<task.bits) {
                result.work += 2;
                if stage(stage(x,a as u8,false),b,false)==y { confirm(a as u8,&mut result.work); }
            }
        } else {
            let middle = stage(y,b,true);
            result.work += 1;
            let index = table.partition_point(|&(v,_)| v < middle);
            for &(v,a) in &table[index..] {
                if v != middle { break; }
                confirm(a,&mut result.work);
            }
        }
    }
}

/// Attackers receive only public pairs. The referee alone verifies the result.
pub fn attack(task: &Challenge, depth:u8, strategy:u8) -> Episode {
    let end = 1u16.checked_shl(u32::from(task.bits)).unwrap_or(0);
    attack_range(task,0,end,depth,strategy)
}

/// Half-open partition of the second-stage key space. A separate external
/// worker builds its own forward table and charges that cost to its result.
pub fn attack_range(task: &Challenge, start:u16, end:u16, depth:u8, strategy:u8) -> Episode {
    let mut out = Episode { verified:false, work:0, branches:0, strategy, candidates:0, key:None };
    if !matches!(task.bits,4|6|8) || depth>3 || strategy>1 { return out; }
    let size = 1usize<<task.bits;
    if start>=end || usize::from(end)>size || end-start < (1u16<<depth) {return out;}
    let mut table = [(0u16,0u8);256];
    if strategy==1 {
        for (a,e) in table[..size].iter_mut().enumerate() {
            *e=(stage(task.pairs[0].0,a as u8,false),a as u8);
        }
        out.work=size as u64;
        table[..size].sort_unstable();
    }
    worker(task,&table[..size],start,end,depth,&mut out);
    if out.candidates!=1 { out.key=None; }
    out
}

pub fn fixture(bits:u8,seed:u32) -> Option<(Challenge,(u8,u8))> {
    if !matches!(bits,4|6|8) { return None; }
    let mut s=seed;
    let mut draw=|| { s=s.wrapping_mul(1664525).wrapping_add(1013904223); (s>>16) as u16 };
    let mask=(1u16<<bits)-1;
    let (a,b)=((draw()&mask) as u8,(draw()&mask) as u8);
    let offset=draw();
    let pairs=[0,0x1234,0xabcd].map(|p| {let x=p^offset;(x,stage(stage(x,a,false),b,false))});
    Some((Challenge{bits,pairs},(a,b)))
}

pub fn run_episode(bits:u8,seed:u32,depth:u8,policy:u8)->Episode {
    let Some((task,key))=fixture(bits,seed) else {
        return Episode {verified:false,work:0,branches:0,strategy:policy,candidates:0,key:None};
    };
    let mut out=attack(&task,depth,policy);
    out.verified=out.key==Some(key);
    out
}

/// Explore both attacks; prefer the lowest observed mean work per recovery.
/// Failed trials charge work too. Prices count stage evaluations only.
#[derive(Default,Clone,Copy)]
pub struct Memory { pub runs:[u32;2], pub wins:[u32;2], pub work:[u64;2] }
impl Memory {
    pub fn choose(&self)->u8 {
        if self.runs[0]==0 {return 0;}
        if self.runs[1]==0 {return 1;}
        if self.wins==[0,0] {return u8::from(self.runs[1]<self.runs[0]);}
        u8::from(self.wins[1]>0 && (self.wins[0]==0 ||
            u128::from(self.work[1])*u128::from(self.wins[0]) < u128::from(self.work[0])*u128::from(self.wins[1])))
    }
    pub fn observe(&mut self,e:&Episode) {
        let i=usize::from(e.strategy);
        if i<2 && e.work>0 {
            self.runs[i]=self.runs[i].saturating_add(1);
            self.wins[i]=self.wins[i].saturating_add(u32::from(e.verified));
            self.work[i]=self.work[i].saturating_add(e.work);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn permutation() {for x in 0..=65535 {assert_eq!(stage(stage(x,37,false),37,true),x);}}
    #[test]
    fn independent_workers_partition_public_pairs_without_double_counting_keys() {
        let public = Challenge { bits: 8, pairs: [(0xee0e,0x6346),(0xfc3a,0x8e9a),(0x45c3,0x0338)] };
        for method in [0,1] {
            let all=attack(&public,3,method);
            let left=attack_range(&public,0,128,0,method);
            let right=attack_range(&public,128,256,0,method);
            assert_eq!(all.key,Some((57,194)));
            assert_eq!((left.candidates,right.candidates),(0,1));
            assert_eq!(all.key,right.key);
            assert_eq!(left.work+right.work,all.work+u64::from(method)*256);
        }
        let impossible=Challenge { bits:8,pairs:[(0xee0e,0x6346),(0xee0e,0x6347),(0x45c3,0x0338)] };
        assert_eq!(attack(&impossible,0,1).candidates,0);
    }
    #[test] fn curriculum_and_delegation() {
        for bits in [4,6,8] {
            let mut memory=Memory::default();
            for (seed,expected) in [(0,0),(42,1),(31415,1)] {
                let p=memory.choose(); assert_eq!(p,expected);
                let e=run_episode(bits,seed,3,p);
                let flat=run_episode(bits,seed,0,p);
                assert!(e.verified); assert_eq!(e.branches,15); assert_eq!((flat.key,flat.work),(e.key,e.work));
                if seed==31415 {assert!(e.work < run_episode(bits,seed,3,0).work);}
                memory.observe(&e);
            }
        }
    }
}
