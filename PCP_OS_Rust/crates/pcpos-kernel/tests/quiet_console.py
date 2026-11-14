"""Boot the kernel in QEMU to check COM1 visibility alongside reported COM2 observations.

Requires cbor2 and the GNU x86-64 assembler and linker for a program that prints one line.
"""
import argparse
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory

from boot_report import REPORT_MODES, boot, read_messages, validate


def console_program(directory):
    """Use ordinary Linux write and exit calls through the kernel's program ABI."""
    assembly = r'''
.global _start
.text
_start:
    mov $1, %eax
    mov $1, %edi
    lea message(%rip), %rsi
    mov $end-message, %edx
    syscall
    mov $60, %eax
    xor %edi, %edi
    syscall
.section .rodata
message: .ascii "program stdout reaches COM1\n"
end:
'''
    obj = directory / "hello.o"
    program = directory / "hello.elf"
    subprocess.run(["as", "--64", "-o", obj], input=assembly, text=True, check=True)
    subprocess.run(["ld", "-m", "elf_x86_64", "-pie", "--no-dynamic-linker",
                    "-o", program, obj], check=True)
    return program


def compare_reports(mode, reference, candidate):
    full = read_messages(mode, reference)
    quiet = read_messages(mode, candidate)
    # Each head identifies its own boot. The ordinary console separately witnessed
    # the expected counts, image and states for the paired boot via validate().
    for rows in (full, quiet):
        assert len(rows) == 3
        assert re.fullmatch(r"[0-9a-f]{64}", rows[-1].pop("head"))
    assert full == quiet, (full, quiet)
    assert len(reference) == len(candidate)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel", type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    kernel = args.kernel.resolve()
    with TemporaryDirectory(prefix="pcpos-quiet-") as tmp:
        directory = Path(tmp)
        program = console_program(directory)
        marker = b"program stdout reaches COM1"
        banner = b"PCP OS kernel "
        def run(case, options, **kwargs):
            return boot(kernel, args.qemu, directory, case, options, **kwargs)

        for mode in REPORT_MODES:
            wire_option = "report=" + mode
            full, full_wire = run(mode + "-full", wire_option)
            validate(mode, full_wire, full, None)
            brief, brief_wire = run(mode + "-brief", wire_option + " checks=brief")
            validate(mode, brief_wire, brief, None, brief=True)
            quiet, wire = run(mode + "-quiet", wire_option + " console=quiet")
            compare_reports(mode, full_wire, wire)
            compare_reports(mode, brief_wire, wire)
            assert quiet.startswith(banner) and b"  pass " not in quiet
            assert b"kernel register:" not in quiet and b"ALL SELF-TESTS PASS" not in quiet
            assert b"self-test totals:" not in quiet
            assert len(quiet.splitlines()) == 1 and len(quiet) <= 128, (mode, quiet)
            assert len(quiet) < len(brief) < len(full)
            print(f"{mode} full/brief/quiet COM1 bytes {len(full)}/{len(brief)}/{len(quiet)}, COM2 {len(wire)}")

        quiet, wire = run("both-options", "report=cbor checks=brief console=quiet")
        totals = re.search(rb"self-test totals: (\d+) passed, (\d+) failures", quiet)
        stop = read_messages("cbor", wire)[-1]
        assert totals and (int(totals[1]), int(totals[2])) == (stop["pass"], stop["fail"])
        assert b"running image" not in quiet and b"  pass " not in quiet

        for name, options, second, has_wire in (
            ("no-report", "console=quiet", True, False),
            ("unknown-report", "console=quiet report=unknown", True, False),
            ("no-port", "console=quiet report=cbor", False, False),
            ("unknown-console", "console=other report=cbor", True, True),
            ("unrelated-option", "xconsole=quiet report=cbor", True, True),
        ):
            console, wire = run(name, options, second=second)
            assert console.startswith(banner) and b"ALL SELF-TESTS PASS" in console and b"  pass " in console, name
            if has_wire:
                validate("cbor", wire, console, None)
            else:
                assert not wire
            if not second:
                assert b"COM2 did not respond" in console

        for name, options, code, safe, session_error in (
            ("program", "auto=hello.elf", 33, False, False),
            ("wrong-image", "auto=hello.elf image=" + "0" * 64, 35, True, False),
            ("missing-program", "auto=absent", 35, False, True),
        ):
            base = "report=cbor2 " + options
            full, full_wire = run(name + "-full", base, expected=code, initrd=program)
            validate("cbor2", full_wire, full, False if safe else None, safe, session_error)
            quiet, wire = run(name + "-quiet", base + " console=quiet", expected=code, initrd=program)
            compare_reports("cbor2", full_wire, wire)
            assert quiet.startswith(banner) and len(quiet) < len(full)
            if safe:
                assert marker not in quiet and b"starting hello.elf" not in quiet
                assert b"FAIL the running image measures" in quiet and b"SAFE and starts no program" in quiet
                assert b"NOT the value the loader named" in quiet and b"auto=hello.elf: not started" in quiet
            elif session_error:
                assert b"auto=absent: the loader handed over no program of that name" in quiet
            else:
                for text in (b"starting hello.elf", marker, b"hello.elf ended DONE"):
                    assert text in full and text in quiet, (text, quiet)
            if code == 35:
                assert b"1 SELF-TEST(S) FAILED" in quiet

    print("21 quiet-console comparison and control boots passed")


if __name__ == "__main__":
    main()
