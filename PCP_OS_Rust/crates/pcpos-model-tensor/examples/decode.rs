//! Host parity helper: one encoded row in, little-endian f32 bytes out.
//! Never marks a model ready or runs generated model output.
use pcpos_model_tensor::{Row, TensorType::*};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 { return Err("usage: decode TYPE COLUMNS INPUT OUTPUT".into()); }
    let kind = match args[1].as_str() {
        "0" => F32, "1" => F16, "6" => Q5_0, "8" => Q8_0,
        "12" => Q4K, "14" => Q6K, "30" => Bf16, _ => return Err("unsupported type".into()),
    };
    let columns: usize = args[2].parse()?;
    if columns == 0 || columns > 1_048_576 { return Err("host helper row limit".into()); }
    let (values, size) = kind.block_layout();
    if columns as u64 % values != 0 || std::fs::metadata(&args[3])?.len() != columns as u64 / values * size {
        return Err("input is not exactly one row".into());
    }
    // Bound the actual read too, in case the file grows after metadata inspection.
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(&args[3])?.take(columns as u64 / values * size + 1).read_to_end(&mut bytes)?;
    let row = Row::new(kind, columns, &bytes).map_err(|e| format!("{e:?}"))?;
    let mut values = vec![0.0; columns];
    row.decode(&mut values).map_err(|e| format!("{e:?}"))?;
    let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&args[4])?;
    use std::io::Write;
    for v in values { output.write_all(&v.to_le_bytes())?; }
    Ok(())
}
