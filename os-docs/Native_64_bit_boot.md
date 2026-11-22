# Native 64-bit boot and addressing

Long-term requirement, unscheduled. PCP OS should ship a documented x86-64 boot path that loads the kernel directly and preserves 64-bit addresses from firmware handoff through memory management. Migration can proceed alongside the current release path.

## Starting point

The Rust kernel already targets `x86_64-unknown-none`. The build converts its linked ELF64 image into an ELF32 container for QEMU's Multiboot 1 loader. The entry stub starts in 32-bit mode, checks long-mode and NX support, then enters 64-bit mode before calling Rust. The guest loader already accepts statically linked little-endian x86-64 ELF programs. These are three distinct interfaces. [Build](../PCP_OS_Rust/crates/pcpos-kernel/run.sh) · [Entry stub](../PCP_OS_Rust/crates/pcpos-kernel/src/boot.s) · [Guest loader](../PCP_OS_Rust/crates/pcpos-kernel/src/exec.rs)

## Migration sequence

1. Inventory address-width assumptions in boot information, memory maps, page tables, allocators, framebuffer and DMA descriptors, syscall copying and guest loading. Classify each 32-bit field by its specification. Preserve fixed-width wire, store and register formats, and use checked conversions at hardware boundaries.
2. Prototype a Rust x86-64 UEFI loader in parallel with the existing image. Load the ELF64 kernel and pass a versioned handoff covering memory regions, modules, command line and framebuffer. Verify image identity and permissions, handle the firmware memory-map change at exit, then enter a shared Rust kernel initialization path. [UEFI specifications](https://uefi.org/specifications) · [Rust bare-metal target](https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html)
3. Boot both paths in CI. On x86-64 QEMU with OVMF and more than 4 GiB of RAM, explicitly allocate, map, write and read a frame above 4 GiB and check its low-address alias remains distinct. Exercise high module addresses and malformed or overflowing ranges. Compare kernel self-tests, NX and user isolation, guest launches, device behavior, image pin failures and register replay against the existing boot path. Verify an actual supported UEFI machine before declaring hardware support.
4. Ship a separately identified image and keep the known-good boot entry and recovery image selectable. Test rollback on a copy of a persistent register and confirm its head and replay match. Switch the default only after compatibility checks and coordinated release review. Decide legacy boot support separately using the supported-machine inventory.
