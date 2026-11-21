# A3 read-back of PCP OS #312

**Decision on `f9743bfbb61e771806c165779d15b854cc29232e` — hold.** When a chosen name uses three pages at 320×200, the person cannot tell whether one or two widget lines are hidden. With a short name, the same guest shows the two different counts. B3 independently reported this with seven pins, three completed sessions, and one versus five lines. [B3’s report](https://github.com/KirtusJ/pcp-os/pull/312#issuecomment-7063373480)

The source tree is `992986aac40fc4e25ed54f23d6ae299eb7d8b821`. Every run below used that source or its hosted tester CD. All guest observations used QEMU on an emulated PC. The two builds used different Rust compilers.

| Record | Result and contents |
| --- | --- |
| `widget-count.tar.xz` | Eight original 320×200 frames, their text, serial, command and test script. Two pins, zero completed sessions. On all three pages of the long name, toggling from one hidden widget line to two left the complete frames byte-identical. Moving to the short pin exposed the changed count. The guest passed 72 self-tests and exited 0. |
| `suite-readback.json`, `kernel-suite.zip` | Local Rust 1.100.0 build. The full default kernel suite ran under QEMU 10.0.13, including the extended soak. Of 20 groups, 19 passed. Of 108 assertions, 107 passed. The recorded Doom demo ended with exception 14 after 62 seconds. The suite exited 1. The archive includes the original assertion records, serials, screenshot PNGs, audio hashes, runner, console log and the QMP transport diff. [Doom tracking issue](https://github.com/KirtusJ/pcp-os/issues/310) |
| `display-readback.json`, `desktop-guests.tar.xz` | Kernel and Queue Desk extracted from the hosted two-program ISO, Rust 1.97.0. The 320×200 and 640×480 guests and both 1920×1080 bar and dock guests each exercised five selections. The 32-byte filename launched and exited 0. The 33- and 63-byte names and both 64-byte aliases retained their identity and displayed the admission refusal. At 320×200 both 64-byte names reconstructed from the three pages and differed on page two alone. All four guests passed 72 self-tests and exited 0. Each frame has the original PPM, decoded rows and a hash. |
| `extra-and-initial-trials.tar.xz` | A 113-byte name was readable across five pages at 320×200, with visible refusal on page five. Both compact trials with this extra module exited 0 but reported one failed self-test. Power-off controls with a single module and no launch isolated the register boundary. At 64 bytes the guest passed. At 65 and 113 bytes it refused ARTIFACT_ADDED and reported one failed self-test. The archive also preserves early graphical samples taken mid-repaint. Repeating the functional cases after the framebuffer settled produced the four runs above. |
| `compact-regression-replay.tar.xz` | Local replay of the repository’s fourteen-pin tester-CD check, its serials, images, per-case results and three altered-screen controls. The first launcher attempt could not create its Unix socket. The subsequent run used QMP over stdio. |
| `SHA256SUMS` | SHA-256 of each archive and both stand-alone result files. |

## Commands and inputs

The local kernel build used `sh src/PCP_OS_Rust/crates/pcpos-kernel/run.sh build` with an isolated build directory. The suite command was

```sh
env -u PCPOS_TESTS_ONLY PCPOS_ADAPTER_CHECKOUT="$PWD/src" python3 -u run_stdio.py "$PWD/kernel.elf32" "$PWD/programs" "$PWD/data" "$PWD/full"
```

For a desktop run, extract the hosted artifacts with `python3 read_disc.py` and run

```sh
python3 guest_names.py src disc-kernel.elf32 disc-rdesk.elf "$QEMU" final-320x200 320x200
python3 widget_count.py "$QEMU"
python3 artifact_name_control.py "$QEMU"
```

The other desktop modes are `640x480`, `bar` and `dock`, each in a fresh output folder. To replay the tracked checker, use `python3 read_plain.py`, then `python3 replay_stdio.py plain-cd replay-pins --qemu "$QEMU"`. Each archive uses paths relative to the same work directory. Set `QEMU` to the full path of the emulator and put the exact source tree in `src`.

The two hosted ISO receipts identify the frozen commit and tree. Their extracted kernels and the kernel used by the hosted pin checker matched byte-for-byte. The local suite kernel has SHA-256 `a0c2725bbf0b2cb1c4e0d3edbb4551a830cd4fa639da38b6bc39b542b93a25c5`. The hosted CD kernel has SHA-256 `fbfb3102d8d1535108e7457b15a2eb59c06487e62adc3c3d51d6f2c3ba3eed38`. The CD and Queue Desk hashes and kernel measurements are in the receipts. [Hosted tester CD workflow](https://github.com/KirtusJ/pcp-os/actions/runs/44908609151)

All five hosted workflows ultimately passed. The first UEFI denial-control attempt timed out waiting 20 seconds for its published-kernel guest to exit. The rerun passed on the same source. A3’s native desktop runs cover the four listed configurations. B3 owns the independent Files review. Physical hardware, other firmware, and an application-calibration score are outside these results. [First firmware attempt](https://github.com/KirtusJ/pcp-os/actions/runs/44908609210/job/136859393354) · [Successful retry](https://github.com/KirtusJ/pcp-os/actions/runs/44908609210/job/136861646557)
