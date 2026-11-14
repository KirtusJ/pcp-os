#!/bin/sh
# Build Doom for PCP OS: doomgeneric (GPL-2.0, https://github.com/ozkl/doomgeneric) with the platform file
# beside this script, as one statically linked, position-independent program for x86_64 with the musl C
# library. The result is a program PCP OS loads; it is not part of the kernel and is never linked with it.
# Music is Chocolate Doom's OPL player (GPL-2.0, https://github.com/chocolate-doom/chocolate-doom): three of its
# files are compiled as they are, and opl_pcpos.c beside this script stands in for its OPL library.
#   build.sh <doomgeneric source dir (the folder holding doomgeneric.c)> <chocolate-doom source dir> <output file>
# Needs: musl-gcc.
set -e
SRC="$1"; CHOC="$2"; OUT="$3"; HERE="$(cd "$(dirname "$0")" && pwd)"
OBJ="$(mktemp -d)"
FILES="dummy am_map doomdef doomstat dstrings d_event d_items d_iwad d_loop d_main d_mode d_net f_finale f_wipe g_game hu_lib hu_stuff info i_cdmus i_endoom i_joystick i_scale i_sound i_system i_timer memio m_argv m_bbox m_cheat m_config m_controls m_fixed m_menu m_misc m_random p_ceilng p_doors p_enemy p_floor p_inter p_lights p_map p_maputl p_mobj p_plats p_pspr p_saveg p_setup p_sight p_spec p_switch p_telept p_tick p_user r_bsp r_data r_draw r_main r_plane r_segs r_sky r_things sha1 sounds statdump st_lib st_stuff s_sound tables v_video wi_stuff w_checksum w_file w_main w_wad z_zone w_file_stdc i_input i_video doomgeneric mus2mid"
# The names of the folders this is built in are kept out of the program, so that the same sources give the same file anywhere.
MAP="-ffile-prefix-map=$OBJ=build -ffile-prefix-map=$HERE=pcpos -ffile-prefix-map=$SRC=doomgeneric -ffile-prefix-map=$CHOC=chocolate-doom"
CFLAGS="-O2 -fPIE -fno-strict-aliasing -DNORMALUNIX -DLINUX -D_DEFAULT_SOURCE -DFEATURE_SOUND -I$HERE -I$SRC $DOOM_CFLAGS $MAP"
for f in $FILES; do musl-gcc $CFLAGS -w -c "$SRC/$f.c" -o "$OBJ/$f.o"; done
musl-gcc $CFLAGS -Wall -c "$HERE/doomgeneric_pcpos.c" -o "$OBJ/doomgeneric_pcpos.o"
CFLAGS="$CFLAGS -DPCP_OPL_MUSIC -I$CHOC/opl"
for f in i_pcpsound opl_pcpos $DOOM_EXTRA; do musl-gcc $CFLAGS -Wall -c "$HERE/$f.c" -o "$OBJ/$f.o"; done
# Chocolate Doom's files are copied before they are compiled, so that the headers they include by name are
# doomgeneric's (the engine they are linked into) and not their neighbours in Chocolate Doom's source.
mkdir "$OBJ/choc"
cp "$CHOC/src/i_oplmusic.c" "$CHOC/src/midifile.c" "$CHOC/src/midifile.h" "$CHOC/opl/opl_queue.c" "$OBJ/choc/"
# The player is built under the name the engine's sound code looks for. One word of the copy is changed first:
# Chocolate Doom defines its music module as unchangeable, doomgeneric declares the one it looks for as
# changeable, and a C compiler will not let one name be both.
sed -i 's/^const music_module_t music_opl_module =/music_module_t music_opl_module =/' "$OBJ/choc/i_oplmusic.c"
grep -q '^music_module_t music_opl_module =' "$OBJ/choc/i_oplmusic.c"
musl-gcc $CFLAGS -w -include "$HERE/choc_compat.h" -Dmusic_opl_module=DG_music_module -c "$OBJ/choc/i_oplmusic.c" -o "$OBJ/i_oplmusic.o"
musl-gcc $CFLAGS -w -include "$HERE/choc_compat.h" -c "$OBJ/choc/midifile.c" -o "$OBJ/midifile.o"
musl-gcc $CFLAGS -w -c "$OBJ/choc/opl_queue.c" -o "$OBJ/opl_queue.o"
musl-gcc -static-pie -o "$OUT" "$OBJ"/*.o -lm
strip "$OUT"
rm -rf "$OBJ"
