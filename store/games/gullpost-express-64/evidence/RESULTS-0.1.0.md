# Lighthouse Island in QEMU

The measured release guest is `gull64.elf`, 82,568 bytes, SHA-256 `0916ea3b5f5dff058907c8f2683d0d1132c81ce51cf4f1539c5286330158987e`. Measurements used QEMU 11.1.0 (v11.1.0-12130-ge470268ff4), software emulation (TCG), the `max` CPU, 256 MiB RAM, std VGA, AC'97 and the included PCP OS 0.3.1 kernel on this Windows PC. The initial ZIP contains the animation captured during this release.

## Three complete guided deliveries

The courier crossed the gap, collected the pier, rooftop and terrace stamps, and reached the red postbox in each run. The guided controller drives ordinary movement and jump inputs. QEMU keyboard events started and restarted each run. Timing starts at the first active gameplay frame and ends when the letter is delivered. The kernel counts 39.9 frames per second across the full guest invocation including the menus.

| Run | Gameplay frames | Gameplay seconds | Mean draw and present | 95th percentile¹ | Maximum | Frames above 25 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 312 | 7.801 | 11.06 ms | 18–19 ms | 26.00 ms | 1 |
| 2 | 312 | 7.801 | 7.51 ms | 16–17 ms | 21.00 ms | 0 |
| 3 | 312 | 7.800 | 7.82 ms | 15–16 ms | 22.00 ms | 0 |

The three runs averaged 8.79 ms per frame for drawing, framebuffer copy and page flip at 512×288. One of 936 gameplay frames exceeded the 25 ms budget. The fixed-step simulation dropped zero ticks. Sleep, input, sound generation and game logic sit outside the draw timer. The frame cadence is deliberately capped at 40 FPS. Headless QEMU measures guest output and excludes desktop window composition and monitor latency.

¹ PCP OS's monotonic clock advances in approximately one-millisecond increments. The guest groups render times into whole-millisecond bins. Per-run averages retain the sum before binning.

## Input, startup and exit checks

The free-play check sent keyboard events through QEMU for walking off the pier, respawning, jumping, air-flapping, orbiting the camera, pausing, resuming, retrying and exiting. Both the free-play guest and the relaunched guided guest exited with status 0. The kernel reported 59,887 free frames before and after both invocations. Six Rust gameplay tests passed, including checkpoint and collectible retention, landing and flap behavior, and successful completion of the same island.

An earlier boot of this release ELF stopped in kernel SAFE before the guest launched. The kernel's keyboard-wakeup self-test reported 19 idle periods out of 22 and marked the result as a failure. The next boot with the same kernel, ELF and QEMU settings passed and produced the timings above. The cause of that boot variation remains unverified. [Prelaunch serial log](prelaunch-safe.txt) · [Measured session log](serial.txt)
