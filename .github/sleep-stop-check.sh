#!/usr/bin/env bash
set -euo pipefail
out="$RUNNER_TEMP/sleep-stop"
mkdir -p "$out"
kernel=PCP_OS_Rust/crates/pcpos-kernel
parent=628f5fe039ec0dbaa451c132e8236e9fbebfce6b
candidate=23fa0b620dd870c3656480142da0a11de98355e8
build() {
  local ref=$1 variant=$2
  git checkout -q "$ref" -- "$kernel"
  python3 .github/sleep-stop-control.py "$kernel/src/boot.s" "$variant"
  bash "$kernel/run.sh" build > "$out/$3-build.log" 2>&1
  cp "$kernel/target/pcpos-kernel.elf32" "$out/$3.elf32"
}
boot() {
  local image=$1 name=$2 expect=$3 clock=$4 rc=0
  local args=()
  if [[ $clock == fixed ]]; then args=(-icount shift=2,align=off,sleep=off); fi
  timeout 90 qemu-system-x86_64 -machine pc -accel tcg -cpu max -m 64M \
    -display none -serial "file:$out/$name.serial" -no-reboot -monitor none -nic none \
    -kernel "$out/$image.elf32" -append then=off -device isa-debug-exit,iobase=0xf4,iosize=0x04 "${args[@]}" || rc=$?
  printf '%s %s\n' "$name" "$rc" | tee -a "$out/exits.txt"
  grep -E 'note stop key while asleep|SELF-TEST|FAIL ' "$out/$name.serial" || true
  [[ $rc == "$expect" ]]
  if [[ $rc == 33 ]]; then
    grep -Fq 'ALL SELF-TESTS PASS' "$out/$name.serial"
  else
    grep -Fq 'FAIL the stop key ends a program that is asleep' "$out/$name.serial"
    grep -Fq '1 SELF-TEST(S) FAILED' "$out/$name.serial"
    grep -Fq 'it is in SAFE and starts no program' "$out/$name.serial"
  fi
}
export PCPOS_KERNEL_STAGE="$RUNNER_TEMP/sleep-stage"
qemu-system-x86_64 --version | tee "$out/qemu.txt"
rustc -vV | tee "$out/rust.txt"
build "$candidate" normal healthy
for n in 1 2 3 4; do boot healthy "ordinary-$n" 33 wall; done
boot healthy fixed 33 fixed
build "$parent" delayed parent-delayed
build "$candidate" delayed candidate-delayed
boot parent-delayed parent-delayed 35 fixed
boot candidate-delayed candidate-delayed 33 fixed
build "$candidate" awake candidate-awake
boot candidate-awake candidate-awake 35 fixed
python3 - "$out" <<'PY'
import json, pathlib, re, sys
out=pathlib.Path(sys.argv[1])
summary={}
for f in sorted(out.glob("*.serial")):
    lines=f.read_text().splitlines()
    note=next(s for s in lines if 'note stop key while asleep:' in s)
    m=re.search(r'idle (\d+) of (\d+) timer periods', note)
    assert m,note
    idle,elapsed=map(int,m.groups())
    if f.stem=='parent-delayed':
        assert idle < 10 and 'Some(Stopped)' in note,note
    else:
        m=re.search(r'(\d+) calls, free frames (\d+) before and (\d+) after',note)
        assert m,note
        calls,before,after=map(int,m.groups())
        assert before==after and 'Some(Stopped)' in note,note
        if f.stem=='candidate-awake':
            assert idle==0 and calls==0 and elapsed>=100,note
        else:
            assert idle>=10 and calls==1 and elapsed<100,note
    summary[f.stem]=note.strip()
(out/'notes.json').write_text(json.dumps(summary,indent=2)+'\n')
PY
sha256sum "$out"/*.elf32 > "$out/kernel-sha256.txt"
rm "$out"/*.elf32
