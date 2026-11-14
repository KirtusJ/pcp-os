# PCP OS

PCP OS runs Adversarial Claim Review (ACR) and keeps a verifiable register of what was examined, who examined it and what evidence they used. This repository contains the Rust host program `pcpos`, its integrity core, the experimental kernel and the source used to build PCP editions.

**[Browse the PCP OS Store](store/README.md)** for packaged applications and built-in research instruments. The [instrument catalog](store/instruments/README.md) gives the inputs for each check, with CLI and MCP examples. The [game shelf](store/index.html) includes offline browser players and native PCP OS guests.

## Run the Rust host

With a Rust toolchain installed, run these commands from this repository. They build the host for your machine and list the instruments in that build.

```sh
cd PCP_OS_Rust
cargo run --locked --release --bin pcpos -- instrument list
```

For build targets, tests, register verification and the kernel's QEMU requirements, see the [Rust workspace README](PCP_OS_Rust/README.md). Rust is the reference implementation for host security and built-in instruments. [Rust primacy policy](RUST_PRIMACY.md)

## Repository guide

| Path | What you will find |
| --- | --- |
| [Store](store/README.md) | Application packages, browser players, native launch instructions and the catalog of built-in host instruments. |
| [Rust workspace](PCP_OS_Rust/) | The host, integrity core, package-manager library, kernel and reference vectors. |
| [Python implementation](PCP_OS_Python/) | The companion implementation tested against the Rust vectors. Place `ACR_1.0_Implementation.zip` beside the Python directory, then run `python3 -m unittest discover -s TESTS` inside it. |
| [OS documentation](os-docs/) | The overview, specification and async execution notes. |
| [Build](build/) | Release scripts, method build scripts and edition evidence. |
| [Audit](audit/) | Simulation, live-session and calibration harnesses and their evidence. |

## Building releases and reproducing checks

From the Rust workspace, run `cargo run --release --bin pcpos-vectors -- vectors` before `cargo test` to generate the 6.3 MB guard-case file. Three Python tests skip their vector comparisons when that file is absent. The [kernel guide](PCP_OS_Rust/crates/pcpos-kernel/README.md) covers QEMU and building the probe and Doom guests.

The [release script](build/release.sh) builds and checks the editions and verifiers, including the host programs for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`. It expects the edition's `ACR_1.0_Implementation.zip` extracted at `src/method`. The [method builders](build/method/) and [evidence file](build/evidence.json) supply release inputs. The [documentation builder](build/build_os_docs.py) produces reading copies and requirements data. The [reconciliation record](RECONCILIATION.md) identifies which source and packaged binary were compared.

## Rights

Source supplied by the rights holder, separate from the distributed editions. Proprietary. All rights reserved. Copyright © 2026 Kirtus Justus.
