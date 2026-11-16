"""Opt-in v2 marker/boot-ID mount guard, tested only on disposable QEMU IDE images.

Expected improvement: an accidental mismatch of *different* 128-bit markers
on the hard-wired IDE primary master fails before the guest can commit data.

Expected residual adverse case: cloning the entire public volume marker and
ID still causes a wrong-disk write. IDs are not secrets or authenticated
device identity. The fixture never formats or opens a physical disk.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import tempfile

from store_probe import format_disk, make_guests
from store_role_probe import (
    gpt_like_disk, qemu_case, verified_disk_read, digest, DISK_BYTES,
)

ID_A = "0123456789abcdeffedcba9876543210"
ID_B = "1123456789abcdeffedcba9876543211"
MARKER_V2 = b"PCPOS USER STORE 2\n"
ID_OFFSET = 32


def format_bound(path: Path, selected_id: str) -> None:
    # The test fixture deliberately uses deterministic, public IDs.
    # Provisioning real IDs is NOT implemented here.
    if len(selected_id) != 32:
        raise ValueError("fixture marker must be exactly 128 bits")
    ident = bytes.fromhex(selected_id)
    if not any(ident):
        raise ValueError("zero marker not allowed")
    format_disk(path)
    data = bytearray(path.read_bytes())
    data[:512] = bytes(512)
    data[:len(MARKER_V2)] = MARKER_V2
    data[ID_OFFSET:ID_OFFSET + len(ident)] = ident
    path.write_bytes(data)


def replay(args: argparse.Namespace, directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=False)
    make_guests(directory, args.clang, args.objcopy)
    kernel = args.kernel.read_bytes()
    print("bound_profile_kernel_sha256", hashlib.sha256(kernel).hexdigest(), flush=True)

    intended = directory / "intended-v2-store.raw"
    other_id = directory / "wrong-id-v2-store.raw"
    same_id = directory / "copied-id-v2-store.raw"
    old_v1 = directory / "legacy-v1-store.raw"
    gpt = directory / "non-store-gpt-sentinel.raw"
    format_bound(intended, ID_A)
    format_bound(other_id, ID_B)
    format_bound(same_id, ID_A)
    format_disk(old_v1)
    gpt_like_disk(gpt)

    initial_intended = verified_disk_read(intended)
    initial_wrong = verified_disk_read(other_id)
    initial_copy = verified_disk_read(same_id)
    initial_v1 = verified_disk_read(old_v1)
    initial_gpt = verified_disk_read(gpt)
    assert initial_intended == initial_copy, "same-ID copy negative fixture is not an exact clone"
    assert initial_intended != initial_wrong, "different ID fixture unexpectedly identical"

    good_arg = " store_id=" + ID_A
    qemu_case(args, directory, "v2-primary-mount-and-fsync",
              [(0, intended), (1, gpt)], 1, 33, boot_suffix=good_arg)
    committed = verified_disk_read(intended)
    assert committed != initial_intended and b"oldbytes" in committed
    assert verified_disk_read(gpt) == initial_gpt, "unrelated GPT-like disk changed"
    qemu_case(args, directory, "v2-cold-boot-saved-data",
              [(0, intended), (1, gpt)], 2, 33, boot_suffix=good_arg)
    assert verified_disk_read(intended) == committed
    print("POSITIVE OPT-IN SCOPE: exact nonzero ID permits read/write and cold-read", flush=True)

    # Different ID carried by *wrong primary* cannot be treated as selected.
    qemu_case(args, directory, "v2-wrong-primary-id",
              [(0, other_id), (1, intended)], 1, 35, boot_suffix=good_arg)
    assert verified_disk_read(other_id) == initial_wrong
    assert verified_disk_read(intended) == committed
    print("SAFE REFUSAL: v2 wrong ID on selected IDE0, both disks unchanged", flush=True)

    # The correct physical port with a wrong requested ID must not write.
    qemu_case(args, directory, "v2-wrong-boot-selector",
              [(0, intended), (1, other_id)], 1, 35,
              boot_suffix=" store_id=" + ID_B)
    assert verified_disk_read(intended) == committed
    assert verified_disk_read(other_id) == initial_wrong

    # Missing, malformed, duplicate and noncanonical selectors must not
    # downgrade to legacy unpinned mounting or run the action.
    rejected = (
        ("v2-missing-identity-token", ""),
        ("v2-duplicate-identity", good_arg + good_arg),
        ("v2-upper-identity", " store_id=" + ID_A.upper()),
        ("v2-short-identity", " store_id=" + ID_A[:-2]),
        ("v2-zero-identity", " store_id=" + "0" * 32),
        ("v2-ambiguous-store", good_arg + " store=another"),
    )
    for label, suffix in rejected:
        qemu_case(args, directory, label,
                  [(0, intended), (1, gpt)], 1, 35, boot_suffix=suffix)
        assert verified_disk_read(intended) == committed, label + " modified selected disk"
        assert verified_disk_read(gpt) == initial_gpt, label + " modified sentinel"
    print("SAFE REFUSAL: six malformed/ambiguous selectors, no writes", flush=True)

    # New profile refuses the *legacy* v1 marker even when the boot argument
    # supplies a matching-looking ID. This is a deliberate no-downgrade gate.
    qemu_case(args, directory, "v2-refuse-old-marker",
              [(0, old_v1), (1, intended)], 1, 35, boot_suffix=good_arg)
    assert verified_disk_read(old_v1) == initial_v1
    assert verified_disk_read(intended) == committed
    print("SAFE REFUSAL: old v1 marker cannot bypass v2 identity gate", flush=True)

    # Device is absent at next boot; the valid intended volume is on IDE1.
    qemu_case(args, directory, "v2-missing-ide0",
              [(1, intended)], 1, 35, boot_suffix=good_arg)
    assert verified_disk_read(intended) == committed
    print("SAFE REFUSAL: selected IDE0 missing, no implicit move to IDE1", flush=True)

    # The cloned-ID counterexample proves no cryptographic/device binding
    # exists. A *different* disk carrying an exact duplicate ID and header
    # is accepted even though intended persisted data remains on IDE1.
    qemu_case(args, directory, "v2-full-clone-wrong-primary",
              [(0, same_id), (1, intended)], 1, 33, boot_suffix=good_arg)
    changed_copy = verified_disk_read(same_id)
    assert changed_copy != initial_copy and b"oldbytes" in changed_copy
    assert verified_disk_read(intended) == committed
    print("EXPECTED ADVERSE: clone with identical v2 marker/ID still accepts a wrong disk", flush=True)
    print("NOT AN ENCRYPTION, AUTHENTICATION, ANTI-BRICKING OR PHYSICAL DEVICE IDENTITY PASS", flush=True)

    for label, data in (
        ("intended_store", committed), ("wrong_ID_unchanged", initial_wrong),
        ("cloned_ID_before", initial_copy), ("cloned_ID_after", changed_copy),
        ("sentinel_unchanged", initial_gpt),
    ):
        print(label + "_sha256", digest(data), flush=True)
    print("PASS: opt-in v2 accidental-swap guard plus cloned-ID adverse residual demonstrated", flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--kernel", type=Path, required=True)
    p.add_argument("--out", type=Path)
    p.add_argument("--qemu", default="qemu-system-x86_64")
    p.add_argument("--clang", default="clang")
    p.add_argument("--objcopy", default="objcopy")
    p.add_argument("--timeout", type=int, default=120)
    args = p.parse_args()
    if not args.kernel.is_file():
        raise SystemExit("refusing test without compiled source-bound kernel")
    if args.out:
        replay(args, args.out.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix="pcpos-v2-ide-") as temp:
            replay(args, Path(temp) / "fixture")
