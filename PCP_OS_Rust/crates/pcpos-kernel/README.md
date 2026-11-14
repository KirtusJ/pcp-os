# PCP OS kernel | early

A new kernel, with no code from any other kernel. Target: x86_64. Language: Rust, with one assembly file for the entry from the loader, the interrupt stubs and the system-call entry. Version 0.3.1.

It has been started only in an emulator (QEMU 8.2.2, without hardware virtualisation). Every device it drives is an emulated one. Read "What this does not show" before relying on any number below.

## What it does

**At start.** `boot.s` asks the processor whether it offers long mode and no-execute, and says so and stops if it does not. The kernel then replaces the loader's memory map with its own: the image (code not writable, data not executable), the files the loader handed over (read-only), a pool of frames for programs (never executable in kernel mode), and nothing else. Address zero, the low mebibyte and the text display are not mapped. Interrupt table, segment table and task state segment are read-only once set. Double fault, non-maskable interrupt and machine check each have a stack of their own, and every stack has an unmapped page below it. Supervisor execution prevention, supervisor access prevention and user-mode instruction prevention are turned on where offered. Then the self-tests, 58 by the time the kernel stops (57 before the launcher appears; the last is that its register verifies at the stop): each forbidden access is attempted and the processor's refusal is the result. The kernel measures its own code and read-only data. Named an expected measurement on the loader's command line (`image=<SHA-256>`; `tools/measure.py <image file>` computes it without running anything), it compares, and the comparison is one more self-test. A kernel in which any self-test failed is in SAFE: it shows its launcher and its register and starts no program. Where a keyboard controller answers, eight further self-tests follow the 57 (`asynchronous` in `main.rs`): a sleep is woken at its millisecond, a key wakes a program that waits for one, and the stop key ends a program that is running, asleep or waiting, with keys handed to the emulated controller by the kernel itself (`dev::keyboard_script`) so that they arrive by the keyboard's interrupt. The counts in this file that say 58 are those of the recorded runs, before these were added.

**Programs.** A program is a statically linked, position-independent ELF file for x86_64, given to QEMU with `-initrd` (several, separated by commas; the name after the path is the name in the launcher). `exec.rs` checks the file before anything of it runs and refuses, with the reason: a truncated file, another processor, a fixed load address, a program that needs a dynamic loader, an entry point outside its own code, and a segment that is writable and executable at once. A process has its own address space, built from frames that are zeroed when handed out and returned when it ends. Floating-point and vector registers are saved and restored for each process.

**System calls.** The numbers and conventions of Linux for x86_64, through the `syscall` instruction, for the subset a single-threaded C program needs: files (open, read, write, seek, stat, vectored I/O, rename, unlink, a directory listing), memory (brk, anonymous mmap, munmap, mprotect without execute-and-write), time (clock_gettime, nanosleep, clock_nanosleep), a terminal on the serial port, exit. A call outside the subset returns "no such call" and is counted; the soak test requires the count to be zero. The interface is borrowed so that existing programs run unmodified; no code is.

**Capabilities.** Each of `/dev/fb` (display), `/dev/keys` (keyboard), `/dev/audio` (sampled sound), `/dev/fm` (music synthesiser), the clock and the store is reached only by a process whose session granted it. The grants of each program are listed in the launcher before it is started. `grant=none` on the kernel's command line starts programs with the console alone; the probe program then counts 18 refusals and exits cleanly.

