#!/usr/bin/env python3
"""Does the list of programs show a program's picture only where the loader's line names its bytes?

    python3 examples/program-pictures/check.py <kernel.elf32> <new output directory>

The kernel lists every file the loader hands over that begins as an ELF file, so the "programs"
here are two small made-up files that are never started. Eight boots under QEMU; the launcher is
opened with Enter and read from a screen dump. P1 to P7 were fixed before the first run; the tile
and P8 were added, and fixed before their first run, when the owner asked that every theme be kept
in mind: the first drawing put the squares straight on the desktop's field, which is light in one
edition.

 P1 named Taylor   1280x720, the line carries taylor_image=<SHA-256 of the file named tailorbird>:
                   the 36 by 36 pixels at (left-44, row+2) beside that row are a dark tile holding
                   Taylor's sixteen by sixteen map at two pixels a square, two pixels in from the
                   tile's edge: every inked square exactly its colour, every other pixel of the
                   tile exactly the tile's colour.
 P2 named Surfbird the same boot carries surfbird_image=<SHA-256 of the file named surfbird>: the
                   same for Surfbird's map beside its row.
 P3 nothing else   the same two files with neither option: the frame holds no pixel of any of the
                   six colours that only the pictures use. Between this frame and P1's, every pixel
                   that differs lies inside the two 36 by 36 tiles. By the kernel's font every row
                   reads the same from the plate's first column on, and in the margin to its left,
                   in the rows the pictures stand beside, no cell reads as a letter or a digit
                   in either frame.
                   (As first written this said "every row of text reads the same", and the first run
                   failed it: the font reader gives "?" for any cell holding something that is no
                   character, the background's rings already did so in the margin, and a picture
                   changes how many such cells there are. The row's words did not change. The
                   predicate now says what was meant. A second run failed on my own mistake in
                   the check: it looked at the margin of every row and found the "Esc" of the key
                   legend at the foot. Both outputs are in FIRST_RUN.txt.)
 P4 not by name    two other files with the same two NAMES and different bytes, the line still
                   naming the first files' digests: both names are on the screen, the frame holds
                   no pixel of the six colours, and where each tile would stand the 36 by 36 pixels
                   are exactly those of the no-option frame at the same place.
 P5 by digest      the options exchanged (taylor_image= the surfbird file's digest and the reverse):
                   Taylor's map is beside the row named surfbird and Surfbird's beside tailorbird.
 P6 small display  800x600 with both options: the plain list names both programs and the frame
                   holds no pixel of the six colours.
 P7 cut line       1280x720, both options, and the line made longer than the kernel takes whole:
                   both programs are listed, the frame holds no pixel of the six colours, and
                   where each tile would stand the pixels are exactly the no-option frame's.
 P9 the controls   the "nothing drawn" reading of P4 and P7 must refuse a frame with a tile-place
                   pixel changed to a colour that is not one of the six: on copies of P4's and P7's
                   own frames, one pixel inside each tile's place is set in turn to each of the
                   pictures' four other colours (9fb4c8, 0c2033, 869da5, 10283f); every such copy
                   must be refused. (The six colours alone passed all of these: Director 2's
                   finding on pull request 420. The whole-tile comparison in P4 and P7 is the answer.)
 P8 every theme    1280x720, both options, with edition=personal (a light field) and with
                   edition=commercial (a teal field): both tiles are, pixel for pixel, the tiles of
                   P1's boot, which is the default, dark, Enterprise edition; and in each boot the
                   field's colour just outside the tile differs from the other two boots', so the
                   three themes were in fact three.

A failed predicate is printed and the run exits 1 after all boots; the frames are kept either way.
The output directory must be new. Standard library, the repository's screen reader, and QEMU.
"""
import hashlib
import runpy
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OPTICAL = runpy.run_path(str(ROOT / "examples/optical-disk-check/check.py"))
READ = runpy.run_path(str(ROOT / "examples/tester-cd/check_compact_pins.py"))["read"]

