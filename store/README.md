# PCP OS Store

| What you want to do | Where to start |
| --- | --- |
| Run a Science, Mathematics, Statistics, Engineering or review instrument | [Host instrument catalog and CLI/MCP examples](instruments/README.md) |
| Get a packaged application or verify its files | [Package index](#package-index) |
| Choose a review module or read its specification | [Released method modules](../README.md#method-modules) |
| Play a game | [Offline browser shelf](index.html) or [native-only games and engine](#native-only-store) |

## PCP OS game store

Download or clone the repository and open [the game shelf](index.html) in your browser. The two browser games below each fit in a single HTML file. If you are browsing on GitHub, open the player link below, use **Download raw file**, then open that file locally. Both games run offline with WebAssembly enabled.

| Game | First move | Browser player | Native guest |
| --- | --- | --- | --- |
| **Night Post · Return Mail** | Choose a route and press Space to take flight. Steer a kingfisher over the mail net and drop the letter just before you reach it. | [Night_Post.html](games/night-post/dist/Night_Post.html) | [nightpost.elf](games/night-post/dist/nightpost.elf) |
| **Bellweather Harbor · The Second Manifest** | Ring the bell. Your first trip lands a crate of salt. Sell it, save it for a contract, or investigate the odd receipt. | [Bellweather_Harbor.html](games/bellweather-harbor/dist/Bellweather_Harbor.html) | [bellweather.elf](games/bellweather-harbor/dist/bellweather.elf) |

The [Night Post flight manual](games/night-post/README.md) covers routes, airbrakes and postmarks. The [Bellweather Harbor guide](games/bellweather-harbor/README.md) covers chartering, trade, manufacturing, and the casebook. Each game directory includes its Rust source, Cargo lockfile and build scripts. Browser saves are local to the browser and file location. Bellweather's logbook can export and import a save when moving between copies.

## Native-only Store

| Shelf | Title | Runtime and contents | Listing |
| --- | --- | --- | --- |
| Games · 2D courier platformer | [Gullpost Express](games/gullpost-express/) | Deliver a mailbag across three coastal routes and collect postage stamps. | [Package](index/kirtusj/gu/ll/gullpost-express) |
| Games · 3D courier platformer | [Gullpost Express 64](games/gullpost-express-64/) | Explore Lighthouse Island and deliver the final letter. | [Package](index/kirtusj/gu/ll/gullpost-express-64) |
| Games · racing | [Sunspur Rally](games/sunspur-rally/) | Single-player time trial, x86-64 PCP OS guest. | [Package](index/kirtusj/su/ns/sunspur-rally) |
| Games · multiplayer sports | [Flux Rink](games/flux-rink/) | Two-player 3D prototype. Stock kernel for shared keyboard; optional cable kernel for linked guests. | [Package](index/kirtusj/fl/ux/flux-rink) |
| Games · first-person shooter | [Doom](games/doom/) | Play the native Doom port with a local IWAD. | [Package](index/kirtusj/do/om/doom) |
| Games · episode-one campaign | [Ultra Omega Barrel](games/ultra-omega-barrel/) | Fight the Omega barrel family across episode one. | [Package](index/kirtusj/ul/tr/ultra-omega-barrel) |
| Games · ten-pin bowling | [Barrel Bowl](games/barrel-bowl/) | Aim and roll barrels through ten scored frames. | [Package](index/kirtusj/ba/rr/barrel-bowl) |
| Games · railway braking | [Midnight Barrel Express](games/midnight-barrel-express/) | Release the interlocks and brake into Deimos Junction. | [Package](index/kirtusj/mi/dn/midnight-barrel-express) |
| Games · loading-dock encounter | [Forklift Certified](games/forklift-certified/) | Lure a forklift through three cargo stacks. | [Package](index/kirtusj/fo/rk/forklift-certified) |
| Engines · 3D rendering | [Heliograph](engines/heliograph/) | Reusable Rust renderer and a native material and performance lab. | [Package](index/kirtusj/he/li/heliograph) |

Use the [native Store launcher](native/README.md) for these entries. The engine's installable artifact is its lab ELF; the renderer crate lives alongside it in the pinned source tree.

<a id="play-both-on-pcp-os"></a>

## Play on PCP OS

From the repository root on a machine with QEMU and the [kernel build toolchain](../PCP_OS_Rust/crates/pcpos-kernel/README.md), run the launcher. It builds and measures the kernel and opens one menu with the six Rust games. Doom and its four mods use a local IWAD and the [Doom launcher instructions](games/doom/).

```sh
sh store/play-native.sh
```

Use Up or Down to highlight a game and Enter to launch. Escape returns to the launcher, where you can select another game in the same session. Native saves last until the guest powers off. To use a previously built kernel, set `PCPOS_KERNEL_IMAGE` to its absolute path. On a QEMU build with SDL audio, append `-audiodev sdl,id=snd -device AC97,audiodev=snd` for sound.

## Package index

The [game build and replay report](../audit/games/README.md) records the tested versions, hashes of the supplied artifacts and limits of the playback checks.

The Store uses the repository's `pcpos-pkg` application entry format. Files in an entry use paths from the repository root and can be fetched from the entry's immutable payload commit. Listings pin the artifacts supplied for each runtime. Native-only packages pin their x86-64 guest executable. The full source is in the same commit alongside them.

| Application | Listing |
| --- | --- |
| `kirtusj/night-post` | [Package entry](index/kirtusj/ni/gh/night-post) |
| `kirtusj/bellweather-harbor` | [Package entry](index/kirtusj/be/ll/bellweather-harbor) |
| `kirtusj/gullpost-express` | [Package entry](index/kirtusj/gu/ll/gullpost-express) |
| `kirtusj/gullpost-express-64` | [Package entry](index/kirtusj/gu/ll/gullpost-express-64) |
| `kirtusj/sunspur-rally` | [Native racing game](index/kirtusj/su/ns/sunspur-rally) |
| `kirtusj/flux-rink` | [Native multiplayer game](index/kirtusj/fl/ux/flux-rink) |
| `kirtusj/heliograph` | [Native engine lab](index/kirtusj/he/li/heliograph) |
| `kirtusj/doom` | [Doom](index/kirtusj/do/om/doom) |
| `kirtusj/ultra-omega-barrel` | [Ultra Omega Barrel](index/kirtusj/ul/tr/ultra-omega-barrel) |
| `kirtusj/barrel-bowl` | [Barrel Bowl](index/kirtusj/ba/rr/barrel-bowl) |
| `kirtusj/midnight-barrel-express` | [Midnight Barrel Express](index/kirtusj/mi/dn/midnight-barrel-express) |
| `kirtusj/forklift-certified` | [Forklift Certified](index/kirtusj/fo/rk/forklift-certified) |

For a separately downloaded player, open its package entry above and compare the SHA-256 for the matching `.html` path with the downloaded file. For example, run `Get-FileHash -Algorithm SHA256 ./Night_Post.html` in PowerShell or `sha256sum ./Night_Post.html` on Linux in the download directory.

To check the catalog identities, file lengths and SHA-256 hashes against the checkout, run the verifier from the repository root.

```sh
cargo run --locked --manifest-path PCP_OS_Rust/Cargo.toml -p pcpos-pkg --example verify-game-store
```

The verifier checks the named applications against local files. The entries pin the upstream payload commit, file sizes and SHA-256 digests. To obtain an application, download its player or use the linked source checkout. The host currently offers no remote `pcpos install` command. [Package format and verification](../PCP_OS_Rust/crates/pcpos-pkg/src/lib.rs) · [Install and undo transaction model](../PCP_OS_Rust/crates/pcpos-pkg/src/txn.rs)

The [install-source study](../os-docs/install-source/GROUNDING.md) and [Stage 2 design review](../os-docs/install-source/Stage_2_review.md) record the alternatives considered during package-manager design.

## Modules and assistant plugins

- Inspect method rules with `pcpos modules <method-directory>`. The host accepts local specifications with `pcpos module add`. The [front-page module table](../README.md#method-modules) links each specification at the release revision checked against this host.
- The assistant's plugin manager installs its own integrations. [Claude plugin marketplace manifest](https://github.com/KirtusJ/pcp/blob/master/PCP_1.0_Plugins/.claude-plugin/marketplace.json)
