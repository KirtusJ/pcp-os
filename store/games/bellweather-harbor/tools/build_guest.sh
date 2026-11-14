#!/usr/bin/env bash
# Requires the Rust x86_64-unknown-linux-musl target and a musl C toolchain.
set -euo pipefail

project="$(cd "$(dirname "$0")/.." && pwd)"
output="${1:-$project/dist/bellweather.elf}"
if [[ "$output" != /* ]]; then
    output="$PWD/$output"
fi
target_dir="${CARGO_TARGET_DIR:-$project/target}"
mkdir -p "$target_dir" "$(dirname "$output")"
target_dir="$(cd "$target_dir" && pwd)"
export CARGO_TARGET_DIR="$target_dir"

cd "$project"
target=x86_64-unknown-linux-musl
cargo build --locked --release --offline --target "$target" -p bellweather-guest
"${MUSL_CC:-musl-gcc}" -static-pie -o "$output" "$target_dir/$target/release/libbellweather_guest.a" -lm
"${STRIP:-strip}" "$output"
echo "Built $output"
