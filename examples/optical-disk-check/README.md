# Does booting the CD write to an attached disk?

The published PCP OS 0.3.1 CD issued one successful 512-byte write to sector zero of a disposable disk on both QEMU PC and Q35. The disk contents were identical before and after each boot. A whole-disk checksum by itself misses this write. For a first boot, follow the published instructions to attach only the virtual CD. [First-boot guide](https://github.com/KirtusJ/pcp/blob/35103c657218a8fb6299c19ea3548cdd2a47c354/audit/public-live-cd/README.md)

The BIOS code in Limine 9.2.3 reads the first sector, then attempts to write those bytes back while identifying optical devices. In both traced boots the write appeared before the kernel serial banner. Upstream subsequently changed BIOS optical detection to avoid this operation. [Limine 9.2.3 source](https://github.com/Limine-Bootloader/Limine/blob/82d2d4253e105e3c938396bf85c3da568f1ef73c/common/drivers/disk.s2.c#L290-L294) · [Upstream correction](https://github.com/Limine-Bootloader/Limine/commit/643e042a937ee3d5cc71b4c71e0276d33136298c)

## Images and observations

| Image | SHA-256 | Guest writes to test disk on PC / Q35 | Final disk bytes |
| --- | --- | --- | --- |
| Published CD | `93ed3b599ad7be0e1e88a64813b1f2b5299d51507df2edca691aaf6189d10d15` | 1 / 1, 512 bytes each | Identical |
| Remastered with original loader | `d1699ce2a9facc8e905e00bc4e14b3cca1569f0b9d25a3609c9663a33a6e196f` | 1 / 1, 512 bytes each | Identical |
| Experimental Limine 10.8.5 CD | `0d2672e284d1bf0d09c3869285e657d3769c7933d83f132ce9dc89a77f07abf8` | 0 / 0 | Identical |

These observations cover the BIOS optical boot path under Linux, QEMU 10.0.13 TCG and SeaBIOS 1.16.3. The test uses a fresh writable 9 MiB raw disk containing an MBR and a FAT16 volume with one file. It reads QEMU’s disk counters before starting the CPU, at the launcher, before power-off and after guest-requested shutdown. It also compares every byte after QEMU exits. On a separate copy, a monitor write changes one known sector and the test requires precisely that change. This checks the byte comparison. The old loader supplies the positive control for the guest write counters. QEMU routes monitor writes differently from guest I/O. [QMP block statistics](https://www.qemu.org/docs/master/interop/qemu-qmp-ref.html#command-query-blockstats)

The candidate preserves the kernel, application, media and boot configuration. The builder updates the BIOS loader, license and loader attribution and generates a new optical catalog. It rereads the contents and checks both loader inputs. The initial unpadded 10.8.5 image stopped while loading the application module. Both comparison images include 300 KiB of trailing optical padding. The successful and failed layouts establish a packaging constraint for this experiment. The precise failing read in the unpadded loader remains unknown.

## Try the combined BIOS candidate in a disposable VM

The artifact `experimental-bios-optical-candidate-keyboard-01` contains the keyboard-01 kernel with the Limine 10.8.5 BIOS loader. Download it from a successful **Optical boot with a disposable disk** workflow run and extract `no-write-candidate.iso`. Verify the extracted ISO against this table before booting.

| keyboard-01 image | SHA-256 | Guest writes on PC / Q35 with a 512-byte logical sector |
| --- | --- | --- |
| Original keyboard-01 ISO | `09e61116cc295f45a91061253d3913301ef867d790ee08678bd0fb25c1a0a00d` | 1 / 1 |
| Same kernel, remastered with original loader | `0e07977299d61d8d09044970f9825077ff6bf807a41af3911e5cb9c7662b879f` | 1 / 1 |
| Combined tester ISO | `5264e891b70a37d66541393e1fc0b135fd301b8e2097022e9b698aacdc5b3d8d` | 0 / 0 |

The combined image preserves every application and media payload from keyboard-01. Its measured kernel is `a9be6b1599bb10eead1344ae4a2ca53ec978074ae9aef1d30e8da10f04d044f3`. The builder normalizes the keyboard image’s primary ISO identifiers and retains the original Rock Ridge filenames. The original source and remastered control both require one successful write to LBA zero in the disk test.

For a first session, create a fresh x86-64 SeaBIOS VM with only the ISO attached. This QEMU command uses a single CPU, 64 MiB of RAM, the ordinary clock and a silent audio backend. Where QEMU is installed under a different executable name, use its x86-64 system emulator.

```sh
qemu-system-x86_64 -machine pc -accel tcg -cpu qemu64 -m 64M -smp 1 -nic none \
  -audiodev none,id=snd -device AC97,audiodev=snd \
  -boot order=d,strict=on -cdrom no-write-candidate.iso
```

Wait for the verified launcher and 71 startup passes. Press Enter and then Space to play the demo through completion. Launch it again and use Ctrl+Alt+Delete in the guest to stop it. R opens the register, Esc returns to the launcher, and P powers off. The application store lasts for this boot. Use virtual optical media for this test. Physical installation and recovery remain unqualified.

## Reproduce

The [workflow](../../.github/workflows/optical-disk-check.yml) pins the published media, Limine binary release and pycdlib. For each pinned input it builds the control and candidate twice, checks their digests, then exercises normal playback, the kernel Stop key, register navigation and power-off on PC and Q35. Both run with an instruction-counted CPU, 256 MiB RAM and the QEMU max CPU model. The combined candidate also runs with the 64 MiB configuration above and a disposable test disk. Successful runs retain the experimental ISO and separate evidence. For a local run, use Linux with QEMU, SeaBIOS, Python 3 and the same pycdlib revision. From the repository root, supply paths to the published ISO, the pinned loader checkout and a new output directory.

```sh
python3 examples/optical-disk-check/build.py PCP-OS.iso /path/to/limine images
python3 examples/optical-disk-check/check.py images/old-loader-control.iso old-results \
  --sha256 d1699ce2a9facc8e905e00bc4e14b3cca1569f0b9d25a3609c9663a33a6e196f --expect-writeback
python3 examples/optical-disk-check/check.py images/no-write-candidate.iso candidate-results \
  --sha256 0d2672e284d1bf0d09c3869285e657d3769c7933d83f132ce9dc89a77f07abf8
```

For a source-built BIOS CD with the same launcher and local media, pass the expected kernel measurement from its build manifest as `--kernel`. You can also compute it from the independently built kernel with `python3 PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py /path/to/kernel.elf32`. The checker compares this value with the measurement reported during boot. The separate `--sha256` argument pins the entire ISO.

The framebuffer check compares the released 1024×768 verified-launcher row and keyboard footer, then the distinct register footer. The probe also requires the measured kernel for the selected input. To reproduce the combined candidate checks, extract the pinned keyboard-01 ISO, build from it and pass `--kernel keyboard-01` and the appropriate image digest to the checker. Add `--tester-profile` to exercise the ordinary clock.

The keyboard-01 job also compares the remastered old loader with the combined tester ISO on Q35 using writable NVMe namespaces with 512-byte and 4,096-byte logical sectors. Both use a GPT with primary and backup headers and a FAT16 partition. Linux `partx` reads the partition bounds and GUID, and `blkid` identifies the filesystem before each run. The old-loader control must write one complete logical sector at LBA zero. The combined ISO must read the disk, issue zero writes and preserve all bytes. For a local run, add `--nvme-sector-size 512` or `--nvme-sector-size 4096` to the keyboard-01 checker command with `--tester-profile`.

The experiment covers emulated IDE and NVMe storage under BIOS optical boot. UEFI, Secure Boot, USB media, physical controllers, durable user files and recovery after interrupted writes require separate qualification. The native launcher describes its application store as lasting until power-off.
