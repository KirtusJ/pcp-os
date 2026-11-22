use pcpos_gguf::{Error, Index, TensorType, Value};

fn string(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len() as u64).to_le_bytes()); out.extend_from_slice(s);
}
fn u32_value(n: u32) -> Vec<u8> {
    let mut out = 4u32.to_le_bytes().to_vec(); out.extend_from_slice(&n.to_le_bytes()); out
}
fn text(s: &[u8]) -> Vec<u8> {
    let mut out = 8u32.to_le_bytes().to_vec(); string(&mut out, s); out
}
fn array(kind: u32, count: u64, data: &[u8]) -> Vec<u8> {
    let mut out = 9u32.to_le_bytes().to_vec();
    out.extend_from_slice(&kind.to_le_bytes()); out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(data); out
}
fn tensor(name: &[u8], dims: &[u64], kind: u32, offset: u64) -> Vec<u8> {
    let mut out = Vec::new(); string(&mut out, name);
    out.extend_from_slice(&(dims.len() as u32).to_le_bytes());
    for d in dims { out.extend_from_slice(&d.to_le_bytes()); }
    out.extend_from_slice(&kind.to_le_bytes()); out.extend_from_slice(&offset.to_le_bytes()); out
}
fn file(kv: &[(&str, Vec<u8>)], tensors: &[Vec<u8>], alignment: usize) -> Vec<u8> {
    let mut out = b"GGUF".to_vec(); out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
    out.extend_from_slice(&(kv.len() as u64).to_le_bytes());
    for (k, v) in kv { string(&mut out, k.as_bytes()); out.extend_from_slice(v); }
    for t in tensors { out.extend_from_slice(t); }
    while out.len() % alignment != 0 { out.push(0); } out
}
fn error(prefix: &[u8], len: u64) -> Error { Index::parse(prefix, len).unwrap_err() }

#[test]
fn large_file_ranges_are_not_host_slice_offsets() {
    let out = file(&[("general.architecture", text(b"qwen3"))],
                   &[tensor(b"output.weight", &[256, 256], 12, 1u64 << 32)], 32);
    let offset = (1u64 << 32) + out.len() as u64;
    let index = Index::parse(&out, offset + 36864).unwrap();
    assert_eq!(index.get("general.architecture"), Some(Value::String("qwen3")));
    let t = &index.tensors()[0];
    assert_eq!((t.offset, t.bytes, t.kind), (offset, 36864, TensorType::Q4K));
    assert_eq!(t.dimensions(), &[256, 256]);
    assert_eq!(error(&out, offset + 36863), Error::TensorRange);
}

#[test]
fn missing_prefix_differs_from_truncated_file_at_every_index_byte() {
    let out = file(&[("general.architecture", text(b"qwen3"))],
                   &[tensor(b"a", &[1], 0, 0)], 32);
    for end in 0..out.len() {
        assert!(matches!(error(&out[..end], out.len() as u64 + 4), Error::NeedMore { .. }), "{end}");
        assert_eq!(error(&out[..end], end as u64), Error::Truncated, "{end}");
    }
    assert!(Index::parse(&out, out.len() as u64 + 4).is_ok());
}

#[test]
fn duplicate_metadata_and_tensor_names_are_not_last_wins() {
    let out = file(&[("general.architecture", text(b"qwen3")),
                     ("general.architecture", text(b"other"))], &[], 32);
    assert_eq!(error(&out, out.len() as u64), Error::DuplicateKey);
    let out = file(&[], &[tensor(b"a", &[1], 0, 0), tensor(b"a", &[1], 0, 32)], 32);
    assert_eq!(error(&out, 1024), Error::DuplicateTensor);
}

#[test]
fn alignment_is_typed_bounded_and_applied_to_absolute_ranges() {
    let out = file(&[("general.alignment", u32_value(64))], &[tensor(b"a", &[1], 0, 64)], 64);
    let index = Index::parse(&out, 1024).unwrap();
    assert_eq!(index.alignment, 64);
    assert_eq!(index.tensors()[0].offset, out.len() as u64 + 64);
    for value in [u32_value(0), u32_value(3), u32_value(8192), text(b"32")] {
        let out = file(&[("general.alignment", value)], &[], 32);
        assert_eq!(error(&out, 1024), Error::Alignment);
    }
    let out = file(&[], &[tensor(b"a", &[1], 0, 1)], 32);
    assert_eq!(error(&out, 1024), Error::Alignment);
}

#[test]
fn unsorted_tensors_work_but_overlaps_fail() {
    let out = file(&[], &[tensor(b"a", &[8], 0, 32), tensor(b"b", &[8], 0, 0)], 32);
    assert!(Index::parse(&out, 1024).is_ok());
    let out = file(&[], &[tensor(b"a", &[16], 0, 0), tensor(b"b", &[8], 0, 32)], 32);
    assert_eq!(error(&out, 1024), Error::Overlap);
}

