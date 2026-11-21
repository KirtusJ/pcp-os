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

## Reproduce

The [workflow](../../.github/workflows/optical-disk-check.yml) pins the published media, Limine binary release and pycdlib. It builds each image twice, checks both digests, then exercises normal playback, the kernel Stop key, register navigation and power-off on PC and Q35. Successful runs retain the experimental ISO and separate evidence. For a local run, use Linux with QEMU, SeaBIOS, Tesseract English, Python 3 and the same pycdlib revision. From the repository root, supply paths to the published ISO, the pinned loader checkout and a new output directory.

```sh
python3 examples/optical-disk-check/build.py PCP-OS.iso /path/to/limine images
python3 examples/optical-disk-check/check.py images/old-loader-control.iso old-results \
  --sha256 d1699ce2a9facc8e905e00bc4e14b3cca1569f0b9d25a3609c9663a33a6e196f --expect-writeback
python3 examples/optical-disk-check/check.py images/no-write-candidate.iso candidate-results \
  --sha256 0d2672e284d1bf0d09c3869285e657d3769c7933d83f132ce9dc89a77f07abf8
```

The experiment covers a virtual IDE-compatible disk and BIOS optical boot. UEFI, Secure Boot, USB media, physical controllers, durable user files and recovery after interrupted writes require separate qualification. The native launcher describes its application store as lasting until power-off.
