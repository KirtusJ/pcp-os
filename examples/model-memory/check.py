#!/usr/bin/env python3
"""Memory a loader filled with a model is kept out of the pool and given, read-only, to one program.

A model is several gibibytes. The kernel has no driver for the stick it boots from, so a loader
stage reads the model's files through the firmware into memory it allocates above four gibibytes.
It marks that memory in the map it hands over with a kind of its own (0x80000001: ordinary memory,
that loader's, holding a model), and names it on the loader's line
(`model_memory=0xADDRESS:0xBYTES[,...]`, at most eight pieces, in the model's own order). The map
says whose the memory is; the line only says which of it is the model and in what order. With the
`model-memory` feature the kernel gives memory so marked and so named to the companion program
alone: mapped readable, never writable or executable, and sealed so that no later call changes
that. It is never in the pool the kernel hands out (the kind is not "usable"), and the kernel maps
it nowhere itself. The model's manifest comes as a loader file named `model.manifest`, and its
SHA-256 on the line (`model_manifest=`); the kernel gives nothing unless all agree. The companion
checks the model's bytes itself.

Here the emulator plays the loader stage: `-device loader` puts two files at two addresses, and
because an emulator's loader cannot mark its map, the kernel is built with `model-memory-fixture`
and the line's `fixture_model_ram=` stands for the marking (the kernel adds those pieces to a copy
of the map as that kind and then reads the copy as it would a loader's map, and says at start that
it did). That feature is for this check only and belongs in no release. The "model" is 41,948,040
bytes whose value at each offset is known, so every byte can be checked.
probe/ is built four ways: model-front, a model-front that takes memory (GRAB=1), model-back
(BACK=1) and a model-back that writes (BACK=1 WRITE=1).

Predicates, fixed before the first run.
 M1 held: the kernel says a model of exactly that many bytes is held. The companion opens it, is
    told the manifest's length, the model's length and the pinned digest; reads the manifest whole
    (length and a position-weighted sum agree with the file); maps the model and reads every byte:
    none differs. The model lies in two pieces at two addresses, the second ending partway through
    a page.
 M2 what must not work, does not: the other program's open is -13; a write to the model -9; a
    mapping that is writable, executable, a byte short, a page long, or a second one, -22; another
    request -25; making a page of it writable or executable afterwards fails and its first byte is
    still the model's; its memory is not taken as a buffer by the kernel (-14).
 M3 out of the pool: with nothing marked, the kernel reports W frames above four gibibytes; with
    the model's memory marked it reports W less the pieces counted in whole 2 MiB pages, and no more.
    A front that first takes all the memory the kernel will give and writes to every page of it
    takes all of those frames, and the companion then still reads every byte of the model unchanged.
 M4 a companion that stores into the model is ended by the processor's refusal (vector 14) at that
    store; the other program goes on to its end; the kernel does not stop.
 M5 nothing is given unless all agrees, and the kernel says why: memory named on the line that
    the map does not mark (not marked at all, or other memory marked); a piece not on a page, below
    four gibibytes, at the end of all addresses (the numbers C3 found stopped the kernel), two
    pieces sharing memory, the word given twice or not in hexadecimal; no manifest file, no pin, a
    pin that is not the file's digest or given twice, a manifest of another length. Memory that is
    marked stays out of the pool whatever the line says; memory that is only named stays in it. In
    every such case the companion's open is -2.
 M6 with a model held and no companion named, no program can open it (-13).
 M7 every run ends with status 33, 71 self-tests pass, none fails, and every frame of both pools
    is free at the end.

Usage: check.py <kernel.elf32 built with model-memory-fixture> <model-front> <taking model-front> <model-back> <writing model-back>
This is an emulator standing in for a loader, and test data standing in for a model.
"""
import argparse
import hashlib
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

BYTES = 40 * 1024 * 1024 + 5000
FIRST = 24 * 1024 * 1024
AT = (0x1_4000_0000, 0x1_4800_0000)
SHARD = 16 * 1024 * 1024
BIG = 2 * 1024 * 1024


def pattern(count):
    mask = (1 << 64) - 1
    return bytes((((i * 0x9E3779B97F4A7C15) & mask) >> 56) ^ ((i >> 12) & 0xFF) for i in range(count))


