#!/bin/sh
# Build the kernel's test program: one statically linked, position-independent program for x86_64 with the musl
# C library, as the kernel's loader requires.   build.sh <output file>      Needs: musl-gcc.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"; OUT="$1"
# The names of the folders this is built in are kept out of the program.
musl-gcc -O2 -fPIE -Wall -ffile-prefix-map="$HERE"=pcpos/probe -ffile-prefix-map="$(dirname "$HERE")"=pcpos -c "$HERE/probe.c" -o "$OUT.o"
musl-gcc -static-pie -o "$OUT" "$OUT.o" -lm
strip "$OUT"; rm -f "$OUT.o"
