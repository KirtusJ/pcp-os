#!/usr/bin/env bash
set -euo pipefail
mode=$1
out="$RUNNER_TEMP/receipts"
export PCPOS_KERNEL_STAGE="$RUNNER_TEMP/kernel-$mode"
bash run.sh build
sha256sum target/pcpos-kernel.elf32 > "$out/$mode-elf.sha256"
boot() {
  local tag=$1 cpu=$2 rc=0
  shift 2
  local log="$out/$tag.serial"
  timeout 90 qemu-system-x86_64 -accel tcg -cpu "$cpu" -m 128M \
    -kernel target/pcpos-kernel.elf32 -append then=off -display none \
    -monitor none -nic none -no-reboot -serial "file:$log" \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 "$@" || rc=$?
  echo "$tag $rc" | tee -a "$out/exits.txt"
  grep -E 'note key while waiting|note stop key while waiting|SELF-TEST|SAFE' "$log" || true
  if [[ $mode == healthy ]]; then
    [[ $rc == 33 ]]
    grep -Eq 'note key while waiting: .*Exited\(0\).* 8 calls' "$log"
    grep -Eq 'note stop key while waiting for a key: .*Stopped.* 6 calls' "$log"
    grep -Fq 'ALL SELF-TESTS PASS' "$log"
  else
    [[ $rc == 35 ]]
    grep -Fq 'SELF-TEST(S) FAILED' "$log"
    grep -Eiq 'fail[^[:alpha:]].*a program waiting in a read of the keyboard' "$log"
  fi
}
if [[ $mode == healthy ]]; then
  for n in 1 2 3 4; do boot "ordinary-max-$n" max; done
  boot ordinary-qemu64 qemu64
  boot fixed-max max -icount shift=2,align=off,sleep=off
else
  boot reversed-fixed-max max -icount shift=2,align=off,sleep=off
fi