# The maps and colours as the brand gives them, written here apart from the kernel's source.
TAYLOR = ("................", ".....HHHHHH.....", "....HHHHHHHH....", "...HHHHHHHHHH...",
          "...HHHHHHHHHH...", ".HHHHHHHHHHHHH..", "....WWWWWWWW....", "...WWKWWWWWWW...",
          "SSSWWWWWWWWWW...", ".SSSWWWWWWWWW...", "....WWWWWWWW....", ".....WWWWWW.....",
          "....GGGGGGGG....", "...GGGGGGGGGG...", "..GGGGGGGGGGGG..", "..GGGGGGGGGGGG..")
TAYLOR_INK = {"H": "f2c230", "W": "f1f4e4", "K": "0c2033", "S": "9fb4c8", "G": "7fae5c"}
SURFBIRD = ("................", ".......CCCC.....", "......CCLLLL....", ".....CCLLKLLD...",
            ".....CLLLLLDDD..", "....CCLLLLL.D...", "...CCCLLLLL.....", "..CCCCLLLLL.....",
            ".DCCCCLLLL......", "..DD..LLLL......", "......D..D......", "E....DD.DD....T.",
            "EETTTTTTTTTTTTT.", ".EWWWWWWWWWWWW..", "..TTTTTTTTTTT...", "................")
SURFBIRD_INK = {"C": "36b9bf", "L": "d3dee3", "K": "102630", "D": "869da5", "T": "087f91", "E": "53e5e4", "W": "d0e3e8"}
# Colours nothing else on these screens is drawn in: three of each picture.
ONLY_PICTURES = [bytes.fromhex(c) for c in ("f2c230", "7fae5c", "f1f4e4", "36b9bf", "d3dee3", "087f91")]


def program(seed):
    """A file the kernel lists as a program: the four bytes an ELF file begins with, then filler."""
    return b"\x7fELF" + hashlib.sha256(seed).digest() * 4


def pixels_of(frame):
    magic, size, depth, pixels = frame.split(b"\n", 3)
    width, height = (int(n) for n in size.split())
    assert magic == b"P6" and depth == b"255" and len(pixels) == width * height * 3
    return width, height, pixels


def stray(frame):
    """How many pixels of the frame are in a colour only the pictures use."""
    _, _, pixels = pixels_of(frame)
    return sum(pixels[i:i + 3] in ONLY_PICTURES for i in range(0, len(pixels), 3))


TAYLOR_GROUND, SURFBIRD_GROUND = bytes.fromhex("10283f"), bytes.fromhex("102630")
TILE = 36


def nothing_drawn(frame, rows, width, reference):
    """Whether no picture stands in this frame: no pixel of the six colours anywhere, and at the
    place of each program's tile exactly the pixels the no-option frame has there."""
    if stray(frame) != 0:
        return False
    return all(tile(frame, box_of(rows, width, name)) == tile(reference, box_of(rows, width, name))
               for name in ("tailorbird", "surfbird"))


def with_pixel(frame, x, y, colour):
    """A copy of the frame with one pixel changed."""
    header_end = len(frame) - len(pixels_of(frame)[2])
    width = pixels_of(frame)[0]
    changed = bytearray(frame)
    at = header_end + (y * width + x) * 3
    changed[at:at + 3] = colour
    return bytes(changed)


def box_of(rows, width, name):
    """Where the picture of the row naming this program would be: (x, y) of its 36 by 36 tile."""
    row = next(i for i, text in enumerate(rows) if (" " + name + " ") in (text + " ") and "KiB" in text or (" " + name + " ") in (text + " ") and " B " in (text + " "))
    return width // 200 * 10 - 44, row * 20 + 2


def tile(frame, at):
    """The 36 by 36 pixels at this place, row by row."""
    width, _, pixels = pixels_of(frame)
    x0, y0 = at
    return b"".join(pixels[((y0 + y) * width + x0) * 3:((y0 + y) * width + x0 + TILE) * 3] for y in range(TILE))


