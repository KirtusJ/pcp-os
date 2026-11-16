# Multiboot2 EFI-amd64 boot information: bounded inspection

**Review-only source; it is not a loader, a kernel change, a boot receipt, or a replacement for the established Multiboot1 entry.**

This isolated, dependency-free `no_std` crate parses one *already copied* Multiboot2 **information structure**, using safe slices and little-endian field reads. It does not dereference physical addresses, call firmware, allocate pages, initialize a framebuffer, exit boot services, or invoke the kernel.

The scope and nomenclature follow the primary [GNU Multiboot2 Specification v2.0](https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html), specifically §3.5 for EFI-amd64 entry and §3.6 for the **information** tags.
The variable EFI memory descriptor size follows [UEFI 2.11 GetMemoryMap](https://uefi.org/specs/UEFI/2.11/07_Services_Boot_Services.html), which requires consumers to use the firmware-returned descriptor byte size; this is a bounded structural reader and does not decode or authorize those memory regions. Those must not be confused with §3.1 **image-header** tags. For example, EFI header tag 9 (EFI-amd64 entry address) is not the same as information tag 9 (ELF sections).

## Supported fields, and deliberate non-claims

- Bounded boot information, 8-byte aligned tag offsets, checked lengths and an exact end tag. The terminal tag does **not** consume the `MAX_TAGS = 512` data-tag budget: exactly 512 data tags plus the end tag are accepted, while the 513th data tag is refused. The caller supplies an independently observed Multiboot2 magic and the physical address of its copied structure; value checks cannot authenticate the caller.
- Multiboot2 framebuffer information **tag 8**: full `u64` physical base, independent `u32` pitch in **bytes**, pixel dimensions, bits per pixel and direct-RGB field positions/sizes. RGB 32-bit **blue-low-byte** and red-low-byte modes are distinguished explicitly. A valid RGB565, indexed-color or EGA-text mode is **reported as present but not compatible with the existing BGRX32 renderer**. Indexed color uses the Multiboot2 **u16** palette count immediately followed by three-byte RGB entries, not a four-byte count. The parser accepts the representable u16 range subject to the bounded buffer length. Unknown framebuffer types are retained as unknown, never silently called absent.
- Generic Multiboot2 memory map **tag 6** and EFI memory map **tag 17** are distinct. The parser retains their original byte views, descriptor sizes and versions. Tag 6 guarantees an 8-byte-multiple entry size; EFI tag 17 instead carries the UEFI firmware-reported `DescriptorSize`, which must be used as supplied. No 8-byte-multiple rule is inferred for EFI descriptors merely from tag 6. No EFI descriptor is cast to a Rust struct or unilaterally treated as a current memory grant. Firmware page ownership, exit-map key and synchronization remain unverified.
- EFI boot-services-not-terminated **tag 18** reports that the **bootloader had not called ExitBootServices before handoff**. A subsequent kernel call does not remove or update that tag. **Absence of tag 18 does not prove ExitBootServices succeeded**, and presence after kernel entry is not proof that boot services remain available now. The parser records the handoff-time claim supplied in bytes, not the firmware's present state.
- Module records use 32-bit addresses (consistent with the EFI-amd64 Multiboot2 placement restriction) and borrowed NUL-delimited command-line bytes. Rejected ranges include overlapping modules and modules that intersect the provided information buffer. Kernel and module image signatures are not checked.
- The `inspect_efi_amd64` function requires the supplied information structure itself to fit **below 4 GiB** with 8-byte alignment. The independent framebuffer physical base can exceed 4 GiB. Per §3.5, the EFI-amd64 **kernel, modules and information structure** must remain below 4 GiB - 1; this parser does not prove the kernel's placement because no loaded kernel image is in scope.

Deliberate limits: the parser does **not** normalize the memory map, confirm that firmware-supplied bytes match the reported address, perform physical mapping, enforce NX/W^X, authorize device access, classify a display's EDID, or link into a running PCP-OS. Truncated/duplicate/overflowing interpreted tags are errors; other tag types are counted as **uninterpreted** without adopting their semantics. That category includes standardized but unimplemented tags (such as ELF-sections information tag 9), not just future unknown tags. Failures do not mean an operating system has failed to boot.

## Source/test entry points

```sh
cargo test --manifest-path PCP_OS_Rust/crates/pcpos-kernel/mb2-inspect/Cargo.toml
cargo test --manifest-path PCP_OS_Rust/crates/pcpos-kernel/mb2-inspect/Cargo.toml --release
cargo check --manifest-path PCP_OS_Rust/crates/pcpos-kernel/mb2-inspect/Cargo.toml --target x86_64-unknown-none
```

At authoring, there was no Rust compiler or QEMU in this Session B container. **The 43 Rust tests are authored, not executed or compiler-validated in this Session B.** They include good high-framebuffer/low-MBI cases, padded pitch, RGB/BGR/RGB565, index and EGA types, retained EFI and generic memory maps, tag-18 presence, invalid magic, high/unaligned MBI, out-of-bounds lengths, missing end, duplicate interpreted tags, malformed masks, valid non-8-byte-multiple future EFI descriptor stride, overlong/truncated palettes, descriptor errors, overlapping modules and short mutated inputs. These tests provide proposed executable oracles, not evidence of compliance.

## Intended review interface with Director PT

Director PT [reported a separate OVMF/GRUB EFI-amd64 emulator preview](https://github.com/KirtusJ/pcp-os/issues/132#issuecomment-6906026073). That producer report is not this Session B's boot result and has not been tied to a source commit in this review. PT should first identify the **actual source revision and loader** whose Multiboot2 tags and EFI transitions were tested. Only then compare its tag parsing, framebuffer geometry, memory map and lifecycle with this bounded candidate. The existing Multiboot1 route and all executable paths are unchanged.

Proposed discriminators for PT: a source-pinned real EFI-amd64 boot-information capture with displayed framebuffer base/stride/masks and exact tag extents, malformed-tag and duplicate-adversary fixtures, missing graphics and high-framebuffer cases, different EFI and generic map versions, and independent observation that the boot-services transition actually occurred. Run in an isolated executable environment, preserve stdout/stderr, exit codes, source/blob digests and any failed tests.

**Status:** draft proposal awaiting source review and an actual Rust test execution. This is an absorption-only PCP session. No OS boot, installation, true calibration, host enforcement, hardware confirmation or release authorization has occurred in this session. Product work stays within the PCP OS repository, separate from the physics research project.
