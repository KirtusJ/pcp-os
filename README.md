# PCP OS

PCP OS runs Adversarial Claim Review (ACR) and keeps a verifiable register of what was examined, who examined it and what evidence they used. This repository contains the Rust host program `pcpos`, its integrity core, the experimental kernel and the source used to build PCP editions.

| If your task is to… | Start here | How to use it |
| --- | --- | --- |
| Connect a language model to PCP | [Run a local Qwen register read](examples/qwen/) | Let a model request `acr_status`, inspect the Rust host's reply and compare it with the model's answer. Includes the connection settings for MCP clients. |
| Measure an assistant’s retrieval of the method | [Enterprise retrieval check](examples/method-readback/retrieval.md) | Generate questions and score the participant’s saved reply offline with the Build 22 tools. |
| Run a calculation or check | [Built-in apps](#built-in-apps) | Discover the host’s executable instruments with `pcpos instrument list`. Agents can also call `acr_instrument` with `{"instrument":"list"}`. |
| Choose the rules for a review | [Method modules](#method-modules) | Read the specification for Core and each selected specialist. `pcpos modules <method-directory>` lists the modules supplied to that host. |
| Find a downloadable application | [Store package index](store/README.md#package-index) · [Games](#games) | Each published package entry identifies its version, payload files and hashes. The package catalog identifies each runtime. |
| Monitor directors from Android | [Director HelpDesk](https://github.com/KirtusJ/android-helpdesk) | Read the public director exchange and import a Parrots worker snapshot. |
| Govern recursive workers | [Parrots all the way Down](store/apps/parrots-all-the-way-down/) · [Package](store/index/kirtusj/pa/rr/parrots-all-the-way-down) (`kirtusj/parrots-all-the-way-down`) | Set depth, live-worker and lifetime budgets. Inspect the native queue or admit a real process with the hosted adapter. |
| Open a web page from a terminal or agent | [Surfbird hosted reader](https://github.com/KirtusJ/surfbird) | Use the shared terminal and machine session. Send hosted changes to the standalone repository. |
| Run Surfbird on the PCP-OS graphical display | [Native guest and QEMU replay](apps/surfbird/guest/) · [Integration and hosted pin](apps/surfbird/) | Follow numbered links with the guest keyboard. The Ethernet build uses DNS and TCP to reach a local HTTP fixture. |

## Cryptography applications

**[Keyrook](store/apps/keyrook/)** runs recursive workers against synthetic cipher challenges. Train two attack strategies in the native graphical arena or divide a public challenge among external workers with the hosted Rust tools. [Store entry](store/index/kirtusj/ke/yr/keyrook)

## Games

Night Post and Bellweather Harbor also ship offline browser players. Download a browser player from its GitHub file page using **Download raw file**, then open the HTML locally. For native play, the [launcher guide](store/README.md#play-on-pcp-os) covers the six games.

| Game | What you do | Play and inspect |
| --- | --- | --- |
| **Night Post · Return Mail** | Fly a coastal mail route and drop letters into nets aboard passing ships. | [Browser player](store/games/night-post/dist/Night_Post.html) · [Manual](store/games/night-post/README.md) · [Package](store/index/kirtusj/ni/gh/night-post) (`kirtusj/night-post`) |
| **Bellweather Harbor · The Second Manifest** | Run a harbor, trade cargo and investigate discrepancies in the shipping records. | [Browser player](store/games/bellweather-harbor/dist/Bellweather_Harbor.html) · [Manual](store/games/bellweather-harbor/README.md) · [Package](store/index/kirtusj/be/ll/bellweather-harbor) (`kirtusj/bellweather-harbor`) |
| **Gullpost Express** · native | Deliver a mailbag across three coastal routes and collect postage stamps. | [Game](store/games/gullpost-express/) · [Package](store/index/kirtusj/gu/ll/gullpost-express) (`kirtusj/gullpost-express`) |
| **Gullpost Express 64** · native | Explore Lighthouse Island and deliver the final letter. | [Game](store/games/gullpost-express-64/) · [Package](store/index/kirtusj/gu/ll/gullpost-express-64) (`kirtusj/gullpost-express-64`) |
| **Sunspur Rally** · native | Drive a desert time trial through five gates. | [Game](store/games/sunspur-rally/) · [Package](store/index/kirtusj/su/ns/sunspur-rally) (`kirtusj/sunspur-rally`) |
| **Flux Rink** · native multiplayer prototype | Play a two-player 3D air-hockey match locally or across two linked QEMU guests. | [Game](store/games/flux-rink/) · [Package](store/index/kirtusj/fl/ux/flux-rink) (`kirtusj/flux-rink`) |

## Native rendering engines

**Heliograph** is a Rust 3D software renderer with an interactive material lab, used by Flux Rink. [Engine, native lab and benchmarks](store/engines/heliograph/) · [Package](store/index/kirtusj/he/li/heliograph) (`kirtusj/heliograph`) · [Native Store launcher](store/native/README.md)

## Built-in apps

These instruments ship in the Rust host. Run `pcpos instrument list` to see what your binary contains. The [instrument guide](store/instruments/README.md) lists inputs and shows how to run the same check from a terminal or an MCP client.

| Area | Instrument | What it checks |
| --- | --- | --- |
| Mathematics | `mathematics.exact` | Evaluates rational expressions and compares their values exactly. |
| Mathematics | `mathematics.protected_bins` | Compares squared norms and odd coordinate gcds of signed columns in corresponding bins. |
| Mathematics | `mathematics.statement` | Checks a Lean statement against a pinned digest and screens for trivial forms. |
| Science | `science.order` | Determines which of two artifacts entered a given register first. |
| Science | `science.incident_bounds` | Checks whether every possible completion of an incident table’s unread binary outcomes stays within a stated fraction range. |
| Statistics | `statistics.grim` | Tests whether an integer sample of the stated size could yield a reported rounded mean. |
| Statistics | `statistics.percent` | Tests whether a reported percentage could arise from a whole count out of the stated total. |
| Engineering | `engineering.units` | Compares physical quantities after conversion to compatible units. |
| Cryptography | `cryptography.digest` | Computes SHA-256 for supplied text and optionally compares a claimed digest. |
| Quality assurance | `quality.coverage` | Reports finding coverage for the claims of a commissioned review. |
| Core | `core.calibration` | Checks the internal consistency of a Calibration Record and the claim made from its counts. |

## Method modules

ACR 1.0 ships one Core specification and six specialist specifications. Core applies when ACR is authorized. Specialist modules extend Core when selected for a task. The [release catalog](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/REFERENCE/Module_Catalog.csv) records each component's status, dependency and specification digest.

| Module | Component ID | Review scope |
| --- | --- | --- |
| [Core](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/01_Core/SOURCE/Specification.md) | `acr.core` | Review authority, claims, evidence, challenges and dispositions. |
| [Mathematics](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/02_Mathematics/SOURCE/Specification.md) | `acr.mathematics` | Definitions, hypotheses, proof steps and counterexamples. |
| [Science](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/03_Science/SOURCE/Specification.md) | `acr.science` | Experimental design, measurements and the evidence for a scientific claim. |
| [Engineering](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/04_Engineering/SOURCE/Specification.md) | `acr.engineering` | Requirements, operating conditions, failure modes and verification. |
| [Cryptography](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/05_Cryptography/SOURCE/Specification.md) | `acr.cryptography` | Security claims, threat models, assumptions and adversarial tests. |
| [Statistics](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/06_Statistics/SOURCE/Specification.md) | `acr.statistics` | Sampling, inference, uncertainty and statistical assumptions. |
| [Quality Assurance](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/07_Quality_Assurance/SOURCE/Specification.md) | `acr.quality_assurance` | Acceptance criteria, test coverage and reproducibility. |

With a method directory on disk, run `pcpos method <method-directory>` to check it against the host’s pinned release, then `pcpos modules <method-directory>` to list its components. Use `pcpos module list <register>` to inspect the workspace profile.

For example, to set an existing workspace’s profile to Core and Science, the person directing the review runs `pcpos workspace <register> --by <label> --method <method-directory> science`. This records the selection. Use `pcpos module list <register>` to confirm it.

## Run the Rust host

With a Rust toolchain installed, run these commands from this repository. They build the host for your machine and list the instruments in that build.

```sh
cd PCP_OS_Rust
cargo run --locked --release --bin pcpos -- instrument list
```

For build targets, tests, register verification and the kernel's QEMU requirements, see the [Rust workspace README](PCP_OS_Rust/README.md). The host is built and tested on Linux, macOS and Windows on every change; on Windows one file check is weaker on ReFS volumes than on NTFS. [Platform status](PCP_OS_Rust/README.md#host-platforms) Rust is the reference implementation for host security and built-in instruments. [Rust primacy policy](RUST_PRIMACY.md)

## Repository guide

| Path | What you will find |
| --- | --- |
| [Store](store/README.md) | Application packages, browser players, native launch instructions and the catalog of built-in host instruments. |
| [Rust workspace](PCP_OS_Rust/) | The host, integrity core, package-manager library, kernel and reference vectors. |
| [Python implementation](PCP_OS_Python/) | The companion implementation tested against the Rust vectors. Place `ACR_1.0_Implementation.zip` beside the Python directory, then run `python3 -m unittest discover -s TESTS` inside it. |
| [OS documentation](os-docs/) | The overview, specification and async execution notes. Reading copies of the 20 November 2026 revision: [Overview](os-docs/PCP_OS_Overview.pdf) · [Specification](os-docs/PCP_OS_Specification.pdf). The Markdown files beside them are the Build 22 text of 13 November 2026. |
| [Build](build/) | Release scripts, method build scripts and edition evidence. |
| [Audit](audit/) | Simulation, live-session and calibration harnesses and their evidence. |

## Building releases and reproducing checks

From the Rust workspace, run `cargo run --release --bin pcpos-vectors -- vectors` before `cargo test` to generate the 6.3 MB guard-case file. The Python machine-table and register-replay comparisons use the shipped vectors. Generating the guard cases also enables the guard comparison. The [kernel guide](PCP_OS_Rust/crates/pcpos-kernel/README.md) covers QEMU and building the probe and Doom guests.

The [release script](build/release.sh) builds and checks the editions and verifiers, including the host programs for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`. It expects the edition's `ACR_1.0_Implementation.zip` extracted at `src/method`. The [method builders](build/method/) and [evidence file](build/evidence.json) supply release inputs. The [documentation builder](build/build_os_docs.py) produces reading copies and requirements data. The [reconciliation record](RECONCILIATION.md) identifies which source and packaged binary were compared.

## Rights

Source supplied by the rights holder, separate from the distributed editions. Proprietary. All rights reserved. Copyright © 2026 Kirtus Justus.
