# A guest built with the processor's floating point

The stock bare target, `x86_64-unknown-none`, does floating point in software. A program that
computes with `f32` or `f64` (a model worker, above all) is many times slower for it. This folder
shows a way to build a guest with the processor's own floating point that needs no custom target,
no nightly compiler and no change to the kernel, and checks on the kernel that such a guest is right.

## The route

Build for the tier-1 target `x86_64-unknown-linux-gnu`, as a `no_std`, `no_main` program linked by
`ld` with no C library and the ordinary guest linker script. That target's shipped `core` and
`alloc` are built for SSE2, so the target, `core` and `alloc` agree and nothing about the bare
target's ABI is switched off. The kernel stays on its own target with no floating point.

Two things in the source:

- `pcp-game-guest` (`store/native/guest`) builds its calls into PCP OS, its entry and its panic
  handler on the bare target only. The feature `hard-float-target` builds them on this target too.
  Never enable it for a program that runs on a host.
- `probe/.cargo/config.toml` holds the flags for both targets.

## Build and check

From `examples/hard-float/probe`, with Rust 1.97.0 and both targets installed:

```sh
cargo build --release --target x86_64-unknown-none                                   # soft
cargo build --release --target x86_64-unknown-linux-gnu --features hard              # hard
BACK=1 cargo build --release --target x86_64-unknown-linux-gnu --features hard       # hard, the program beside
CLOBBER=1 cargo build --release --target x86_64-unknown-linux-gnu --features hard    # hard, the red-zone control
BACK_SERIES=1 cargo build --release --target x86_64-unknown-none                     # soft, the other series
```

Copy each `target/<target>/release/float-probe` aside before the next build, then, with a kernel
built with the `companion` feature:

```sh
python3 examples/hard-float/check.py kernel.elf32 soft hard back clobber series
```

The predicates are in `check.py`. It needs `objdump` and `qemu-system-x86_64`.

## What it shows, and what it does not

Shown, in an emulator: the hardware build prints the same bits as the software build for an `f32`
sum, an `f64` sum and a sum over numbers kept on a heap; the red zone below the stack pointer is
left alone by the kernel across 400,000,000 turns of a loop, and a control that writes there is
caught every turn; two hardware-float programs running at once, on a processor with 256-bit
registers and on one with SSE2 only, each print what they should.

Not shown:

- **Speed on a real machine.** In the emulator the software build took about sixteen times as long
  for the same sums; that ratio says only that software floating point was the cost there.
- AVX or AVX2 code, and more than one processor. This is scalar SSE2.
- A kernel that fails to keep a program's registers, to show the two-program test can fail. The
  red-zone test has its control; this one does not.
- Anything on hardware.

A program with nothing writable is linked with an empty segment at address zero, which the kernel
refuses ("a segment lies outside the memory a program may have"). Any real program has data.
