# HP EliteBook 840 G7 — first physical trial

**Hardware trial pending.** The owner chooses the image and whether to run this short live boot. Leave the internal disk untouched. The current proposal covers a VM preview. [Release proposal](https://github.com/KirtusJ/pcp-os/issues/406)

## Before touching the firmware

- [ ] Save your work and make sure your normal system and backup are accessible. Use the built-in display and keyboard, AC power and a clear, ventilated surface. Disconnect other removable drives and docks. On a managed machine, or if its security settings are unfamiliar, stop here and consult its administrator.
- [ ] On the normal system, write down the laptop model, BIOS version, processor model, reported cores/logical processors, and current Secure Boot and disk-encryption states. On Windows, use **System Information** for BIOS/UEFI and Secure Boot, and **Task Manager → Performance → CPU** for the counts. Check Windows drive protection with `manage-bde -status C:` if C: is the system drive. If encrypted, confirm access to its matching recovery key from a second trusted device. Keep the key off this sheet and out of photos. Follow the normal administrator-approved encryption procedure for firmware changes. [Microsoft recovery-key guide](https://support.microsoft.com/en-us/windows/security/encryption/find-your-bitlocker-recovery-key)
- [ ] Use a spare USB prepared for this trial. The person preparing it supplies the image name, byte count, SHA-256, source pins, enabled features and USB readback result. Compare the image with the independently supplied digest. On Windows, `Get-FileHash -Algorithm SHA256 -LiteralPath 'C:\path\to\image.iso'` reads the image file. The USB readback must cover the image-length portion of the written stick. Record the writer, mode and result. If the stick or image identity is uncertain, stop before changing firmware.
- [ ] Decide whether to test **desktop/input only**, or **desktop/input plus diagnostics**. For diagnostics, arrange the capture described below before powering on.

| Trial record | Fill in before boot |
| --- | --- |
| Date, time zone, machine model, BIOS revision | |
| CPU model, normal-OS core count / logical processor count | |
| Original Secure Boot / UEFI mode / encryption protection state | |
| Image name, bytes, SHA-256, kernel / loader commits and features | |
| USB make, capacity, physical port, writer and readback result | |
| Diagnostic output channel, if available | |

## One boot from USB

1. [ ] With the machine off, power on and repeatedly tap **Esc** for the HP Startup Menu. Choose **F10 BIOS Setup**. Find **Security → Secure Boot Configuration** and record the setting before changing it. Turning this off allows untrusted startup software to run. For this trial with the identified unsigned loader, change **Secure Boot** from enabled to disabled, keep **UEFI** mode, then choose **Save Changes and Exit**. Follow any confirmation displayed by HP. On restart, reopen F10, confirm the new setting and record it. Leave the TPM, Secure Boot keys, Legacy Support and permanent boot order alone. If the menu is locked or the choices do not match, exit without saving. If Secure Boot started disabled, record that and leave it as found unless the owner separately chooses a change. [HP 840 G7 BIOS simulator](https://h10032.www1.hp.com/ctg/Manual/c06692772.pdf) · [HP Secure Boot instructions](https://support.hp.com/us-en/document/ish_6930187-6931079-16)
2. [ ] Insert the identified stick. At the next HP logo tap **Esc**, then **F9 Boot Device Options**, or tap **F9** directly. Choose the **UEFI USB** entry matching the stick. Record its label. If the USB entry is missing or rejected, record the menu or error and go to recovery. [HP 840 G7 guides](https://support.hp.com/us-en/product/setup-user-guides/hp-elitebook-840-g7-notebook-pc/33386515)
3. [ ] Write down each stage you see, from the HP menu and first loader message to the PCP OS desktop, SAFE screen or last visible stage. Note elapsed time, any reported display mode, clipped edges, unreadable text, blank screen or restart. Photograph the full panel. If progress stops, record the last change and elapsed time, then recover. A firmware menu responding to keys establishes only firmware input.
4. [ ] At the **PCP OS desktop**, tap **F1** on the built-in keyboard. Look for **KEYS ON THE DESKTOP**. Tap **Esc** to return. Tap **F5**, copy the verification result and any failed check, then **Esc**. Record the exact key combination, including Fn if used. If the display is blank, keys cannot be evaluated. If SAFE or a failed check appears, retain its wording and go to shutdown and recovery.
5. [ ] If diagnostic capture is available, retain the startup report from this same boot and complete the two rows below. Stop for a thermal warning, unusual heat or a current limiting/critical flag. Keep the machine at idle.
6. [ ] On the desktop press **F10**. Expect **Switch PCP OS off?**. Press **Esc** once and check that the desktop returns. Press **F10** again, release the keys, then press a fresh **Enter**. Record the final text, power light, fan and whether the machine switched itself off. On the reviewed kernel the physical stop path halts the processor and can leave a stopped screen instructing you to hold the power button. Photograph it, then follow that instruction until the machine is off. Record a manual power-off separately. A dark display alone does not establish that power went off. [Power prompt and stopped screen](../../PCP_OS_Rust/crates/pcpos-kernel/src/session.rs) · [Physical stop path](../../PCP_OS_Rust/crates/pcpos-kernel/src/arch.rs)

## Processor and heat reports

At `355e5c13` the kernel sends these lines to its **COM1 serial console**. F5 shows self-test verification. The model-free build enables only `proposals`. Its two archived VM boots both reported a missing firmware root pointer and emitted no heat line. For processor enumeration, the diagnostic handoff should identify a loader that supplies the firmware ACPI root and a kernel that consumes it. The diagnostic image needs `heat` for the sensor and `processors` for secondary startup. [Build recipe](basic.py) · [VM receipts and logs](https://github.com/KirtusJ/pcp-os/issues/406#issuecomment-7081732160) · [Kernel report formatting](../../PCP_OS_Rust/crates/pcpos-kernel/src/main.rs)

The preparer records how to reach a tested on-screen view on the exact diagnostic image or how to capture this machine's physical COM1 output. A USB serial adapter plugged into the EliteBook does not by itself capture COM1. Start the recorder before power-on. If neither route is available, mark the diagnostic rows **not visible** or **not run** and complete the desktop trial. [Serial driver and sensor access](../../PCP_OS_Rust/crates/pcpos-kernel/src/arch.rs)

| Observation | What to write down |
| --- | --- |
| Firmware processor list | Copy the whole `processors: the firmware lists … usable` line, or `processors: not known (…)` with its reason. If present, copy the separate `other processors:` startup/parked result and counter. Compare to the normal OS's logical count. Disagreement is a finding. A firmware count by itself establishes neither successful secondary startup nor parallel application execution. |
| First sensor sample | Copy the complete `heat:` line, the time and whether the machine was idle. Record **degrees below TjMax on the reporting processor** and copy each flag about current limiting, earlier limiting or critical status. Keep the reported distance in its original units. Absolute temperature and the throttling threshold require separate measurements. If the kernel says `heat: not known`, copy its reason. If the feature or capture is missing, record that separately and leave the numerical value blank. [Sensor decoder](../../PCP_OS_Rust/crates/pcpos-kernel/src/arch/heat.rs) |

## Return to the normal system after every outcome

- [ ] If boot fails, note the last visible stage and error. Exit a responsive HP menu normally. If the internal OS started instead, shut it down normally. If the PCP OS trial is frozen, hold the power button until the machine turns off. Remove the USB once off. Record a forced stop as recovery.
- [ ] Power on into **Esc → F10**. Restore the exact original Secure Boot setting, save and exit. If the baseline was **enabled**, reopen setup to confirm **enabled** before starting the internal OS. If the normal OS does not start automatically, use **F9** to select its original internal boot manager once. If this fails, photograph the error and seek normal device support.
- [ ] If Windows requests BitLocker recovery, verify that you are on the expected internal Windows recovery screen. Match the displayed key ID to the private backup. If the ID does not match or support is needed, stop and contact the administrator. Keep recovery credentials out of PCP OS, GitHub and the trial report. Leave firmware defaults, TPM contents and disk partitions intact.
- [ ] Sign in to the normal OS. Compare Secure Boot and encryption protection with the recorded baseline. If protection was suspended for the trial, resume it and verify its status. Record whether ordinary startup and access to your files returned. Keep recovery **open** until those checks succeed.

| Outcome | Result and evidence |
| --- | --- |
| UEFI USB selection and last boot stage | |
| Built-in display, F1/Esc, F5 and exact verification text | |
| Processor line / secondary startup / heat line and flags | |
| F10 cancel, confirmed halt, automatic or manual power-off | |
| USB removed, Secure Boot restored, normal OS and protection verified | |

Use **observed**, **failed**, **not visible** or **not run** for each row. Keep the original notes or photos privately. Share only redacted copies without serial numbers, asset tags, UUIDs or recovery data.
