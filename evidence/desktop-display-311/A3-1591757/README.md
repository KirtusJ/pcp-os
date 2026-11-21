# Director A3’s read-back of #312 at 1591757

**Hold the merge on the ten-page Home hint.** The widget count and both paging arrows fit for all three pages of a 64-byte name, including place 17/17. A 255-byte name also appears on Home. At place 17/17, the first page reads `Place 17/17 | name 1/10 <-..` and the last reads `Place 17/17 | name 10/10 <..`. All ten parts joined to the supplied name. The guest also recorded a refused ARTIFACT_ADDED and one failed self-test. The 64-byte controls powered off cleanly. Both the local and the hosted kernel gave these results for the 255-byte case.

Source commit `15917575b9e205e9042da2840492751945f32fff` has tree `5553028dbc6a1601e092fd36fc4c1175650c4f3f`. The full default kernel suite exited 1. 19/20 groups and 107/108 assertions passed. The single failed assertion required the recorded Doom demo to exit DONE. Its guest reported exception 14. The suite used a local Rust 1.100.0 build. All five hosted workflows passed.

| Archive | What it contains |
| --- | --- |
| `kernel-suite.zip` | Default suite, original per-assertion JSON, console and guest serials, saved images, WAV hashes, source runner, environment and QMP transport diff. |
| `launch-guests.tar.xz` | Original QEMU frames, text, per-case checks and guest serials for 32-, 33-, 63- and 64-byte names. Rust 1.97.0 tester-CD kernel. Four displays, including the 1920×1080 bar and dock. |
| `widget-pages.tar.xz` | Original 320×200 frames for zero, one and two hidden lines at places 15/15 and 17/17. Includes the independent hosted and local runs on the 255-byte, ten-page pin. |
| `programs-and-help.tar.xz` | Seven Programs-screen boots plus two compact Home key-help boots. Includes normal and console-only controls, changed-height controls, and two labeled copies of a good frame with an individual row erased. |
| `replay-and-workers.tar.xz` | Replay of the tracked fourteen-pin regression checker, three rejected altered-screen controls, PCP dispatch commands and final task states. The replay uses QMP over stdio. |

The stand-alone JSON files give counts, hashes, text read from the guest and outcomes. Reproduce using the commands in the records. Supply a checkout of the exact commit in `src` and the QEMU executable in place of the recorded absolute path. Each output folder must be fresh.

The desk workers ran with the tracked Rust `pcp-delegate` dispatcher. I separately read the files after they exited. The program-row and F1 findings are assigned to #314. The kernel suite failure remains on #310. Files is B3’s independent review. Hardware, real firmware and production behavior are outside this QEMU check.
