"""Build the Rust x86-64 EFI model loader from an independently pinned assembly receipt."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile

from record import executable, identity, publish, write_record

HERE = Path(__file__).resolve().parent
CRATE = HERE.parents[1] / "PCP_OS_Rust/crates/pcpos-efi-loader/Cargo.toml"
sys.path.insert(0, str(HERE.parent / "release-media"))
from toolchain import build_environment, tool_record

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("parts", type=Path)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--receipt-sha256", required=True)
    parser.add_argument("--gnu-efi", type=Path, default=Path("/usr"))
    parser.add_argument("--build-record", type=Path, help="write an additional, new JSON record of the tools, commands and output")
    parser.add_argument("output", type=Path)
    a = parser.parse_args()
    prefix = a.gnu_efi.resolve()
    lib = prefix / "lib"
    a.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=a.output.parent, prefix=".model-efi-") as temp:
        work = Path(temp).resolve()
        env = build_environment(work)
        config = work / "pins.rs"

        commands = []
        native_tools = {name: executable(name) for name in ("cc", "ld", "objcopy")}
        paths = {"cargo": Path(env["CARGO"]), **native_tools}
        records = {}
        if a.build_record:
            records = tool_record(env)
            records.update({name: identity(path) for name, path in native_tools.items()})
        roots = {str(work): "<build>", str(a.parts.resolve()): "<parts>",
                 str(a.receipt.resolve()): "<receipt>", str(HERE.parents[1]): "<checkout>",
                 str(prefix): "<gnu-efi>"}
        def portable(arg):
            text = str(arg)
            for source in sorted(roots, key=len, reverse=True):
                text = text.replace(source, roots[source])
            return text

        def run(tool, *args):
            commands.append([tool, *(portable(arg) for arg in args)])
            subprocess.run([paths[tool], *args], cwd=work, env=env, check=True)

        run("cargo", "run", "--locked", "--manifest-path", CRATE, "--features", "assembly",
            "--bin", "pin-inputs", "--", a.parts.resolve(), a.receipt.resolve(), a.receipt_sha256, config)
        env.update(PCPOS_EFI_CONFIGURATION=str(config), CARGO_ENCODED_RUSTFLAGS="\x1f".join((
            "-C", "relocation-model=pic", "-C", "no-redzone=yes",
            "--remap-path-prefix", f"{HERE.parents[1]}=/pcpos-source",
            "--remap-path-prefix", f"{work}=/pcpos-efi-build",
        )))
        run("cargo", "build", "--locked", "--manifest-path", CRATE, "--lib", "--profile", "efi", "--target", "x86_64-unknown-none")
        run("cc", "-O2", "-Wall", "-Wextra", "-Werror", "-fpic", "-fshort-wchar", "-ffreestanding",
            "-fno-stack-protector", "-mno-red-zone", "-DGNU_EFI_USE_MS_ABI", "-I"+str(prefix/"include/efi"),
            "-I"+str(prefix/"include/efi/x86_64"), "-c", HERE/"firmware.c", "-o", "firmware.o")
        run("cc", "-c", HERE.parent/"firmware-owned-uefi/handoff.S", "-o", "handoff.o")
        exports = work / "exports.map"
        exports.write_text("{ global: _start; efi_main; local: *; };\n", encoding="ascii")
        run("ld", "--version-script", exports, "-nostdlib", "-znocombreloc", "-T", lib/"elf_x86_64_efi.lds", "-shared", "-Bsymbolic",
            "--gc-sections", "--no-undefined", lib/"crt0-efi-x86_64.o", "firmware.o", "handoff.o",
            work/"target/x86_64-unknown-none/efi/libpcpos_preloaded_loader.a", "-L"+str(lib), "-lefi", "-lgnuefi", "-o", "loader.so")
        sections = (".text", ".rodata", ".sdata", ".data", ".dynamic", ".dynsym", ".rel", ".rela", ".reloc")
        run("objcopy", "--strip-all", *(arg for section in sections for arg in ("-j", section)),
            "--target=efi-app-x86_64", "loader.so", "BOOTX64.EFI")
        binary = work / "BOOTX64.EFI"
        receipt = work / "build.json" if a.build_record else None
        if receipt is not None:
            write_record(receipt, binary=binary, input_pin=a.receipt_sha256, config=config,
                         lock=HERE.parents[1]/"PCP_OS_Rust/Cargo.lock",
                         tools=records, sdk=prefix, commands=commands)
        publish(binary, a.output, receipt, a.build_record)
    print(hashlib.sha256(a.output.read_bytes()).hexdigest(), a.output)

if __name__ == "__main__":
    main()
