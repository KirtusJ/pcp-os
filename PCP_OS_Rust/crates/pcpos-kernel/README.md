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

The self-tests cover rollback of a refused group of built-in programs. For the slower frame-exhaustion test, boot with `test=frame-cutoffs` (for example, `PCPOS_APPEND="then=off test=frame-cutoffs" sh run.sh`). It reserves the frame pool, then leaves zero through seven frames available to a built-in program and checks the pool on every return.

## Machine boot report

To receive boot health without parsing the console, attach a second 16550-compatible UART (COM2, I/O 0x2F8) and select `report=json`, `report=cbor` or `report=cbor2` on the loader command line. Leave the option out for ordinary console operation. Configure the receiver to read only the second UART. The first UART still carries diagnostics and user-program output. In QEMU the order of the two `-serial` arguments selects the ports:

```sh
qemu-system-x86_64 -cpu max -m 128M -kernel target/pcpos-kernel.elf32 \
  -display none -monitor none -no-reboot -serial file:console.log -serial file:health.jsonl \
  -append 'then=off report=json' -device isa-debug-exit,iobase=0xf4,iosize=0x04
```

**JSON** emits one UTF-8 object per LF, with no terminal carriage returns. This is the text form for models and tools. **CBOR** emits a sequence of self-delimiting maps using the existing Rust R1 value encoder. Version 1 uses integer key 70 for `event`, text for the other keys, and 32-byte strings for the two SHA-256 values. JSON gives those digests in full lowercase hex. The positional CBOR form (`report=cbor2`) sends the same observations in fixed-order arrays. No register record is encoded or rehashed for this report. [CBOR Sequences, RFC 8742](https://www.rfc-editor.org/rfc/rfc8742)

A named observation carries integer `v=1` and a text `event`. A positional record starts with the integer layout version 2 and an integer event tag. Each record is a complete, definite-length CBOR item. Array lengths and positions are fixed. The values have the same types as the named observations, including the 32-byte digest strings, the system name or null, and the three possible image comparison results.

| Observation | Positional CBOR item in wire order |
|---|---|
| Boot | `[2, 0, image, match]` |
| Session entry | `[2, 1, pass, fail, system, admit]` |
| Stop | `[2, 2, pass, fail, replay, events, head]` |

Measured COM2 bytes from this kernel under QEMU are below. The admission prefix contains the boot and session items. The complete capture adds the stop item.

| Observed outcome | JSON prefix / complete | Named CBOR prefix / complete | Positional CBOR prefix / complete |
|---|---:|---:|---:|
| Admitted, image unchecked or matched | 192 / 334 | 111 / 191 | 57 / 100 |
| Held, image mismatch and SAFE | 187 / 328 | 104 / 183 | 50 / 92 |
| Admitted, requested program missing later | 192 / 334 | 111 / 191 | 57 / 100 |

The positional framing saves 91 bytes per complete capture by encoding the event tag and field order directly. All three kernel writers stream the data to COM2 from borrowed values and stack buffers. Larger counter values can change the total size in either layout. Both binary forms produced the same host verdict for each case. For example, an admitted, unchecked boot produced a 35-byte admission reply and a 34-byte final health reply, including LF. These measurements cover COM2 and the host verdict. The independent COM1 console has its own output volume.

For a binary link, select either CBOR form and use `-serial file:health.cbor` as the second serial argument. The host can then render the captured report for a model or a JSON consumer:

```sh
pcpos boot-report health.cbor > health.jsonl
```

The command detects the binary layout from the first item, requires that layout throughout, and emits each complete observation as the same canonical v1 JSON line. Check its exit status as well as the fields. A zero exit means the three-message report was structurally complete. Use `fail`, `replay`, `match` and `admit` to interpret what the kernel reported. A partial capture can yield valid earlier lines and a nonzero exit. The `head` is the digest the guest reported. Independently checking the register requires its bytes.

| Event | Fields | Meaning |
|---|---|---|
| `boot` | `image`, `match` | Measured SHA-256 of kernel code and read-only data. Match is true or false when the loader supplied an expected digest and null when it supplied none. |
| `session` | `system`, `admit`, `pass`, `fail` | Register-derived system state (or null if unavailable), whether verification allows the session to start programs, and cumulative counts at entry. Admission is eligibility, regardless of whether a program actually ran. |
| `stop` | `pass`, `fail`, `replay`, `events`, `head` | Final passed-check count and cumulative failure count, result of reopening and checking the stopped register, its event count and full head digest. Failures can also come from session or recording errors. |

A complete successful run has a complete `stop` with `fail=0` and `replay=true`. A register can replay correctly after failed verification. Missing, truncated, out-of-order or unknown-version messages give an incomplete or unsupported result. An early halt, panic, or disconnected UART can leave the stream empty or partial. The first `report=` option selects the format; an unknown value disables the report. A missing reporting port is noted on the console. For unattended boots, add `checks=brief` to omit individual successful check descriptions on COM1. Every check still runs, individual failures print immediately, and the cumulative totals print at shutdown. This option also works without COM2. Detailed diagnostics and program output remain on COM1. Run `python3 tests/boot_report.py target/pcpos-kernel.elf32` with QEMU and Python's `cbor2` installed to exercise all three encodings.

### Reading admission during a session

A controller can read a dedicated copy of COM2 from the beginning of a boot. Attach to a binary-clean byte stream in the format selected by the loader. For example, with `report=cbor2` and a named pipe connected to that stream, run

```sh
pcpos boot-report --admission cbor2 < com2.fifo
```

The receiver returns as soon as the `boot` and `session` records validate, even while the writer stays open. A typical result is `{"admit":true,"image":"unchecked"}`. Admission describes whether verification allowed the kernel to start programs at session entry. It is an observation, **not a request for host authorization**: the kernel does not wait for a COM2 acknowledgement before proceeding to its session. A program may already be running when a separate supervisor reads the verdict, and it can fail later. A future pre-execution host gate would require a separate protocol and an enforceable wait/deny path. Image values describe the kernel's comparison with the loader's expected digest. An omitted expectation gives `unchecked`. A successful comparison gives `matched`.

### Reading final health

Capture one boot through shutdown, close the capture and run:

```sh
pcpos boot-report --health json < health.jsonl
pcpos boot-report --health cbor < maps.cbor
pcpos boot-report --health cbor2 < arrays.cbor
```

Both summary modes write one JSON line and send diagnostics to stderr. They accept up to 4,096 bytes through the requested observation. Each record in either binary layout fits in a 128-byte input buffer. Select the producer’s encoding explicitly for `--admission` and `--health`. Final health requires all three records in order, both full digest fields, consistent counters and EOF. A live stream waiting for bytes can stay open indefinitely. Supervisors with a deadline should treat expiration before a verdict as unknown.

| Exit | Admission | Final health |
|---|---|---|
| 0 | `{"admit":true,"image":"unchecked"}` | `{"boot":"ok","image":"unchecked"}` |
| 1 | `"admit":false` with the image result, failure count and system state | `"boot":"failed"` with the image result, admission, failure count, replay and system state |
| 6 | `{"admit":null}` when the two records are incomplete or invalid | `{"boot":"unknown"}` when the completed capture is incomplete or invalid |
| 2 | Command usage or local I/O error | Command usage or local I/O error |

Give admission its own stream or capture branch. The receiver may read ahead and exits after reporting admission. Preserve the full capture separately for later health checks and the image and register digests. These summaries describe the emitter's observations. Keep the COM2 capture bound to one guest boot and preserve the process/VM identity outside the stream. The three records contain no shared boot identifier or challenge; they cannot independently detect a capture assembled from portions of different boots. A healthy reply is not authenticated attestation of the running image, nor independent replay of its register. Those guarantees require separate evidence or a versioned binding mechanism ([tracking issue #64](https://github.com/KirtusJ/pcp-os/issues/64)). Pass `--host /path/to/pcpos` to `tests/boot_report.py` to exercise all three encodings during interactive sessions and after shutdown.

To reproduce the capture-boundary limitation with two measured kernel images, run the following from this crate after building the kernel and release host. Choose a new output directory to retain the captures.

```sh
python3 tests/cross_boot_capture.py target/pcpos-kernel.elf32 \
  --host ../../target/release/pcpos --out cross-boot-captures
```

The experiment boots one image that matches its loader pin and then fails, and another that finishes healthy with no pin. It passes the first image's boot record and the second image's session and stop records to the receiver, preserving the original bytes. The receiver gives the same matched, healthy verdict it gives a genuine positive control. The test exercises JSON, named CBOR and positional CBOR.

### Quiet boot console for a machine receiver

With `console=quiet`, a kernel that successfully initializes its selected COM2 reporter omits routine boot diagnostics on COM1. For example, use `then=off report=cbor console=quiet` for an unattended boot. COM1 still carries the early kernel banner, individual failed checks, image mismatch details, SAFE diagnostics, fatal errors, session messages and application output. Adding `checks=brief` explicitly also prints the shutdown check totals. The checks, device setup and machine report run in each mode.

If no report format is selected or COM2 fails its initialization probe, the kernel follows the ordinary console settings. The probe checks the UART hardware. The operator is responsible for attaching a consumer and retaining the machine report. Quiet mode intentionally omits the successful image and register digests from COM1. Both are present in the complete report on COM2.

These healthy QEMU boots used one image at a fixed path and no loader-supplied application. Each run reported 71 passed checks, zero failures, 43 register events and successful replay.

| Loader options in addition to `then=off` | COM1 | COM2 | Total bytes |
|---|---:|---:|---:|
| `report=cbor` | 9,641 | 191 | 9,832 |
| `checks=brief report=cbor` | 3,257 | 191 | 3,448 |
| `console=quiet report=cbor` | 68 | 191 | 259 |
| `console=quiet report=cbor2` | 68 | 100 | 168 |

An image mismatch with the last combination produced 636 console bytes and a 92-byte report. A missing requested program after admission produced 153 and 100 bytes respectively. Console lengths vary with diagnostic text, command lines and program output. Run `python3 tests/quiet_console.py target/pcpos-kernel.elf32` to compare the modes on a built image, including a user program that writes to its console.

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
