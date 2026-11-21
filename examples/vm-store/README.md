# Persistent user files in a QEMU PC

The optional `vm-store` kernel feature lets a guest program save files across cold boots. The supported device is one purpose-created 132,608-byte raw file on QEMU’s emulated primary IDE master, with 512-byte logical sectors. The standard tester CD continues to use RAM for user files. This experiment uses QEMU’s direct Multiboot kernel loader on an x86-64 BIOS PC.

## Build and replay

From the repository root, with Python 3, QEMU, SeaBIOS, clang, GNU binutils, lld and the kernel’s Rust toolchain installed:

```sh
export RUSTUP_TOOLCHAIN=1.97.0
rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal -c rust-src -t x86_64-unknown-none
stage=$(mktemp -d)
PCPOS_KERNEL_STAGE="$stage" PCPOS_KERNEL_FEATURES=vm-store sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
kernel=PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32
python3 examples/vm-store/check.py "$kernel" vm-store-results
```

The replay creates its own data file and records each guest’s serial stream and disk digests. It checks a saved file, an unsynced replacement interrupted by stopping the VM, a synced replacement, refusal of an oversized checkpoint, volatile operation, device selection and recovery from modeled partial writes. In the partial-write cases the host constructs the intermediate sector images. In the interrupted replacement case the host actually terminates the running VM. The machine boots separately for each observation.

## Try the queue desk

The repository includes a graphical program that uses this save contract. After building the opt-in kernel, build the queue and run the cold-boot check from the repository root:

```sh
export CARGO_TARGET_DIR="$PWD/vm-queue-target"
(cd store/apps/parrots-all-the-way-down/guest && cargo build --locked --release --target x86_64-unknown-none)
queue="$CARGO_TARGET_DIR/x86_64-unknown-none/release/rdesk"
python3 examples/vm-store/check_queue.py "$kernel" "$queue" vm-queue-results
```

This check types `cold morning` in the graphical queue, verifies the label and **Disk save** in the framebuffer, exits the program and waits for power-off. A new QEMU process opens the same volume. From the desktop, the checker presses F3 to open Files and reads the saved queue’s name, displayed size and volume status. It then opens the launcher with Space and starts the queue. The checker reads **Queue restored** and the label from that screen, cancels the F10 power-off question, checks that the queue is still selected, then presses F10 and Enter to shut down. It decodes the journal and compares the two disk images byte-for-byte. The automated keyboard and framebuffer replay uses QMP on a POSIX host.

To type your own entry, supply the built `rdesk` as the program in the next section. From the desktop, press Space to open the launcher and Enter to start the queue, press N, type a label, then press Enter and look for **Disk save**. Escape returns to the launcher, where F10 asks whether to power off. Press Esc to cancel or Enter to confirm. Relaunch the same VM directory to reopen your queue.

## Attach a new volume to a guest program

Use a new directory for each volume. Keep its ID together with its data file. Run the following from the repository root with the opt-in kernel built as above. Replace `path/to/program.elf` with a PCP OS-compatible x86-64 guest program that uses the file API. The short kernel and program filenames keep the Multiboot command line within the loader’s limit.

```sh
python3 examples/vm-store/create.py personal-vm
cp "$kernel" personal-vm/kernel.elf
cp path/to/program.elf personal-vm/program.elf
measurement=$(python3 PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py personal-vm/kernel.elf)
(cd personal-vm && qemu-system-x86_64 -machine pc -accel tcg -cpu max -m 256M -nic none \
  -kernel kernel.elf -initrd program.elf \
  -append "store=ide0 store_id=$(cat volume-id.txt) image=$measurement" \
  -drive if=none,id=store,format=raw,file=data.raw \
  -device ide-hd,drive=store,bus=ide.0,unit=0,serial=PCP-VM-STORE)
```

Use the same directory and launch command on the next boot. The launcher reports that the VM disk saves on `fsync` when mounting succeeds. Booting without either store option leaves the namespace volatile. An incomplete selection, different volume marker, unexpected device serial or different geometry stops program launch. The role serial and volume ID check the intended attachment. Anyone able to copy the disk can copy its ID.

## Save semantics and limits

With the store grant, write the file and call `fsync` on a stored file or the guest directory. A successful call checkpoints the whole writable namespace. Creating, closing, renaming and powering off do not themselves commit. Check the return value. `ENOSPC` leaves the previous checkpoint available. Application memory and immutable loader files are outside the checkpoint.

The two alternating slots each hold up to 64 KiB, including names and record headers. The store admits up to 256 files and 255 bytes per stored name. The kernel writes the inactive payload, flushes the virtual disk, writes its generation and checksums, then flushes again before returning success. Mount picks the newest valid generation. Use one writer per data file and the normal QEMU flush settings. The experiment covers this emulated device and guest-process interruption. Physical media, host power loss and other controllers require separate tests.
