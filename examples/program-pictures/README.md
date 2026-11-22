# Pictures in the list of programs

On a display of 1280 by 720 or more, the list of programs shows a small picture in the margin beside a program this system knows by sight: Taylor's head for Tailorbird, the parrot on its board for Surfbird. Each is sixteen by sixteen squares at two pixels a square, on a dark tile of thirty-six pixels as its mark has, so that it reads the same on the dark, the teal and the light edition's field. The kernel draws it itself, from a map in [`pictures.rs`](../../PCP_OS_Rust/crates/pcpos-kernel/src/session/pictures.rs). The kernel decodes no image.

**A picture is given by digest, never by name.** It is shown only where the loader's line, taken whole, names the program's SHA-256:

    taylor_image=<SHA-256 of the Tailorbird program>
    surfbird_image=<SHA-256 of the Surfbird program>

A file that calls itself `tailorbird` and is other bytes has no picture. A loader's line that was cut gives no picture to anything. On a smaller display, where the plain list is drawn, there are no pictures.

No recipe in this repository puts these options on a loader's line yet, so no image built today shows a picture. Adding them to a recipe is for the director whose recipe it is.

## The check

From the repository root, on Linux with QEMU, after building the kernel:

    sh PCP_OS_Rust/crates/pcpos-kernel/run.sh build
    python3 -B examples/program-pictures/check.py PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 /a/new/folder

[`check.py`](check.py) states its eleven predicates before its code: the two pictures where named, nothing anywhere else, not by name, by digest when the options are exchanged, nothing on a small display, nothing on a cut line, the same tiles in all three editions' themes, and the names: the files the loader calls `tailorbird` and `surfbird` are read as Tailorbird and Surfbird in the list, and a file with any other name is read exactly as given ([`shown.rs`](../../PCP_OS_Rust/crates/pcpos-kernel/src/session/shown.rs), tested on the host by [`test_shown.rs`](../../PCP_OS_Rust/crates/pcpos-kernel/tests/test_shown.rs)). The file's own name is still what the serial line, the register and Details say. It makes its own two small files for the kernel to list and starts neither. [`FIRST_RUN.txt`](FIRST_RUN.txt) keeps the first two runs, in which one predicate failed, and the two mistakes made in the check when the names were added, and says why.

These are emulated screens at two sizes. Nothing here was shown on a physical display.
