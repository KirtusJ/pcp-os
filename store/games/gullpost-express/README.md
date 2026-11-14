# Gullpost Express

**Alpha v1.0 · 1.0.0-alpha.1 · Native PCP OS**

Take a gull courier from Saltwharf to Mooncap Light. A full mailbag weighs nothing here, but a badly timed hop will still drop you into the tide. Deliver to all three lighthouses and open the album to see how many of the 39 postage stamps you brought home.

## Play

Start the native Store launcher from the repository root.

```sh
sh store/play-native.sh
```

Use the arrow keys to select **gullpost.elf** and Enter to launch. Press Space or Enter on the cover to set out. The launcher needs the game-capable PCP OS kernel and QEMU. The [Store guide](../../README.md#play-on-pcp-os) covers setup and sound. The [native game executable](dist/gullpost.elf) is ready to play.

| Control | Action |
| --- | --- |
| Left / Right or A / D | Walk. Release to slow down. |
| Space or Enter | Jump. Hold for height, release for a short hop. Press again in midair for one wing flap. |
| P | Pause or resume. |
| R | Retry from the last postbox, or the beginning of the route. |
| Escape | Return to the PCP OS menu. |

A crab can be bounced on from above. Striped springs launch you automatically. The little postbox marks a checkpoint. Collected stamps survive a retry. Deliveries advance even if you leave stamps behind. Your album covers the current run and closes when you leave the game.

| Route | Postmarks | Landmark |
| --- | ---: | --- |
| Saltwharf | 12 | Low jetties above the peach-colored harbor |
| Reedlight Fen | 13 | Springboards between stretches of reed bank |
| Mooncap Light | 14 | A chain of moonlit islands |

## Build or check

The game logic, original pixel artwork, synthesizer and native device adapter are written in Rust. The guest requests the framebuffer, keyboard, sound and console. It uses a fixed-size framebuffer and runs without a heap or runtime libraries. The kernel validates the x86-64 ELF at launch.

From this game directory, with Rust 1.91.1 or compatible and the bare-metal target installed, run the following.

```sh
cargo test --locked --lib
rustup target add x86_64-unknown-none
cargo build --locked --release --target x86_64-unknown-none --bin gullpost
cp target/x86_64-unknown-none/release/gullpost dist/gullpost.elf
```

The tests exercise all three deliveries from controller input, variable jump height, a single air flap, crab collisions, springboards, pause and checkpoint recovery. The native release was also exercised from the PCP OS 0.3.1 launcher in QEMU. On the title screen, **Space** starts the route. **Escape** returns to the menu and the same ELF can be launched a second time.
