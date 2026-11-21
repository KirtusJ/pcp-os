# Boot the same BIOS tester CD on three desktop hosts

The source-built CD from [tester run 44856685648](https://github.com/KirtusJ/pcp-os/actions/runs/44856685648) reached the verified graphical launcher and powered off from the guest’s P key on all three hosts. Each ran the same x86-64 SeaBIOS guest under QEMU software emulation on the ordinary clock, with a read-only CD and no NIC or writable disk. [Desktop qualification run](https://github.com/KirtusJ/pcp-os/actions/runs/44857100752).

| Host | Host architecture | QEMU | Startup checks | Guest exit |
| --- | --- | --- | --- | --- |
| Ubuntu 24.04 | x86-64 | 8.2.2 | 71 passed | 0 |
| macOS 15 | Arm64 | 11.1.3 | 71 passed | 0 |
| Windows Server 2025 | AMD64 | 11.1.3 | 71 passed | 0 |

The ISO was 2,498,560 bytes, SHA-256 `160b2d5057ae435dbdce48c47697e0a269c175333a60ab977968cda77a881409`. Its kernel text and read-only data measured `82b96a61b5e949caa63abb9e262f0c7e135de443c8a8576344687aa84f2d6cde`. The build receipt names GitHub's test merge `38816bf5cd8971616f1af7922d126f0e9765cf68`, which includes recipe head `8244de377ba6d54ecd683a38eba16d1230fe4900`. The receipt's file list describes assembly inputs. xorriso updated the boot-information table and added the optical catalog to the final ISO.

The [original observations](observations.zip) contain each unmodified Actions ZIP, the source-build receipt and the original archive checksums. The outer SHA-256 is `14a4586e65b4d37c08193eea783db05d86fba882748663b2173d508ee3d1b683`. The three serial logs and framebuffer captures agree on the expected kernel measurement and verified status row. QEMU reported `guest-shutdown` and exited with status zero on every host. These runs cover startup and shutdown of this particular x86-64 virtual machine. Playback, Stop and register navigation were tested separately in the source builder's Linux run. Physical boot and native Arm64 execution remain untested here.

## Check another build

1. Obtain the source-build run ID, the SHA-256 of its complete ISO and the kernel measurement calculated from its ELF. The run must retain an artifact named `unpublished-tester-cd` containing `pcp-os-tester.iso`.
2. Run the [desktop qualification workflow](../../.github/workflows/desktop-source-cd.yml) in this repository with those three inputs. Each host downloads the same artifact, verifies the ISO before boot, and checks the running measurement, 71 startup passes, the verified status row and the guest shutdown event.
3. Download the three `source-bios-*` observation archives from the qualification run. They contain the QEMU version and arguments, serial log, launcher framebuffer and result. Once a source artifact expires, use an available build and record its own digests.
