# Showing only the rows that changed

On a display with one page a program draws in memory of its own and the kernel shows each finished picture ([one-page display](../one-page-display/check.py)). The kernel keeps what it last put on the display for that program and writes, in each row, only from the first pixel that differs to the last. A typed letter in Tailorbird at 1920 by 1080 writes 36 to 38 rows of 1,080.

The kernel's copy is in frames of the pool, not in its heap, and goes back to the pool when the program ends. It is trusted only while nothing else has drawn: every screen the kernel draws itself, another program's picture and a change of mode each make the next picture write every row. Where the pool cannot spare the frames, every row is written and no picture fails.

## The check

From the repository root, on Linux with QEMU, OVMF, GRUB's x86_64-efi modules and mtools, with a Tailorbird ELF built from its own repository:

    PCPOS_KERNEL_FEATURES=proposals,present-rows-demo sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build   # the kernel under test
    python3 -B examples/changed-rows-present/check.py <that kernel> <a control kernel> <Tailorbird ELF> /a/new/folder

The control is the kernel before this change, built with `proposals`. [`check.py`](check.py) states its seven predicates before its code. The feature `present-rows-demo` only makes the kernel say on the serial line how many rows each picture wrote, and lets the loader's line stand in for a pool that refuses the frames (`present_keep=refuse`); neither is in a kernel anyone boots for use.

The decisions themselves are pure functions in [`framebuffer.rs`](../../PCP_OS_Rust/crates/pcpos-kernel/src/arch/framebuffer.rs), tested on the host by [`test_framebuffer.rs`](../../PCP_OS_Rust/crates/pcpos-kernel/tests/test_framebuffer.rs), which CI runs.

This check is not in CI: it needs firmware, a loader and a program from another repository. It shows how many rows are written, not how long they take; an emulator writes a framebuffer at the speed of memory.
