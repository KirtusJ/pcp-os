"""B3's independent hosted test of the display opt-in on a fixed kernel revision."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import traceback
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
WORK = Path(os.environ["RUNNER_TEMP"]) / "display-shadow"
CAPTURES = WORK / "captures"
APP = ROOT / "guest-checkout"
PIN = "745e8831cb25090ce478fcf246dcf6c8a65ec7b2"
KERNEL = "4d0a0bf60d2350bb6f9249e3fe94e7ac7d481061"
REPO = "https://github.com/KirtusJ/pcp-os.git"
RAW = "https://raw.githubusercontent.com/KirtusJ/pcp-os/"


def run(*args, cwd=ROOT, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


def old_records(commit, filename, blob, names):
    url = RAW + commit + "/evidence/tailorbird-loader/" + filename
    archive = WORK / filename
    urllib.request.urlretrieve(url, archive)
    actual = subprocess.check_output(["git", "hash-object", archive], text=True).strip()
    assert actual == blob
    with tarfile.open(archive, "r:xz") as saved:
        for source, target in names.items():
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(saved.extractfile(source).read())


def build_guest(name):
    target = WORK / ("target-" + name)
    env = {**os.environ, "CARGO_TARGET_DIR": str(target), "RUSTUP_TOOLCHAIN": "1.97.0"}
    run("sh", "build.sh", "--offline", cwd=APP, env=env)
    shutil.copyfile(target / "x86_64-unknown-none/release/tailorbird", WORK / "bin" / (name + ".elf"))


def main():
    WORK.mkdir()
    CAPTURES.mkdir()
    (WORK / "bin").mkdir()
    old_records("c7def585e4de43b7d9e143793aa92d78bcd4f8ca", "B3-Tailorbird-UEFI.tar.xz",
                "dca2919b05a5e0f9c047eee48209aa9045da7e2d", {
                    n: WORK / n for n in ("boot_current.py", "titles.py", "fixture.diff")})
    old_records("94039584e07361211b7a57553fdcd9314f3ff2ad", "B3-display-present-baseline.tar.xz",
                "44c42a52b395ecd9cf2618d836bc6078fcb2b312", {
                    "boot_edge.py": WORK / "boot_edge.py", "edge/pixels.py": WORK / "edge/pixels.py",
                    "edge/main.rs": WORK / "edge/main.rs", "bin/edge.elf": WORK / "bin/legacy.elf",
                    "edge-1024x768-ramfb-baseline/result.json": WORK / "direct-baseline.json"})
    # Keep the original optical/USB observers, firmware guard and font masks in the same fixture.
    run("git", "fetch", "--depth=1", REPO, PIN, KERNEL)
    run("git", "worktree", "add", "--detach", WORK / "src", PIN)
    run("git", "apply", WORK / "fixture.diff", cwd=WORK / "src")
    run("git", "worktree", "add", "--detach", WORK / "kernel-src", KERNEL)
    ksrc = WORK / "kernel-src"
    run("git", "apply", ksrc / "examples/firmware-owned-uefi/load-base.patch", cwd=ksrc)
    kdir = ksrc / "PCP_OS_Rust/crates/pcpos-kernel"
    run("sh", "run.sh", "build", cwd=kdir, env={**os.environ, "RUSTUP_TOOLCHAIN": "1.100.0",
                                              "PCPOS_KERNEL_STAGE": str(WORK / "kernel-stage")})
    kernel = kdir / "target/pcpos-kernel.elf32"
    device = APP / "pcp-os/store/native/guest/src/device.rs"
    assert subprocess.check_output(["git","hash-object",device],text=True).strip() == "3efcab9071d1953a5635a8e9f03d9850d3558bc9"
    client = (HERE / "client.rs").read_text()
    device.write_text(client)
    build_guest("app")
    # Only the instrumented edge fixture makes the two additional lifecycle calls.
    # The ordinary Tailorbird binary above consumed the unmodified proposed client.
    before = "        let base = call(9, 0, bytes, 3, 1, fd as usize, 0);"
    assert client.count(before) == 1
    device.write_text(client.replace(before, r'''        if shadow {
            assert_eq!(ioctl(fd, 0x5003, 0), -22);
            log(b"B3 opt-in shadow; present before mapping returned EINVAL\n");
        } else {
            log(b"B3 opt-in direct\n");
        }
''' + before + r'''
        if base >= 0 && shadow {
            assert_eq!(call(9, 0, bytes, 3, 1, fd as usize, 0), -22);
            log(b"B3 repeated shadow mapping returned EINVAL\n");
        }'''))
    shutil.copyfile(WORK / "edge/main.rs", APP / "guest/src/main.rs")
    build_guest("edge")
    fixture = WORK / "src/examples/firmware-owned-uefi"
    spec = importlib.util.spec_from_file_location("firmware_image", fixture / "image.py")
    image = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(image)
    for name in ("legacy", "edge", "app"):
        elf = WORK / "bin" / (name + ".elf")
        binary = elf.read_bytes()
        pin = hashlib.sha256(binary).hexdigest()
        os.environ["B3_TAILORBIRD_ELF"] = str(elf)
        filename, symbol, _ = image.MEMBERS[0]
        assert filename == "tailorbird.elf"
        image.MEMBERS = ((filename, symbol, pin), *image.MEMBERS[1:])
        folder = WORK / ("media-" + name)
        folder.mkdir()
        report = image.make(ROOT / "distribution/PCP-OS.iso", kernel, kdir / "tools/measure.py",
                            folder, fixture, usb_vm=True)
        (folder / "image.json").write_text(json.dumps(report, indent=2) + "\n")
        assert (folder / "usb.img").read_bytes().count(binary) == 1
        shutil.copyfile(folder / "image.json", CAPTURES / ("media-" + name + ".json"))
        shutil.copyfile(elf, CAPTURES / (name + ".elf"))
    qemu = shutil.which("qemu-system-x86_64")
    assert qemu
    firmware = "/usr/share/OVMF"
    sys.path.insert(0, str(WORK))
    from edge import pixels
    direct = json.loads((WORK / "direct-baseline.json").read_text())
    outcomes = []
    cases = [("edge","800x600","ramfb"), ("edge","800x600","bochs"), ("legacy","800x600","ramfb"),
             ("edge","1024x768","ramfb"), ("app","800x600","ramfb"), ("app","800x600","bochs"),
             ("app","1280x720","bochs"), ("app","1920x1080","bochs"), ("app","2560x1440","bochs")]
    for name, size, driver in cases:
        folder = WORK / f"{name}-{size}-{driver}"
        script = "boot_current.py" if name == "app" else "boot_edge.py"
        record = {"guest":name, "size":size, "driver":driver, "passed":False}
        try:
            run(sys.executable, "-B", WORK/script, size, qemu, firmware, name,
                cwd=WORK, env={**os.environ, "B3_VIDEO":driver})
            report = json.loads((folder/"result.json").read_text())
            serial = (folder/"serial.txt").read_text()
            assert report["verified"]
            if name == "legacy":
                assert report["launch_outcome"] == "exit 2 before first frame"
            else:
                assert report["launch_outcome"] == "rendered and exited 0"
            if name == "edge":
                width,height = map(int,size.split("x"))
                for stage,inverted in (("pattern-a",False),("pattern-b",True)):
                    assert pixels.verify((folder/(stage+".ppm")).read_bytes(),width,height,inverted)
                    if size=="1024x768":
                        assert report["frames"][stage] == direct["frames"][stage]
                if size=="800x600":
                    assert "B3 opt-in shadow; present before mapping returned EINVAL" in serial
                    assert "B3 repeated shadow mapping returned EINVAL" in serial
                else:
                    assert "B3 opt-in direct" in serial
            record.update(passed=True, launch=report["launch_outcome"], pitch=report["pitch"],
                          startup_and_final_passes=report["passed"], elf=report["guest_sha256"])
        except Exception:
            record["error"] = traceback.format_exc()
        finally:
            if folder.is_dir():
                shutil.move(folder, CAPTURES / folder.name)
            outcomes.append(record)
            (CAPTURES/"summary.json").write_text(json.dumps(outcomes,indent=2)+"\n")
            print(json.dumps(record),flush=True)
    assert all(case["passed"] for case in outcomes)


if __name__ == "__main__":
    main()
