# Surfbird guest HTTP experiment

A bare-metal Rust ELF sends HTTP/1.1 requests and displays the pages it receives. The Ethernet guest can [resolve a hostname over DNS](DNS.md) or [connect to a fixed address](TCP.md). Both run TCP inside the guest with COM2 disconnected. The cable build below uses a preconnected serial stream. Both use the same page parser, keyboard controls and two-slot history. Keys **1** and **2** follow links, **B** restores the previous page from guest memory and **Esc** returns to the PCP menu.

An optional [local video and stereo audio replay](LOCAL_MEDIA.md) exercises the native framebuffer and sound device with supplied media files.

## Serial cable

The guest writes HTTP bytes to COM2. QEMU connects its serial backend to a loopback HTTP fixture running on the host. The fixture only learns which link was chosen when it receives the next request. The replay uses `-nic none` and a fixed HTTP Host of `fixture.test`. QEMU owns the TCP connection. [QEMU character device backends](https://www.qemu.org/docs/master/system/invocation.html#character-device-options)

The loader must grant the ELF digest before the process can open the serial device. The Ethernet build uses a separate raw-frame device and runs its own TCP state machine. The DNS build resolves a direct IPv4 address for the local HTTP fixture. Native HTTPS still requires TLS. [Guest device open and I/O](../../../PCP_OS_Rust/crates/pcpos-kernel/src/sys.rs) · [Loader grant](../../../PCP_OS_Rust/crates/pcpos-kernel/src/session.rs)

## Document and protocol limits

The guest accepts an HTTP/1.1 200 OK response with one decimal Content-Length and a text/html content type. Header and body together fit in 2 KiB. It rejects transfer and content encodings and enforces an eight-second guest-time deadline. The parser checks every field name against the HTTP token grammar and rejects folded lines. [HTTP field syntax](https://www.rfc-editor.org/rfc/rfc9112.html#section-5) · [Token grammar](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.2)

The fixture profile accepts one title, one paragraph and up to two numbered anchors, in order, containing bounded printable ASCII. Links point to absolute paths on the fixed origin. There is one step of cached back history. See the parser for the exact grammar. [HTTP/1.1 message framing](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3) · [Guest parser](src/lib.rs)

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

Rust 1.91.1, QEMU 11.1.0 (TCG), measured game-link kernel `9e0cd083e00b836b4209a640826ff7945f026e56c9a4d6f3ef41c5a6779d51a1`, guest ELF `85ac5e56ae7996492c47f7f806c8344a15e63170ca83fdd48cdb2fdf0b56dc8c`.

| Arm | Observation |
| --- | --- |
| Correct grant | On the first GET the fixture chose the suffix `d915183ea51e`. The guest displayed the harbour, followed link 2 to the jetty, restored the harbour with B and followed link 1 to the bar. The fixture recorded exactly three requests. PCP counted four frames and an exit code of zero. |
| Hosted comparison | The independent hosted reader reached the same two suffixed paths, titles and body text after receiving the same ordered links. Back returned the same original page. |
| Wrong ELF digest | The guest reported that the cable grant was unavailable. The fixture received zero HTTP requests. |
| Ambiguous response | The fixture sent both a Content-Length and a Transfer-Encoding. The guest rejected it and the kernel recorded zero frames shown. |
| Malformed field name | A space before the colon in "Transfer-Encoding : chunked" triggered a syntax error. The guest displayed zero frames. |
| Folded field | A tab-indented Transfer-Encoding line after an extension field triggered the same syntax error. The guest displayed zero frames. |


All five guest boots verified their kernel registers at shutdown. This historical replay covers the experimental game-link kernel and HTTP over its preconnected cable. The [Ethernet replay](TCP.md) tests the separate guest TCP/IP build.
