# Native renderer tests

The launcher booted the packaged PCP OS 0.3.1 kernel under QEMU 11.1.0, single virtual CPU, TCG, standard VGA with 32 MiB of video RAM, and 256 MiB guest RAM. The host was a Windows machine with an Intel Core i5-10210U. QEMU's monitor captured the displayed framebuffer. Each successful suite returned through the OS menu and powered down with the kernel verification passing.

## How to read the timings

Host times use the monotonic-clock arrival of a two-byte serial marker immediately after each guest page flip. These are headless frame-production intervals. They include emulator scheduling and marker transport. Physical display refresh, input latency and gameplay simulation are outside the measurement. The two warm-up frames are present in the raw data. Median and nearest-rank p95 exclude them. FPS is the reciprocal of the **mean** interval. The raw samples retain every measured frame and its host timestamp. The shorter guest times use the kernel monotonic clock and include drawing, framebuffer transfer and page flip. Guest timing alone substantially overstated throughput in some runs.

## Display limit

One 49,152-triangle mesh rotates through the same series of poses at each resolution. The camera's vertical field of view stays fixed. The resolution probe clears the buffers, draws the mesh, copies to the back page, then flips. QEMU produced a nonblank screenshot at every accepted dimension. Each line below uses ten timed intervals. This is a short characterization of this machine.

| Native framebuffer | Median host interval | Host p95 | Mean throughput | Median guest draw + present |
| --- | ---: | ---: | ---: | ---: |
| 640×360 | 30.0 ms | 37.4 ms | 32.3 fps | 21.5 ms |
| 1280×720 | 60.9 ms | 65.6 ms | 16.3 fps | 42.0 ms |
| 1920×1080 | 156.9 ms | 166.8 ms | 6.3 fps | 108.5 ms |
| 2560×1440 | 249.3 ms | 258.4 ms | 4.0 fps | 172.0 ms |
| 2560×1600 | 440.4 ms | 596.2 ms | 2.1 fps | 307.0 ms |
| 3840×2160 | Driver rejected mode | | | |

The Bochs backend accepts widths 320–2560 in multiples of eight, heights 200–1600, and only modes that fit two 32-bit pages. Default QEMU standard VGA on this host reports 16 MiB. Double-buffered 1920×1080 fits within 16 MiB. The measured maximum needs 31.25 MiB. The launcher requests 32 MiB. [Driver limits](https://github.com/KirtusJ/pcp-os/blob/21ef0955eb8828739c1b95bf85f3d880cdaf4eb6/PCP_OS_Rust/crates/pcpos-kernel/src/dev.rs#L187-L213) · [Full-size QEMU capture](2560x1600.png) · [Resolution samples](resolution.tsv)

## Geometry throughput at 640×360

Each mode uses 40 measured frames at matching angular steps. The gallery holds 24 meshes. The meshes have the same topology at three tessellation densities. The counters below distinguish triangles accepted for processing and triangles covering at least one sampled pixel. Depth testing may subsequently hide them.

| Scene | Submitted triangles / frame | Mean triangles hitting a pixel | Median host interval | Host p95 | Mean throughput |
| --- | ---: | ---: | ---: | ---: | ---: |
| One medium mesh | 6,144 | 2,850 | 40.7 ms | 92.7 ms | 20.9 fps |
| One dense mesh | 49,152 | 20,031 | 34.9 ms | 45.2 ms | 29.4 fps |
| Gallery, low | 27,648 | 12,883 | 65.7 ms | 134.1 ms | 13.2 fps |
| Gallery, medium | 147,456 | 55,915 | 66.7 ms | 105.7 ms | 12.9 fps |
| Gallery, adaptive | 62,592–67,584 | 27,542 | 48.8 ms | 59.0 ms | 20.4 fps |
| Gallery, full detail | 1,179,648 | 123,422 | 349.7 ms | 647.0 ms | 2.6 fps |

The adaptive and full galleries share poses and 1,179,648 source triangles. Across six host-side reference images using the same render code, adaptive detail disagreed with the full-detail silhouette on 9,129 of 553,946 pixels in their union (1.65%). Mean absolute RGB channel error over that union was 8.02 on a scale of 255. The fixed phase order and host scheduling affected results. In particular, the single medium mesh had slower measured cadence than the dense mesh in this short suite. These observations do not establish a general mesh throughput curve. [Gallery screenshot](gallery.png) · [Geometry samples](geometry.tsv)

## Correctness and remaining defects

Six Rust tests passed. They check the baked assets and an out-of-range index, exact shared-edge coverage, depth ordering when draw order reverses, five near-plane cases, whole-instance rejection before vertex work, and the six-pose adaptive image comparison. Gold and jade materials were captured in the native guest. Live hold, material switch, scene selection, automatic benchmarking and exit were exercised over emulated keyboard input.

Hard specular edges show quantization and silhouette shimmer as the mesh rotates. The captures use an 8-bit packed normal per component, a nearest-sampled 128×128 material lookup and one coverage sample per pixel. The studio contact shadow is approximate. A full game adds CPU work beyond these timings. Dynamic lighting, animated skinned meshes and texture-filtered game assets still need separate benchmarks.

Four exploratory cold boots stopped before launching the renderer when the kernel's interrupt-driven keyboard self-test failed. Subsequent clean boots passed. A sample of the failed boot is included. Early engineering runs also showed large differences between the guest clock and independently observed wall time. That prompted the short-marker measurement used for these tables. [Example failed boot](safe-boot.txt) · [Successful native resolution sweep](resolution-serial.txt) · [Successful native geometry sweep](geometry-serial.txt)
