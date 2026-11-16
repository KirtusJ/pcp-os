# Network, firewall, GUI, Ethernet and wireless audit

**Director 13. Date: 16 November 2026, B.C. Pacific Time (PCT, UTC−07:00).**

**Checked:** exact master 59f26ebe6397e00e8a9b8cce08487ad8a5319c0c, 735-entry recursive Git tree, named files and Git blobs below. This is an engineering source and bounded-test audit, not independent accreditation, an ACR participant calibration score, an installed-hardware test or authorization to ship.

## Source authority and distinct surfaces

1. **Admission.** PCP_OS_Rust/crates/pcpos-kernel/src/session/ethernet.rs (blob 72d8bd158121c57e1106b4e87e09daee5aae1362) authorizes raw Ethernet only if the exact boot command has exactly one image digest matching the loaded ELF, with explicit grant=none denial. Its host Rust tests cover pin and command truncation.
2. **Device.** dev/virtio_tx.rs (blob a108eaaf48d2ea3a34dda5307df818fd93bc370e) and dev/virtio_tx/guest.rs (blob 7fb167007226450c71030c6e35c2851fe33284d9) implement one QEMU legacy virtio-net function, checked rings, bounded raw frames, reset/reclaim and session-scoped I/O. The checked device MAC and requirement of exactly one matching NIC are fixture constraints, not physical multi-NIC support.
3. **Kernel process boundary.** sys.rs (blob 98cf103c9d824966150f9249bd86e1898ffa86d9) maps /dev/ethernet under a process capability with O_RDWR and forwards raw send/receive; user.rs (blob eb231fb37e45051ce5622d6e91a4096a44244fef) installs the capability. This is not a shared socket/filter/connection management service.
4. **Launcher/GUI.** session.rs (blob c516712ef12e113bb6c077d4c62eafce6f7321f7) displays program identity, file digest and any image grant. It has no Link status, Wi-Fi scan, SSID/authentication, IP/DNS, firewall policy, Connect, Disconnect or Reconnect controls. The grant string is not a measurement of network availability.
5. **Application networking.** apps/surfbird/guest/src/tcp.rs (blob abc69df784289f259193895b76a94d0a724f16f2) initializes a smoltcp IPv4 interface at the static fixture address 10.0.2.15/24 and gateway 10.0.2.2, then creates one TCP connection in the app, with its own timeout/close. The current app can exercise fixture DNS and HTTP, and separate opt-in HTTPS probes, but does not implement an OS-wide DHCP/NIC manager.
6. **Configuration/feature gates.** kernel Cargo.toml (blob 891279afb1ef6a5474453165c7975bf112b069e2) makes guest-ethernet opt-in. Without that feature, no Ethernet session interface exists. This is distinct from historical kernel-owned test-frame probes, hosted application sockets and a serial cable.
7. **Wireless.** The full current 735-entry Git tree has no Wi-Fi/802.11 network driver, radio management service, scan/association/supplicant or Wi-Fi settings application. This is an absence observation for this exact tree, not a statement about unexamined external hardware, future branches or the host OS.

All source links below use pinned commit, not an unpinned “latest”:
https://github.com/KirtusJ/pcp-os/tree/59f26ebe6397e00e8a9b8cce08487ad8a5319c0c

## Existing firewall work is not silently counted as master

