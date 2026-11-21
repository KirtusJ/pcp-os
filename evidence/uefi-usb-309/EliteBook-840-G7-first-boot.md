# HP EliteBook 840 G7 first-boot limits

**B3 disposition, 21 November 2026. The current virtual USB is unsuitable for a physical trial.** The owner identified the machine as an HP EliteBook 840 G7 Notebook PC. Z3 reports that the owner's Windows Device Manager screenshot shows the HP PS/2 keyboard and two HID keyboard entries. Their ability to supply input after firmware handoff has not been measured. The owner believes legacy BIOS boot is unavailable, so UEFI is the planning assumption. The firmware revision, current Secure Boot state and initial framebuffer mode remain unverified. HP lists 1920×1080 internal panels. Canonical also recorded an AT Translated Set 2 keyboard on its certified configuration. [Owner's hardware observations as relayed by Z3](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7067816715) · [HP Maintenance and Service Guide, product description](https://kaas.hpcloud.hp.com/pdf-public/pdf_12437502_en-US-1.pdf) · [Canonical certification](https://ubuntu.com/certified/202006-27960/20.04%20LTS)

## What a future trial may touch

B3 and A3 agree with Z3’s reading of the owner’s direction to prioritize safety. These limits govern requests PCP itself makes. Firmware can update its own records before it hands control to PCP. The EliteBook’s actual behavior remains unmeasured. [Owner’s reply as relayed by Z3](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7068153962) · [A3’s response](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7068184887)

| Resource | Working boundary for a first trial |
|---|---|
| Removable USB | Firmware and PCP may read the image. PCP must issue zero writes, format requests and discards. Creating the removable medium is a separate operation. |
| Internal storage | PCP must issue zero requests to the internal storage medium, including reads, mounts, writes, flushes and discards. Inventory PCI configuration reads separately, including reads of a storage controller’s identity. Firmware can enumerate an internal disk independently. A rejected boot can also return control to firmware, which chooses what runs next. |
| Firmware settings and flash | PCP must issue zero requests to change boot order, firmware variables, Secure Boot policy, enrolled keys or firmware contents. The owner controls setup and has accepted Secure Boot off as a condition for a future unsigned trial. PCP has no authority to change that setting. Firmware can write its own persistent state before any PCP code executes. |
| Volatile state and I/O | Permit image loading into RAM, normal processor and interrupt setup, the firmware framebuffer, built-in keyboard input, and an owner-confirmed shutdown request. Any additional device operations need to be identified for the exact trial build before it is qualified. |

Firmware may select another boot option after a refusal. That fallback is part of the whole-machine outcome. [UEFI boot manager](https://uefi.org/specs/UEFI/2.11/03_Boot_Manager.html)

## What was measured

- The USB fixture's loader returns unsupported on physical hardware before its allocation path. The extracted EFI executable from the tested build has an empty PE certificate table. The ordinary and factory-enrolled firmware tests used the exact same read-only virtual USB. [Loader entry](https://github.com/KirtusJ/pcp-os/blob/39200277158b3b6c5d6d3b9fda9f62463768ab37/examples/firmware-owned-uefi/loader.c#L160) · [USB fixture build](https://github.com/KirtusJ/pcp-os/blob/39200277158b3b6c5d6d3b9fda9f62463768ab37/examples/firmware-owned-uefi/image.py#L96)
- Under ordinary OVMF, five virtual USB boots retained the original removable image and internal NVMe bytes. Across those five profiles the variable store changed by 5,588 or 6,047 bytes before Home and by zero bytes from Home through shutdown. In a separate boot-order test, listing the internal drive second produced 12 reads before Home. Both experiments measured zero internal writes. [Five USB profiles](https://github.com/KirtusJ/pcp-os/issues/314#issuecomment-7067643479) · [Internal-disk boot-order control](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7064387505)
- Under factory Microsoft-enrolled OVMF the firmware displayed Access Denied for that USB. No PCP loader trace or kernel banner appeared. The host stopped the VM at the firmware prompt. Its variable store had already changed by **3,509 bytes**, including BootOrder and Boot0001. The six measured Secure Boot policy and enrollment values retained the same contents. This guest had no internal disk. That refusal and the ordinary guest's 72 passing checks are both recorded in the companion archive. [Enrolled-firmware evidence](B3-enrolled-USB-3920027.tar.xz) · [UEFI image validation](https://uefi.org/specs/UEFI/2.11/32_Secure_Boot_and_Driver_Signing.html)

## Reading the check count

Two Q35/OVMF virtual USB boots of working kernel `d92f5f6` produced the following counts. Both guests received F5 and F10 keyboard input. The demonstration feature simulates a controller that cannot replay test keystrokes. The laptop’s response to the probe is unknown.

| Keyboard replay profile | Passed at startup on F5 and serial | Passed after register closure on serial |
|---|---:|---:|
| Default QEMU controller | 71 | 72 |
| Replay disabled by the demonstration feature | 58 | 59 |

The demo omitted 13 checks. Six covered rejected batches, task-name length or memory recovery after refused admission. F5 described only keys, sleep and stopping a program. The default USB checker refused the 58-pass run at Home. Adjusting its expected count alone allowed an erased skip explanation to pass the screen check. A reader for the hardware candidate needs to check the reason and the expected coverage for that build. A3 also found that this revision’s stopped screen shows the startup count after register closure. [B3’s original captures, serials and offline replay](B3-UEFI-keyboard-replay-d92f5f6.tar.xz) · [A3’s stopped-screen result](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7067996506)

## What remains before a physical trial

Z3 relayed that the owner accepts an unsigned trial with Secure Boot off after being told that Windows may request its recovery key. This settles the owner's willingness on that specific setup choice. The actual firmware state has not been inspected here. The directors still need a distinct hardware-targeted candidate, an accounting of its device operations, and evidence that its storage and firmware requests meet the agreed limits. B3’s host test of the linked GNU-EFI initializer observed reads of `EFIDebug` and `Lang`, protocol queries and volatile pool allocations. Those calls belong in the hardware build’s inventory. [Loader initialization evidence](loader-initialization/README.md)

The hardware candidate also needs an observed allocation-refusal path consistent with owner-confirmed shutdown. The current VM loader requests shutdown automatically on that path. [Owner's disposition as relayed by Z3](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7067838711) Issue 309 also retains the built-in keyboard and visible failure requirements. Physical media preparation and a real laptop boot remain pending. [Issue 309](https://github.com/KirtusJ/pcp-os/issues/309)
