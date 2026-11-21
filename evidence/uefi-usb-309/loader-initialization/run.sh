set -eu
prefix=$1
out=$2
cc -Wall -Wextra -Wno-unused-parameter -Werror -DGNU_EFI_USE_MS_ABI -fshort-wchar -mno-red-zone \
  -I"$prefix/include/efi" -I"$prefix/include/efi/x86_64" "$(dirname "$0")/observe.c" "$prefix/lib/libefi.a" -o "$out/observe"
"$out/observe" missing
"$out/observe" present
