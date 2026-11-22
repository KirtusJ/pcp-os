"""Build the Rust x86-64 EFI model loader from an independently pinned assembly receipt."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
CRATE = HERE.parents[1] / "PCP_OS_Rust/crates/pcpos-efi-loader/Cargo.toml"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("parts", type=Path)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--receipt-sha256", required=True)
    parser.add_argument("--gnu-efi", type=Path, default=Path("/usr"))
    parser.add_argument("output", type=Path)
    a = parser.parse_args()
    prefix = a.gnu_efi.resolve()
    lib = prefix / "lib"
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=a.output.parent, prefix=".model-efi-") as temp:
        work = Path(temp).resolve()
        env = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
        config = work / "pins.rs"

        def run(*cmd):
            subprocess.run(cmd, cwd=work, env=env, check=True)

        run("cargo", "run", "--locked", "--manifest-path", CRATE, "--features", "assembly",
            "--bin", "pin-inputs", "--", a.parts.resolve(), a.receipt.resolve(), a.receipt_sha256, config)
        env.update(PCPOS_EFI_CONFIGURATION=str(config), RUSTFLAGS="-C relocation-model=pic -C no-redzone=yes")
        run("cargo", "build", "--locked", "--manifest-path", CRATE, "--lib", "--release", "--target", "x86_64-unknown-none")
        run("cc", "-O2", "-Wall", "-Wextra", "-Werror", "-fpic", "-fshort-wchar", "-ffreestanding",
            "-fno-stack-protector", "-mno-red-zone", "-DGNU_EFI_USE_MS_ABI", "-I"+str(prefix/"include/efi"),
            "-I"+str(prefix/"include/efi/x86_64"), "-c", HERE/"firmware.c", "-o", "firmware.o")
        run("cc", "-c", HERE.parent/"firmware-owned-uefi/handoff.S", "-o", "handoff.o")
        run("ld", "-nostdlib", "-znocombreloc", "-T", lib/"elf_x86_64_efi.lds", "-shared", "-Bsymbolic",
            "--gc-sections", "--no-undefined", lib/"crt0-efi-x86_64.o", "firmware.o", "handoff.o",
            work/"target/x86_64-unknown-none/release/libpcpos_preloaded_loader.a", "-L"+str(lib), "-lefi", "-lgnuefi", "-o", "loader.so")
        sections = (".text", ".rodata", ".sdata", ".data", ".dynamic", ".dynsym", ".rel", ".rela", ".reloc")
        run("objcopy", *(arg for section in sections for arg in ("-j", section)),
            "--target=efi-app-x86_64", "loader.so", "BOOTX64.EFI")
        os.link(work/"BOOTX64.EFI", a.output)
    print(hashlib.sha256(a.output.read_bytes()).hexdigest(), a.output)

if __name__ == "__main__":
    main()
