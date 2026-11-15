"""Build original local media and check the guest's real QEMU screen and audio outputs."""
import argparse
from array import array
import hashlib
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import time
import wave


HEADER = b"P6\n48 24\n255\n"
IMAGES = 32
PALETTE = [(192, 60, 45), (56, 156, 117), (72, 117, 208), (211, 160, 63)]
NOTES = (500, 750, 600, 375)


def check(value, message):
    if not value:
        raise RuntimeError(message)


def make_media(path, variant):
    frames = []
    pcm = bytearray()
    for frame in range(IMAGES):
        group = (frame // 8 + variant) % 4
        bg = PALETTE[group]
        # Unique moving vertical line permits a captured screen to identify its source frame.
        frames.append(b"".join(bytes((238, 240, 245) if x == frame + 4 else bg)
                               for y in range(24) for x in range(48)))
        for sample in range(4800):
            t = (frame % 8) * 4800 + sample
            tone = round(9000 * math.sin(2 * math.pi * NOTES[group] * t / 48000)) if 9600 <= t < 28800 else 0
            pcm.extend(struct.pack("<hh", tone if group % 2 == 0 else 0, tone if group % 2 else 0))
    (path / "clip.ppm").write_bytes(b"".join(HEADER + frame for frame in frames))
    with wave.open(str(path / "clip.wav"), "wb") as wav:
        wav.setparams((2, 2, 48000, 0, "NONE", "not compressed"))
        wav.writeframes(pcm)
    return frames


def screenshot_frame(path, frames):
    with path.open("rb") as stream:
        check(stream.readline().rstrip() == b"P6", "unexpected screen format")
        dims = stream.readline()
        while dims.startswith(b"#"):
            dims = stream.readline()
        check(dims.split() == [b"640", b"360"] and stream.readline().strip() == b"255", "unexpected screen dimensions")
        raster = stream.read()
    check(len(raster) == 640 * 360 * 3, "incomplete screenshot")
    sampled = bytearray()
    for y in range(24):
        for x in range(48):
            i = ((36 + 12*y + 6)*640 + 32 + 12*x + 6)*3
            sampled.extend(raster[i:i+3])
    check(bytes(sampled) in frames, "guest display differs from all source frames")
    index = frames.index(bytes(sampled))
    border = bytes.fromhex("0c1218")
    expected = bytearray(border * (640 * 36))
    for y in range(24):
        source_row = frames[index][y*48*3:(y+1)*48*3]
        scan = b"".join(source_row[x:x+3]*12 for x in range(0,len(source_row),3))
        expected.extend((border*32 + scan + border*32)*12)
    expected.extend(border * (640 * 36))
    check(expected == raster, "framebuffer pixel mismatch")
    return index


def captured_audio(path, variant):
    capture = path.read_bytes()
    check(capture[:4] == b"RIFF" and capture[8:16] == b"WAVEfmt " and capture[36:40] == b"data",
          "unrecognized QEMU capture")
    check(capture[16:36] == struct.pack("<IHHIIHH", 16, 1, 2, 48000, 192000, 4, 16),
          "unexpected QEMU audio format")
    lengths = (int.from_bytes(capture[4:8],"little"), int.from_bytes(capture[40:44],"little"))
    # isa-debug-exit on some QEMU builds leaves both RIFF length placeholders at zero.
    check(lengths in ((0, 0), (len(capture)-8, len(capture)-44)) and (len(capture)-44)%4 == 0,
          "invalid QEMU capture lengths")
    samples = array("h", capture[44:])
    with wave.open(str(path.with_name("captured-playable.wav")), "wb") as wav:
        wav.setparams((2, 2, 48000, 0, "NONE", "not compressed"))
        wav.writeframes(capture[44:])
    if sys.byteorder != "little":
        samples.byteswap()
    # 10-ms windows identify the four tone bursts in the independently captured device output.
    windows = []
    for at in range(0, len(samples)-959, 960):
        channels = [samples[at+c:at+960:2] for c in (0, 1)]
        peaks = [max(map(abs, ch)) for ch in channels]
        active = max(peaks)
        channel = peaks.index(active)
        if active > 500:
            check(peaks[1-channel] < active / 15, "unexpected stereo leakage")
            windows.append((at//960, channel))
    runs = []
    for tick, side in windows:
        if not runs or tick != runs[-1][1] + 1 or side != runs[-1][2]:
            runs.append([tick, tick, side])
        else:
            runs[-1][1] = tick
    check(len(runs) == 4, f"expected four audible bursts, got {runs}")
    summary = []
    for j, (start, end, side) in enumerate(runs):
        group = (j + variant) % 4
        check(side == group % 2 and 35 <= end-start+1 <= 46, f"wrong channel/duration: {runs}")
        if j:
            check(75 <= start - runs[j-1][0] <= 86, f"wrong burst spacing: {runs}")
        block = samples[(start+2)*960+side:(end-1)*960+side:2]
        cycles = sum(a <= 0 < b for a,b in zip(block,block[1:]))
        hz = round(48000*cycles/(len(block)-1), 1)
        check(abs(hz-NOTES[group]) <= 6, f"wrong pitch: {hz}")
        summary.append({"start_ms":10*start, "duration_ms":10*(end-start+1), "channel":side, "Hz":hz})
    return summary


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args):
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=False)
    frames = make_media(work, args.variant)
    if args.case == "bad-wave":
        with (work / "clip.wav").open("r+b") as w:
            w.seek(32)
            w.write(b"\x02\x00")
    shutil.copyfile(args.kernel, work / "kernel.elf32")
    shutil.copyfile(args.guest, work / "surf.elf")
    measure_tool = Path(__file__).resolve().parents[3] / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
    measure = subprocess.check_output([sys.executable, str(measure_tool), str(args.kernel)], text=True).strip()
    cmd = [args.qemu]
    if args.firmware:
        cmd.extend(["-L", str(args.firmware.resolve())])
    cmd.extend(["-accel", "tcg", "-cpu", "max", "-m", "256M", "-vga", "std", "-display", "none",
                "-nic", "none", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
                "-audiodev", "wav,id=sound,path=capture.wav,out.frequency=48000,out.channels=2",
                "-device", "AC97,audiodev=sound", "-serial", "file:serial.log", "-monitor", "stdio",
                "-kernel", "kernel.elf32", "-initrd", "surf.elf,clip.ppm,clip.wav", "-append", "image="+measure])
    serial = work / "serial.log"
    requested_frames = list(range(IMAGES)) if args.all_frames else [3, 11, 19, 27]
    def wait(vm, mark):
        until = time.monotonic() + 32
        while time.monotonic() < until:
            data = serial.read_text(errors="replace") if serial.exists() else ""
            if mark in data:
                return
            if vm.poll() is not None or ("SURFBIRD error" in data and args.case == "play"):
                break
            time.sleep(.012)
        raise RuntimeError(f"missing {mark} in {work}: {data[-550:]}")
    def monitor(vm, command):
        vm.stdin.write((command+"\n").encode("ascii"))
        vm.stdin.flush()
    with (work / "stderr.log").open("wb") as errors:
        vm = subprocess.Popen(cmd, cwd=work, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=errors)
        try:
            wait(vm, "a program that cannot be admitted")
            time.sleep(.45)
            monitor(vm, "sendkey ret 60")
            if args.case == "play":
                wait(vm, "SURFBIRD media ready")
                monitor(vm, "sendkey spc 50")
                for frame in requested_frames:
                    wait(vm, f"SURFBIRD media-frame {frame}\n")
                    monitor(vm, f"screendump frame-{frame}.ppm")
                wait(vm, "SURFBIRD media drained")
            else:
                wait(vm, "SURFBIRD error invalid local stereo track")
            wait(vm, "frames shown (")
            time.sleep(.35)
            monitor(vm, "sendkey p 60")
            check(vm.wait(timeout=12) == 33, "abnormal exit")
        finally:
            if vm.poll() is None:
                vm.kill()
                vm.wait()
            vm.stdin.close()
    data = serial.read_text()
    check("kernel's register of its own start, verification and stop verifies" in data, "boot register failed")
    receipt = {"arm":args.case, "variant":args.variant, "kernel_measure":measure,
               "kernel_sha256":digest(work/"kernel.elf32"), "guest_sha256":digest(work/"surf.elf"),
               "video_sha256":digest(work/"clip.ppm"), "audio_sha256":digest(work/"clip.wav")}
    if args.case == "play":
        check("surf.elf ended DONE: exit 0" in data and "; 33 frames shown (" in data
              and "3200 ms of sound played, the device ran out of sound 0 time(s)" in data,
              "kernel did not record complete uninterrupted playback")
        observed = [int(line.split()[-1]) for line in data.splitlines() if line.startswith("SURFBIRD media-frame ")]
        check(observed == list(range(IMAGES)), f"missing or reordered frames: {observed}")
        receipt["screen_source_frames"] = [screenshot_frame(work / f"frame-{i}.ppm", frames) for i in requested_frames]
        check(all(i//8 == wanted//8 for i,wanted in zip(receipt["screen_source_frames"], requested_frames)),
              "wrong screen ordering")
        if args.all_frames:
            check(receipt["screen_source_frames"] == requested_frames, "all-frame capture lost a frame")
        receipt["audio_bursts"] = captured_audio(work / "capture.wav", args.variant)
    else:
        check("SURFBIRD media-frame" not in data and "SURFBIRD media ready" not in data, "invalid media presented")
        check("surf.elf ended FAILED: exit 1" in data and "; 0 frames shown (" in data
              and "0 ms of sound played" in data, "unexpected bad-input output")
    (work / "result.json").write_text(json.dumps(receipt, indent=2)+"\n")
    print(json.dumps(receipt))


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("kernel", type=Path)
    p.add_argument("guest", type=Path)
    p.add_argument("output", type=Path, help="new directory for all inputs, captures and logs")
    p.add_argument("--qemu", default="qemu-system-x86_64")
    p.add_argument("--firmware", type=Path)
    p.add_argument("--variant", type=int, choices=(0,1), default=0)
    p.add_argument("--all-frames", action="store_true", help="capture and compare all 32 complete QEMU screenshots")
    p.add_argument("--case", choices=("play","bad-wave"), default="play")
    run(p.parse_args())
