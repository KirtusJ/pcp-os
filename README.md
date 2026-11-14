# PCP 1.0 | Source

The rights holder's own files. Not part of any edition.

**PCP OS is primarily Rust. Rust holds primacy for security reasons.** Before contributing, read the [permanent Rust policy](RUST_PRIMACY.md). It identifies the reference implementation, where host security fixes and instruments belong, and how the Python implementation is checked against the Rust vectors.

| Folder | What it is |
| --- | --- |
| `PCP_OS_Rust/` | The native core, the host program `pcpos`, the kernel, the published vectors. `cargo run --release --bin pcpos-vectors -- vectors` first (it writes the 6.3 MB case file, which is generated and not kept here), then `cargo test`; `crates/pcpos-kernel/run.sh` (it compiles the kernel in one fixed folder, so the image is the same file wherever this archive is unpacked); `crates/pcpos-kernel/programs/probe/build.sh` and `programs/doom/build.sh` for the programs the kernel's tests load. The programs supplied are built for the targets `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, as `build/release.sh` does. Without that first step three of the Python tests are skipped, and say why |
| `PCP_OS_Python/` | The second implementation, tested against the Rust vectors. Place `ACR_1.0_Implementation.zip` beside this folder and run `python3 -m unittest discover -s TESTS` in it |
| `os-docs/` | The PCP OS Overview and Specification as written; `build/build_os_docs.py` makes the reading copies, the requirements data and the backgrounds |
| `build/` | `release.sh` runs the whole build in order: tests, design tokens, plugins, the documented events through each packaged program, validation, the machine form, the three editions and their verifiers |
| `audit/` | The harnesses of the eighth to thirteenth audits: `sim/sim.py` drives the packaged program through constructed scenarios, and both implementations read every register a scenario leaves; `live/matrix.py` runs the method's behavioural probes in each arm of delivery; `live17/run.sh` to `live20/run.sh` hold the live conversations on the plugins of those builds; `calibration/` holds the Calibration: `run.py` (five deliveries to two models, every conversation read twice), `hidden.sh` (runs a measured session where the probes and their criteria are not on the disk it sees) and `reexamine21.py` (the second look at build 21's conversations) |

The method packet is not here: it is `ACR_1.0_Implementation.zip` inside every edition, and the build reads it extracted at `src/method`. `build/method/` holds the scripts that make it, as they stand after the fifteenth round, with the edits of the twelfth to fifteenth rounds. `build/evidence.json` holds what each edition's evidence statement says of live sessions and Calibrations, so that this archive and a release's base editions rebuild the editions.

Proprietary; all rights reserved. Copyright (c) 2026 Kirtus Justus.
