# Stage 1 firmware handoff and processor count

Director A3 tested loader PR #398 at `7aa76bc1942abc9a0494cab9271c6d4fd7501749` together with kernel PR #397 at `77b471293d995281e0d2e71ebf89bb5e91d21ff2`. Both had base `2f7d9d4315fdc485c6e0693e325643456993e1df`.

## UEFI

QEMU 10.0.13 booted a private 0.5B FAT image read-only on xHCI with fresh OVMF variables and no network. This Q35 machine had two sockets with three cores apiece, 1,536 MiB RAM and 512 MiB below 4 GiB. The run ended under QMP control following the startup checks and the first Tailorbird frame. No model request was submitted.

| Observation | Value |
| --- | --- |
| Verified model bytes | 491,400,064 |
| Loader word | `acpi_rsdp=0x000000001fb7e014` |
| XSDT and MADT addresses | `0x1fb7d0e8` and `0x1fb78000` |
| Kernel serial | 6 usable at the address the loader named, 0 beyond limit, 0 disabled, running on one |
| QMP APIC IDs | 0, 1, 2, 4, 5, 6 |
| APIC IDs decoded from the captured MADT | 0, 1, 2, 4, 5, 6 |
| PR #397 Rust parser run separately on those same bytes | 0, 1, 2, 4, 5, 6 |
| Kernel startup checks | 71 passed |

The guest printed the count. The identity list comes from the emulator, firmware bytes and a separate execution of the exact parser. Secondary-processor execution and physical firmware qualification remain open.

The UEFI kernel used the checked-in 32 MiB relocation patch and the proposals/model-memory configuration with Rust 1.100.0. The loader was built with the normal receipt-based builder, Rust 1.97.0 and GNU-EFI. The build record, input receipt and image digest are included. These are local test media.

## BIOS

The stock, unrelocated kernel was separately built at #397 with Rust 1.100.0. On Q35/SeaBIOS it reported the configured count in all five boots (1, 2, 4, 8 and the two-socket 6). Each matched the named kernel measurement, printed 72 passing checks in total, and returned QEMU debug-exit status 33. An initial checker expected 71 in the full log. Supplying the image measurement adds one check to the unpinned direct-boot invocation. All five cases were rerun using the corrected expectation.

## Evidence and replay

The ZIP contains original serial logs, QMP observations, captured ACPI bytes, test results and the portable EFI build record. Its SHA-256 is `110bd0ba0ca21ee62ca72f5fa22268f579e88837e98f9dba56cb25101e60e9d6`. For the parser replay, place the checked-out kernel source at `kernel-stock/` alongside `replay.rs`, compile with `rustc --edition=2021 replay.rs` and run `./replay sparse-uefi 1fb7e014` against the extracted capture directory. The two Python scripts record the emulator invocations. Original host and processor unit suites passed 15 and 6 tests respectively.

[Loader candidate](https://github.com/KirtusJ/pcp-os/pull/398) · [Kernel candidate](https://github.com/KirtusJ/pcp-os/pull/397) · [Processor rollout](https://github.com/KirtusJ/pcp-os/issues/396)
