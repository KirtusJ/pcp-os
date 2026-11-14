#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
mkdir -p dist
case "${1:-all}" in
  guest|web|all) mode=${1:-all} ;;
  *) echo 'usage: tools/build.sh [guest|web|all]' >&2; exit 2 ;;
esac
if [ "$mode" != web ]; then
  cargo build --locked --release --target x86_64-unknown-linux-musl -p night-post-guest
  "${MUSL_CC:-musl-gcc}" -static-pie -o dist/nightpost.elf target/x86_64-unknown-linux-musl/release/libnight_post_guest.a -lm
  strip dist/nightpost.elf
fi
if [ "$mode" != guest ]; then
  cargo build --locked --release --target wasm32v1-none -p night-post-web
  python3 tools/pack_web.py
fi
