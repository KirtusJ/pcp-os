#!/bin/sh
# Build the kernel and start it in QEMU (`run.sh build` only builds it). Needs: a Rust toolchain with its source (for building `core`),
# ld.lld, objcopy and qemu-system-x86_64. Exit status 33 from QEMU means every self-test passed.
set -e
cd "$(dirname "$0")"
# The same source gives the same image wherever it is kept. The build tool names every function after the folder
# its crate is compiled in, and the order of the functions in the image follows those names: so the crates are
# copied to one fixed folder and compiled there, and that folder's name is kept out of the image as well. What still
# belongs to the image's identity: the compiler's version and the folder the toolchain is installed in.
HERE="$(pwd)"; STAGE="${PCPOS_KERNEL_STAGE:-/tmp/pcpos-kernel-build}"
mkdir -p "$STAGE" target
# The compiled files of an earlier build are kept; the sources are replaced whole.
[ -d "$STAGE/crates/pcpos-kernel/target" ] && { rm -rf "$STAGE/kept-target"; mv "$STAGE/crates/pcpos-kernel/target" "$STAGE/kept-target"; }
rm -rf "$STAGE/crates" "$STAGE/vectors" "$STAGE/Cargo.toml" "$STAGE/Cargo.lock"
(cd ../.. && tar -cf - --exclude=target --exclude=programs --exclude=tests --exclude=guard_cases.jsonl Cargo.toml Cargo.lock crates vectors) | tar -xf - -C "$STAGE"
[ -d "$STAGE/kept-target" ] && mv "$STAGE/kept-target" "$STAGE/crates/pcpos-kernel/target"
(cd "$STAGE/crates/pcpos-kernel" && RUSTC_BOOTSTRAP=1 cargo build --release ${PCPOS_KERNEL_FEATURES:+--features "$PCPOS_KERNEL_FEATURES"} --config "target.x86_64-unknown-none.rustflags=['--remap-path-prefix=$STAGE=pcpos']")
objcopy -I elf64-x86-64 -O elf32-i386 "$STAGE/crates/pcpos-kernel/target/x86_64-unknown-none/release/pcpos-kernel" target/pcpos-kernel.elf32
[ "$1" = build ] && exit 0
set +e
qemu-system-x86_64 -cpu ${PCPOS_QEMU_CPU:-max} -kernel target/pcpos-kernel.elf32 -m 128M -display none -serial stdio -no-reboot -append "${PCPOS_APPEND:-then=off}" \
  -device isa-debug-exit,iobase=0xf4,iosize=0x04
status=$?
echo "qemu exit status: $status (33 = all self-tests passed)"
[ "$status" = 33 ]
