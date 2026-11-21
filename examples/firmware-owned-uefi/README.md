# Full-size kernel handoff under OVMF

An experimental x86-64 UEFI optical loader booted PCP OS with its 16 MiB static heap in two Q35 virtual machines. The loader asks firmware to allocate the entire range containing the kernel, modules, Multiboot data and transition stack before writing to it. The alternate image links at 32 MiB. The workflow applies the patch in a detached worktree and boots a temporary unsigned CD. The loader waits for a host test controller on the second serial port.

| RAM | Control | Firmware result | Guest result |
| --- | --- | --- | --- |
| 128 MiB | Occupy one required page first | Whole-range allocation returned EFI_NOT_FOUND | Refused before copying or transfer |
| 128 MiB | Normal handoff | ExitBootServices succeeded on the first call | 9,216 program frames, playback, keyboard Stop, register and power-off |
| 256 MiB | Allocate an additional page after reading the map | First exit returned EFI_INVALID_PARAMETER. Refreshed map kept the page reserved and the second exit succeeded | 35,840 program frames, playback, keyboard Stop, register and power-off |

In both successful boots the arena was `[0x02000000, 0x031f1000)`. Before releasing the loader to exit boot services, the observer read physical memory back through QEMU and compared all five ELF load segments, including their zero-filled tails, and all three modules to their source bytes. The static heap starts at `0x020c8000`. Five ACPI NVS ranges had the same hashes before allocation, after copying, at the launcher and after keyboard Stop in each VM. Each final program pool lay entirely in conventional memory. The two displayed application tasks ended DONE and STOPPED, and the guests powered off with exit code zero.

The kernel ELF SHA-256 is `7b181985e8417c956743d771fd7c059601234e356d9b5e99abd29297f49f4af1`. Its running text and read-only data measured `b1e19b972d450ee602c9cae8fabcdeda1b51623b718902cb6ee72cffd0df62ec`. Moving the image initially exposed two protection checks that referred to the old address of kernel code. The patch makes those checks use the linked text address and preserves their expected fault properties. [Initial failed run](https://github.com/KirtusJ/pcp-os/actions/runs/44858238341)

[Download the original observations](observations.zip). Archive SHA-256 is `c572430696d6b90c8a2214d90c05180bb76880e64bb3e56798ae5931e5b1813a`. It contains the measured ELF, image geometry, per-segment and module hashes, final maps, serial traces, raw register screens, NVS digests, QEMU arguments and power events. The capture used QEMU 8.2.2 on Linux with OVMF, TCG, an instruction-counted clock, one CPU and no attached user disk or NIC. [Capture run](https://github.com/KirtusJ/pcp-os/actions/runs/44859091993)

Run the [workflow](../../.github/workflows/firmware-owned-uefi.yml) to repeat all three arms. The fixed-address and serial-gated experiment covers these virtual machines. Secure Boot, independently bootable UEFI tester media, physical firmware, and durable storage need separate qualification. [Placement issue](https://github.com/KirtusJ/pcp-os/issues/281)