def expected(tiles, inks, ground):
    """The whole tile as the brand gives it: ground, and the map two pixels in."""
    out = bytearray(ground * (TILE * TILE))
    for gy, line in enumerate(tiles):
        for gx, symbol in enumerate(line):
            if symbol != ".":
                for py in range(2):
                    i = ((2 + gy * 2 + py) * TILE + 2 + gx * 2) * 3
                    out[i:i + 6] = bytes.fromhex(inks[symbol]) * 2
    return bytes(out)


def shows(frame, at, tiles, inks, ground):
    """Whether the tile at this place is exactly the picture on its ground."""
    return tile(frame, at) == expected(tiles, inks, ground)


def boot(kernel, output, tag, size, files, line_extra):
    """One boot: open the launcher, return (rows read by the kernel's font, the frame)."""
    folder = output / tag
    folder.mkdir()
    paths = []
    for name, data in files:
        (folder / "files").mkdir(exist_ok=True)
        path = folder / "files" / name
        path.write_bytes(data)
        paths.append(str(path))
    measure = ROOT / "PCP_OS_Rust/crates/pcpos-kernel/tools/measure.py"
    pin = subprocess.check_output([sys.executable, str(measure), str(kernel)], text=True).strip()
    command = f"image={pin} desktop={size} ui_scale=1" + line_extra
    (folder / "command-line.txt").write_text(command + "\n")
    invocation = ["qemu-system-x86_64", "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "256M",
                  "-kernel", kernel.name, "-initrd", ",".join(paths), "-append", command,
                  "-vga", "std", "-display", "none", "-nic", "none", "-serial", "file:" + str(folder / "serial.txt"),
                  "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "stdio"]
    with subprocess.Popen(invocation, stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=kernel.parent) as vm:
        try:
            monitor = OPTICAL["Monitor"](vm)
            monitor.call("qmp_capabilities")
            screens = OPTICAL["DesktopScreens"](monitor, folder, 30)
            screens.read = READ
            screens.wait("home", lambda rows: any("Programs" in r for r in rows))
            monitor.key("ret")
            time.sleep(0.15)
            names = [name for name, _ in files]
            rows = screens.wait("programs", lambda rows: all(any(n in r for r in rows) for n in names))
            time.sleep(0.5)
            rows = screens.wait("programs-settled", lambda rows: all(any(n in r for r in rows) for n in names))
            (folder / "programs.txt").write_text("\n".join(rows) + "\n")
            return rows, (folder / "programs-settled.ppm").read_bytes()
        finally:
            vm.kill()


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    kernel, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    taylor, surf = program(b"the tailorbird file"), program(b"the surfbird file")
    other_t, other_s = program(b"another file named tailorbird"), program(b"another file named surfbird")
    dt, ds = hashlib.sha256(taylor).hexdigest(), hashlib.sha256(surf).hexdigest()
    files = [("tailorbird", taylor), ("surfbird", surf)]
    both = f" taylor_image={dt} surfbird_image={ds}"
    failed = []

    def predicate(name, holds, detail=""):
        print(("PASS " if holds else "FAIL ") + name + (" " + detail if detail else ""))
        if not holds:
            failed.append(name)

    rows, named = boot(kernel, output, "named", "1280x720", files, both)
    width = pixels_of(named)[0]
    at_t, at_s = box_of(rows, width, "tailorbird"), box_of(rows, width, "surfbird")
    predicate("P1 named Taylor", shows(named, at_t, TAYLOR, TAYLOR_INK, TAYLOR_GROUND), f"at {at_t}")
    predicate("P2 named Surfbird", shows(named, at_s, SURFBIRD, SURFBIRD_INK, SURFBIRD_GROUND), f"at {at_s}")

    plain_rows, plain = boot(kernel, output, "no-option", "1280x720", files, "")
    _, _, a = pixels_of(named)
    _, _, b = pixels_of(plain)
    inside = lambda x, y: any(bx <= x < bx + TILE and by <= y < by + TILE for bx, by in (at_t, at_s))
    differing = [(i % width, i // width) for i in range(len(a) // 3) if a[i * 3:i * 3 + 3] != b[i * 3:i * 3 + 3]]
    outside = [p for p in differing if not inside(*p)]
    plate = width // 200 * 10 // 10
    same_words = [r[plate:] for r in rows] == [r[plate:] for r in plain_rows]
    beside = sorted({y // 20 for _, by in (at_t, at_s) for y in (by, by + TILE - 1)})
    margin = "".join(view[r][:plate] for view in (rows, plain_rows) for r in beside)
    no_letters = set(margin) <= set(" ?")
    predicate("P3 nothing else", stray(plain) == 0 and not outside and bool(differing) and same_words and no_letters,
              f"stray {stray(plain)}, differing {len(differing)}, outside the boxes {len(outside)}, "
              f"words equal from column {plate} {same_words}, margin cells read as {sorted(set(margin))}")

    other_rows, other = boot(kernel, output, "same-names-other-bytes", "1280x720",
                             [("tailorbird", other_t), ("surfbird", other_s)], both)
    listed = all(any(n in r for r in other_rows) for n in ("tailorbird", "surfbird"))
    predicate("P4 not by name", listed and nothing_drawn(other, other_rows, width, plain), f"listed {listed}, stray {stray(other)}")

    swap_rows, swapped = boot(kernel, output, "exchanged", "1280x720", files, f" taylor_image={ds} surfbird_image={dt}")
    st, ss = box_of(swap_rows, width, "tailorbird"), box_of(swap_rows, width, "surfbird")
    predicate("P5 by digest", shows(swapped, ss, TAYLOR, TAYLOR_INK, TAYLOR_GROUND) and shows(swapped, st, SURFBIRD, SURFBIRD_INK, SURFBIRD_GROUND)
              and not shows(swapped, st, TAYLOR, TAYLOR_INK, TAYLOR_GROUND))

    small_rows, small = boot(kernel, output, "small", "800x600", files, both)
    listed = all(any(n in r for r in small_rows) for n in ("tailorbird", "surfbird"))
    predicate("P6 small display", listed and stray(small) == 0, f"listed {listed}, stray {stray(small)}")

    cut_rows, cut = boot(kernel, output, "cut-line", "1280x720", files, both + " padding=" + "x" * 1200)
    listed = all(any(n in r for r in cut_rows) for n in ("tailorbird", "surfbird"))
    predicate("P7 cut line", listed and nothing_drawn(cut, cut_rows, width, plain), f"listed {listed}, stray {stray(cut)}")

    refused = tried = 0
    for frame, view in ((other, other_rows), (cut, cut_rows)):
        for name in ("tailorbird", "surfbird"):
            bx, by = box_of(view, width, name)
            for colour in ("9fb4c8", "0c2033", "869da5", "10283f"):
                tried += 1
                refused += not nothing_drawn(with_pixel(frame, bx + 4, by + 3, bytes.fromhex(colour)), view, width, plain)
    predicate("P9 the controls", tried == 16 and refused == 16, f"{refused} of {tried} changed frames refused")

    def beside(frame, at):
        width_, _, pixels = pixels_of(frame)
        i = ((at[1] + TILE // 2) * width_ + at[0] - 4) * 3
        return pixels[i:i + 3]

    fields, same = {beside(named, at_t)}, True
    for edition in ("personal", "commercial"):
        e_rows, e_frame = boot(kernel, output, "edition-" + edition, "1280x720", files, both + " edition=" + edition)
        et, es = box_of(e_rows, width, "tailorbird"), box_of(e_rows, width, "surfbird")
        same = same and tile(e_frame, et) == tile(named, at_t) and tile(e_frame, es) == tile(named, at_s)
        fields.add(beside(e_frame, et))
    predicate("P8 every theme", same and len(fields) == 3, f"tiles equal {same}, fields beside the tile {sorted(f.hex() for f in fields)}")

    if failed:
        sys.exit("failed: " + ", ".join(failed))
    print("all nine hold")


if __name__ == "__main__":
    main()
