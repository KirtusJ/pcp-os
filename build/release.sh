#!/bin/bash
# The whole build, in order. Run from the folder that holds build/ and src/.
#   build/release.sh <dir with the three base editions as the method build made them, extracted> <cargo registry src>
# Without the method build at hand, the base editions can be recovered from a release:
#   python3 build/rebase_from_release.py <dir with a release's editions, extracted> base out/os build/assemble_release.py
# Needs: cargo with the rust-src component and the targets x86_64-unknown-linux-musl and aarch64-unknown-linux-musl (the programs'
# bytes depend on the toolchain), claude, unzip, WeasyPrint 70.0 and markdown-it-py.
set -euo pipefail
BASE="$1"; CARGO_SRC="${2:-$HOME/.cargo/registry/src}"
find src -name __pycache__ -type d -prune -exec rm -rf {} +
( cd src/os-rust && cargo build --release 2>&1 | tail -1 && ./target/release/pcpos-vectors vectors > /dev/null && cargo test 2>&1 | grep -E "^test result" | grep -v " 0 passed" && cargo build --release 2>&1 | tail -1 && ./target/release/pcpos-vectors vectors
  # The programs that are supplied: statically linked, so that they need no library from the system they run on.
  cargo build --release --bin pcpos --target x86_64-unknown-linux-musl 2>&1 | tail -1
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld cargo build --release --bin pcpos --target aarch64-unknown-linux-musl 2>&1 | tail -1 )
PROGRAM=src/os-rust/target/x86_64-unknown-linux-musl/release/pcpos
export PCP_PROGRAM_AARCH64=src/os-rust/target/aarch64-unknown-linux-musl/release/pcpos
# The second implementation against the cases the native one has just written. It needs the method packet beside it: src/ACR_1.0_Implementation.zip.
( cd src/os-python && python3 -B -m unittest discover -s TESTS 2>&1 | tail -3 | tr '\n' ' ' && echo )
python3 build/design_tokens.py src/design > /dev/null
python3 build/design_sheet.py src/design > /dev/null
PCP_VALIDATED_WITH="$(claude --version | cut -d' ' -f1)" RUSTC_VERSION="$(rustc --version | cut -d' ' -f1-2)" python3 build/build_plugins.py src/method $PROGRAM src/design out/plugins > /dev/null
for e in Personal Commercial Enterprise; do
  python3 build/cookbook_run.py out/plugins/$e/plugins/pcp-os out/plugins/cookbook.json | tail -2 | head -1
  for t in $e $e/plugins/pcp $e/plugins/pcp-os; do claude plugin validate --strict out/plugins/$t 2>&1 | tail -1; done
done
python3 build/build_machine.py src/method src/os-rust src/design out/machine > /dev/null
python3 build/check_modules.py src/method $PROGRAM src/os-python out/machine/pcp.machine.json out/plugins/Personal/plugins/pcp | head -1
for e in Personal Commercial Enterprise; do python3 build/build_os_docs.py src/os-docs src/design $(echo $e | tr a-z A-Z) out/os/$e > /dev/null; done
rm -rf release/_work release/*.zip
python3 build/assemble_release.py "$BASE" out/plugins out/machine src/design "$CARGO_SRC" release out/os > /dev/null
for e in Personal Commercial Enterprise; do ( cd release/_work/PCP_1.0_${e}_Edition && python3 -B verify_edition.py | head -1 ); ( cd release/_work/PCP_1.0_Plugins_${e}_Edition && python3 -B verify_plugins.py | head -1 ); done
# What is delivered is what installs: each archive, extracted afresh, validated, and the launcher tried with and without its permission.
python3 build/check_packaged_plugins.py release
