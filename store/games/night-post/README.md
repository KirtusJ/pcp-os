# Night Post · Return Mail

Fly a kingfisher on three airmail runs. Each route starts open and has three addresses. Fly above an amber net and drop as you approach. Letters keep a little forward motion as they fall. Hit the center six pixels either side of the midpoint for a gold postmark. If you miss, the route loops.

| Route | Addresses | Flying conditions |
| --- | --- | --- |
| Lighthouse Coast | Pine Key, Moth Bank, Gull Rock | Fixed island nets at dusk |
| Lantern Flotilla | Minnow, Paper Moon, Last Ferry | Three bobbing ships with moving nets |
| Aurora Express | Juniper, Alcyone, North Star | A faster northern circuit with airships that change altitude |

## The postbox

Nell has left a welcome card at the sorting office. Each of the nine addresses sends its own illustrated reply when you deliver there. Every stamp already in your album also opens its corresponding reply.

Press **M** at the chart, during a flight, while paused, or after landing to open the postbox. Space turns a card over. Left and Right browse cards you have received. M closes it and returns to exactly where you were. Flight time and falling letters stand still while you read.

On the browser player, **Save postcard** downloads the picture or message currently facing you as a PNG. On the message side, **Read postcard text** opens the same words as selectable browser text.

## Play

Open `dist/Night_Post.html` in a browser, or launch `dist/nightpost.elf` in PCP OS. The offline browser file contains its Rust WebAssembly module, pixel artwork, and synthesized sound. On touchscreens the controls appear below the game.

| Input | Action |
| --- | --- |
| Left / Right or A / D at the chart or ending | Choose a route |
| Space or Enter | Take flight, drop a letter, or replay |
| Arrows or WASD | Steer |
| Hold X or Shift | Airbrake and display a dotted forecast of the drop at the current speed. Release to accelerate. |
| P | Pause or resume |
| Space while paused | Return to the route chart |
| Esc on PCP OS | Return to the OS launcher |

Each delivery adds its destination to the album immediately. Earned gold stays with its stamp. Your browser keeps the album in local storage when available. PCP OS keeps it in `nightpost.stamps` across game launches until the guest powers off. The album tracks stamps independently of a run. Starting a run begins a fresh set of deliveries.

For PCP OS, give QEMU the guest ELF as a Multiboot module at kernel startup:

```sh
qemu-system-x86_64 -cpu max -m 256M -kernel /path/to/pcpos-kernel.elf32 \
  -vga std -initrd dist/nightpost.elf
```

Select `nightpost.elf` and press Enter. The game uses the display, keyboard, and store grants. For sound, add an AC'97 device and an audio backend supported by your QEMU installation. The native target runs on PCP OS kernel 0.3.1.

## Build and test

The gameplay, graphics, type, and sound are written in Rust. The library in `game/` is shared by the PCP OS adapter in `guest/` and the WebAssembly adapter in `web/`. The browser uses JavaScript to present completed frames, send input, and play synthesized samples. There are no third-party game crates.

With a Rust toolchain supporting both targets, Python 3, GNU binutils, and an x86-64 musl C linker:

```sh
rustup target add x86_64-unknown-linux-musl wasm32v1-none
cargo test --locked -p night-post
sh tools/build.sh all
```

Use `web` or `guest` instead of `all` for a single target. `MUSL_CC` can name a different x86-64 musl compiler driver. The guest build uses musl for its static program startup and system calls.
