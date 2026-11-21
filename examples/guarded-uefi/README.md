# Guarded x86-64 UEFI kernel experiment

A separately built PCP kernel ran the Surfbird demo under Q35/OVMF with 128 MiB and 256 MiB of guest RAM. Both guests verified all 71 startup checks, played the demo to completion, stopped the second launch from the keyboard, displayed both task outcomes in the register and powered off. The released 0.3.1 ELF was refused before handoff in both configurations. [Original CI run](https://github.com/KirtusJ/pcp-os/actions/runs/44848935967).

The experimental build retains a fixed link address of 1 MiB and reduces the static heap to 4 MiB. This allows its page-rounded load ranges to fit below the ACPI NVS conflict reported in [#281](https://github.com/KirtusJ/pcp-os/issues/281). It selects one contiguous frame pool from the complete Multiboot memory map. Reserved descriptors take priority over usable descriptors. The ordinary build still has a 16 MiB heap. The frame-selection code also applies to that build.

| Guest RAM | Program frame pool (exclusive end) | Visible Surfbird tasks |
| --- | --- | --- |
| 128 MiB | `0xa00000..0x7000000` | T15 DONE, T16 STOPPED |
| 256 MiB | `0xa00000..0xf000000` | T15 DONE, T16 STOPPED |

The candidate ELF SHA-256 was `398b5d3ed9ae90e3aaf3aba1800688acd2ec90b5e8297e901064500fddcce292`. Each running kernel reported measurement `0f305e0c2c1e3f799c5b4282313c85882eb52a2737ccdc609e31a9ea6795f3e7`. SHA-256 values for all five GRUB-reported ACPI NVS intervals agreed before admission, at the verified launcher and after playback and register navigation. These are observations at three points in time. [Original status records, serials and framebuffers](observations.zip), ZIP SHA-256 `ac83adad70d2420da59e7de379bc43221bdc5a6f96abba6331bdf27018c1fe7a`.

The [CI workflow](../../.github/workflows/guarded-uefi.yml) runs GRUB in a disposable VM and waits for an explicit decision from its host observer before loading the ELF. The observer validates the current page ranges against the live firmware map. That check does not reserve the pages or implement a production UEFI loader. The experiment uses software emulation, an instruction-counted clock and a read-only CD. Physical hardware, Secure Boot, long-running workloads on the smaller heap, and persistent storage and recovery remain unqualified. For desktop VM testers, use the [qualified BIOS download](https://github.com/KirtusJ/pcp/tree/b15e346cd9cdae483ebe1dad477e189bfa25464a/audit/public-live-cd/candidates/keyboard-01-limine-10).
