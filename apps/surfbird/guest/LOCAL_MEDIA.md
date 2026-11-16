# Native local video and stereo audio

Build with `--features local-media` and supply both `clip.ppm` and `clip.wav` as loader modules. Press Space on the Surfbird cover to play, or Esc to exit. If neither file is present, Surfbird enters its existing HTTP reader. The replay uses an x86 guest with an emulated display and AC'97 output, with networking disconnected.

The files have a deliberately small profile. The picture file contains 32 consecutive binary PPM images, each with the literal header `P6\n48 24\n255\n` followed by 48 × 24 RGB pixels. The rate is fixed at ten frames per second. The audio file contains a 44-byte PCM WAVE header followed by 153,600 signed 16-bit stereo sample pairs at 48 kHz. The guest validates headers and exact file lengths before showing the cover. The loader can supply different pictures and audio in this format. [Netpbm PPM specification](https://netpbm.sourceforge.net/doc/ppm.html) · [Microsoft RIFF documentation](https://learn.microsoft.com/en-us/windows/win32/xaudio2/resource-interchange-file-format--riff-)

The guest writes the decoded pixels to its framebuffer. It retries short audio writes and subtracts the sound device's queued bytes from the number accepted to select a picture. The kernel reports the queue in 512-pair DMA pieces plus a partial write. This clock does not measure physical display or loudspeaker latency. Esc stops early. A 12-second guest-time deadline bounds a stalled device.

## Reproduce

From this directory, with a built [PCP kernel](../../../PCP_OS_Rust/crates/pcpos-kernel/README.md), Rust's `x86_64-unknown-none` target, Python 3 and QEMU:

```sh
cargo build --locked --release --target x86_64-unknown-none --features local-media
guest=target/x86_64-unknown-none/release/surfcable
python3 replay_local_media.py "$kernel" "$guest" run-first --all-frames
python3 replay_local_media.py "$kernel" "$guest" run-second --variant 1
python3 replay_local_media.py "$kernel" "$guest" run-invalid --case bad-wave
```

Set `kernel` to the ELF32 boot image. Each invocation requires a new output directory. The harness creates original media, boots the guest, sends the play key and retains screenshots, a WAV capture, guest serial output, input and boot hashes and a JSON result. It checks each requested full framebuffer raster independently against the loaded pixel stream. It measures the pitch, channel, duration and spacing of each tone from QEMU's captured waveform. The WAV backend uses file output. An installation without speakers can replay `captured-playable.wav` later. Some QEMU builds leave the RIFF size fields at zero on debug exit. The harness validates the format and preserves the original bytes alongside a correctly sized copy with identical PCM.

## Observed

QEMU 11.1.0, Rust 1.91.1, kernel ELF SHA-256 `d2fae4ef3df316ff4fa0c1a88744e66cc17bd794856979fa55188b8402f7222f`, guest ELF SHA-256 `116abc19197bf1905847569f30d1c41877b81a3614297f45ec4a57d641f27eab`.

| Input | Measured output |
| --- | --- |
| First sequence and audio | All 32 full screenshots matched their source frames. Four 400-ms tones began at 0.2, 1.0, 1.8 and 2.6 seconds. Their pitches and channels were 500 Hz left, 750 Hz right, 600 Hz left and 375 Hz right. |
| Second sequence and audio, same ELF | The captured pictures followed the rotated source palette. Recorded tones moved to 750 Hz right, 600 Hz left, 375 Hz right and 500 Hz left. |
| Invalid WAVE block alignment | The guest rejected the track before showing a picture or writing audio. |

Both positive boots reported 3,200 ms of sound, zero underruns, 33 display submissions including the cover, normal process exit and a verified shutdown register. The native guest still needs HTTPS and a compatible Web player to play a YouTube watch page.

During development one concurrent boot failed the kernel's keyboard self-test before launching the guest. Another loaded run exposed a transient reversal in the AC'97 queue estimate after an underrun. The player now holds the last published picture across this condition, yields while filling the device and handles a temporary full-queue result. The acceptance replay fails any run with an underrun, a missed frame, or a changed audio sequence. Running the full-screen capture while the host is busy can also miss a screenshot.
