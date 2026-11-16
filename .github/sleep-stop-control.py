"""Modify only the single longest-sleep boot fixture in a disposable checkout."""
from pathlib import Path
import sys

p = Path(sys.argv[1])
variant = sys.argv[2]
if variant != "normal":
    source = p.read_text()
    marker = source.index("USER_PROGRAM sleeper_long")
    start = source.index("    subq $16, %rsp", marker)
    end = source.index("USER_END sleeper_long", start)
    assert start < end
    if variant == "delayed":
        # Delay the real nanosleep without issuing any additional syscalls.
        # The counter fixes the instruction count in the fixed-clock control.
        prelude = "    movl $900000, %ecx\n8:  decl %ecx\n    jnz 8b\n"
    elif variant == "awake":
        prelude = "8:  jmp 8b\n"
    else:
        raise ValueError(variant)
    p.write_text(source[:start] + prelude + source[start:])
