"""Run the Rust policy tests on the host."""
import argparse
from build import CRATE
import subprocess

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gnu-efi", help=argparse.SUPPRESS)  # retained for existing callers
    parser.parse_args()
    subprocess.run(["cargo", "test", "--locked", "--manifest-path", str(CRATE), "--features", "assembly"], check=True)
