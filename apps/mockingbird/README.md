# Mockingbird

A Rust text editor for the x86-64 PCP OS guest. Its document lives in RAM. In the Windows VM package, Ctrl+V pastes plain text from Windows and Ctrl+X cuts selected text to Windows. Ctrl+C copies. Use Shift with the arrows to select text, or Ctrl+A to select the document. Esc opens a discard prompt when the document contains text.

The document holds up to 32 KiB of UTF-8. Unsupported glyphs display as a question mark and retain their original text when copied. The editor uses the kernel's existing DejaVu Sans Mono glyphs. A running instance can edit with an internal clipboard on a kernel without the optional guest cable.

## Build the Windows VM package

From this directory, with Python 3.11 or newer, pycdlib and the Windows GNU Rust toolchain including x86_64-unknown-none, run the following with a new output directory.

~~~powershell
python package.py D:/path/to/Mockingbird --toolchain D:/path/to/rust-toolchain
~~~

Open **Open Mockingbird.cmd** in the output directory. The launcher finds x86-64 QEMU on PATH or uses PCPOS_QEMU, which can point to the executable. It expects the unsigned x86-64 OVMF code and variables in QEMU's share directory.

The package uses the daily driver's model-free assets and builds the PCP OS kernel with its existing proposals and game-link features. The bootstrap grants the clipboard cable to the exact editor ELF digest. The local Windows helper exchanges plain text on explicit clipboard commands, using the emulated COM2 serial port. The VM has no virtual network card. The launcher gives each invocation its own firmware variables and attaches the boot files read-only.
