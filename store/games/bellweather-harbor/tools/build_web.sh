#!/usr/bin/env bash
# Requires rustc and Cargo 1.84+, the wasm32v1-none target, and Python 3.
set -euo pipefail

project="$(cd "$(dirname "$0")/.." && pwd)"
output="${1:-$project/dist/Bellweather_Harbor.html}"
if [[ "$output" != /* ]]; then
    output="$PWD/$output"
fi
target_dir="${CARGO_TARGET_DIR:-$project/target}"
mkdir -p "$target_dir"
target_dir="$(cd "$target_dir" && pwd)"
export CARGO_TARGET_DIR="$target_dir"

cd "$project"
target=wasm32v1-none
sysroot="$(rustc --print sysroot)"
build_flags=()
if ! compgen -G "$sysroot/lib/rustlib/$target/lib/libcore-*.rlib" >/dev/null; then
    # Some offline toolchains supply Rust sources and their vendored dependencies
    # instead of prebuilt wasm targets. Prefer the stable precompiled target.
    rust_source="$sysroot/lib/rustlib/src/rust/library"
    if [[ ! -d "$rust_source/vendor" ]]; then
        echo "Install the compiler target with: rustup target add $target" >&2
        exit 1
    fi
    export RUSTC_BOOTSTRAP=1
    build_flags=(-Z build-std=core,compiler_builtins -Z build-std-features=compiler-builtins-mem
        --config 'source.crates-io.replace-with="bundled"'
        --config "source.bundled.directory='$rust_source/vendor'")
fi
cargo build --release --offline --locked --target "$target" -p bellweather-web "${build_flags[@]}"
python3 tools/pack_web.py "$target_dir/$target/release/bellweather_web.wasm" web/player.html "$output"
