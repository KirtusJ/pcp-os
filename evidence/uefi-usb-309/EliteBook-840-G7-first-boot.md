# HP EliteBook 840 G7 first-boot limits

**B3 disposition, 21 November 2026. The current virtual USB is unsuitable for a physical trial.** The owner identified the machine as an HP EliteBook 840 G7 Notebook PC. Its firmware version, enrolled trust policy, initial display mode and built-in keyboard behavior have not been measured in this work. HP lists 1920×1080 internal panels, and Canonical recorded an AT Translated Set 2 keyboard on its certified configuration. Neither identifies what the owner's firmware will present to PCP. [HP Maintenance and Service Guide, product description](https://kaas.hpcloud.hp.com/pdf-public/pdf_12437502_en-US-1.pdf) · [Canonical certification](https://ubuntu.com/certified/202006-27960/20.04%20LTS)

## What a future trial may touch

These are proposed limits for the owner and directors to judge before preparing physical media.

| Resource | Proposed boundary for the first trial |
|---|---|
| Removable USB | Firmware and PCP may read the image. PCP must issue zero writes, format requests and discards. Creating the removable medium is a separate operation. |
| Internal storage | PCP must issue zero requests to internal storage, including reads, mounts, writes, flushes and discards. Firmware can enumerate an internal disk independently. A rejected boot can also return control to firmware, which chooses what runs next. |
| Firmware settings and flash | PCP must issue zero requests to change boot order, firmware variables, Secure Boot policy, enrolled keys or firmware contents. The owner retains the current setup. Firmware can write its own persistent state before any PCP code executes. |
| Volatile state and I/O | Permit image loading into RAM, normal processor and interrupt setup, the firmware framebuffer, built-in keyboard input, and an owner-confirmed shutdown request. Any additional device operations need to be identified for the exact trial build before it is qualified. |

**If the requirement is zero persistent changes by any component during the entire boot attempt, B3 cannot clear the laptop on current evidence.** A commitment about PCP's own write requests is narrower and can be tested. Firmware policy, including any fallback to another boot option, is part of the whole-machine outcome. [UEFI boot manager](https://uefi.org/specs/UEFI/2.11/03_Boot_Manager.html)

## What was measured

- The USB fixture's loader returns unsupported on physical hardware before its allocation path. The extracted EFI executable from the tested build has an empty PE certificate table. The ordinary and factory-enrolled firmware tests used the exact same read-only virtual USB. [Loader entry](https://github.com/KirtusJ/pcp-os/blob/39200277158b3b6c5d6d3b9fda9f62463768ab37/examples/firmware-owned-uefi/loader.c#L160) · [USB fixture build](https://github.com/KirtusJ/pcp-os/blob/39200277158b3b6c5d6d3b9fda9f62463768ab37/examples/firmware-owned-uefi/image.py#L96)
- Under ordinary OVMF, five virtual USB boots retained the original removable image and internal NVMe bytes. Across those five profiles the variable store changed by 5,588 or 6,047 bytes before Home and by zero bytes from Home through shutdown. In a separate boot-order test, listing the internal drive second produced 12 reads before Home. Both experiments measured zero internal writes. [Five USB profiles](https://github.com/KirtusJ/pcp-os/issues/314#issuecomment-7067643479) · [Internal-disk boot-order control](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7064387505)
- Under factory Microsoft-enrolled OVMF the firmware displayed Access Denied for that USB. No PCP loader trace or kernel banner appeared. The host stopped the VM at the firmware prompt. Its variable store had already changed by **3,509 bytes**, including BootOrder and Boot0001. The six measured Secure Boot policy and enrollment values retained the same contents. This guest had no internal disk. That refusal and the ordinary guest's 72 passing checks are both recorded in the companion archive. [Enrolled-firmware evidence](B3-enrolled-USB-3920027.tar.xz) · [UEFI image validation](https://uefi.org/specs/UEFI/2.11/32_Secure_Boot_and_Driver_Signing.html)

## What remains before a physical trial

The directors would need a distinct hardware-targeted candidate that boots under the owner's existing trust policy, an accounting of that candidate's device operations, and evidence that its storage and firmware requests meet the agreed limits. The owner’s input is needed if firmware-managed persistent changes fall outside the acceptable boundary. Issue 309 also retains the built-in keyboard and visible failure requirements. Physical media preparation and a real laptop boot remain pending. [Issue 309](https://github.com/KirtusJ/pcp-os/issues/309)
