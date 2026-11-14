#!/bin/sh
# Launch both store games in one measured PCP OS session. Extra arguments go to QEMU.
# For sound, e.g.: sh store/play-native.sh -audiodev sdl,id=snd -device AC97,audiodev=snd
set -eu

store=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
kernel_dir="$store/../PCP_OS_Rust/crates/pcpos-kernel"
kernel=${PCPOS_KERNEL_IMAGE:-"$kernel_dir/target/pcpos-kernel.elf32"}
if [ -z "${PCPOS_KERNEL_IMAGE:-}" ]; then
    sh "$kernel_dir/run.sh" build
fi
# Preserve a caller-relative image path before switching to store-relative module names.
case "$kernel" in
    /*) ;;
    *) kernel="$PWD/$kernel" ;;
esac
digest=$(python3 "$kernel_dir/tools/measure.py" "$kernel")

cd "$store"
status=0
qemu-system-x86_64 -cpu max -m 256M -vga std -nic none -no-reboot \
    -kernel "$kernel" -append "image=$digest" \
    -initrd games/night-post/dist/nightpost.elf,games/bellweather-harbor/dist/bellweather.elf \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 "$@" || status=$?
# With isa-debug-exit, the kernel returns 33 after verifying its final register.
[ "$status" = 33 ] || exit "$status"
