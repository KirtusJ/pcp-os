#!/usr/bin/env bash
# Build the model-free desktop from the recorded sources in a new directory.
set -euo pipefail
if [[ $# != 1 ]]; then echo "Usage: rebuild.sh NEW_OUTPUT_DIRECTORY" >&2; exit 2; fi
mkdir -- "$1"
out=$(cd -- "$1" && pwd)
here=$(cd -- "$(dirname -- "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
os_head=92623fa6759bbb4e4f27ce5aa381de213a55ae0c
bird_head=645b4c266390e51b6d5abb818dce96e6cf916d22
fetch_commit() {
  git -C "$1" fetch --quiet --no-tags --depth=1 "$2" "$3"
}
git init -q "$work/os"
git init -q "$work/bird"
fetch_commit "$work/os" https://github.com/KirtusJ/pcp-os.git "$os_head"
fetch_commit "$work/bird" https://github.com/KirtusJ/tailorbird.git "$bird_head"
guest_head=$(git -C "$work/bird" rev-parse "$bird_head:pcp-os")
fetch_commit "$work/os" https://github.com/KirtusJ/pcp-os.git "$guest_head"
curl --fail --location --silent --show-error https://raw.githubusercontent.com/KirtusJ/pcp/03d68793f754a6057d6fcde87c210a1931e53ee2/PCP-OS.iso -o "$work/assets.iso"
python3 "$here/../basic.py" --os "$work/os" --os-head "$os_head" \
  --tailorbird "$work/bird" --tailorbird-head "$bird_head" \
  --assets-iso "$work/assets.iso" --assets-sha256 93ed3b599ad7be0e1e88a64813b1f2b5299d51507df2edca691aaf6189d10d15 \
  --grub-modules /usr/lib/grub/x86_64-efi --grub-license /usr/share/common-licenses/GPL-3 \
  "$out/parts"
pin=$(sha256sum "$out/parts/inputs.json" | cut -d' ' -f1)
python3 "$here/../build.py" "$out/parts" --inputs-sha256 "$pin" "$out/pcp-os-x86_64-uefi-model-free.iso"
cp "$out/parts/inputs.json" "$out/inputs.json"
python3 "$here/package.py" "$out"