**Drivers** (`dev.rs`).
- *Display.* The Bochs/QEMU display adapter, found by scanning the PCI bus; its memory is mapped writable and non-executable; 32 bits a pixel, mode set through the adapter's own registers: 1024 by 768 for the kernel's own screens, and the mode a program asks for while it runs. The adapter's memory holds two pages: a program draws a whole picture into the hidden page and the kernel switches pages, so no half-drawn picture is shown. No VGA BIOS call is made.
- *Keyboard.* PS/2, scan-code set 1 through the controller's translation, interrupt-driven, delivered to the process that holds the capability as press and release events. Ctrl+Alt+Del is never delivered: it ends the running program and returns to the launcher.
- *Sampled sound.* AC'97, 48 kHz stereo, a ring of buffers refilled on the device's interrupt. A write blocks only while the ring is full. Underruns are counted and reported at the end of the process.
- *Music.* An OPL2 FM synthesiser at ports 0x388/0x389. A program hands the kernel register writes with times; the kernel performs each at its millisecond, so music does not depend on when the program next runs. Lateness is measured and reported.
- *Timer.* 1 kHz. A sleeping process is woken at its millisecond; a kernel with nothing to run halts the processor until the next interrupt.

**Session** (`session.rs`, `ui.rs`). With a display, the kernel draws a launcher: the programs it was handed, what each is granted, the kernel's state, and a view of its own register. Enter starts a program; when it ends, by itself or by the stop key, the launcher is back and says how it ended. `auto=<name>` starts a program without the launcher; `then=off` stops the kernel when the program ends, which is how tests end. Without a display the kernel runs what `auto=` names and writes to the serial port. Typeface: 10 by 20 cells of coverage rasterised once from DejaVu Sans Mono by `tools/make_font.py` (see `NOTICE.md` of this workspace). Colours: the design tokens of the edition.

**Register.** The kernel's start, verification, every process as a task, and the stop are events in a register checked by the native ACR guards.

## Rules of this crate

`unsafe` in two files only: `arch.rs` (processor, memory, the copy between kernel and user memory) and `dev.rs` (device registers and device memory). Each use states why it is sound. The crate denies it everywhere and those two files alone lift the denial, so the loader, the system calls, the process table, the session and the drawing code are checked by the compiler in full. A program's memory is read and written only through two functions in `arch.rs` that check the range against the process's own map. Arithmetic overflow stops the kernel in release builds.

## Doom

Doom is the test of the above taken together. It is not part of the kernel or of PCP OS and none of it is in this release. `programs/doom/build.sh` takes two source trees you obtain yourself (doomgeneric, commit e257a095; Chocolate Doom, commit a2dfd51f; both GPL-2.0) and builds one separate program with `musl-gcc`. Six small files here connect it to the kernel: `doomgeneric_pcpos.c` (display, keys, time), `i_pcpsound.c` (sound effects, mixed at 48 kHz), `opl_pcpos.c` (Chocolate Doom's OPL music player writing timed register writes to `/dev/fm`), `choc_compat.h`, with `pcpos.h` (the names of the kernel's devices) and an empty `SDL_mixer.h`. The first four are offered under GPL-2.0 so that the combined program can be distributed; they are not part of the proprietary material. Game data is whatever `doom1.wad` or other IWAD you hand over as a second `-initrd` file.

```
programs/doom/build.sh <doomgeneric/doomgeneric> <chocolate-doom> doom.elf
qemu-system-x86_64 -cpu max -m 256M -kernel target/pcpos-kernel.elf32 \
  -initrd "doom.elf doom,/path/doom1.wad doom1.wad" \
  -audiodev <your driver>,id=snd0 -device AC97,audiodev=snd0 -device adlib,audiodev=snd0
```

Arrow keys move, Ctrl fires, Space opens, Shift runs, Alt strafes, Enter and Escape work the menus. Saved games are kept in memory until the kernel stops. The program draws at 640 by 400, twice the game's own resolution.

## Checked how

`tests/run_tests.py <kernel> <programs dir> <game data dir> <out dir>` starts the kernel in QEMU for each of 19 tests (17 until build 22) and reads three things that the kernel does not control: the emulator's exit status, pictures of the screen taken by the emulator, and the sound the emulator wrote to a file. Keys are sent through the emulator. The run recorded for build 21 is in the audit record (`audit12/kernel/`). For build 22 the suite was run again on a second machine of the same kind, set up from nothing, on the kernel image and programs built there (`audit13/kernel/`): the 17 tests of build 21 passed again on build 21's source rebuilt there, and all 19 on this build's. The figures in the table are build 21's; the second machine's differ in speed and are in that record.

