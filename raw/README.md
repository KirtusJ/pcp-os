# Installed Tailorbird interrupted-edit run

Two ordinary UEFI/GRUB boots on the same writable raw disk exercised the recovery-default build at `6cc994667c4a9826a70c11fb2c4d35f593d4342e`. The operator selected the desktop entry at each boot. Both used 96 MiB guest RAM, fresh firmware variable stores and separate QMP-verified guest UUIDs. Both launched the measured Tailorbird executable and logged Qwen worker exit 3.

In the first guest, the operator approved `create lark print original`. After `edit lark print changed then run lark` staged a second proposal, the driver killed QEMU with SIGKILL. The next guest mounted one saved file. A late Alt+A preceded a separate proposal that the operator stopped. The store retained the exact bytes of the approved original across both checkpoints. An explicit run displayed `original`. The operator then approved a fresh edit and its queued run displayed `replacement`. The second guest completed its power-off path with QEMU exit 33.

Each gzip file expands to the original captured bytes. The five store captures include both commit slots. The original and replacement P6 frames are complete 960 × 540 images. Their output fields match the measured captures from the earlier direct-boot test. The argv files retain the emulator configuration with portable path placeholders. The host process receipts in `result.json` record UUID checks and exits. `disk-comparison.json` records a direct comparison of the whole factory and post-run disks. Earlier incomplete attempts are in the parent evidence directory.

## Replay

Use the standard Python library and the public Tailorbird checker at PCP OS commit `b3c7cb67eb3ae85e419ac22365efdbb7e3a766c1`. From this evidence directory, run the following command with that checkout's checker directory.

```sh
python3 -O replay.py /path/to/pcp-os/os-docs/tailorbird-stop
```

With the pinned factory image available, add `--factory /path/to/PCP-OS.img`. The verifier reads the factory image, substitutes the captured final store in memory and compares the reconstructed whole-image digest to the measured post-run disk.

[Original case and prior attempts](https://github.com/KirtusJ/pcp-os/pull/197) · [QEMU QMP reference](https://www.qemu.org/docs/master/interop/qemu-qmp-ref.html)
