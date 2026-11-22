"""Run host tests against the production loader's C functions using simulated EFI services."""
import argparse
from pathlib import Path
import subprocess
import tempfile

here = Path(__file__).resolve().parent
p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--gnu-efi", type=Path, default=Path("/usr"))
a = p.parse_args()
with tempfile.TemporaryDirectory() as tmp:
    work = Path(tmp)
    (work/"config.h").write_text("""
#define MODEL_BYTES 14717ULL
#define SHARD_BYTES 8192ULL
#define SHARD_COUNT 2U
#define KERNEL_BASE 0x2000000U
#define KERNEL_ENTRY 0x200000cU
#define KERNEL_END 0x3000000U
#define INPUT_0_SIZE 100U
#define INPUT_1_SIZE 100U
#define INPUT_2_SIZE 100U
#define INPUT_3_SIZE 120U
#define PIN_TEXT "pin"
#define UI_TEXT "ui"
#define WORKER_TEXT "worker"
#define MEASUREMENT_TEXT "measurement"
static const UINT8 pins[4][32] = {{0}};
static const struct segment segments[] = {{KERNEL_BASE,0,100}};
""", encoding="ascii")
    include = a.gnu_efi.resolve()/"include/efi"
    binary = work/"test-loader"
    subprocess.run(["cc","-O2","-fshort-wchar","-DGNU_EFI_USE_MS_ABI","-Wall","-Wextra","-Werror",
                    "-ffunction-sections","-fdata-sections","-Wl,--gc-sections","-I"+str(include),
                    "-I"+str(include/"x86_64"),"-I"+str(work),str(here/"test_loader.c"),"-o",str(binary)],check=True)
    subprocess.run([binary],check=True)
