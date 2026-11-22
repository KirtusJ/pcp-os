# Taylor’s offline Qwen3-8B greeting

PCP OS booted from a read-only UEFI virtual DVD with networking disabled. Tailorbird opened automatically. Taylor’s reply to “Say hello in one short sentence.” was **Hello, I'm Taylor, your PCP OS assistant.**

| Observation | Elapsed wall time |
| --- | ---: |
| VM launch to Taylor Ready | 420.23 s |
| Enter to first visible answer | 151.140–151.639 s |
| Enter to full answer visible | 215.974–216.373 s |
| Enter to natural completion recorded by Tailorbird | 219.631–219.697 s |

The test used a fresh conversation, one virtual CPU, 6,656 MiB of guest RAM, QEMU 11.1.0 and Windows Hypervisor Platform on an Intel Core i5-10210U. The guest CPU was `max,vmx=off,smx=off`. The worker selected `AVX2 Q8_K`. Host time includes UI redraw and the period between screen captures. The time range brackets each observation from the last capture without it to the first with it. The serial reader independently bracketed completion against submission using the host monotonic clock.

The independent screen decoder read the prompt on the person’s row and the answer on Taylor’s row in the original 1920 × 1080 guest frames. The previous frame contained zero answer-colored pixels on Taylor’s row. The first text frame contained 345 and read “Hello”. The complete frame read back all 41 UTF-8 bytes reported by Tailorbird. The serial log includes the verified model byte count, automatic UI launch, worker readiness and normal completion. Each timer starts from the Enter injection, except the separately labelled boot measurement.

## Candidate identity

| Component | SHA-256 |
| --- | --- |
| Optical image, 10,283,589,632 bytes | `85e3fdea58edbbf67472c0784eaed26cce9f5e56f3a30913a52f1978dccefb9a` |
| Approved inventory, 82 files | `d56e1e7297fccd1d532fab8e21b4ac7c66c9a56ede11812588c8dc330ba4eb46` |
| Qwen3-8B Q4_K_M, 5,027,783,488 bytes | `d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785` |
| Corrected kernel, #415 qualification build | `11e253efc177ebf15d95373fe15dbac0395ede28f4d4bf2884528a51b890999e` |
| Bound EFI loader | `df21437adbe23938a8cc2f89213ae29c6c06f44fda727c003eb089f8fc6fb40f` |
| #33 model worker | `1400620be191ec0d6d18e6ff4da363cb6983913115067e3ee424abce5131a32c` |

The release-media checker recovered the approved files from the FAT and Joliet views and checked the GPT EFI partition against the El Torito boot image. The host reconstructed the ISO on FAT32 from 154 sector extents, including the existing model shards on D:, and rehashed every logical byte before boot. The guest had a single optical drive, disposable firmware variables and no hard disk or network adapter. The C: model was read only by the mastering process.

## Earlier attempts and scope

The initial CPU profile exposed nested virtualization and OVMF raised a general protection fault before entering the DVD loader. Disabling VMX and SMX for the guest allowed the uncorrected ISO to boot. Tailorbird opened, then its worker stopped before Ready with processor vector 19. Returning to the desktop recovered all 100,864 allocator frames. A process on the physical Intel host independently read MXCSR as zero after restoring the old template and `0x1f80` after restoring the corrected one. A software-emulated control of the uncorrected ISO ended during model verification before a prompt was submitted.

Z3 corrected initialization, A3 checked the first instruction of a user program on three CPU profiles, and B3 built the matching kernel and EFI loader. The revised inventory changes only those two files. This timing qualifies one private optical build on one virtual machine. Physical EliteBook boot, hardware thermal readings, Rufus-written media and release signing remain separate checks. [Kernel correction](https://github.com/KirtusJ/pcp-os/pull/415) · [A3’s inventory check](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7082476557) · [B3’s component receipt](https://github.com/KirtusJ/pcp-os/pull/276#issuecomment-7082448588)

## Returning to the desktop and shutting down

Escape returned to the desktop with Tailorbird exit 0. The desktop gave the application a runtime of 25 seconds, shorter than the independently measured wall time for the greeting alone. The timings above use the host monotonic clock. The guest clock discrepancy needs investigation before relying on that desktop number. From the desktop, F10 and Enter shut down the guest. The final register replay passed and both frame pools reported every allocation returned.