#[test]
fn dimensions_and_range_overflow_fail_before_allocation_of_weights() {
    for dims in [&[][..], &[0][..], &[1, 1, 1, 1, 1][..]] {
        let out = file(&[], &[tensor(b"a", dims, 0, 0)], 32);
        assert_eq!(error(&out, u64::MAX), Error::Dimensions);
    }
    for (dims, kind, offset) in [(vec![u64::MAX, 2], 0, 0), (vec![u64::MAX], 0, 0),
                                (vec![16], 0, u64::MAX - 31)] {
        let out = file(&[], &[tensor(b"a", &dims, kind, offset)], 32);
        assert_eq!(error(&out, u64::MAX), Error::Overflow);
    }
    let out = file(&[], &[tensor(b"a", &[1], 0, u64::MAX - 31)], 32);
    assert_eq!(error(&out, u64::MAX), Error::Overflow); // addition of directory size
}

#[test]
fn quantized_rows_must_be_whole_blocks_even_if_total_elements_fit() {
    let out = file(&[], &[tensor(b"a", &[128, 2], 12, 0)], 32);
    assert_eq!(error(&out, 1024), Error::QuantizedRow);
    let out = file(&[], &[tensor(b"a", &[256, 2], 14, 0)], 32);
    assert_eq!(Index::parse(&out, 1024).unwrap().tensors()[0].bytes, 420);
}

#[test]
fn invalid_types_boolean_utf8_and_keys_are_refused() {
    for kind in [4, 5, 99] {
        let out = file(&[], &[tensor(b"a", &[256], kind, 0)], 32);
        assert_eq!(error(&out, 1024), Error::TensorType);
    }
    let out = file(&[("x", vec![7, 0, 0, 0, 2])], &[], 32);
    assert_eq!(error(&out, 1024), Error::Boolean);
    let out = file(&[("x", text(&[0xff]))], &[], 32);
    assert_eq!(error(&out, 1024), Error::Utf8);
    for key in ["", "General.a", "general..a", "general.a\0", "a-b"] {
        let out = file(&[(key, u32_value(1))], &[], 32);
        assert_eq!(error(&out, 1024), Error::Key);
    }
    let out = file(&[("x", 99u32.to_le_bytes().to_vec())], &[], 32);
    assert_eq!(error(&out, 1024), Error::ValueType);
    let out = file(&[("x", array(99, 0, &[]))], &[], 32);
    assert_eq!(error(&out, 1024), Error::ValueType);
}

#[test]
fn string_arrays_preserve_bytes_order_and_empty_values() {
    let mut encoded = Vec::new();
    for s in ["", "hello", "é\0尾"] { string(&mut encoded, s.as_bytes()); }
    let out = file(&[("tokenizer.ggml.tokens", array(8, 3, &encoded))], &[], 32);
    let index = Index::parse(&out, out.len() as u64).unwrap();
    let Some(Value::Array(a)) = index.get("tokenizer.ggml.tokens") else { panic!() };
    assert_eq!((a.len(), a.value_type()), (3, 8));
    assert_eq!(a.iter().collect::<Result<Vec<_>, _>>().unwrap(),
               vec![Value::String(""), Value::String("hello"), Value::String("é\0尾")]);
}

#[test]
fn nested_arrays_have_depth_and_total_work_limits() {
    let child = array(4, 1, &7u32.to_le_bytes());
    let out = file(&[("x", array(9, 1, &child[4..]))], &[], 32);
    let index = Index::parse(&out, out.len() as u64).unwrap();
    let Some(Value::Array(a)) = index.get("x") else { panic!() };
    let Value::Array(b) = a.iter().next().unwrap().unwrap() else { panic!() };
    assert_eq!(b.iter().next().unwrap().unwrap(), Value::U32(7));
    let mut deep = u32_value(1);
    for _ in 0..9 { deep = array(u32::from_le_bytes(deep[..4].try_into().unwrap()), 1, &deep[4..]); }
    let out = file(&[("x", deep)], &[], 32);
    assert_eq!(error(&out, 1024), Error::Limit);
    let out = file(&[("x", array(0, 1_000_001, &[]))], &[], 32);
    assert_eq!(error(&out, u64::MAX), Error::Limit);
    let million = vec![0u8; 1_000_000];
    let out = file(&[("x", array(0, 1_000_000, &million)),
                     ("y", array(0, 1_000_000, &million))], &[], 32);
    assert_eq!(error(&out, out.len() as u64), Error::Limit);
}

#[test]
fn forged_counts_and_string_sizes_cannot_force_unbounded_reservation() {
    let mut out = file(&[], &[], 32);
    out[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(error(&out, u64::MAX), Error::Limit);
    out[8..16].copy_from_slice(&0u64.to_le_bytes());
    out[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(error(&out, u64::MAX), Error::Limit);
    let mut value = 8u32.to_le_bytes().to_vec(); value.extend_from_slice(&u64::MAX.to_le_bytes());
    let out = file(&[("x", value)], &[], 32);
    assert_eq!(error(&out, u64::MAX), Error::Limit);
}

#[test]
fn unsupported_version_endianness_and_nonzero_padding_are_explicit() {
    let mut out = file(&[], &[], 32);
    out[0] = b'X'; assert_eq!(error(&out, 32), Error::Magic); out[0] = b'G';
    out[4..8].copy_from_slice(&2u32.to_le_bytes()); assert_eq!(error(&out, 32), Error::Version);
    out[4..8].copy_from_slice(&3u32.to_be_bytes()); assert_eq!(error(&out, 32), Error::Version);
    out[4..8].copy_from_slice(&3u32.to_le_bytes()); out[31] = 1;
    assert_eq!(error(&out, 32), Error::Padding);
}
