"""Build a local Mockingbird VM from the repository's model-free boot media."""

import argparse
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tomllib
import zipfile
from pathlib import Path

import pycdlib

APP = Path(__file__).resolve().parent
REPO = APP.parents[1]
KERNEL = REPO / "PCP_OS_Rust/crates/pcpos-kernel"
PROFILE = REPO / "examples/release-media/daily-driver"


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def archive_package(out):
    with (APP / "Cargo.toml").open("rb") as manifest:
        version = tomllib.load(manifest)["package"]["version"]
    target = out / f"mockingbird-{version}-windows-vm.zip"
    files = [
        out / name for name in ("Open Mockingbird.cmd", "launch.py", "Mockingbird.ico")
    ]
    files.extend(path for path in (out / "media").rglob("*") if path.is_file())
    with zipfile.ZipFile(target, "x") as archive:
        for path in sorted(files, key=lambda p: p.relative_to(out).as_posix()):
            entry = zipfile.ZipInfo(path.relative_to(out).as_posix())
            entry.create_system = 3
            entry.external_attr = 0o100644 << 16
            data = path.read_bytes()
            if path.name == "launch.py":
                data = data.replace(b"\r\n", b"\n")
            archive.writestr(entry, data, compress_type=zipfile.ZIP_DEFLATED)
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "output", type=Path, help="new directory for the launcher and boot files"
    )
    parser.add_argument(
        "--archive", action="store_true", help="also write the portable Store ZIP"
    )
    parser.add_argument(
        "--toolchain",
        required=True,
        type=Path,
        help="Windows GNU Rust toolchain root with x86_64-unknown-none installed",
    )
    args = parser.parse_args()
    out = args.output.resolve()
    if out.exists():
        parser.error("Choose a new output directory")
    tool = args.toolchain.resolve()
    host = tool / "lib/rustlib/x86_64-pc-windows-gnu/bin"
    env = os.environ.copy()
    env["PATH"] = str(tool / "bin") + os.pathsep + env["PATH"]
    env["CARGO_TARGET_X86_64_UNKNOWN_NONE_LINKER"] = str(host / "rust-lld.exe")
    build = Path(env.get("CARGO_TARGET_DIR", str(APP / "target"))).resolve()
    env["CARGO_TARGET_DIR"] = str(build)
    for directory, extra in [
        (KERNEL, ["--features", "proposals,game-link"]),
        (APP, []),
    ]:
        subprocess.run(
            [
                str(tool / "bin/cargo.exe"),
                "build",
                "--release",
                "--locked",
                "--target",
                "x86_64-unknown-none",
                *extra,
            ],
            cwd=directory,
            env=env,
            check=True,
        )
    image = json.loads((PROFILE / "image.json").read_bytes())
    receipt = json.loads((PROFILE / "inputs.json").read_bytes())
    with zipfile.ZipFile(PROFILE / image["archive"]) as archive:
        raw = archive.read(image["filename"])
    assert hashlib.sha256(raw).hexdigest() == image["sha256"]
    media = out / "media"
    iso = pycdlib.PyCdlib()
    iso.open_fp(io.BytesIO(raw))
    for name, expected in receipt["files"].items():
        path = media / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("wb") as f:
            iso.get_file_from_iso_fp(
                f, iso_path="/" + name.upper().replace("-", "_") + ";1"
            )
        if digest(path) != expected:
            raise ValueError("Incorrect boot component: " + name)
    iso.close()
    bins = build / "x86_64-unknown-none/release"
    kernel = media / "PCPOS/KERNEL.ELF"
    subprocess.run(
        [
            str(host / "rust-objcopy.exe"),
            "-I",
            "elf64-x86-64",
            "-O",
            "elf32-i386",
            str(bins / "pcpos-kernel"),
            str(kernel),
        ],
        check=True,
    )
    measured = subprocess.check_output(
        [sys.executable, str(KERNEL / "tools/measure.py"), str(kernel)], text=True
    ).strip()
    editor = media / "PCPOS/EDITOR.ELF"
    shutil.copyfile(bins / "mockingbird", editor)
    shutil.copyfile(
        APP / "brand/Source-Sans-LICENSE.txt", media / "PCPOS/Source-Sans-LICENSE.txt"
    )
    cfg = media / "EFI/BOOT/grub.cfg"
    body = cfg.read_text()
    body = body.replace(
        "image=" + image["kernel_measurement"],
        "image="
        + measured
        + " auto=Mockingbird then=menu link_image="
        + digest(editor),
        1,
    )
    body = body.replace(
        "module /PCPOS/TAILOR.ELF",
        "module /PCPOS/EDITOR.ELF Mockingbird\nmodule /PCPOS/TAILOR.ELF",
        1,
    )
    cfg.write_bytes(body.encode("ascii"))
    shutil.copy2(APP / "launch.py", out / "launch.py")
    shutil.copy2(APP / "brand/mockingbird.ico", out / "Mockingbird.ico")
    (out / "Open Mockingbird.cmd").write_bytes(
        b'@echo off\r\nstart "" pythonw "%~dp0launch.py"\r\n'
    )
    # The script uses environment variables so paths containing quotes, spaces,
    # or PowerShell syntax are always data.
    subprocess.run(
        [
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            """
            $ErrorActionPreference = 'Stop'
            $out = $env:MOCKINGBIRD_PACKAGE
            $shell = New-Object -ComObject WScript.Shell
            $link = $shell.CreateShortcut((Join-Path $out 'Mockingbird.lnk'))
            $link.TargetPath = $env:MOCKINGBIRD_PYTHONW
            $link.Arguments = '"' + (Join-Path $out 'launch.py') + '"'
            $link.WorkingDirectory = $out
            $link.IconLocation = (Join-Path $out 'Mockingbird.ico') + ',0'
            $link.Description = 'Mockingbird for PCP OS'
            $link.Save()
            """,
        ],
        env={
            **env,
            "MOCKINGBIRD_PACKAGE": str(out),
            "MOCKINGBIRD_PYTHONW": str(Path(sys.executable).with_name("pythonw.exe")),
        },
        check=True,
    )
    print(out / "Mockingbird.lnk")
    if args.archive:
        print(archive_package(out))


if __name__ == "__main__":
    main()
