# What happens when the virtual USB loader cannot allocate its arena

All six boots used the same unmodified, read-only QEMU USB image and stock OVMF. Each boot started with a fresh copy of the variable store and an identical synthetic internal NVMe disk. Only guest RAM varied. The 64 MiB loader failure was reproduced without sending a key. At 80 MiB the kernel displayed its failed verification. At 128 MiB it reached the verified desktop. Both desktops displayed an F10 question and stopped after Enter.

The image was built from 711a585af4f4a46ecb5ef3b03120b55dc92c2b1d. Its loader.c is the same Git blob, 638be9eb9e15a139495f3e8a4093b1961c0f107f, as in d92f5f630a1d452466113a61252f135219143506. The embedded kernel is the earlier build. The USB fixture checks for QEMU TCG before performing allocations. No physical machine was involved.

The original observer expected every desktop boot to pass all self-tests. Its first 80 MiB run retained the real two failed checks and exited with an assertion. The repeat explicitly checked the observed SAFE outcome, the zero program-frame pool and the visible failure status. Both runs and their serial output are included. All three cases were repeated independently.

The two raw variable-store snapshots in each case allow byte-by-byte checks. The identical synthetic internal disk is included once. The USB and EFI SHA-256 values identify the original image. Each receipt contains the read-back of the USB and the VM event and block-device counters. The QEMU end frame at 64 MiB shows the firmware logo and boot text. The refusal token was on the debug channel.

To replay, extract originals.tar.xz, run sha256sum -c SHA256SUMS there, then run python -B replay.py /path/to/711a585 /path/to/extracted. The replay uses NumPy, Pillow and the repository's usb_observer.py. readback.json gives the expected result.
