"""Boot the newly compiled default kernel through the published Limine BIOS CD and its demo modules."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile


def run(distribution, kernel, out):
    tools = distribution / "audit/public-live-cd"
    sys.path.insert(0, str(tools / "candidates/keyboard-01"))
    from candidate import check_boot_table, digest, extract, inspect_catalog, measure
    from check import IMAGE_SHA, ISO_SHA, check
    from check_register import check_register
    source = distribution / "PCP-OS.iso"
    if digest(source) != ISO_SHA:
        raise ValueError("Unexpected source CD")
    elf = kernel / "target/pcpos-kernel.elf32"
    measurement = measure(kernel / "tools", elf)
    out.mkdir()
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        original, rebuilt = work / "original", work / "rebuilt"
        extract(source, original)
        key = ("image=" + IMAGE_SHA).encode()
        config = (original / "limine.conf").read_bytes()
        if config.count(key) != 1 or measurement == IMAGE_SHA:
            raise ValueError("Expected one measured grant and a newly built kernel")
        config = config.replace(key, ("image=" + measurement).encode())
        conf = work / "limine.conf"
        conf.write_bytes(config)
        cd = work / "default.iso"
        subprocess.run(["xorriso", "-no_rc", "-indev", source, "-outdev", cd,
                        "-boot_image", "any", "replay", "-map", elf, "/kernel.elf32",
                        "-map", conf, "/limine.conf", "-commit", "-end"], check=True)
        extract(cd, rebuilt)
        def members(directory):
            return {p.relative_to(directory) for p in directory.rglob("*") if p.is_file()}
        files = members(original)
        if files != members(rebuilt) or digest(rebuilt / "kernel.elf32") != digest(elf) or (rebuilt / "limine.conf").read_bytes() != config:
            raise ValueError("Unexpected recovered files or measured grant")
        boot = Path("limine-bios-cd.bin")
        for item in files - {boot, Path("boot.cat"), Path("kernel.elf32"), Path("limine.conf")}:
            if digest(original / item) != digest(rebuilt / item):
                raise ValueError(f"Changed carried file {item}")
        a, b = ((p / boot).read_bytes() for p in (original, rebuilt))
        if a[:8] != b[:8] or a[24:] != b[24:]:
            raise ValueError("Changed Limine boot code outside its optical table")
        for image, tree, payload in ((source, original, a), (cd, rebuilt, b)):
            inspect_catalog(image, (tree / "boot.cat").read_bytes(), check_boot_table(image, payload))
        receipt = {"source_iso": ISO_SHA, "test_iso": digest(cd), "elf": digest(elf), "measurement": measurement}
        (out / "image.json").write_text(json.dumps(receipt, indent=2) + "\n")
        session = out / "session"
        check(cd, session, expected_iso=receipt["test_iso"], expected_image=measurement, debug_exit=False,
              check_register=True)
        check_register(kernel / "src/font.rs", session)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("distribution", "kernel", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    run(args.distribution.resolve(), args.kernel.resolve(), args.output.resolve())
