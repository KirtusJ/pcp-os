#!/usr/bin/env python3
"""Build the Ultra Omega Barrel Forklift user-mode ELF on x86-64 Linux."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile


FILES = """
dummy am_map doomdef doomstat dstrings d_event d_items d_iwad d_loop d_main
d_mode d_net f_finale f_wipe g_game hu_lib hu_stuff info i_cdmus i_endoom
i_joystick i_scale i_sound i_system i_timer memio m_argv m_bbox m_cheat m_config
m_controls m_fixed m_menu m_misc m_random p_ceilng p_doors p_enemy p_floor
p_inter p_lights p_map p_maputl p_mobj p_plats p_pspr p_saveg p_setup
p_sight p_spec p_switch p_telept p_tick p_user r_bsp r_data r_draw r_main
r_plane r_segs r_sky r_things sha1 sounds statdump st_lib st_stuff s_sound
tables v_video wi_stuff w_checksum w_file w_main w_wad z_zone w_file_stdc
i_input i_video doomgeneric mus2mid omega fork fork_ui
""".split()


def validate_elf(path):
    data = path.read_bytes()
    if data[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<HH", data, 16) != (3, 62):
        raise ValueError("Build requires an x86-64 little-endian PIE.")
    offset = struct.unpack_from("<Q", data, 32)[0]
    stride, count = struct.unpack_from("<HH", data, 54)
    for index in range(count):
        kind, flags = struct.unpack_from("<II", data, offset + index * stride)
        if kind == 3 or (kind == 1 and flags & 3 == 3):
            raise ValueError("ELF requests a dynamic loader or a writable executable segment.")
    dynamic = subprocess.check_output(["readelf", "-dW", str(path)], text=True)
    if "(NEEDED)" in dynamic:
        raise ValueError("ELF has a shared-library dependency.")


def build(args, work):
    src, choc, port = [path.resolve() for path in (args.engine, args.chocolate, args.port)]
    options = ["-O2", "-fPIE", "-fno-strict-aliasing", "-DNORMALUNIX", "-DLINUX",
               "-D_DEFAULT_SOURCE", "-DFEATURE_SOUND", "-I" + str(port), "-I" + str(src)]
    for route, label in [(work, "build"), (port, "pcpos"), (src, "doomgeneric"), (choc, "chocolate-doom")]:
        options.append(f"-ffile-prefix-map={route}={label}")
    sources = [(src / (stem + ".c"), ["-w"]) for stem in FILES]
    sources.append((port / "doomgeneric_pcpos.c", ["-Wall"]))
    music = ["-DPCP_OPL_MUSIC", "-I" + str(choc / "opl")]
    sources.extend((port / (stem + ".c"), music + ["-Wall"]) for stem in ("i_pcpsound", "opl_pcpos"))
    # Place these files together so Chocolate Doom's code includes the engine's common headers.
    for filename in ("i_oplmusic.c", "midifile.c", "midifile.h"):
        shutil.copyfile(choc / "src" / filename, work / filename)
    shutil.copyfile(choc / "opl" / "opl_queue.c", work / "opl_queue.c")
    opl = work / "i_oplmusic.c"
    body, count = re.subn(r"(?m)^const music_module_t music_opl_module =",
                          "music_module_t music_opl_module =", opl.read_text())
    if count != 1:
        raise ValueError("Unexpected OPL source. Use the Chocolate Doom revision named in BUILD.txt.")
    opl.write_text(body)
    compat = music + ["-w", "-include", str(port / "choc_compat.h")]
    sources.extend([(opl, compat + ["-Dmusic_opl_module=DG_music_module"]),
                    (work / "midifile.c", compat), (work / "opl_queue.c", music + ["-w"])])

    def compile_one(source_options):
        source, extra = source_options
        obj = work / (source.stem + ".o")
        subprocess.run([args.cc, *options, *extra, "-c", str(source), "-o", str(obj)], check=True)
        return obj

    with ThreadPoolExecutor(max_workers=args.jobs) as workers:
        objects = sorted(workers.map(compile_one, sources))
    binary = work / "omega.elf"
    subprocess.run([args.cc, "-static-pie", "-o", str(binary), *map(str, objects), "-lm"], check=True)
    validate_elf(binary)
    if args.symbols is not None:
        shutil.copy2(binary, args.symbols)
    subprocess.run(["strip", str(binary)], check=True)
    shutil.copy2(binary, args.output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True, help="patched directory containing omega.c")
    parser.add_argument("--chocolate", type=Path, required=True, help="directory containing src/ and opl/")
    parser.add_argument("--port", type=Path, required=True, help="directory containing doomgeneric_pcpos.c")
    parser.add_argument("--cc", default="musl-gcc", help="x86-64 musl compiler with static PIE support")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--symbols", type=Path, help="also keep an unstripped ELF at this path")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    try:
        with tempfile.TemporaryDirectory() as work:
            build(args, Path(work))
    except (ValueError, OSError, subprocess.CalledProcessError) as exc:
        parser.exit(1, f"{parser.prog}: {exc}\n")
    print(f"Built {args.output}")


if __name__ == "__main__":
    main()