- [Open issue #118](https://github.com/KirtusJ/pcp-os/issues/118) accurately distinguishes a digest grant from a firewall. It records an earlier two-boot private QEMU negative control where a granted raw app sent an unexpected UDP destination and received a different unsolicited frame; mismatched ELF digest denied all traffic. Those private packets were not retrieved by this Director 13 source audit.
- [Open PR #122](https://github.com/KirtusJ/pcp-os/pull/122) proposes one opt-in IPv4/UDP endpoint with a single matching reverse tuple and a five-second window. It is **unmerged, not mergeable at the observed base, and absent from current master**. Its own description excludes TCP, IPv6, ARP, VLAN, fragments, mutable policy, broad state tracking, hosted sockets and kernel-owned management traffic. This cannot honestly be described as a general native firewall, even after eventual merge without more features and tests.

## Current disposition, by daily-driver capability

| Domain | Verified source scope | Unmet daily-driver requirement |
| --- | --- | --- |
| Firewall | ELF identity grant (current); separate unmerged single UDP filter | Kernel-level inbound/outbound/forward policy, fail-closed default, protocol guards, legitimate return traffic, peer/destination restrictions, state exhaustion, revocation, counters and local operator recovery across every applicable path |
| Ethernet | Opt-in, raw QEMU legacy virtio-net with bounded driver reset | Runtime NIC enumeration/hotplug, link carrier and MAC reporting beyond fixture, physical hardware, PHY state, repeated detach/reconnect, stable device naming |
| IP/DNS/TCP | One guest app owns IPv4/ARP/TCP and DNS; static 10.0.2.x test network | DHCP INIT/BOUND/RENEW/REBIND/NAK, address conflict detection, IPv6 if selected, per-interface routes/DNS, stale connection invalidation, independent connectivity status |
| Wireless | No wireless driver/manager in pinned current tree | Radio and firmware qualification, scan/associate/authenticate/roam/disassociate, WPA2/WPA3 profile handling, secure credential storage, regulatory domain, rfkill and reconnection |
| GUI | Kernel displays identity and raw-frame grant in launcher | Human-readable network inventory, genuine observed Connected/Disconnected/Configuring/Failed/Limited state, Connect/Disconnect/Reconnect/Forget, Wi-Fi security, firewall state and recovery actions |
| Both interfaces | No shared network service with separate Ethernet/Wi-Fi devices | Explicit interface selection and failover, route/DNS priority, no credentials or packets leaked on rejected interface, policy consistency across switches |
| Repeated lifecycle | Surfbird constructs one app-scoped TCP connection; NIC driver is initialized per process session | Multiple cycles under loss, failure, reconnect, suspend/resume and reboot with real policy/IP/route state, bounded timeouts, non-stale GUI and safe stop |

**Special link nuance.** The checked virtio driver negotiates MAC but not VIRTIO_NET_F_STATUS and therefore does not observe STATUS/LINK_UP; OASIS Virtio allows a driver that did not negotiate STATUS to assume link active. This is not by itself a virtio protocol violation, but it is insufficient evidence for a daily-driver truthful link indication or recovery controller.

## General daily-driver GUI audit, separate from the network GUI

The current native kernel launcher is a keyboard-selectable, nonwindowed display with a fixed 10×20 pixel character grid and a foreground user-program handoff. This is not equivalent to a general desktop compositor, a window manager, simultaneous desktop applications, or a mouse-driven network settings service. One program can own the screen; the kernel's Ctrl+Alt+Delete mechanism can stop it and return to the launcher. The source has visible selection styling and readable headings in its normal fixed-input case, but these do not establish accessibility or scalable layout under every installed program count.

**Deterministic source-bound layout risk, not a guest screenshot:** The [GUI geometry discriminator](gui_geometry.py) models only the explicit row increments and footer draw in session.rs, checked against the exact 10×20 ui.rs/font.rs constants. With one digest-granted Ethernet program and one loader file per program, an 800×600 launcher has 30 text rows, a footer occupying rows **28–29**, and its displayed store line lands at **row 29** with **four** programs; it is overpainted by the footer. At 1024×768, 38 rows have footer **36–37**, and the analogous display line lands at **row 37** with **seven** programs. These corrected row indices include the PROGRAMS title's row increment (source lines 192–194), omitted in the first version of the audit model. Both are testable counterexample inputs, not claims that these counts were present in an actual current boot. A later QEMU guest must load and navigate the corresponding program count, capture pixels and test every focusable row. Any source change invalidates the modeled claim until rechecked.

For an interface intended for daily driving, test multi-app launcher scroll/paging, clearly visible keyboard focus, focus never hidden by footer/overlaid panels, predictable Escape/Back/Stop, resizable or display-scaled text, error and long-field overflow, keyboard-only access to all network operations and distinguishable disabled/pending/failed states. Compare with **W3C WCAG 2.2** keyboard/focus guidance and **W3C WCAG2ICT** application notes for non-web software (reference practice, *no conformance claim*). Sources: https://www.w3.org/TR/WCAG22/ and https://www.w3.org/TR/wcag2ict-22/. The register-view 800/1024 event-outcome clipping and optical source fixes are separately owned by Directors 2/10 on #233/#235; this audit does not alter them or silently accept their independent promotion gates.

## Separation of concerns required before implementation

1. **NIC/PHY and WLAN driver:** signed or pinned firmware/device identities, IRQ/hotplug and carrier/association events; no app or GUI deciding device truth.
2. **Network configuration:** per-interface DHCP, IPv4 conflict detection, optional IPv6 ND and address lifetimes, routes and DNS, bounded retry/backoff, current selected profile and credential provider. Do not ship a VM-only hard-coded subnet as a general configuration service.
3. **Trusted firewall:** one enforceable ingress and egress decision point for each network path including raw user frames, kernel-originated probes and external hosted adapters; separate forwarding and non-IP controls; rule validation, atomic update/rollback, flow tracking and bounded state, observed counters and dropped packets.
4. **Connection service:** actual link versus operator intent versus IP configured versus reachability, independent of Surfbird or Tailorbird; finite asynchronous connect/disconnect/reconnect with cancellation, safe stale-flow invalidation, no silent background reconnect after an explicit disconnect.
5. **GUI / API:** ask the service for authoritative status, show pending/error/limited clearly, allow a person to disconnect and later reconnect, expose firewall rule changes without changing security rules directly from an untrusted UI.
6. **Application:** user processes receive separately scoped socket/network capabilities. Browser HTTPS/certification, DNS parsing and page navigation are app concerns, never substitutes for firewall authorization or host connectivity management.
7. **Installed OS:** distinguish kernel, hosted Rust host, emulator network, Docker host, and real hardware. Passing a QEMU fixture does not imply a installed Wi-Fi hardware driver or an independent model process.

## Grounding in external standard practice

- **NIST SP 800-41 Rev.1**: explicit, documented firewall policy, default-deny for traffic not needed, inbound/outbound and application restrictions, testing and change control. https://csrc.nist.gov/pubs/sp/800/41/r1/final
- **OASIS Virtio 1.2**, network device section: feature negotiation, STATUS and LINK_UP, MTU, configuration changes. https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html
- **RFC 2863**: separate interface administrative versus observed operational state (up/down/dormant/notPresent/lowerLayerDown), with link transitions and flapping. https://www.rfc-editor.org/rfc/rfc2863.html
- **RFC 2131** §§3.6–3.7 and 4.4: each DHCP-equipped interface is configured independently; verify/reacquire address after a disconnection and obey lease/NAK/renew/rebind. https://datatracker.ietf.org/doc/html/rfc2131
- **RFC 5227**: detect IPv4 address conflict on newly configured/reused address; **RFC 4861**: neighbor/router discovery if IPv6 is supported. https://www.rfc-editor.org/rfc/rfc5227.html and https://www.rfc-editor.org/rfc/rfc4861.html
- **RFC 9293**: TCP abort/reset and half-open recovery after failure; a stale socket must not be falsely reported as a successful connected service. https://www.rfc-editor.org/rfc/rfc9293.html
- **NIST SP 800-153**: WLAN security is end-to-end across radio, client configuration, operations and monitoring. https://csrc.nist.gov/pubs/sp/800/153/final
- **Linux cfg80211/nl80211**: a contemporary example of separating 802.11 radio drivers from configuration/control, not a requirement to copy Linux code. https://www.kernel.org/doc/html/v6.9/driver-api/80211/cfg80211.html
- **NetworkManager CLI manual**: comparison for user-visible Wi-Fi scan/connect and deliberate device disconnect (including suppression of unwanted automatic activation), not evidence that PCP OS offers those functions. https://networkmanager.pages.freedesktop.org/NetworkManager/NetworkManager/nmcli.html

## Adversarial acceptance tests and honest gates

| Test | Positive and adverse conditions | Independent witness |
| --- | --- | --- |
| F1 firewall | Permit one explicit egress flow and its return, refuse different peer/port, unsolicited inbound, invalid headers, fragmentation and unsupported IP family. Repeat on raw digest-granted app, kernel-originated packet and hosted adapter separately. | On-wire pcap + process read buffer + effective kernel decision/counter |
| F2 policy changes | Apply and reject a rule atomically; revoke flow mid-session, protect ARP/ND as required, handle state exhaustion and policy reload/reboot. | Durable trusted policy state + actual emitted/received frames |
| E1 carrier | Start with virtual link down, restore, force at least six flaps; vary no NIC, two NICs, unexpected MAC and no RX reply; later repeat on actual hardware. | Guest recorded NIC state, external QMP/carrier events, pcap and app result |
| E2 DHCP/IP | Discover/request/ack, NAK/restart, lease expiry, renew/rebind, duplicate ARP, changed subnet and DNS. | DHCP/ARP traces, route/IP service readback, no old address use |
| E3 reconnect intent | Connect, deliberately Disconnect (no spontaneous reconnect), Reconnect explicitly, service crash during transition, timeout, resume/reboot. | GUI+API state and service outcome, independent packet checks |
| W1 Wi-Fi | Scan, correct/incorrect credentials, auth failure, lost AP, roam, reconnect, forget, airplane/rfkill; test WPA2/WPA3 under identified hardware/regulatory scope. | Radio/802.11 exchanges, protected credential lifecycle, truthful status |
| W2 mixed links | Concurrent Ethernet and Wi-Fi, priority and failover, DNS policy, captive/limited network. | Per-interface route/firewall/DNS and no traffic leakage |
| G1 GUI | Local keyboard/mouse action and machine API lead to same effective state; pending/failed/recovery visible; test long SSID text and safe warning/error displays. | Native framebuffer pixels and trusted controller readback |
| I1 installed | Actual PCP OS kernel and target hardware/device, persistent profiles and verified updated ISO, permitted/offline model if claimed. | Independent boot/driver/protocol evidence, source/build/model identity |

## New limited QEMU probe

The diagnostic script [ethernet_cycle.py](ethernet_cycle.py), the independent source-only [gui_geometry.py](gui_geometry.py) and path-scoped [CI workflow](https://github.com/KirtusJ/pcp-os/blob/director-13/network-lifecycle-audit-20261116/.github/workflows/director-13-network-audit.yml) build the existing opt-in kernel/Surfbird and run a host-loopback HTTP fixture. QMP sets virtual carrier down before a first guest is launched; the first guest should fail to browse without HTTP requests. QMP then sets carrier up and a **fresh app process** is launched; a local GET and intact kernel shutdown are required.

This deliberately does **not** claim an in-session reconnect, a native Connect button, a Wi-Fi radio, DHCP, firewall or physical NIC. A QMP command acknowledgement does not show the guest recognized link state. Actual CI result and captured serial/QMP/pcap are required; until inspected this case is **NOT_MEASURED**.

## Status

**Daily-driver firewall and Wi-Fi: OPEN / not implemented on examined master. GUI network manager and Ethernet carrier/configuration/reconnection: OPEN.** Basic gated virtual Ethernet and app-level TCP/HTTP are present in bounded scope. No review record or technical test here grants permission for default-media promotion. Per the Principal's direction, any later coordinated master push must come with a refreshed ISO. This test-only draft stays off master pending other owners' integration and release evidence.
