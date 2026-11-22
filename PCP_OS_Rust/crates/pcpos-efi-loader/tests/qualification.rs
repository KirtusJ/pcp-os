use pcpos_model_media::Manifest;
use pcpos_preloaded_loader::{elf::Elf, memory::*, payload::*, Result};
use sha2::{Digest, Sha256};
fn digest(b: &[u8]) -> [u8; 32] {
    Sha256::digest(b).into()
}
fn descriptor(kind: u32, start: u64, pages: u64, attr: u64) -> [u8; 48] {
    let mut b = [0; 48];
    b[..4].copy_from_slice(&kind.to_le_bytes());
    for (pos, v) in [(8, start), (24, pages), (32, attr)] {
        b[pos..pos + 8].copy_from_slice(&v.to_le_bytes())
    }
    b
}
fn map(b: &[u8]) -> Map<'_> {
    Map::new(b, 48, 1).unwrap()
}
#[test]
fn restricted_memory_and_changed_final_attributes() {
    for attr in [
        0,
        4,
        8 | 1 << 63,
        8 | 0x40000,
        8 | 0x8000,
        8 | 0x2000,
        8 | 0x1000,
        8 | 0x20000,
        8 | 0x4000,
    ] {
        let b = [
            descriptor(7, HIGH, 16, attr),
            descriptor(7, 0x0200_0000, 16, attr),
        ]
        .concat();
        assert_eq!(high_plan(map(&b), PAGE).unwrap_err(), 0);
        assert!(low(map(&b), PAGE, PAGE).is_err());
        let mut out = [Range::default(); 8];
        assert_eq!(translate(map(&b), &[], &mut out).unwrap(), 2);
        let types: [u32; 2] = [out[0].kind, out[1].kind];
        assert_eq!(types, [2, 2]);
        let piece = Span {
            start: HIGH,
            size: PAGE,
        };
        assert!(translate(map(&descriptor(2, HIGH, 1, attr)), &[piece], &mut out).is_err());
        assert!(translate(map(&descriptor(7, HIGH, 1, 15)), &[piece], &mut out).is_err());
        assert_eq!(
            translate(map(&descriptor(2, HIGH, 1, 15)), &[piece], &mut out).unwrap(),
            1
        );
        let marked = out[0].kind;
        assert_eq!(marked, MODEL);
    }
}
#[test]
fn fragmented_high_capacity_and_final_map_coverage() {
    let mut raw = Vec::new();
    for i in 0..9 {
        raw.extend(descriptor(7, HIGH + 4096 * 32 * i, i + 1, 15));
    }
    raw.extend(descriptor(7, END, 1000, 15));
    let total = (2..=9).sum::<u64>() * PAGE;
    assert_eq!(high_plan(map(&raw), total + PAGE).unwrap_err(), total);
    let (plan, n) = high_plan(map(&raw), total - PAGE).unwrap();
    assert_eq!(n, 8);
    assert_eq!(plan[0].size, 9 * PAGE);
    assert_eq!(plan[7].size, PAGE);
    let mut final_map = Vec::new();
    for p in plan {
        final_map.extend(descriptor(2, p.start, p.size / PAGE, 15))
    }
    let mut ranges = [Range::default(); 16];
    assert_eq!(translate(map(&final_map), &plan, &mut ranges).unwrap(), 8);
    assert!(translate(map(&final_map[48..]), &plan, &mut ranges).is_err());
    assert!(translate(map(&final_map), &plan, &mut ranges[..1]).is_err());
    assert!(Map::new(&descriptor(7, u64::MAX - 4095, 1, 15), 48, 1).is_err());
    assert!(Map::new(&raw, 39, 1).is_err());
}
#[derive(Default)]
struct Ledger {
    live: Vec<Span>,
    limit: usize,
    released: Vec<Span>,
}
impl Pages for Ledger {
    fn claim(&mut self, p: Span) -> Result<()> {
        if self.live.len() == self.limit {
            return Err("full");
        }
        self.live.push(p);
        Ok(())
    }
    fn free(&mut self, p: Span) -> Result<()> {
        let i = self.live.iter().position(|&s| s == p).ok_or("unknown")?;
        self.live.remove(i);
        self.released.push(p);
        Ok(())
    }
}
#[test]
fn allocation_rolls_back_and_does_not_release_staging_twice() {
    for permitted in 0..=4 {
        let mut fw = Ledger {
            limit: permitted,
            ..Ledger::default()
        };
        let mut owned = Owned::default();
        for i in 0..4 {
            let result = owned.claim(
                &mut fw,
                Span {
                    start: HIGH + i * PAGE,
                    size: PAGE,
                },
            );
            if result.is_err() {
                break;
            }
        }
        if permitted >= 2 {
            owned.release(&mut fw, HIGH + PAGE).unwrap();
            assert!(owned.release(&mut fw, HIGH + PAGE).is_err())
        }
        assert!(owned.rollback(&mut fw));
        assert!(fw.live.is_empty());
        assert_eq!(fw.released.len(), permitted);
    }
}
fn manifest(data: &[u8], width: usize) -> Vec<u8> {
    let mut m = b"PCPMDL01".to_vec();
    m.extend((data.len() as u64).to_le_bytes());
    m.extend((width as u32).to_le_bytes());
    m.extend((data.len().div_ceil(width) as u32).to_le_bytes());
    m.extend(digest(data));
    for s in data.chunks(width) {
        m.extend(digest(s))
    }
    m
}
struct Files {
    shards: Vec<Vec<u8>>,
    pos: usize,
    closed: usize,
    error_at: Option<usize>,
    progress: Vec<u64>,
}
impl Media for Files {
    type File = usize;
    fn open(&mut self, name: &[u8]) -> Result<usize> {
        self.pos = 0;
        let id = std::str::from_utf8(&name[6..11]).unwrap().parse().unwrap();
        Ok(id)
    }
    fn read(&mut self, i: usize, out: &mut [u8]) -> Result<usize> {
        if self.error_at == Some(self.pos) {
            return Err("I/O");
        }
        let data = &self.shards[i][self.pos..];
        let n = 317.min(out.len()).min(data.len());
        out[..n].copy_from_slice(&data[..n]);
        self.pos += n;
        Ok(n)
    }
    fn close(&mut self, _: usize) -> Result<()> {
        self.closed += 1;
        Ok(())
    }
    fn progress(&mut self, n: u64, _: u64) {
        self.progress.push(n)
    }
}
#[test]
fn verified_destination_and_refusal_cases() {
    let data: Vec<u8> = (0..14717).map(|i| (i % 251) as u8).collect();
    for trial in 0..7 {
        let mut bytes = manifest(&data, 8192);
        let mut file = Files {
            shards: data.chunks(8192).map(<[u8]>::to_vec).collect(),
            pos: 0,
            closed: 0,
            error_at: None,
            progress: vec![],
        };
        match trial {
            1 => file.shards[0][4096] ^= 1,
            2 => {
                file.shards[1].pop();
            }
            3 => file.shards[0].push(0),
            4 => bytes[24] ^= 1,
            5 => file.error_at = Some(317),
            6 => bytes[56] ^= 1,
            _ => {}
        }
        let m = Manifest::parse(&bytes, digest(&bytes)).unwrap();
        let mut a = vec![0xa5; 4098];
        let mut b = vec![0xa5; 3 * 4096 + 2];
        let r = load_model(
            &mut file,
            &m,
            &mut [&mut a[1..4097], &mut b[1..1 + 3 * 4096]],
        );
        assert_eq!(r.is_ok(), trial == 0);
        assert_eq!([a[0], a[4097], b[0], b[3 * 4096 + 1]], [0xa5; 4]);
        assert!(file.closed >= 1);
        if trial == 0 {
            assert_eq!(&a[1..4097], &data[..4096]);
            assert_eq!(&b[1..1 + data.len() - 4096], &data[4096..]);
            assert!(b[1 + data.len() - 4096..1 + 3 * 4096]
                .iter()
                .all(|&x| x == 0));
            assert_eq!(file.progress, [8192, 14717]);
        }
    }
}
#[test]
fn runtime_rechecks_manifest_after_pin_and_rejects_malformed_geometry() {
    let mut m = manifest(&[4u8; 2048], 1024);
    let trusted = digest(&m);
    m[0] ^= 1;
    assert!(Manifest::parse(&m, trusted).is_err());
    assert!(Manifest::parse(&m, digest(&m)).is_err());
    let mut b = vec![0u8; 160];
    b[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
    b[16..18].copy_from_slice(&2u16.to_le_bytes());
    b[18..20].copy_from_slice(&3u16.to_le_bytes());
    for (i, v) in [
        (20, 1u32),
        (24, 0x200000),
        (28, 52),
        (52, 1),
        (56, 128),
        (60, 0x200000),
        (64, 0x200000),
        (68, 8),
        (72, 32),
        (76, 5),
    ] {
        b[i..i + 4].copy_from_slice(&v.to_le_bytes())
    }
    b[42..44].copy_from_slice(&32u16.to_le_bytes());
    b[44..46].copy_from_slice(&1u16.to_le_bytes());
    let elf = Elf::parse(&b).unwrap();
    assert_eq!(elf.end - elf.base, PAGE);
    assert_eq!(elf.segments()[0].file, 8);
    for (offset, val) in [
        (56, 158u32),
        (60, 42),
        (68, 33),
        (72, u32::MAX),
        (76, 4),
        (28, u32::MAX),
    ] {
        let mut changed = b.clone();
        changed[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
        assert!(Elf::parse(&changed).is_err())
    }
}
fn entry(name: &str, size: u64) -> Vec<u8> {
    let mut b = vec![0u8; 80];
    for c in name.encode_utf16().chain([0]) {
        b.extend(c.to_le_bytes())
    }
    let n = b.len() as u64;
    b[..8].copy_from_slice(&n.to_le_bytes());
    b[8..16].copy_from_slice(&size.to_le_bytes());
    b
}
#[test]
fn directory_is_complete_order_independent_and_bounded() {
    let m = manifest(&[7u8; 1200], 1024);
    let m = Manifest::parse(&m, digest(&m)).unwrap();
    for trial in 0..8 {
        let mut files = vec![
            entry("other", 24),
            entry("SHARD-00001.BIN", 176),
            entry("shard-00000.bin", 1024),
        ];
        match trial {
            1 => {
                files.pop();
            }
            2 => files.push(files[1].clone()),
            3 => files.push(entry("shard-00002.bin", 1)),
            4 => files[1] = entry("shard-00001.bin", 177),
            5 => files[1][72] = 16,
            6 => files[1].truncate(81),
            7 => files[1] = entry("shard-0000x.bin", 176),
            _ => {}
        }
        let mut files = files.into_iter();
        let r = inventory(&m, |b| {
            let Some(f) = files.next() else { return Ok(0) };
            b[..f.len()].copy_from_slice(&f);
            Ok(f.len())
        });
        assert_eq!(r.is_ok(), trial == 0);
    }
}

#[test]
fn exit_refreshes_each_stale_key_and_stops_after_permanent_errors() {
    for (stale, hard, attempts, ok) in [
        (1, false, 2, true),
        (12, false, 8, false),
        (0, true, 1, false),
    ] {
        let mut maps = 0;
        let mut exits = 0;
        let result = exit_with_map(
            || {
                maps += 1;
                Ok(maps)
            },
            |key| {
                exits += 1;
                assert_eq!(key, exits);
                if hard {
                    return Err("device error");
                }
                Ok(exits > stale)
            },
        );
        assert_eq!(result.is_ok(), ok);
        assert_eq!(maps, attempts);
        assert_eq!(exits, attempts);
    }
    assert!(exit_with_map(|| Err("map unavailable"), |_| panic!("unexpected exit")).is_err());
}
#[test]
fn pinned_single_file_closes_after_every_short_read_or_refusal() {
    let data = vec![0x6b; 8192];
    for case in 0..5 {
        let mut f = Files {
            shards: vec![data.clone()],
            pos: 0,
            closed: 0,
            error_at: None,
            progress: vec![],
        };
        let mut pin = digest(&data);
        match case {
            1 => pin[0] ^= 1,
            2 => {
                f.shards[0].pop();
            }
            3 => f.shards[0].push(1),
            4 => f.error_at = Some(317),
            _ => {}
        };
        let mut destination = vec![0; data.len()];
        assert_eq!(
            pinned(&mut f, b"shard-00000.bin", &mut destination, pin).is_ok(),
            case == 0
        );
        assert_eq!(f.closed, 1);
        if case == 0 {
            assert_eq!(destination, data);
        }
    }
}
