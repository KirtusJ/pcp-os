# Surfbird guest HTTP experiment

A bare-metal Rust ELF opens the existing, digest-granted `/dev/game-link` device, sends an HTTP/1.1 GET and displays a page from the response. Keys **1** and **2** follow the links that arrived in the HTML. **B** restores the previous page from guest memory. **Esc** returns to the PCP menu. The framebuffer and COM1 log use the same parsed page.

## Where the connection runs

The guest writes HTTP bytes to COM2. QEMU connects its serial backend to a loopback HTTP fixture running on the host. The fixture only learns which link was chosen when it receives the next request. The replay uses `-nic none` and a fixed HTTP Host of `fixture.test`. QEMU owns the TCP connection. [QEMU character device backends](https://www.qemu.org/docs/master/system/invocation.html#character-device-options)

The guest reuses the kernel's user-mode byte-stream interface. The loader must grant the ELF digest before the process can open that device. The virtio Ethernet probes exercise fixed kernel workloads. An ordinary guest application still needs a process-accessible network interface and TCP/IP implementation to connect to an endpoint it chooses. DNS and TLS would be additional steps for general HTTPS browsing. [Guest device open and I/O](../../../PCP_OS_Rust/crates/pcpos-kernel/src/sys.rs) · [Loader grant](../../../PCP_OS_Rust/crates/pcpos-kernel/src/session.rs) · [Ethernet probes](../../../PCP_OS_Rust/crates/pcpos-kernel/README.md#isolated-ethernet-transmit-probe)

## Document and protocol limits

The guest accepts an HTTP/1.1 200 OK response with one decimal Content-Length and a text/html content type. Header and body together fit in 2 KiB. It rejects transfer and content encodings and enforces an eight-second guest-time deadline. The fixture profile accepts one title, one paragraph and up to two numbered anchors, in order, containing bounded printable ASCII. Links point to absolute paths on the fixed origin. There is one step of cached back history. See the parser for the exact grammar. [HTTP/1.1 message framing](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3) · [Guest parser](src/lib.rs)

The hosted Surfbird reader retains its HTML5 parser and JSON machine API. The native experiment uses keyboard controls. A machine drives those controls through QEMU and observes the guest serial output.

## Replay

Use a dedicated checkout for the kernel build. From the repository root, with the [kernel build prerequisites](../../../PCP_OS_Rust/crates/pcpos-kernel/README.md), Python 3 and QEMU available:

```sh
stage=$(mktemp -d)
PCPOS_KERNEL_STAGE="$stage" PCPOS_KERNEL_FEATURES=game-link sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
cargo build --locked --manifest-path apps/surfbird/Cargo.toml
cd apps/surfbird/guest
rustup target add x86_64-unknown-none
cargo test --locked --lib
cargo build --locked --release --target x86_64-unknown-none
python3 replay.py ../../../PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 \
  target/x86_64-unknown-none/release/surfcable --hosted ../target/debug/surfbird
```

Pass the actual executable paths if you set `CARGO_TARGET_DIR`. The replay emits per-run JSON and saves each arm’s full serial and QEMU error logs. The optional `--hosted` check drives the existing hosted reader’s JSON API against the same page bodies after the guest exits.

## Observed in QEMU

Rust 1.91.1, QEMU 11.1.0 (TCG), measured game-link kernel `9e0cd083e00b836b4209a640826ff7945f026e56c9a4d6f3ef41c5a6779d51a1`, guest ELF `3345866b36ce3a832f3510110aa63b106362821432330a18691d713358aa521f`.

| Arm | Observation |
| --- | --- |
| Correct grant | On the first GET the fixture chose the suffix `dcf80872eb70`. The guest displayed the harbour, followed link 2 to the jetty, restored the harbour with B and followed link 1 to the bar. The fixture recorded exactly three requests. PCP counted four frames and an exit code of zero. |
| Hosted comparison | The independent hosted reader reached the same two suffixed paths, titles and body text after receiving the same ordered links. Back returned the same original page. |
| Wrong ELF digest | The guest reported that the cable grant was unavailable. The fixture received zero HTTP requests. |
| Ambiguous response | The fixture sent both a Content-Length and a Transfer-Encoding. The guest rejected it and the kernel recorded zero frames shown. |

All three guest boots verified their kernel registers at shutdown. The native code runs in the experimental game-link kernel. This test establishes HTTP over a preconnected cable and leaves guest Ethernet/TCP, general HTML rendering and extensions open.
