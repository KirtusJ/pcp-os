//! Host-only diagnostic: reads at most the bounded index prefix, never all weights.
use std::{fs::File, io::Read};
use pcpos_gguf::{Index, MAX_PREFIX_BYTES};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("usage: inspect MODEL.gguf [--prefix-file-bytes WHOLE_FILE_SIZE]")?;
    let file = File::open(path)?;
    let actual_size = file.metadata()?.len();
    let (size, scope) = match args.next() {
        None => (actual_size, "local-file-index-only"),
        Some(flag) if flag == "--prefix-file-bytes" => {
            let size = args.next().ok_or("missing whole-file size")?
                .to_str().ok_or("non-UTF-8 size")?.parse::<u64>()?;
            if size < actual_size { return Err("declared whole-file size is shorter than the prefix file".into()); }
            (size, "prefix-only-declared-size-unverified-weights")
        }
        Some(_) => return Err("unknown argument".into()),
    };
    if args.next().is_some() { return Err("unexpected extra argument".into()); }
    let mut prefix = Vec::new();
    file.take(MAX_PREFIX_BYTES as u64).read_to_end(&mut prefix)?;
    let index = Index::parse(&prefix, size).map_err(|e| format!("GGUF index refused: {e:?}"))?;
    println!("scope={scope}");
    println!("file_bytes={} prefix_bytes={} data_offset={} alignment={} metadata={} tensors={}",
             size, prefix.len(), index.data_offset, index.alignment,
             index.metadata().len(), index.tensors().len());
    println!("architecture={:?}", index.get("general.architecture"));
    for tensor in index.tensors() {
        println!("{:?}\t{:?}\t{:?}\t{}\t{}", tensor.name, tensor.dimensions(),
                 tensor.kind, tensor.offset, tensor.bytes);
    }
    Ok(())
}
