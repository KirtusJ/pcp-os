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

B3 built working kernel `be2d4dd42521b6d49f7cdc3ab0d39682db6f22d3` and tested three Q35/OVMF USB configurations. The laptop’s actual keyboard and response to the replay probe are unmeasured.

| Virtual configuration | Passed before register closure | Passed after closure on serial | Interaction |
|---|---:|---:|---|
| Normal PS/2 controller | 71 | 72 | F5 showed 71. F10 and Enter confirmed shutdown. |
| PS/2 controller with test replay suppressed | 65 | 66 | F5 explained the six key-dependent checks that did not run. F10 and Enter confirmed shutdown. |
| PS/2 controller absent, USB keyboard attached | 65 | 66 | The kernel reported no keyboard controller, passed over the desktop and requested guest shutdown. The observer sent zero keystrokes. |

The USB-only guest booted the same image as the normal guest. QEMU listed the USB keyboard and mass-storage device. The seven checks for group rejection, sleep, admission and memory recovery ran in every profile. All three preserved the removable image and internal disk. The four stopped screens in a separate virtual PC test showed 72, 66, 66 and 66, each matching final serial. [B3’s original frames, serials and replay](B3-UEFI-keyboard-be2d4dd.tar.xz)

On the earlier `d92f5f6` build the replay-disabled profile passed 58 at startup and 59 after closure. It skipped 13 checks, and the stopped screen showed the pre-closure count. The new runs supersede those results for the revised kernel. [Earlier evidence](B3-UEFI-keyboard-replay-d92f5f6.tar.xz)

Counting six fewer passes can still hide a missing admission check if a different pass takes its place. The checker integrated in #325 checks each pass label and its multiplicity. It accepted the intact logs and rejected all five altered pairs that the previous reader accepted. [Integrated checker](https://github.com/KirtusJ/pcp-os/pull/325)

## When the intact USB returns to firmware

B3 booted the same intact read-only USB in four Q35/OVMF guests with USB-only input. With the CPU hypervisor flag withheld, OVMF invoked the USB executable and received `Unsupported` before the first loader trace or kernel handoff. With the bootable internal NVMe listed second, OVMF then invoked its diagnostic EFI executable, which also returned `Unsupported`. OVMF reached its shell after 22 internal reads totaling 270,336 bytes. With the attached NVMe omitted from the boot list, OVMF reached the same shell with zero internal reads.

With the flag present, both controls reached the kernel and completed 66 checks. The internal disk recorded 12 reads when listed second and zero when omitted. All four runs preserved both media hashes and recorded zero internal writes, flushes or discards. The stopped screens seen under direct kernel boot cannot be reached by this USB when its loader refuses entry. The EliteBook’s own boot order and fallback behavior remain unmeasured. [Original firmware, serial, framebuffer and block-counter records](B3-UEFI-intact-USB-fallback-8395a86.tar.xz)

A3 also selected the removable USB from OVMF’s one-time menu. B3 checked the saved boot variables, block counters and screens from seven guests. With the internal NVMe first and USB second, OVMF read the internal drive 48 times (359,424 bytes) before the choice. Selecting the USB or returning to the picker after refusal added zero internal reads. Selecting Continue added ten reads (215,040 bytes). These counts describe the tested firmware and boot order. [A3’s one-time-menu experiments](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7069031092) · [B3’s independent readback](B3-USB-allocation-refusal-0cf86db.tar.xz)

## What remains before a physical trial

Z3 relayed that the owner accepts an unsigned trial with Secure Boot off after being told that Windows may request its recovery key. This settles the owner's willingness on that specific setup choice. The actual firmware state has not been inspected here. The directors still need a distinct hardware-targeted candidate, an accounting of its device operations, and evidence that its storage and firmware requests meet the agreed limits. B3’s host test of the linked GNU-EFI initializer observed reads of `EFIDebug` and `Lang`, protocol queries and volatile pool allocations. Those calls belong in the hardware build’s inventory. [Loader initialization evidence](loader-initialization/README.md)

The guarded virtual loader now displays a fixed-memory allocation failure and waits for the owner. In QEMU the explanation remained visible for 310 seconds without input. F10 displayed the confirmation and cleared queued input. Esc cancelled it, and Enter at a fresh confirmation requested shutdown. A hardware-targeted candidate still needs its own observation of this path, the built-in keyboard and visible failures. Physical media preparation and a real laptop boot remain pending. [Merged correction](https://github.com/KirtusJ/pcp-os/pull/327) · [Virtual USB evidence](B3-USB-allocation-refusal-0cf86db.tar.xz) · [Owner’s disposition](https://github.com/KirtusJ/pcp-os/issues/309#issuecomment-7067838711)
