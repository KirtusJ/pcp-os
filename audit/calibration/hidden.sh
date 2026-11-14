#!/bin/sh
# hidden.sh <folder for the host's stored conversations> <command> [arguments]
#
# Runs the command in a private mount namespace in which every folder named in PCP_HIDE (separated by colons) is
# an empty folder and every file named there is an empty file, and the host's own store of conversations is the folder given. Nothing outside the command's
# own process tree is changed. Needs to be run as root (it uses unshare and mount).
#
# Why: a session being measured must not be able to read the items it is measured with, their criteria, earlier
# readings or this very run's keys. In build 21 that was done by confining the session's file tools, which also
# kept two deliveries from their own files and so measured something nobody uses (Core 14.4, fifteenth round).
# Here the session keeps its tools as they are and the material is simply not there.
set -e
STORE="$1"; shift
mkdir -p "$STORE"
export PCP_STORE="$STORE"
exec unshare -m sh -c '
  IFS=:
  for d in $PCP_HIDE; do
    if [ -d "$d" ]; then mount -t tmpfs -o size=64m tmpfs "$d"; elif [ -e "$d" ]; then mount --bind /dev/null "$d"; fi
  done
  unset IFS
  mkdir -p /root/.claude/projects && mount --bind "$PCP_STORE" /root/.claude/projects
  exec "$@"' sh "$@"
