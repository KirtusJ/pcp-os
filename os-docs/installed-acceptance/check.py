#!/usr/bin/env python3
"""Exercise the recovery, desktop and Qwen entries on an existing installed PCP OS disk."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import tempfile
import time
import uuid


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def checkpoint(directory, result):
    # Each run owns its results directory. Readers see either complete JSON version.
    pending = directory / "result.json.tmp"
    pending.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    pending.replace(directory / "result.json")


def wait_serial(process, log, marker, deadline):
    while True:
        data = log.read_bytes() if log.exists() else b""
        if marker in data:
            return
        if process.poll() is not None or time.monotonic() >= deadline:
            tail = data[-700:].decode(errors="replace")
            raise RuntimeError(f"Waiting for {marker!r}; guest status {process.poll()}: {tail}")
        time.sleep(0.1)


def key(process, name):
    # QEMU's human monitor accepts one command per line on its private stdin.
    process.stdin.write(f"sendkey {name} 45\n".encode("ascii"))
    process.stdin.flush()
    time.sleep(0.18)


def pinned_kernel_measurement(path, manifest):
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != manifest["component_sha256"]["kernel"]:
        raise ValueError("Kernel ELF SHA-256 differs from the release manifest")
    tool = Path(__file__).resolve().parents[2] / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
    try:
        return runpy.run_path(str(tool))["measure"](raw)
    except SystemExit as error:
        raise ValueError(f"Cannot measure release kernel: {error}") from error


def verify_log(data, mode, manifest, expected_tokens, kernel_measurement=None):
    lines = data.decode(errors="replace").replace("\r", "").splitlines()
    components = manifest["component_sha256"]
    titles = {"recovery": "PCP OS recovery (no model)",
              "desktop": "PCP OS with Tailorbird and local Qwen",
              "diagnostic": "Offline Qwen acquisition test"}
    # GRUB can echo the same selection on both terminals. A menu listing is not a selection.
    selections = re.findall(r"Booting `([^'\r\n]+)'", "\n".join(lines))
    if any(title != titles[mode] for title in selections):
        raise ValueError(f"{mode}: GRUB selected a different entry: {selections}")
    measured = [match for line in lines if (match := re.fullmatch(
        r"running image \(code and read-only data\), SHA-256: ([0-9a-f]{64}) "
        r"\(measured; equal to the value the loader named\)", line))]
    if len(measured) != 1:
        raise ValueError(f"{mode}: expected one measured running image")
    if kernel_measurement is not None and measured[0][1] != kernel_measurement:
        raise ValueError(f"{mode}: running kernel differs from the manifest-matched ELF")
    arguments = ["image=" + measured[0][1]]
    if mode == "desktop":
        arguments.extend(["tailor_ui=" + components["tailorbird"],
                          "userdisk=" + uuid.UUID(manifest["factory_disk_guid"]).bytes_le.hex(),
                          "q=" + components["qwen-worker"]])
    elif mode == "diagnostic":
        arguments.extend(["auto=qwen.elf", "grant=files", "then=off"])
    commands = [line for line in lines if line.startswith("command line: ")]
    if len(commands) != 1 or sorted(commands[0].removeprefix("command line: ").split()) != sorted(arguments):
        raise ValueError(f"{mode}: kernel command line differs from the release boot profile")
    records = [
        r".*PCP OS EFI amd64 entry, native 64-bit handoff.*",
        r"protections: .*long mode true",
        re.escape(commands[0]),
        re.escape(measured[0][0]),
    ]
    launches = [line for line in lines if line.startswith("starting ")]
    if mode == "recovery":
        records.append(r"PCP OS launcher ready: 0 programs, 0 files")
        if launches or any(line.startswith("QWEN READY") for line in lines):
            raise ValueError("The recovery entry started a payload")
    else:
        if mode == "desktop":
            launch = (r"starting Tailorbird \([0-9]+ bytes, SHA-256 " +
                      re.escape(components["tailorbird"]) + r"\)")
            records.extend([
                r"Persistent application store mounted: .*",
                r"PCP OS launcher ready: .*",
                launch,
                r"Qwen worker admitted in separate address space: .*",
                r"TAILORBIRD MCP CONNECTED(?: [0-9]{4}-[0-9]{2}-[0-9]{2})?",
                r"QWEN READY",
                r"Tailorbird ended DONE: exit 0(?: after [0-9]+ s; .+)?",
            ])
        else:
            launch = (r"starting qwen\.elf \([0-9]+ bytes, SHA-256 " +
                      re.escape(components["qwen-worker"]) + r"\) with: 3 17 488 220 17 284")
            records.extend([
                launch,
                r"QWEN READY",
                *[f"QWEN TOKEN {token}" for token in expected_tokens],
                r"QWEN COMPLETE",
                r"qwen\.elf ended DONE: exit 0(?: after [0-9]+ s; .+)?",
            ])
            tokens = [int(line.removeprefix("QWEN TOKEN ")) for line in lines
                      if line.startswith("QWEN TOKEN ")]
            if tokens != expected_tokens:
                raise ValueError(f"Qwen generated {tokens}; expected {expected_tokens}")
        if len(launches) != 1 or not re.fullmatch(launch, launches[0]):
            raise ValueError(f"{mode}: expected exactly one manifest-matched payload launch")
    records.append(r"ALL SELF-TESTS PASS(?:\. .+)?")
    remaining = iter(lines)
    for record in records:
        if not any(re.fullmatch(record, line) for line in remaining):
            raise ValueError(f"{mode}: missing or out-of-order record {record!r}")
    if any(line.lstrip().startswith(("FAIL ", "QWEN ERROR")) for line in lines):
        raise ValueError(f"{mode}: guest reported a failure")


def boot(args, mode, manifest, expected_tokens, temp):
    log = args.results / f"{mode}.serial"
    variables = temp / f"{mode}.fd"
    shutil.copyfile(args.ovmf_vars, variables)

    # QEMU escapes a comma within a drive filename as two commas.
    def drive_path(path):
        return str(path.resolve()).replace(",", ",,")

    machine = "q35,accel=tcg"
    memory_mib = 192 if mode == "recovery" else args.model_memory_mib
    if args.ram_directory and mode != "recovery":
        machine += ",memory-backend=guestmem"
    command = [args.qemu, "-machine", machine, "-cpu", "max", "-m", f"{memory_mib}M",
               "-nic", "none", "-display", "none", "-vga", "none", "-device",
               "VGA,edid=on,xres=1280,yres=720", "-no-reboot", "-monitor", "stdio",
               "-serial", "file:" + str(log.resolve()), "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-drive", f"if=pflash,format=raw,readonly=on,file={drive_path(args.ovmf_code)}",
               "-drive", f"if=pflash,format=raw,file={drive_path(variables)}",
               "-drive", f"if=none,format=raw,snapshot=on,id=os,file={drive_path(args.disk)}",
               "-device", "virtio-blk-pci,drive=os,disable-modern=on,bootindex=1"]
    if args.ram_directory and mode != "recovery":
        ram_file = temp / "guest.ram"
        command.extend(["-object", f"memory-backend-file,id=guestmem,size={args.model_memory_mib}M,"
                        f"mem-path={drive_path(ram_file)},share=on"])
    if args.firmware:
        command.extend(["-L", str(args.firmware)])
    deadline = time.monotonic() + args.timeout
    with (args.results / f"{mode}.stderr").open("xb") as errors, subprocess.Popen(
        command, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=errors
    ) as process:
        try:
            if mode != "recovery":
                wait_serial(process, log, b"Offline Qwen acquisition test", deadline)
                key(process, "up" if mode == "desktop" else "down")
                key(process, "ret")
            if mode == "desktop":
                wait_serial(process, log, b"PCP OS launcher ready:", deadline)
                time.sleep(0.7)
                key(process, "ret")
                wait_serial(process, log, b"QWEN READY", deadline)
                key(process, "esc")
                wait_serial(process, log, b"Tailorbird ended DONE: exit 0", deadline)
            if mode != "diagnostic":
                wait_serial(process, log, b"PCP OS launcher ready:", deadline)
                time.sleep(0.7)
                key(process, "p")
            wait_serial(process, log, b"ALL SELF-TESTS PASS", deadline)
            status = process.wait(timeout=max(0.1, deadline - time.monotonic()))
            if status != 33:
                raise RuntimeError(f"{mode}: QEMU exited {status}, expected 33")
            verify_log(log.read_bytes(), mode, manifest, expected_tokens, args.kernel_measurement)
        except (Exception, KeyboardInterrupt) as error:
            error.qemu_exit_before_cleanup = process.poll()
            raise
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            if args.ram_directory and mode != "recovery":
                ram_file.unlink(missing_ok=True)
    return {"serial_sha256": sha256(log), "qemu_exit": status}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("disk", "manifest", "ovmf-code", "ovmf-vars", "results"):
        parser.add_argument("--" + arg, type=Path, required=True)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--kernel-elf", type=Path,
                        help="verify the running measurement against the manifest-matched kernel ELF")
    parser.add_argument("--modes", nargs="+", choices=("recovery", "desktop", "diagnostic"),
                        default=("recovery", "desktop", "diagnostic"),
                        help="boot paths to exercise, in order (default: all three)")
    parser.add_argument("--firmware", type=Path, help="QEMU firmware search directory")
    parser.add_argument("--ram-directory", type=Path, help="parent for disposable file-backed guest RAM")
    parser.add_argument("--model-memory-mib", type=int, default=1152,
                        help="RAM for each model-capable VM in MiB")
    parser.add_argument("--timeout", type=int, default=360, help="seconds per boot")
    parser.add_argument("--expected-tokens", type=int, nargs=3, required=True,
                        help="three token IDs from a separate reference for this installed diagnostic")
    args = parser.parse_args()
    if len(args.modes) != len(set(args.modes)):
        parser.error("Each boot path can be selected once per run")
    if args.model_memory_mib < 1 or args.timeout < 1:
        parser.error("RAM and timeout must be positive")
    manifest = json.loads(args.manifest.read_bytes())
    profile = manifest.get("tailorbird_request", {}).get("factory_profile", "")
    if not re.fullmatch(r"recovery (?:[1-9]|[1-5][0-9]|60)", profile):
        parser.error("This check requires the recovery-default Tailorbird release profile")
    original = sha256(args.disk)
    if original != manifest["output_sha256"]["PCP-OS.img"]:
        parser.error("The installed disk does not match the release manifest")
    args.results.mkdir(parents=True, exist_ok=False)
    result = {"source_revision": manifest["git_commit"], "disk_sha256_before": original,
              "expected_tokens": args.expected_tokens, "selected_modes": args.modes,
              "model_memory_mib": args.model_memory_mib, "boots": {},
              "status": "in_progress", "active_mode": None}
    checkpoint(args.results, result)
    try:
        args.kernel_measurement = None
        if args.kernel_elf is not None:
            args.kernel_measurement = pinned_kernel_measurement(args.kernel_elf, manifest)
        result["release_kernel_measurement"] = args.kernel_measurement
        result["manifest_sha256"] = sha256(args.manifest)
        result["ovmf_code_sha256"] = sha256(args.ovmf_code)
        result["ovmf_vars_sha256"] = sha256(args.ovmf_vars)
        result["qemu"] = subprocess.check_output([args.qemu, "--version"], text=True).splitlines()[0]
        with tempfile.TemporaryDirectory(dir=args.ram_directory) as directory:
            for mode in args.modes:
                result["active_mode"] = mode
                checkpoint(args.results, result)
                result["boots"][mode] = boot(args, mode, manifest,
                                             args.expected_tokens, Path(directory))
                result["active_mode"] = None
                checkpoint(args.results, result)
    except (Exception, KeyboardInterrupt) as error:
        result["failure"] = {"mode": result["active_mode"] or "setup",
                             "reason": str(error) or type(error).__name__}
        if hasattr(error, "qemu_exit_before_cleanup"):
            result["failure"]["qemu_exit_before_cleanup"] = error.qemu_exit_before_cleanup
        if result["active_mode"] is not None:
            streams = {}
            for name in ("serial", "stderr"):
                path = args.results / f"{result['active_mode']}.{name}"
                try:
                    streams[name] = {"bytes": path.stat().st_size, "sha256": sha256(path)}
                except FileNotFoundError:
                    continue
                except OSError as read_error:
                    streams[name] = {"read_error": str(read_error)}
            if streams:
                result["failure"]["streams"] = streams
        result["active_mode"] = None
        result["status"] = "failed"
        checkpoint(args.results, result)
        raise
    finally:
        result["active_mode"] = None
        try:
            result["disk_sha256_after"] = sha256(args.disk)
            if result["disk_sha256_after"] != original:
                raise RuntimeError("The installed disk changed during the check")
        except (Exception, KeyboardInterrupt) as error:
            result["disk_verification_error"] = str(error) or type(error).__name__
            result["status"] = "failed"
            checkpoint(args.results, result)
            raise
        if "failure" not in result and list(result["boots"]) == list(args.modes):
            result["status"] = "complete"
        checkpoint(args.results, result)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
