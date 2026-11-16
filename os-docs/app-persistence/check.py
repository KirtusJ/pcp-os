"""Read a PCP OS app from the active generation of an offline application store.

Use --offset with the byte offset of the independently identified store partition
when supplying a complete disk. A store-partition extract starts at offset zero.
"""
import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

SECTOR = 512
SLOT_SECTORS = 129
STORE_BYTES = (1 + 2 * SLOT_SECTORS) * SECTOR
MARKER = b"PCPOS USER STORE 1\n".ljust(SECTOR, b"\0")


def decode_files(payload):
    files = {}
    cursor = 0
    while cursor < len(payload):
        if len(payload) - cursor < 6 or len(files) >= 256:
            raise ValueError("invalid file count or truncated record")
        name_len, data_len = struct.unpack_from("<HI", payload, cursor)
        cursor += 6
        start = cursor + name_len
        end = start + data_len
        if end > len(payload) or not 1 <= name_len <= 255:
            raise ValueError("invalid file length")
        name = payload[cursor:start].decode("utf-8")
        if (name in files or name.startswith("dev/") or "\0" in name
                or any(part in ("", ".", "..") for part in name.split("/"))):
            raise ValueError("invalid or repeated file name")
        files[name] = payload[start:end]
        cursor = end
    return files


def decode_slot(data):
    head = data[:SECTOR]
    if head[:8] != b"PCPSLOT1" or hashlib.sha256(head[:52]).digest() != head[52:84]:
        raise ValueError("missing or invalid commit header")
    generation, size = struct.unpack_from("<QI", head, 8)
    if generation == 0 or size > (SLOT_SECTORS - 1) * SECTOR:
        raise ValueError("invalid generation or payload length")
    if any(head[84:]) and (head[84:88] != b"TBM1" or head[88] not in (0, 1)
                           or hashlib.sha256(head[:89]).digest() != head[89:121]):
        raise ValueError("invalid mode extension")
    payload = data[SECTOR:SECTOR + size]
    if hashlib.sha256(payload).digest() != head[20:52]:
        raise ValueError("payload checksum mismatch")
    return generation, decode_files(payload)


def inspect(store, name, expected):
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,47}", name):
        raise ValueError("invalid Tailorbird application name")
    if len(store) != STORE_BYTES or store[:SECTOR] != MARKER:
        raise ValueError("short store or wrong format marker")
    valid = {}
    rejected = {}
    for index in range(2):
        start = (1 + index * SLOT_SECTORS) * SECTOR
        slot = store[start:start + SLOT_SECTORS * SECTOR]
        try:
            generation, files = decode_slot(slot)
        except ValueError as error:
            rejected[index] = str(error)
            continue
        if generation in valid:
            raise ValueError("ambiguous equal generations")
        valid[generation] = files
    if not valid:
        raise ValueError("no valid generation")
    generation = max(valid)
    path = f"apps/{name}.txt"
    actual = valid[generation].get(path)
    if expected != actual:
        raise ValueError(f"generation {generation}: {path} did not match the expected state")
    return {"generation": generation, "path": path, "present": actual is not None,
            "source_sha256": hashlib.sha256(actual).hexdigest() if actual is not None else None,
            "region_sha256": hashlib.sha256(store).hexdigest(), "rejected_slots": rejected}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("app", help="one application name without the apps/ prefix or .txt suffix")
    parser.add_argument("--offset", type=int, default=0, help="partition start in bytes")
    state = parser.add_mutually_exclusive_group(required=True)
    state.add_argument("--source", type=Path, help="file containing the exact expected source bytes")
    state.add_argument("--absent", action="store_true")
    args = parser.parse_args()
    if args.offset < 0 or args.offset % SECTOR:
        parser.error("provide a nonnegative sector-aligned byte offset")
    try:
        with args.image.open("rb") as image:
            image.seek(args.offset)
            region = image.read(STORE_BYTES)
        expected = args.source.read_bytes() if args.source else None
        print(json.dumps(inspect(region, args.app, expected), sort_keys=True))
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
