# PCP OS Store

| What you want to do | Where to start |
| --- | --- |
| Run a Science, Mathematics, Statistics, Engineering or review instrument | [Host instrument catalog and CLI/MCP examples](instruments/README.md) |
| Get a packaged application or verify its files | [Package index](#package-index) |
| Choose a review module or read its specification | [Released method modules](../README.md#method-modules) |
| Play a game | [Offline browser shelf](index.html) or [native PCP OS launcher](#play-both-on-pcp-os) |

## PCP OS game store

Download or clone the repository and open [the game shelf](index.html) in your browser. Each game also fits in a single HTML file. If you are browsing on GitHub, open the player link below, use **Download raw file**, then open that file locally. Both games run offline with WebAssembly enabled.

| Game | First move | Browser player | Native guest |
| --- | --- | --- | --- |
| **Night Post · Return Mail** | Choose a route and press Space to take flight. Steer a kingfisher over the mail net and drop the letter just before you reach it. | [Night_Post.html](games/night-post/dist/Night_Post.html) | [nightpost.elf](games/night-post/dist/nightpost.elf) |
| **Bellweather Harbor · Standing Order Thirteen** | Ring the bell and listen for Nell. Your first trip brings a crate of salt. Visit her on the quay or sell it at the exchange. | [Bellweather_Harbor.html](games/bellweather-harbor/dist/Bellweather_Harbor.html) | [bellweather.elf](games/bellweather-harbor/dist/bellweather.elf) |

The [Night Post flight manual](games/night-post/README.md) covers routes, airbrakes and postmarks. The [Bellweather Harbor guide](games/bellweather-harbor/README.md) covers chartering, trade, manufacturing, and the casebook. Each game directory includes its Rust source, Cargo lockfile and build scripts. Browser saves are local to the browser and file location. Bellweather's logbook can export and import a save when moving between copies.

## Play both on PCP OS

From the repository root on a machine with QEMU and the [kernel build toolchain](../PCP_OS_Rust/crates/pcpos-kernel/README.md), run the launcher. It builds and measures the kernel and opens one menu with both games.

```sh
sh store/play-native.sh
```

Use Up or Down to highlight a game and Enter to launch. Escape returns to the launcher, where you can start the other game in the same session. Native saves last until the guest powers off. To use a previously built kernel, set `PCPOS_KERNEL_IMAGE` to its absolute path. On a QEMU build with SDL audio, append `-audiodev sdl,id=snd -device AC97,audiodev=snd` for sound.

## Package index

The Store uses the repository's `pcpos-pkg` application entry format. Files in an entry use paths from the repository root and can be fetched from the entry's immutable payload commit. Each listing pins the two files players need, the standalone HTML player and its x86-64 native executable. The full source is in the same commit alongside them.

| Application | Listing |
| --- | --- |
| `kirtusj/night-post` | [Package entry](index/kirtusj/ni/gh/night-post) |
| `kirtusj/bellweather-harbor` | [Package entry](index/kirtusj/be/ll/bellweather-harbor) |

For a separately downloaded player, open its package entry above and compare the SHA-256 for the matching `.html` path with the downloaded file. For example, run `Get-FileHash -Algorithm SHA256 ./Night_Post.html` in PowerShell or `sha256sum ./Night_Post.html` on Linux in the download directory.

To check the catalog identities, file lengths and SHA-256 hashes against the checkout, run the verifier from the repository root.

```sh
cargo run --locked --manifest-path PCP_OS_Rust/Cargo.toml -p pcpos-pkg --example verify-game-store
```

The verifier checks the named applications against local files. The entries pin the upstream payload commit, file sizes and SHA-256 digests. To obtain an application, download its player or use the linked source checkout. The host currently offers no remote `pcpos install` command. [Package format and verification](../PCP_OS_Rust/crates/pcpos-pkg/src/lib.rs) · [Install and undo transaction model](../PCP_OS_Rust/crates/pcpos-pkg/src/txn.rs)

## Modules and assistant plugins

- Inspect method rules with `pcpos modules <method-directory>`. The host accepts local specifications with `pcpos module add`. The [front-page module table](../README.md#method-modules) links each specification at the release revision checked against this host.
- The assistant's plugin manager installs its own integrations. [Claude plugin marketplace manifest](https://github.com/KirtusJ/pcp/blob/master/PCP_1.0_Plugins/.claude-plugin/marketplace.json)
