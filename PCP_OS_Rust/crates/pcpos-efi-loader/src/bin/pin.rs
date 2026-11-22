//! Bind the independent assembly receipt to the binary before any medium is made.
use pcpos_model_media::{Manifest, HEADER_BYTES, MAX_SHARDS};
use pcpos_preloaded_loader::{
    elf::{measurement, Elf},
    FIXTURE, NAMES,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn hex(v: &[u8]) -> String {
    v.iter().map(|x| format!("{x:02x}")).collect()
}
fn limited(p: &Path, max: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let f = fs::File::open(p)?;
    let meta = f.metadata()?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > max {
        return Err("missing or oversized input".into());
    }
    let mut v = Vec::new();
    f.take(max + 1).read_to_end(&mut v)?;
    if v.len() as u64 != meta.len() {
        return Err("input length changed".into());
    }
    Ok(v)
}
fn generate(parts: &Path, receipt: &Path, pin: &str) -> Result<String> {
    let raw = limited(receipt, 1 << 20)?;
    if hex(&Sha256::digest(&raw)) != pin {
        return Err("assembly receipt differs from its independent pin".into());
    }
    let r: Value = serde_json::from_slice(&raw)?;
    if r["format"] != "pcpos-preloaded-inputs/1" {
        return Err("unknown receipt format".into());
    }
    let features = r["kernel_features"]
        .as_array()
        .ok_or("missing kernel_features")?;
    if !["proposals", "model-memory"]
        .iter()
        .all(|v| features.iter().any(|s| s == *v))
        || features.iter().any(|s| s == "model-memory-fixture")
    {
        return Err("expected normal kernel features without fixture".into());
    }
    let files = r["files"].as_object().ok_or("missing input pins")?;
    if files.len() != NAMES.len() {
        return Err("unexpected file inventory".into());
    }
    let mut input = Vec::new();
    let mut pins = [[0u8; 32]; 4];
    let mut sizes = [0usize; 4];
    for (i, &name) in NAMES.iter().enumerate() {
        let bytes = limited(
            &parts.join(name),
            if i == 3 {
                (HEADER_BYTES + 32 * MAX_SHARDS as usize) as u64
            } else {
                64 << 20
            },
        )?;
        pins[i] = Sha256::digest(&bytes).into();
        sizes[i] = bytes.len();
        if files.get(name).and_then(Value::as_str) != Some(&hex(&pins[i])) {
            return Err(format!("input SHA-256 differs: {name}").into());
        }
        input.push(bytes);
    }
    if input[0].windows(FIXTURE.len()).any(|s| s == FIXTURE) {
        return Err("fixture banner in kernel".into());
    }
    for data in &input[1..=2] {
        if data.get(..7) != Some(b"\x7fELF\x02\x01\x01") || data.get(18..20) != Some(&[62, 0]) {
            return Err("expected x86-64 guest ELF".into());
        }
    }
    let manifest_pin = hex(&pins[3]);
    if r["worker_manifest_sha256"].as_str() != Some(&manifest_pin)
        || !input[2].windows(64).any(|s| s == manifest_pin.as_bytes())
    {
        return Err("worker build manifest pin differs".into());
    }
    let m = Manifest::parse(&input[3], pins[3]).map_err(|_| "invalid pinned manifest")?;
    let elf = Elf::parse(&input[0])?;
    let measurement = hex(&measurement(&input[0])?);
    Ok(format!("pub const SIZES:[usize;4]={sizes:?};\npub const PINS:[[u8;32];4]={pins:?};\npub const BASE:u64={};\npub const END:u64={};\npub const MODEL:u64={};\npub const MEASUREMENT:&str={measurement:?};\npub const UI:&str={:?};\npub const WORKER:&str={:?};\npub const MANIFEST:&str={manifest_pin:?};\n",elf.base,elf.end,m.model_bytes(),hex(&pins[1]),hex(&pins[2])))
}
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 4 {
        return Err("expected PARTS RECEIPT SHA256 OUTPUT".into());
    }
    fs::write(
        &a[3],
        generate(
            Path::new(&a[0]),
            Path::new(&a[1]),
            a[2].to_str().ok_or("invalid pin")?,
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put32(b: &mut [u8], position: usize, value: u32) {
        b[position..position + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn kernel() -> Vec<u8> {
        let mut b = vec![0; 0x3000];
        b[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
        for (p, v) in [
            (16, 2u16),
            (18, 3),
            (42, 32),
            (44, 1),
            (46, 40),
            (48, 4),
            (50, 3),
        ] {
            b[p..p + 2].copy_from_slice(&v.to_le_bytes());
        }
        for (p, v) in [
            (20, 1),
            (24, 0x200000),
            (28, 52),
            (32, 128),
            (52, 1),
            (56, 0x1000),
            (60, 0x200000),
            (64, 0x200000),
            (68, 0x2000),
            (72, 0x2000),
            (76, 5),
        ] {
            put32(&mut b, p, v);
        }
        let names = b"\0.text\0.rodata\0.shstrtab\0";
        for (i, name, addr, off, len) in [
            (1, 1, 0x200000, 0x1000, 0x1000),
            (2, 7, 0x201000, 0x2000, 0x1000),
            (3, 15, 0, 350, names.len() as u32),
        ] {
            let p = 128 + i * 40;
            for (j, v) in [(0, name), (12, addr), (16, off), (20, len)] {
                put32(&mut b, p + j, v);
            }
        }
        b[350..350 + names.len()].copy_from_slice(names);
        b
    }
    #[test]
    fn independent_receipt_binds_every_input_and_worker_pin() {
        let tmp = std::env::temp_dir().join(format!(
            "pcpos-pin-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&tmp).unwrap();
        let mut manifest = b"PCPMDL01".to_vec();
        manifest.extend(7u64.to_le_bytes());
        manifest.extend(1024u32.to_le_bytes());
        manifest.extend(1u32.to_le_bytes());
        let h = Sha256::digest([0; 7]);
        manifest.extend(h);
        manifest.extend(h);
        let mut ui = vec![0; 64];
        ui[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        ui[18] = 62;
        let mut worker = ui.clone();
        let pin = hex(&Sha256::digest(&manifest));
        worker.extend(pin.as_bytes());
        let mut files = serde_json::Map::new();
        for (name, data) in NAMES.iter().zip([kernel(), ui, worker, manifest]) {
            fs::write(tmp.join(name), &data).unwrap();
            files.insert(name.to_string(), Value::String(hex(&Sha256::digest(data))));
        }
        let original = serde_json::json!({"format":"pcpos-preloaded-inputs/1","kernel_features":["proposals","model-memory"],
            "worker_manifest_sha256":pin,"files":files});
        let path = tmp.join("receipt.json");
        let write = |r: &Value| {
            let b = serde_json::to_vec(r).unwrap();
            fs::write(&path, &b).unwrap();
            hex(&Sha256::digest(b))
        };
        let p = write(&original);
        assert!(generate(&tmp, &path, &p).unwrap().contains("MEASUREMENT"));
        assert!(generate(&tmp, &path, &"0".repeat(64)).is_err());
        for (key, value) in [
            ("worker_manifest_sha256", Value::String("0".repeat(64))),
            (
                "kernel_features",
                serde_json::json!(["proposals", "model-memory", "model-memory-fixture"]),
            ),
        ] {
            let mut changed = original.clone();
            changed[key] = value;
            assert!(generate(&tmp, &path, &write(&changed)).is_err());
        }
        let mut k = kernel();
        k.extend(FIXTURE);
        fs::write(tmp.join(NAMES[0]), &k).unwrap();
        assert!(generate(&tmp, &path, &write(&original))
            .unwrap_err()
            .to_string()
            .contains("input SHA-256 differs"));
        let mut repinned = original;
        repinned["files"][NAMES[0]] = Value::String(hex(&Sha256::digest(&k)));
        assert!(generate(&tmp, &path, &write(&repinned))
            .unwrap_err()
            .to_string()
            .contains("fixture banner"));
        fs::remove_dir_all(tmp).unwrap();
    }
}
