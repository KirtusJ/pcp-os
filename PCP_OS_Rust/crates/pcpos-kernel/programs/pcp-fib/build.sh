#!/bin/sh
# Build the single preinstalled user-mode worker. Requires rustc with
# x86_64-unknown-none core and its bundled rust-lld.
set -eu
src=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
out=${1:?usage: build.sh OUTPUT}
rustc --edition 2021 --target x86_64-unknown-none -C opt-level=s -C panic=abort \
    -C relocation-model=static -C "link-arg=-T$src/link.ld" -C link-arg=--build-id=none \
    -C strip=symbols "$src/main.rs" -o "$out"
