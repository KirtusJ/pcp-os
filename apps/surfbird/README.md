# Surfbird on PCP-OS

The [native guest](guest/) draws a page on the PCP-OS display. Use **1** or **2** to follow a numbered link, **B** to go back and **Esc** to return to the menu. PCP-OS owns the guest renderer, parser, kernel device interfaces and QEMU replays. The Ethernet guest can [resolve a fixture hostname over DNS](guest/DNS.md) or [connect to a fixed address over TCP](guest/TCP.md). The [serial build](guest/README.md#serial-cable) uses a preconnected HTTP fixture. These experiments exercise local HTTP.

The [standalone Surfbird repository](https://github.com/KirtusJ/surfbird) owns the hosted Rust reader, its terminal controls and machine API. Submit hosted changes there. This directory includes a [pinned hosted copy and its matching manual](https://github.com/KirtusJ/surfbird/tree/28b863bbc1822f424a173a51e618b0c7d898c3ab) for comparisons with the guest.

## Build and test

From the PCP-OS repository root, build the hosted reader and run both host-side test suites. With the locked dependencies in Cargo's cache, these commands work offline. For a fresh cache, run `cargo fetch --locked --manifest-path` with each manifest path first.

```sh
cargo build --frozen --manifest-path apps/surfbird/Cargo.toml
cargo test --frozen --manifest-path apps/surfbird/Cargo.toml
cargo test --frozen --manifest-path apps/surfbird/guest/Cargo.toml --lib
```

The hosted suite uses loopback fixtures and leaves its live HTTPS test opt-in. The guest library suite runs on the host. To boot the graphical guest, use the [QEMU instructions](guest/README.md#replay).

## Verify or update the hosted copy

The import at this pin contains eleven files across `src/`, `tests/`, `Cargo.toml`, `Cargo.lock` and `surfbird.svg`. This README and `guest/` are maintained in PCP-OS. From the repository root in a POSIX shell, compare the **committed** snapshot with its upstream Git tree. The fetch only runs if the upstream commit is absent locally.

```sh
pin=28b863bbc1822f424a173a51e618b0c7d898c3ab
git cat-file -e "$pin^{commit}" 2>/dev/null ||
    git fetch --no-tags https://github.com/KirtusJ/surfbird.git "$pin"
git diff --exit-code --no-ext-diff --no-textconv "$pin^{tree}" HEAD:apps/surfbird -- \
    src tests Cargo.toml Cargo.lock surfbird.svg
```

An empty diff with exit code zero means all tracked paths, modes and contents in the five import roots match. It detects additions and deletions at any depth under `src/` and `tests/`. For an update, review the upstream commit, replace the imported trees and files as a unit, and update both the pin link and the command. If a new top-level file is needed for integration, add it explicitly to the import and check. Run both suites and verify the committed import before submitting the PCP-OS integration PR.
