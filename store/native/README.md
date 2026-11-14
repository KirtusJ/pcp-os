# Native Store launcher

The launcher is Rust. Games and the Heliograph lab execute as freestanding Rust guests inside PCP OS. These Store releases have no browser player. QEMU, Rust and the [kernel build prerequisites](../../PCP_OS_Rust/crates/pcpos-kernel/README.md) are needed for a source checkout.

From the repository root, build a stock kernel, then choose a guest. The launcher needs the ELF32 converted image emitted by the kernel's build script. A previously measured PCP OS 0.3.1 ELF32 can also be supplied. `PCPOS_QEMU` may specify an absolute QEMU executable.

```sh
sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
cargo run --locked --release --manifest-path store/native/Cargo.toml -- PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 sunspur
```

Use `gullpost` for the 2D courier or `gull64` for Lighthouse Island. Use `fluxrink` for the rink or `heliograph` for the rendering lab. Press Enter at the OS menu. Escape returns from each program and P at the OS menu powers down. The launcher loads the versioned Store ELF, measures the kernel's code and read-only pages, requests 32 MiB of VGA memory for Heliograph's resolution ladder and gives QEMU short module names.

## Linked Flux Rink

Build the opt-in kernel in its own staging directory. Save the image separately from the stock image. On Windows, preserve LF in the published machine table, which the kernel compares byte-for-byte during startup.

```sh
PCPOS_KERNEL_STAGE=/tmp/pcpos-flux-kernel PCPOS_KERNEL_FEATURES=game-link sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
cp PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 flux-kernel.elf32
cargo run --locked --release --manifest-path store/native/Cargo.toml -- flux-kernel.elf32 fluxrink host 29455
```

In a second terminal use the same kernel, checkout and port:

```sh
cargo run --locked --release --manifest-path store/native/Cargo.toml -- flux-kernel.elf32 fluxrink peer 29455
```

The host listens only on 127.0.0.1. The optional kernel grant binds the guest's complete ELF SHA-256 to COM2. The verification console stays on COM1. Ordinary launches leave the cable ungranted. Either process can be closed independently.

## Guest source builds

From the individual guest directory, use Rust 1.91.1 or compatible with the bare-metal target installed. The committed linker script and Cargo configuration apply when building from that directory. Set `BIN` to `sunspur`, `fluxrink` or `heliograph`. The games depend on the neighbouring Heliograph and guest support crates, so keep the repository layout.

```sh
rustup target add x86_64-unknown-none
cargo test --locked --release --lib
cargo build --locked --release --target x86_64-unknown-none --bin "$BIN"
cp "target/x86_64-unknown-none/release/$BIN" "dist/$BIN.elf"
```