| Test | What was measured in the recorded run |
|---|---|
| Kernel alone, a processor with every optional protection and one with none | 58 self-tests pass on each |
| Display and keyboard, nothing to run | The launcher appears; the kernel switches off when asked |
| A C program with every grant | 19 checks of its own pass: files, memory, floating point, time, display, keys, sound |
| The same with no grant | 18 refusals, each the expected one; the program still exits cleanly |
| A program that writes to kernel memory | Ended by the kernel; the kernel continues |
| Six files that are not valid programs | Each refused with its reason before any of it runs |
| Doom's own benchmark, three demos of the shareware data and three of Freedoom, limiter off | 198 to 235 pictures a second, every picture drawn and shown; the demos end at the tic the data says |
| A demo at the speed of play | 2,134 game tics in 61 seconds by the kernel's clock, as the game's 35 tics a second require; the processor idle about two thirds of the time; sound never ran dry |
| Doom played by keys: menu, new game, walk, turn, fire, save, load, stop key, start again, quit from its own menu | Each step changes the picture as it should; the saved game loads; frames in the pool are the same before and after; sound never ran dry; music at most 42 ms late |
| Left running 240 seconds | No fault, no call the kernel does not offer, about 16,700 pictures |
| No display | The program says so and exits; the kernel continues |
| The image named: the value `tools/measure.py` computes from the file (since build 22) | The kernel's own measurement equals it; the kernel verifies, with the comparison as a 59th self-test |
| Another value named (since build 22) | Verification fails on that one test; the kernel says which value was named, starts no program and stops with a failing status; its register still closes and replays |

## What this does not show

- **No real computer.** Nothing here has run on hardware, on another emulator, or with hardware virtualisation. The drivers are written to the devices as QEMU presents them. Real AC'97 and OPL2 devices are decades out of production; a real display adapter needs a driver of its own or a firmware framebuffer, and neither exists here.
- **Speed is the emulator's.** Pictures a second measure this kernel and this program under QEMU's translation on one machine. They say that the kernel is not the limit at the speed of play; they predict nothing about hardware.
- **Nobody played it.** Keys were sent by a script and pictures compared by a script. How it feels, whether sound is pleasant, whether a key is ever missed by a person's hands: not measured.
- **Sound was examined as numbers.** The captured file is checked for being non-silent where sound is expected and for underruns the kernel counted. Nobody listened.
- **No mouse, no joystick, no network play.**
- **Nothing is kept.** Saved games and anything else written live in memory and are gone at the stop. The game's settings are not saved at all (the port used does not write them).
- **One program at a time has the screen.** There is no window system and no switching between running programs.
- **The system-call subset is what two programs needed.** Another program may ask for a call that is not there.
- **The kernel's own gaps.** No address randomisation, no signed image, nothing to compare its measurement with, no defence against attacks on speculative execution, one processor.

## Building it twice

The same source gives the same files wherever it is built.

- **The kernel image.** `run.sh` copies the workspace to one fixed folder (`/tmp/pcpos-kernel-build`, or the folder named in `PCPOS_KERNEL_STAGE`), compiles it there and keeps that folder's name out of the image. Until build 22 it compiled in place, and the image's digest followed the name of the folder: the build tool names every function after the folder its crate is in, and the order of functions in the image follows those names. Checked for build 22 by building from two folders, and once more from nothing: one digest. What still belongs to the image's identity is the compiler's version and the folder the toolchain is installed in; neither was varied.
- **Doom.** `programs/doom/build.sh` keeps folder names out of the program. Built for build 22 on a second machine from the two public source trees fetched afresh, it is the same file to the byte as the one build 21 tested.
- **The test program.** `programs/probe/build.sh` builds `probe.elf` (there was no script for it before build 22).

## Loaders

Multiboot 1 only, as provided by QEMU's `-kernel`. UEFI and other loaders are not implemented.
