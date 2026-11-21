# Full-size kernel handoff under OVMF

An experimental x86-64 UEFI optical loader booted PCP OS with its 16 MiB static heap in two Q35 virtual machines. The loader asks firmware to allocate the entire range containing the kernel, modules, Multiboot data and transition stack before writing to it. The alternate image links at 32 MiB. The workflow applies the patch in a detached worktree and boots a temporary unsigned CD. The loader waits for a host test controller on the second serial port.

The separate removable USB fixture runs only under QEMU TCG. Its loader returns control to firmware on a physical machine.

| RAM | Control | Firmware result | Guest result |
| --- | --- | --- | --- |
| 128 MiB | Occupy one required page first | Whole-range allocation returned EFI_NOT_FOUND | Refused before copying or transfer |
| 128 MiB | Normal handoff | ExitBootServices succeeded on the first call | 9,216 program frames, playback, keyboard Stop, register and power-off |
| 256 MiB | Allocate an additional page after reading the map | First exit returned EFI_INVALID_PARAMETER. Refreshed map kept the page reserved and the second exit succeeded | 35,840 program frames, playback, keyboard Stop, register and power-off |

In both successful boots the arena was `[0x02000000, 0x031f1000)`. Before releasing the loader to exit boot services, the observer read physical memory back through QEMU and compared all five ELF load segments, including their zero-filled tails, and all three modules to their source bytes. The static heap starts at `0x020c8000`. Five ACPI NVS ranges had the same hashes before allocation, after copying, at the launcher and after keyboard Stop in each VM. Each final program pool lay entirely in conventional memory. The two displayed application tasks ended DONE and STOPPED, and the guests powered off with exit code zero.

The kernel ELF SHA-256 is `7b181985e8417c956743d771fd7c059601234e356d9b5e99abd29297f49f4af1`. Its running text and read-only data measured `b1e19b972d450ee602c9cae8fabcdeda1b51623b718902cb6ee72cffd0df62ec`. Moving the image initially exposed two protection checks that referred to the old address of kernel code. The patch makes those checks use the linked text address and preserves their expected fault properties. [Initial failed run](https://github.com/KirtusJ/pcp-os/actions/runs/44858238341)

[Download the original observations](observations.zip). Archive SHA-256 is `c572430696d6b90c8a2214d90c05180bb76880e64bb3e56798ae5931e5b1813a`. It contains the measured ELF, image geometry, per-segment and module hashes, final maps, serial traces, raw register screens, NVS digests, QEMU arguments and power events. The capture used QEMU 8.2.2 on Linux with OVMF, TCG, an instruction-counted clock, one CPU and no attached user disk or NIC. [Capture run](https://github.com/KirtusJ/pcp-os/actions/runs/44859091993)

Run the [workflow](../../.github/workflows/firmware-owned-uefi.yml) to repeat all three arms. The fixed-address and serial-gated experiment covers these virtual machines. Secure Boot, independently bootable UEFI tester media, physical firmware, and durable storage need separate qualification. [Placement issue](https://github.com/KirtusJ/pcp-os/issues/281)

## Removable-media VM experiment

The USB trial builds a separate x86-64 OVMF fixture. QEMU attaches its FAT volume as a read-only USB mass-storage device on xHCI. OVMF finds the application at the removable-media path `EFI/BOOT/BOOTX64.EFI`. The trial records the boot device reported by OVMF and the actual QEMU attachments. [UEFI removable-media boot](https://uefi.org/specs/UEFI/2.11/03_Boot_Manager.html#removable-media-boot-behavior)

The USB application checks for the QEMU TCG fixture. It reserves and verifies the same full-heap memory range as the optical experiment, then reads back the copied kernel, zero-filled tail and all three modules before leaving boot services. The application uses the bytes embedded at build time for this comparison. The USB guest boots without input on a second serial port. The kernel reports its running measurement on the first serial port.

On the active OVMF console, the loader describes a linear 32-bit BGR framebuffer to the kernel when the stride and reported extent cover the mode. It keeps the current mode. The kernel can use this description when it has no native display driver.

The test starts fresh standard-VGA and RAMFB Q35 guests at 128 and 256 MiB, and a virtio VGA guest at 128 MiB. The virtio VGA and RAMFB trials match the visible Home dimensions to the kernel report, compare the reported address with the firmware handoff, and calculate the mapped size from the firmware pitch and picture height. The RAMFB pair also checks that the address changes when guest memory increases. Each has a disposable writable internal NVMe drive with a GPT/FAT fixture, a private writable copy of the OVMF variable store, and no NIC. At Home it opens Verification, returns with Esc, and confirms power-off. The retained test artifact includes the exact unsigned USB volume, raw screens, serial and firmware logs, QMP shutdown, disk I/O counters, and raw variable-store snapshots before boot, at Home and after guest shutdown. Hashes after QEMU exits also cover the final backing files.

A fresh OVMF store can change before Home. The observer reports raw byte differences and the names and value hashes of active variables at each checkpoint. These intervals include firmware activity. QMP pflash write counters alone cannot establish whether the variable store changed. The experiment covers unsigned virtual firmware and media. Physical laptops, Secure Boot and physical USB media remain unqualified.

A separate diagnostic guest checks that the observers detect a known change. It recognizes the QEMU TCG environment and the named GPT/FAT test partition, checks the marker in `KEEP.TXT`, changes its first byte, and creates and reads back one private diagnostic variable in the disposable firmware store. The runner checks the disk byte offsets, block counters and variable value. This EFI application is built separately from the PCP OS USB image.
