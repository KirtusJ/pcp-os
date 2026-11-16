#!/usr/bin/env python3
"""Check an archived installed PCP OS boot transcript against its release kernel."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from check import pinned_kernel_measurement, verify_log


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=("recovery", "desktop", "diagnostic"))
    for name in ("serial", "manifest", "kernel-elf"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--expected-tokens", type=int, nargs=3,
                        help="three IDs from an independent reference for the diagnostic")
    args = parser.parse_args()
    if (args.expected_tokens is not None) != (args.mode == "diagnostic"):
        parser.error("--expected-tokens applies to, and is required for, diagnostic mode")
    try:
        raw_manifest = args.manifest.read_bytes()
        manifest = json.loads(raw_manifest)
        revision = manifest["git_commit"]
        measurement = pinned_kernel_measurement(args.kernel_elf, manifest)
        serial = args.serial.read_bytes()
        verify_log(serial, args.mode, manifest, args.expected_tokens, measurement)
    except (OSError, ValueError, KeyError, TypeError, IndexError, struct.error) as error:
        parser.exit(1, f"Transcript verification failed: {error}\n")
    receipt = {"scope": "archived-transcript", "mode": args.mode, "source_revision": revision,
               "serial_bytes": len(serial), "serial_sha256": hashlib.sha256(serial).hexdigest(),
               "manifest_sha256": hashlib.sha256(raw_manifest).hexdigest(),
               "kernel_elf_sha256": manifest["component_sha256"]["kernel"],
               "release_kernel_measurement": measurement}
    if args.expected_tokens is not None:
        receipt["expected_tokens"] = args.expected_tokens
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
