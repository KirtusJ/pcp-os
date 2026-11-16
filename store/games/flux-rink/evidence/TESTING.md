# Flux Rink native acceptance

The first shared-screen arena occupied a narrow strip of a 384×224 display. The North Dock revision turns the court sideways, uses 640×360, and puts the entire width of the ice between the goals. Each mallet and the puck are indexed Heliograph meshes. Arena structure, lamps, boards and the ice lighting are cached. This allows surface detail to be increased without repainting the building every frame. The production image is a QEMU framebuffer capture; the earlier image is retained for comparison. [First rink](first-rink.png) · [North Dock](north-dock.png) · [Linked peer view](linked-peer.png) · [Settings](settings.png)

## Frame timing

On the Windows i5-10210U test machine, QEMU 11.1.0 used a single emulated x86-64 CPU, TCG, standard VGA and headless display. A monotonic host clock timestamped a two-byte marker emitted immediately after each page flip. Two 120-interval windows were collected with the same video detail, standard camera and FPS overlay. Direction inputs were sent during both windows. The kernel and emulator clocks need not progress at wall-clock speed. The displayed counter measures guest time. The figures below measure host time.

| Frame limit | Mean presented frames/s | Median frame interval | 95th percentile |
| --- | ---: | ---: | ---: |
| 60 guest FPS | 39.8 | 25.15 ms | 28.32 ms |
| Uncapped | 320.0 | 2.80 ms | 4.63 ms |

These short runs measure page production in local play. They do not establish physical refresh rate, network latency, or sustained performance across an entire match. Earlier runs with separate settings and host loads are not pooled into these numbers. [60-cap timings](moving-60-cap.csv) · [Uncapped timings](moving-uncapped.csv)

## Gameplay, settings and transport

- Two keyboard layouts independently moved the mallets on the stock kernel. The guest saved changed options, exited, relaunched in the same PCP OS boot, and restored them. Rust tests cover first-to-five, bounds, scoring, and malformed settings.
- Two separate QEMU guests exchanged 16-byte frames on the opt-in COM2 link. The host registered input from the peer and moved Jade. The peer observed the new authoritative position. Both guests separately returned to PCP OS and shut down successfully. A negative arm attached a second UART and supplied a deliberately different payload digest. The game could not open the device. Rust checks cover damaged frames, fragmented delivery, wrapping sequences and rejected out-of-range state.
- Early 24-byte snapshots intermittently lost their tails when one emulated guest ran behind the other. The accepted snapshot fits the UART's 16-byte receive FIFO. The public method for checking malformed or stale updates is exercised by the wire tests.
- The kernel's existing keyboard interrupt timing self-test occasionally put a cold VM into SAFE before any game began. Successful runs used fresh boots with the full verification gate intact. The game uses a private local cable. Lossy WAN behavior, peer authentication and Internet access have not been tested.
- The arena remains stylized. The rails and goal nets can show single-pixel stair steps. Specular highlights come from fixed material captures; they do not reflect moving players. A human player preference study has not been conducted.
