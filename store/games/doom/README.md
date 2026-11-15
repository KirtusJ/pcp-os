# Doom on PCP OS

Run the Doom guest with your own IWAD or a copy of Freedoom Phase 1. From the repository root, supply your built PCP OS kernel and local data file to the native launcher. The guest runs without networking. The launcher stages a private copy of the game data and removes that copy when QEMU exits. [Freedoom 0.13.0 release](https://github.com/freedoom/freedoom/releases/tag/v0.13.0) · [Kernel build](../../../PCP_OS_Rust/crates/pcpos-kernel/README.md)

~~~sh
cargo run --locked --release --manifest-path store/native/Cargo.toml -- KERNEL.elf32 doom /path/to/freedoom1.wad
~~~

Use the arrow keys to move and turn, Ctrl to fire, Shift to run, Alt to strafe, Space to operate a door or switch, and Escape for the game menu. The Store distributes guest code and original additions. The player supplies the IWAD.

## Play a mod

With Python 3.9+ installed, use one of these game IDs in place of `doom`. Pass an unmodified Freedoom Phase 1 v0.13.0 IWAD as the last argument. Its SHA-256 is `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`. A classic Ultimate Doom 1.9 IWAD is also recognized for locally assembling the original episode geometry. The archive recipes were written for shareware 1.9. The Store mod launcher uses Freedoom or Ultimate Doom. The unmodified engine can play shareware on its own. The native play checks below used Freedoom. The Ultimate Doom option has not been play-tested with a commercial copy.

| Game ID | What to do |
| --- | --- |
| [ultra-omega-barrel](../ultra-omega-barrel/) | Fight the Omega barrel family in episode one. |
| [barrel-bowl](../barrel-bowl/) | Bowl a ten-frame game on Phobos Lanes. |
| [midnight-barrel-express](../midnight-barrel-express/) | Release both interlocks and brake the train into Deimos Junction. |
| [forklift-certified](../forklift-certified/) | Lure the forklift driver through three cargo stacks. |

For the three E1M9 games, the pack installs the original custom geometry, things, textures, music and gameplay states over your local base. The Omega campaign uses the base game's episode-one levels. On Freedoom the installer translates the entire final map so its all-skill Baron at (1152, -1568) occupies the recovered engine's boss position (304, 2864). Relative map positions, node boxes and blockmap cells are preserved. This is a Freedoom adaptation. The original pickup coordinates, foundry wall and sector edits apply only to the matching Ultimate Doom map set. Environmental barrel frames come from the player's base. The 27 replacement boss, husk and mini frames come from the supplied sprite generator.

To prepare a WAD for a separate run, choose a new output name:

~~~sh
python3 -B store/games/doom/prepare.py barrel-bowl /path/to/freedoom1.wad /path/to/my-bowl.wad
~~~

Keep the generated IWAD with your own game data. The launcher's temporary image has the same contents. Choose a local QEMU executable with `PCPOS_QEMU` and a QEMU audio backend with `PCPOS_AUDIO`. Set `PCPOS_PYTHON` if the mod launcher needs a particular Python executable. [Native launcher](../../native/)

## Rebuild the guests

The [base guest source and build guide](source/) supplies the engine, platform and runtime sources, the release build recipe and the expected ELF digest. Each mod ships the recovered native ELF and corresponding source. To rebuild a mod, stage its sources in a new directory with Python 3.12+ and Git. This expands the common upstream source tree and applies that game's engine patch. The staged `BUILD.txt` names its compiler, dependencies and exact command.

~~~sh
python3 store/games/ultra-omega-barrel/source/stage-source.py barrel-bowl /path/to/new-bowl-source
cd /path/to/new-bowl-source/omega-source
sh build-musl.sh
python3 build_pcpos.py --cc ./toolchain/bin/musl-gcc --engine engine/doomgeneric --chocolate chocolate-doom --port pcpos-port bowl.elf
~~~

The original builders used x86-64 GNU/Linux, GCC 14.2.0, musl 1.2.5 and GNU binutils. The archive includes the engine, platform interface, Chocolate Doom OPL sources, musl sources, build scripts and upstream notices. Each derivative patch applies to this common tree. Staging reproduced every file in the original source archives for Bowl (237), Express (239) and Forklift (237). All four mod source trees also compiled with Zig 0.14.1 and Alpine musl. Each resulting static PIE booted on PCP OS and accepted input on its named map. The shipped mod ELFs are the originals from the authoring bundles. Each game directory retains its original art, map and score generators. The generators can require Pillow, and some drawing tools use fonts identified in their source. [Engine license](../ultra-omega-barrel/source/COPYING) · [PCP OS notice](../ultra-omega-barrel/source/PCP-OS-NOTICE.md)

## Native play checks

The base guest rendered Freedoom E1M1 at 640 × 400 on PCP OS 0.3.1. Each mod was booted separately on PCP OS 0.3.1 in a network-isolated QEMU guest using the final assembled Freedoom v0.13.0 image. In Omega the guest placed the 1,050-HP boss on E1M8 and firing consumed ammunition. Barrel Bowl scored a throw and reported the next ball and standing pins. Express started at 72 km/h, displayed the port interlock and awarded shells during movement. Forklift displayed Dock 07, the charging driver and three cargo stacks. These checks covered the named maps and controls. Campaign completion, certification and audible playback were outside the runs.

The Store screenshots show these native guests using local Freedoom Phase 1 v0.13.0 data. Freedoom is copyright the Freedoom project and its contributors, distributed under its included license. [Freedoom copying terms](evidence/Freedoom-COPYING.txt) · [Upstream credits and release](https://github.com/freedoom/freedoom/releases/tag/v0.13.0)