def manifest_of(data):
    shards = [data[i:i + SHARD] for i in range(0, len(data), SHARD)]
    return (b"PCPMDL01" + struct.pack("<QII", len(data), SHARD, len(shards)) + hashlib.sha256(data).digest()
            + b"".join(hashlib.sha256(s).digest() for s in shards))


def weighted(data):
    total = 0
    for place, byte in enumerate(data, 1):
        total = (total + place * (byte + 1)) % 1_000_000_007
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("kernel", "front", "taking", "back", "writing"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    args = parser.parse_args()
    kernel, front, taking, back, writing = (p.resolve() for p in (args.kernel, args.front, args.taking, args.back, args.writing))
    if not (front.name == taking.name == "model-front" and back.name == writing.name == "model-back"):
        sys.exit("the programs must be in files named model-front and model-back")
    work = Path(tempfile.mkdtemp(prefix="model-memory-"))
    data = pattern(BYTES)
    manifest = manifest_of(data)
    (work / "model.manifest").write_bytes(manifest)
    (work / "part1").write_bytes(data[:FIRST])
    (work / "part2").write_bytes(data[FIRST:])
    other = work / "other"
    other.mkdir()
    (other / "model.manifest").write_bytes(manifest_of(data[:-1]))
    pin = hashlib.sha256(manifest).hexdigest()
    pieces_word = "0x%x:0x%x,0x%x:0x%x" % (AT[0], FIRST, AT[1], BYTES - FIRST)
    marked = "fixture_model_ram=" + pieces_word
    memory = marked + " model_memory=" + pieces_word
    fixture_words = "FIXTURE, NOT FOR ANY RELEASE: the loader's line (fixture_model_ram=), not the loader's map, marks memory as holding a model in this boot"
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    problems = []

    def boot(name, front_file, back_file, words, manifest_file=work / "model.manifest", worker=True):
        files = [front_file, back_file] + ([manifest_file] if manifest_file else [])
        append = "auto=model-front then=off " + ("worker_for=%s worker_image=%s " % (digest(front_file), digest(back_file)) if worker else "") + words
        try:
            done = subprocess.run(
                [args.qemu, "-machine", "q35", "-accel", "tcg", "-cpu", "max", "-m", "6G", "-kernel", kernel.name, "-initrd", ",".join(str(f) for f in files),
                 "-append", append, "-device", "loader,file=%s,addr=0x%x,force-raw=on" % (work / "part1", AT[0]),
                 "-device", "loader,file=%s,addr=0x%x,force-raw=on" % (work / "part2", AT[1]),
                 "-display", "none", "-nic", "none", "-serial", "stdio", "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=kernel.parent, timeout=600)
            status, lines = done.returncode, done.stdout.decode(errors="replace").splitlines()
        except subprocess.TimeoutExpired as late:
            status, lines = None, (late.stdout or b"").decode(errors="replace").splitlines()
        passes = sum(1 for line in lines if line.startswith("  pass "))
        if status != 33 or passes != 71 or any(line.startswith("  FAIL") for line in lines):
            problems.append("%s: status %r, %d passes, or a failure" % (name, status, passes))
        if (fixture_words in lines) != ("fixture_model_ram=" in words):
            problems.append("%s: the kernel did not say the fixture was in use exactly when it was" % name)
        low = [m.groups() for line in lines if (m := re.search(r"frames: (\d+) of (\d+) free", line))]
        high = [tuple(int(g) for g in m.groups()) for line in lines
                if (m := re.match(r"memory above four gibibytes: (\d+) of (\d+) frames free, (\d+) handed out and (\d+) given back", line))]
        if len(low) != 1 or low[0][0] != low[0][1] or len(high) != 1 or high[0][0] != high[0][1] or high[0][2] != high[0][3]:
            problems.append("%s: not every frame of both pools is free at the end: %r %r" % (name, low, high))
        said = lambda who: [line[len(who):] for line in lines if line.startswith(who)]
        return lines, said("front: "), said("back: "), (high[0] if len(high) == 1 else (0, 0, 0, 0))

    def require(name, got, wanted):
        if got != wanted:
            problems.append("%s: said %r, not %r" % (name, got, wanted))

    held_words = "a model of %d bytes is held in memory, out of the pool, for a companion to read; its manifest (%d bytes) is the one the loader's line pins" % (BYTES, len(manifest))
    back_reads = ["opening the model answered 0", "waiting for the front answered 1", "asking what is held answered 0", "asking something else of it answered -25",
                  "manifest bytes %d" % len(manifest), "model bytes %d" % BYTES, "sum of the pinned digest's bytes %d" % sum(hashlib.sha256(manifest).digest()),
                  "the read after the manifest's end answered 0", "manifest bytes read %d" % len(manifest), "weighted sum of the manifest %d" % weighted(manifest),
                  "a write to the model answered -9", "mapping it writable answered -22", "mapping it executable answered -22", "mapping one byte less answered -22",
                  "mapping a page more answered -22", "mapping it readable answered 0", "mapping it a second time answered -22"]
    back_whole = back_reads + ["model bytes read %d" % BYTES, "of those, bytes that were not the model's 0"]
    after = ["its first byte is still the model's 1", "writing the console from the model's memory answered -14"]
    front_plain = ["opening the model answered -13", "telling the companion to go on answered 1", "the companion ended and the channel answered 0"]

    # M5 first, for W: the pool with no model named.
    lines, said_front, said_back, whole = boot("no model named", front, back, "")
    require("no model named, back", said_back, ["opening the model answered -2", "waiting for the front answered 1"])
    require("no model named, front", said_front, front_plain)
    if any("model" in line and "is held" in line or "no model is given" in line for line in lines):
        problems.append("no model named: the kernel spoke of a model")
    pieces = sum(((at + size + BIG - 1) // BIG - at // BIG) * (BIG // 4096) for at, size in ((AT[0], FIRST), (AT[1], BYTES - FIRST)))
    less = whole[1] - pieces
    print("no model named: %d frames above four gibibytes; the model's pieces are %d frames in whole 2 MiB pages" % (whole[1], pieces))

    # M1, M2.
    lines, said_front, said_back, high = boot("held", front, back, "%s model_manifest=%s" % (memory, pin))
    if held_words not in lines:
        problems.append("held: the kernel did not say the model is held")
    require("held, front", said_front, front_plain)
    if said_back[:len(back_whole)] != back_whole or said_back[-2:] != after or len(said_back) != len(back_whole) + 4 \
            or not all(re.fullmatch(r"making its first page (writable|executable) answered -\d+", line) for line in said_back[len(back_whole):-2]):
        problems.append("held, back: said %r" % said_back)
    if high[1] != less:
        problems.append("M3: with the model held the pool above four gibibytes is %d frames, not %d" % (high[1], less))
    print("held: " + "; ".join(said_back[-8:]))

    # M3.
    lines, said_front, said_back, high = boot("the front takes all memory first", taking, back, "%s model_manifest=%s" % (memory, pin))
    taken = [int(line.rsplit(" ", 1)[1]) for line in said_front if line.startswith("mebibytes taken and written before the companion goes on ")]
    if len(taken) != 1 or high[1] != less or high[2] != less or said_back[:len(back_whole)] != back_whole:
        problems.append("M3: the front took %r MiB, the pool above was %r, or the companion did not read the model whole and unchanged: %r" % (taken, high, said_back[-6:]))
    print("the front takes all memory first: %r MiB taken; all %d high frames handed out; model bytes that differ: %s" % (taken, high[2], said_back[len(back_whole) - 1:len(back_whole)]))

    # M4.
    lines, said_front, said_back, _ = boot("a companion that writes", front, writing, "%s model_manifest=%s" % (memory, pin))
    if said_back[-1:] != ["storing a byte into the model"] or any("WENT THROUGH" in line for line in lines) \
            or not any(line.startswith("companion model-back: the processor refused something it did (vector 14)") for line in lines):
        problems.append("M4: the store was not refused by the processor: %r" % said_back[-3:])
    require("a companion that writes, front", said_front, front_plain)

    # M5.
    kept = "the memory marked for it stays out of the pool and is given to nothing"
    wrong_pin = pin[:-1] + ("0" if pin[-1] != "0" else "1")
    usual = work / "model.manifest"
    with_pin = lambda named: "%s model_memory=%s model_manifest=%s" % (marked, named, pin)
    for name, words, manifest_file, why, pool in (
            ("named and not marked", "model_memory=%s model_manifest=%s" % (pieces_word, pin), usual, "is not given (a piece is not wholly memory the loader's map marks as holding a model", whole[1]),
            ("other memory marked", "fixture_model_ram=0x%x:0x%x model_memory=%s model_manifest=%s" % (0x1_8000_0000, BYTES, pieces_word, pin), usual,
             "is not given (a piece is not wholly memory the loader's map marks as holding a model", whole[1] - ((0x1_8000_0000 + BYTES + BIG - 1) // BIG - 0x1_8000_0000 // BIG) * (BIG // 4096)),
            ("more named than marked", with_pin("0x%x:0x%x,0x%x:0x%x" % (AT[0], FIRST + 0x1000, AT[1], BYTES - FIRST)), usual, "is not given (a piece is not wholly memory", less),
            ("a piece not on a page", with_pin("0x%x:0x%x" % (AT[0] + 0x800, FIRST - 0x800)), usual, "is not given (a piece does not begin on a page", less),
            ("a piece below four gibibytes", with_pin("0x%x:0x%x" % (0x4000_0000, BYTES)), usual, "is not given (a piece lies outside", less),
            ("a piece at the end of all addresses", with_pin("0xfffffffffffff000:0xfff"), usual, "is not given (a piece lies outside", less),
            ("a piece that wraps", with_pin("0xfffffffffffff000:0x2000"), usual, "is not given (a piece runs past the end of all addresses", less),
            ("two pieces sharing memory", with_pin("0x%x:0x%x,0x%x:0x%x" % (AT[0], FIRST, AT[0] + 0x1000, 0x1000)), usual, "is not given (two pieces share memory", less),
            ("the memory named twice", "%s model_memory=%s model_manifest=%s" % (memory, pieces_word, pin), usual, "is not given (model_memory is given more than once", less),
            ("not hexadecimal", with_pin("%d:%d" % (AT[0], BYTES)), usual, "is not given (model_memory is not address:bytes", less),
            ("no manifest file", "%s model_manifest=%s" % (memory, pin), None, "the loader gave no file named model.manifest; " + kept, less),
            ("no pin", memory, usual, "model_manifest is not given; " + kept, less),
            ("a pin that is not the manifest's digest", "%s model_manifest=%s" % (memory, wrong_pin), usual, "is not the manifest the loader's line pins", less),
            ("a pin given twice", "%s model_manifest=%s model_manifest=%s" % (memory, pin, pin), usual, "model_manifest is given more than once", less),
            ("a manifest of another length", "%s model_manifest=%s" % (memory, digest(other / "model.manifest")), other / "model.manifest",
             "the manifest is of a model of %d bytes and the memory named holds %d" % (BYTES - 1, BYTES), less),
            ("a pin and no memory", "model_manifest=%s" % pin, usual, "model_manifest is given and no model_memory", whole[1])):
        lines, said_front, said_back, high = boot(name, front, back, words, manifest_file)
        said = [line for line in lines if line.startswith("no model is given to any program: ")]
        if len(said) != 1 or why not in said[0] or any("is held in memory" in line for line in lines):
            problems.append("%s: the kernel said %r, not why (%r)" % (name, said, why))
        require(name + ", back", said_back, ["opening the model answered -2", "waiting for the front answered 1"])
        require(name + ", front", said_front, front_plain)
        if high[1] != pool:
            problems.append("%s: the pool above four gibibytes is %d frames, not %d" % (name, high[1], pool))
        print("%s: %s" % (name, said[0][len("no model is given to any program: "):] if said else "nothing said"))

    # M6.
    lines, said_front, said_back, _ = boot("held, no companion named", front, back, "%s model_manifest=%s" % (memory, pin), worker=False)
    if held_words not in lines or said_front != ["opening the model answered -13"] or said_back or any(line.startswith("companion ") or "is started with its companion" in line for line in lines):
        problems.append("M6: with no companion named the model was opened, or a companion ran: %r %r" % (said_front, said_back))

    if problems:
        for problem in problems:
            print("FAIL " + problem, file=sys.stderr)
        sys.exit(1)
    print("model memory: held out of the pool, read whole and unchanged by the companion alone, never writable; nothing given unless memory, manifest and pin agree")


if __name__ == "__main__":
    main()
