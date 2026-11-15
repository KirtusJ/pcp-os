# Native HTTPS watch-page probe

The `https-probe` ELF requests one public YouTube watch page. It resolves `www.youtube.com` through QEMU's DNS service, routes over the QEMU IPv4 gateway, verifies a TLS 1.3 server handshake, sends a GET and recognizes the status, HTML content type and first element of the response. This gives the native browser an authenticated connection and a document prefix. The guest still needs a compatible HTML, JavaScript and media runtime to play the video. The probe requests no media or player scripts. [Requested watch page](https://www.youtube.com/watch?v=M7lc1UVf-VE) · [YouTube IFrame API](https://developers.google.com/youtube/iframe_api_reference)

## What the guest verifies

The kernel grants raw Ethernet only to the selected ELF digest. The ELF holds the TCP/IP, DNS, TLS and HTTP state. smoltcp asks QEMU's resolver at 10.0.2.3 for the actual IPv4 destination and sends nonlocal packets through 10.0.2.2. QEMU provides unprivileged address translation. The host supplies a local port-randomization seed and starts the emulator. It does not supply a resolved address, peer certificate, TLS plaintext or HTTP response. [QEMU user-mode networking](https://www.qemu.org/docs/master/system/devices/net.html#using-the-user-mode-network-stack)

Rustls uses WebPKI's ordinary server-auth verification against the roots in the locked `webpki-roots` crate and the requested DNS name. TLS randomness comes from getrandom's checked RDRAND backend on the virtual Intel CPU. Certificate time comes from the kernel's RTC-backed wall clock. The emulator, its clock and entropy source, boot measurement, and the selected trust-store distribution are trusted inputs. DNS poisoning alone would still require an acceptable certificate and proof of private-key possession. The probe does not require certificate transparency or revocation checks. The RustCrypto provider is a prerelease. This is an isolated compatibility experiment. [Rustls](https://docs.rs/rustls/) · [Root distribution](https://docs.rs/webpki-roots/) · [Entropy backend](https://docs.rs/getrandom/) · [RustCrypto provider](https://github.com/RustCrypto/rustls-rustcrypto)

The program uses a bounded 3 MiB allocation arena for one connection. The kernel reclaims it on exit. It collects up to 32 KiB for response headers and the first bytes of the body, recognizes an unencoded HTML response with a literal HTML doctype and then closes the connection. It handles a first HTTP chunk large enough to hold that prefix. It does not follow redirects, parse arbitrary HTML, or download the rest of the document.

## Reproduce

Build the [guest Ethernet kernel](TCP.md) and the Surfbird guest on a toolchain supporting `x86_64-unknown-none`. Run these commands from this directory. The harness uses public network access on an isolated QEMU NIC and needs Python 3. Each run creates a new directory.

```sh
cargo build --locked --release --target x86_64-unknown-none --features https-probe
guest=target/x86_64-unknown-none/release/surfcable
python3 replay_https.py "$kernel" "$guest" run-watch
python3 replay_https.py "$kernel" "$guest" run-name --arm wrong-host
python3 replay_https.py "$kernel" "$guest" run-issuer --arm untrusted
```

The harness saves the encrypted packet capture, guest serial, image identity and measurements. It checks that a guest DNS query and TCP connection to a public endpoint on port 443 were present. The observed endpoints can change. The readable status and first bytes come from the verified guest endpoint. Packet contents and sizes alone do not identify the returned resource.

| Arm | Guest observation |
| --- | --- |
| Ordinary roots and reference name | `HTTP/1.1 200 OK`, `text/html`, `<!DOCTYPE html>` from the requested watch URL. |
| Wrong reference name | The original server name is still sent as SNI. After the normal handshake the same stock WebPKI verifier checks the live chain against `wrong-host.invalid`, before the GET. It rejects the name. |
| Empty trust store | Same SNI and requested name. The handshake ends with unknown issuer before application data is sent. |

The initial 8 KiB header cap was too small for the observed page. Initial experiments with SNI removed returned an untrusted default certificate, which could not isolate the hostname comparison. Some emulator boots stopped in the kernel's existing keyboard self-test before the guest ran. The harness preserves each failed run and requires actual guest/network evidence for a successful receipt.
