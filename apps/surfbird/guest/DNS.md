# Native Surfbird DNS replay

In QEMU, the same PCP OS guest asked for the IPv4 address of `fixture.test`, opened HTTP to the returned address, and followed links discovered in the first page. The retained runs returned `10.0.2.40` and `10.0.2.163` for the same name. Each capture contains a DNS exchange, a guest TCP handshake to the returned address on port 80, and three GETs with matching response bodies. The Back action displayed the cached home page between the two link visits. [First capture](evidence/dns/allowed.pcap) · [Changed address](evidence/dns/alternate.pcap)

## Fixture and scope

The Rust guest sends its own UDP query through the digest-granted Ethernet device. It uses smoltcp for UDP, DNS wire parsing and TCP. It checks the resolver endpoint, transaction ID, echoed question name, type and Internet class before accepting an answer or an error. Before using an address or a negative result, the guest walks the declared resource records in all three sections and requires their lengths to account for the whole datagram. Its supported positive answer is one directly owned A record. It compares DNS labels case-insensitively and accepts a compressed owner name. The loader supplies the resolver port and separate random values for the query ID and guest UDP source port. The HTTP address reaches the guest through the DNS reply. [Guest resolver](src/dns.rs) · [Response matching and Rust tests](src/dns_answer.rs) · [DNS message format](https://www.rfc-editor.org/rfc/rfc1035.html#section-4) · [DNS response matching](https://www.rfc-editor.org/rfc/rfc5452.html#section-9.1)

The Python fixture binds DNS and HTTP on host loopback. QEMU's user network exposes the resolver through `10.0.2.2`; its `guestfwd` rule maps the chosen guest-visible HTTP address to the local web server. Every HTTP request carries the original hostname. No public resolver or hosts-file change is involved. The native build supports a single HTTP origin over IPv4. It reports truncation as an error. A general-purpose resolver also needs DNS over TCP, including for truncated replies. [Reserved test names](https://www.rfc-editor.org/rfc/rfc6761.html#section-6.2) · [QEMU network options](https://www.qemu.org/docs/master/system/invocation.html#network-options) · [DNS over TCP](https://www.rfc-editor.org/rfc/rfc7766.html#section-5)

## QEMU 11.1 results

| Fixture behavior | Guest and packet result |
| --- | --- |
| Answer at first address | One query, three GETs, four page presentations, 23 Ethernet frames. |
| Same name at a different address | One query, three GETs to the second address, four page presentations, 25 frames. |
| Drop the first query | Two queries. The next reply supplied `10.0.2.41` and both links opened. |
| NXDOMAIN | The guest reported that the name did not exist. Zero TCP packets or page presentations. |
| Truncated reply carrying an A record | The guest reported truncation. Zero TCP packets or page presentations. |
| Mismatched replies only | Across three queries the fixture sent twelve replies with a wrong ID, question, answer owner or source port. The guest timed out. Zero TCP packets or page presentations. |
| Invalid record sections | Across three queries the fixture sent nine replies carrying a usable-looking A record with a missing authority record, missing additional record or an undeclared trailing byte. The guest timed out. Zero TCP packets or page presentations. |

All seven boots in the retained suite passed the kernel register check. Earlier boots intermittently entered SAFE at the existing keyboard-wakeup self-test before the guest launched. On this Windows setup QEMU also emitted a socket-event warning in some replay logs. The captures, normal shutdown and register results are checked separately.

## Reproduce

Build the `guest-ethernet` kernel and hosted comparison binary using the [TCP build instructions](TCP.md#replay). From `apps/surfbird/guest`, run:

```sh
cargo test --locked --lib
cargo build --locked --release --target x86_64-unknown-none --features dns
python3 replay.py ../../../PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 \
  target/x86_64-unknown-none/release/surfcable --dns --hosted ../target/debug/surfbird
```

The runner chooses a free UDP port by default. The published suite used `--dns-port 53` on a machine where loopback port 53 was free. Each arm writes its pcap, fixture record, kernel log and QEMU error log to a fresh directory. To verify the published files offline:

```sh
python3 dns_trace.py evidence/dns/alternate.pcap evidence/dns/alternate.json
python3 -m unittest test_dns_trace test_packet_trace
```

The offline tests replay all seven captures. They also move a TCP SYN ahead of DNS, move the retry behind the successful answer, and insert an extra query after a completed lookup. The verifier rejects all three altered traces. [Captures and fixture records](evidence/dns/) · [Verifier](dns_trace.py)
