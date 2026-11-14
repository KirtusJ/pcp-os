# Guest TCP/IP over virtio Ethernet

Surfbird opened a connection from its user-mode ELF inside PCP OS and used it for three HTTP requests. QEMU provided an emulated Ethernet card and translated the guest traffic to a loopback HTTP server. The first page supplied the two destinations at run time. The guest followed link 2, restored the home page from memory and followed link 1 on the same connection. Only COM1 was attached for the kernel log.

## Connection and authority

The optional `guest-ethernet` kernel grants raw frame access to a single ELF selected by the exact SHA-256 in `net_image=`. The menu displays this grant. Missing, duplicate or mismatched pins prevent admission to `/dev/ethernet`. A granted ELF can transmit arbitrary Ethernet frames on the selected NIC. Run the experiment on the QEMU user network.

The driver owns the legacy virtio queue descriptors and DMA pages, copies complete frames across the process boundary and checks the returned descriptor head, length and producer index before reading a receive buffer. One chain per direction is outstanding at a time. It resets the device before releasing its DMA storage on process exit. [Virtio driver](../../../PCP_OS_Rust/crates/pcpos-kernel/src/dev/virtio_tx/guest.rs) · [Virtio specification](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

The guest ELF uses smoltcp for ARP, IPv4, retransmission and TCP sequencing. Its address is `10.0.2.15/24`. It actively opens a socket to `10.0.2.2` using the port supplied by the loader in the first two bytes of `net.cfg` (big-endian). QEMU’s user network translates that destination to the host, where the fixture listens on IPv4 loopback. The fixture tests a fixed-address plain HTTP origin. DNS, TLS and general WebExtension support remain outside this guest. [Guest TCP implementation](src/tcp.rs) · [smoltcp TCP socket](https://docs.rs/smoltcp/0.14.0/smoltcp/socket/tcp/struct.Socket.html) · [QEMU user networking](https://www.qemu.org/docs/master/system/devices/net.html#using-the-user-mode-network-stack)

For each boot, the fixture supplies eight fresh OS-random bytes to seed smoltcp and two independent bytes for ephemeral-port selection. The guest incorporates its monotonic clock and performs the active open itself. The packet test checks the sequence and acknowledgment numbers for the three-way handshake. Internet-facing operation would also require review of sequence-number generation, entropy renewal and hostile-peer behavior. [TCP connection establishment](https://www.rfc-editor.org/rfc/rfc9293.html#section-3.5) · [TCP initial sequence numbers](https://www.rfc-editor.org/rfc/rfc9293.html#section-3.4.1)

## Replay

Use a dedicated kernel build directory and the [kernel prerequisites](../../../PCP_OS_Rust/crates/pcpos-kernel/README.md). From the repository root:

```sh
stage=$(mktemp -d)
PCPOS_KERNEL_STAGE="$stage" PCPOS_KERNEL_FEATURES=guest-ethernet sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
cargo build --locked --manifest-path apps/surfbird/Cargo.toml
cd apps/surfbird/guest
rustup target add x86_64-unknown-none
cargo test --locked --lib
cargo build --locked --release --target x86_64-unknown-none --features tcp-ip
python3 replay.py ../../../PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 \
  target/x86_64-unknown-none/release/surfcable --tcp --hosted ../target/debug/surfbird
```

The runner saves each boot log and pcap in its own directory. Substitute actual binary paths when using an external Cargo target directory. Add `--arm allowed --extra-visits 130` to exercise queue reuse on a longer sequence. A failed kernel self-test stops a run before the guest starts.

## Packet evidence

To recheck the retained capture offline, run the following from this directory. The fixture record supplies the expected paths, bodies, port and capture digest. As a negative control, flipping one bit in the saved SYN-ACK acknowledgment caused the handshake check to fail.

```sh
python3 packet_trace.py evidence/guest-http.pcap evidence/guest-http.json
```

In the retained 33-frame capture the socket was `10.0.2.15:50543 → 10.0.2.2:64425`. The verifier checked the ARP exchange, the initial guest SYN, the peer’s SYN-ACK and the guest’s matching ACK. It reassembled three requests and 413 response bytes, matched the complete bodies against the fixture, and observed a guest FIN. The request targets were `/harbour`, `/jetty-9cf8e0e895f8` and `/bar-9cf8e0e895f8`. The hosted reader’s independent JSON client reproduced the paths and document contents. All six TCP replay arms verified their kernel registers at shutdown. [Open the Ethernet capture](evidence/guest-http.pcap) · [Packet verifier](packet_trace.py)

| Replay arm | Result on QEMU 11.1.0, Rust 1.91.1 |
| --- | --- |
| Correct ELF grant | Three GETs, four page presentations including Back, guest exit 0. Kernel free frames returned to their launch count. |
| Mismatched digest | Zero Ethernet frames and zero HTTP requests. |
| Conflicting framing headers | A response with both Content-Length and Transfer-Encoding was rejected before display. |
| Space before the field colon | The malformed field was rejected before display. |
| Folded field line | The malformed field was rejected before display. |
| Bound port without a listener | This host’s QEMU backend returned no handshake reply. The guest retransmitted its SYN and hit its eight-second connect deadline after four SYN attempts. Zero HTTP payload bytes. A backend that reports refusal with RST is checked against the guest’s refusal result. |

The captured kernel measurement was `0232c02a8946d66216da0ac65953a246ce3d41d7a966ae31aa83c3335b3259e3`, the guest ELF SHA-256 was `49107154cd8d6a9d399d504ca2f94710a07c147b67431d92b904258ad4156b32`, and the pcap SHA-256 was `b0e30fda0ab5df8f10fdda80bb5ad59ed40f9742e5ada596135478d70ca33f60`.

The longer run issued 133 guest GETs on one connection and displayed 264 pages, including cached Back navigation. Its 1,078 Ethernet frames included 537 from the guest and 541 from the peer. All 14,973 HTTP response bytes matched the fixture. This exercises repeated reuse of both virtqueues. A separate run used 1,077 frames for the same navigation, so the replay checks the streams and page results without prescribing a packet count.

During separate serial-cable regression boots, the pre-existing kernel keyboard-wakeup self-test entered SAFE before Surfbird launched. The logs were retained. The five cable arms subsequently passed across individual boots.

During cross-review, all six arms passed on the corrected kernel. The raw device accepts `O_RDWR`. Rebuilding the guest with `--features probe-ethernet-modes` and running `--tcp --arm allowed` verifies that read-only, write-only and reserved mode 3 each return `EINVAL` before completing navigation. The denial arm requires an empty capture. Run `python3 -m unittest test_packet_trace` for the spoofed-source-MAC negative control.
