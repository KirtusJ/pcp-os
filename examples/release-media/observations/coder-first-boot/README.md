# Tailorbird 0.5B offline first-boot trial

Director A3, 22 November 2026. The same 1,038,960,640-byte hybrid ISO booted under Q35/OVMF as a read-only optical disc and as a raw read-only xHCI USB device. The whole-image SHA-256 was `8de0bbcce21d44925b23c2f5d444ce67c8daf8f3189404199782200e5332569f`. Each boot used fresh firmware variables, 4096 MiB RAM, TCG, standard VGA, no NIC and no internal disk.

## Build inputs

| Component | Identity |
| --- | --- |
| Kernel | [`1af35d14`](https://github.com/KirtusJ/pcp-os/tree/1af35d14ab5d37aaee3963be20380fff65d53a12), built with rustc 1.100.0 (`0ef1bc386`). Apply `examples/firmware-owned-uefi/load-base.patch` on the clean checkout with `git apply -C0`, then run `PCPOS_KERNEL_FEATURES=proposals,model-memory sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build`. |
| Foreground and worker | [Tailorbird #20 at `ffffc081`](https://github.com/KirtusJ/tailorbird/tree/ffffc081dfd545fa2c65a94dc9a01cd144b14c34), OS gitlink `0de2dd69`, rustc 1.97.0. Build the foreground binary `tailorbird` with no features and `model-worker` with the single `model-worker` feature. The [worker receipt](worker-build.json) records the target and manifest pin. |
| Model | Official Qwen2.5-Coder-0.5B-Instruct Q4_K_M, 491,400,064 bytes. The original SHA-256 is `1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`. Eight ordered pieces and a 312-byte manifest came from [#383’s model staging](https://github.com/KirtusJ/pcp-os/blob/0508e4e8ff2048e5d88a5d5e3163512c4cd699d0/examples/release-media/README.md). |
| Firmware | [#385 at `4daf2765`](https://github.com/KirtusJ/pcp-os/tree/4daf2765aa6414df772dfa6a28c6864a5f638e58/examples/firmware-model-loader), using its C reference implementation. Its documented build command took the four measured files and [input receipt](preloaded-inputs.json), SHA-256 `1f7bdf1224c5c0def69ab20c62e28397cf88ddfdce6d6b964b97b644cdb87945`. B3 owns the production Rust conversion. |

I reopened all eight shards and checked their ordered concatenation against the original model digest. I staged the firmware at `EFI/BOOT/BOOTX64.EFI` and the files at their prescribed `PCPOS/` paths on a 520 MiB FAT32 image. Each copied file was read back and hashed. The [master receipt](master.json) lists all 13 file digests, the FAT digest and the final ISO digest. The [hybrid builder](https://github.com/KirtusJ/pcp-os/blob/1af35d14ab5d37aaee3963be20380fff65d53a12/examples/model-media-iso/build.py) verified the Joliet copy. I independently read GPT partition 2 from the resulting ISO and matched its complete FAT digest.

## Raw USB

Both boots automatically started the UI and opened the bundled worker channel. On raw USB the firmware verified the model, the worker preflighted and the UI displayed Ready. I typed `Hello` into the note editor. Esc refused the first proposal. The second proposal displayed the final path, exact contents and digest. F12 exposed the confirmation and Enter authorized creation. These were separate kernel consent screens.

The [serial log](raw-usb/serial.txt) records DENIED for `taylor/my-first-project/note-1.txt` and CREATED for `note-2.txt`. The approved bytes were `Hello` followed by LF, SHA-256 `66a045b452102c59d840ec097d59d9467e13a3f34f6494e539ffd32c1bb35f18`. I reopened the approved file through Tailorbird. System Stop then ended both processes and returned to the desktop. All low and high frames were free, 72 kernel checks passed and QEMU exited with 33. The temporary file lasted until power-off.

[Loading](raw-usb/loading.png) · [Ready](raw-usb/ready.png) · [First proposal](raw-usb/asked-one.png) · [Second proposal](raw-usb/asked-two.png) · [Confirmation](raw-usb/confirmed-two.png) · [Reopened file](raw-usb/reopened.png) · [Desktop](raw-usb/desktop.png)

The images are lossless conversions of the captured framebuffers. Each conversion was decoded and compared pixel-for-pixel with its original. The adjacent text files contain the kernel-font readback of the approval panels. This is a 0.5B emulator qualification with the reference loader. The optional 8B/12B image, Rust loader and physical first boot have separate acceptance records.

## Rust loader and Tailorbird #22

A second hybrid used the Rust loader from [`87c627b8`](https://github.com/KirtusJ/pcp-os/tree/87c627b8c991cb9d2221d3ab8d7f17e2613f22f4) and the sequential prefill worker from [`dd8633d8`](https://github.com/KirtusJ/tailorbird/tree/dd8633d8516cb1bdc0fd8f258260594ad98848f1). The kernel, foreground, model and manifest digests matched the earlier trial. I rebuilt the EFI against a [new input receipt](rust-loader-usb/inputs.json) that pins the new worker. The finished hybrid was 1,039,247,360 bytes, SHA-256 `9aded90cccc152c4e2bbacfd4b3875bfc424e566ef504c951758989da9d5c90c`. [Whole-image and component digests](rust-loader-usb/master.json) · [Worker build](rust-loader-usb/worker-build.json)

I cold-booted that ISO as a raw read-only xHCI USB device with fresh OVMF variables and no network. The Rust loader admitted all eight real-model shards and Tailorbird reached Ready. One note was refused and the next was separately approved, created and reopened with the same six-byte digest. System Stop returned to the desktop. All 72 checks passed and QEMU exited 33. This run exercised first boot and file consent. B3’s subsequent reproducible-EFI revision and the timed conversation runs have their own binary identities and results. [Serial log](rust-loader-usb/serial.txt) · [Ready](rust-loader-usb/ready.png) · [Approval](rust-loader-usb/confirmed-two.png) · [Reopened file](rust-loader-usb/reopened.png)
