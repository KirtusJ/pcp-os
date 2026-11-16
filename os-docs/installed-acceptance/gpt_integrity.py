"""Validate both GPT metadata copies on an offline 512-byte-sector factory image.

A partition lister may recover a damaged GPT from one surviving header.
Installed release acceptance should not silently treat that as a fully sound disk.
"""
import hashlib
import os
import struct
import zlib

SECTOR_BYTES = 512
HEADER_BYTES = 92


def _copy(fd, disk_sectors, lba):
    block = os.pread(fd, SECTOR_BYTES, lba * SECTOR_BYTES)
    if len(block) != SECTOR_BYTES:
        raise ValueError("Incomplete GPT header")
    (signature, revision, header_size, crc, reserved, current, alternate,
     first_usable, last_usable, disk_guid, entries_lba, count,
     entry_size, entries_crc) = struct.unpack_from("<8sIIIIQQQQ16sQIII", block)
    if signature != b"EFI PART" or revision < 0x10000 or reserved:
        raise ValueError("Invalid GPT signature, revision or reserved field")
    if not HEADER_BYTES <= header_size <= SECTOR_BYTES:
        raise ValueError("Invalid GPT header size")
    header = bytearray(block[:header_size])
    header[16:20] = b"\0\0\0\0"
    if zlib.crc32(header) != crc:
        raise ValueError("GPT header CRC mismatch")
    if current != lba or alternate >= disk_sectors or alternate == current:
        raise ValueError("Invalid GPT header LBA references")
    if not 2 <= first_usable <= last_usable < disk_sectors - 1:
        raise ValueError("Invalid GPT usable range")
    if count == 0 or entry_size < 128 or entry_size % 8:
        raise ValueError("Invalid GPT entry array dimensions")
    length = count * entry_size
    start = entries_lba * SECTOR_BYTES
    end = start + length
    if entries_lba < 2 or end > disk_sectors * SECTOR_BYTES:
        raise ValueError("GPT entry array outside disk")
    # Metadata must not occupy a usable partition sector or either header.
    first_entry_lba, last_entry_lba = entries_lba, (end - 1) // SECTOR_BYTES
    if (first_entry_lba <= current <= last_entry_lba or
            first_entry_lba <= alternate <= last_entry_lba or
            not (last_entry_lba < first_usable or first_entry_lba > last_usable)):
        raise ValueError("GPT entry array intersects usable disk or header")
    digest = hashlib.sha256()
    checksum = 0
    position = start
    while position < end:
        data = os.pread(fd, min(1024 * 1024, end - position), position)
        if not data:
            raise ValueError("Truncated GPT entry array")
        checksum = zlib.crc32(data, checksum)
        digest.update(data)
        position += len(data)
    if checksum != entries_crc:
        raise ValueError("GPT partition entry CRC mismatch")
    return {
        "current": current, "alternate": alternate, "first": first_usable,
        "last": last_usable, "guid": disk_guid, "revision": revision,
        "header_size": header_size, "entries_start": start, "entries_end": end,
        "count": count, "entry_size": entry_size,
        "entries_crc": entries_crc, "entries_digest": digest.digest(),
    }


def _active_entries(fd, primary):
    """Check the active entry map after primary/backup CRC and identity validation."""
    empty_guid = bytes(16)
    seen_guids = set()
    extents = []
    entry_bytes = primary["entry_size"]
    batch_count = max(1, (1024 * 1024) // entry_bytes)
    for start_index in range(0, primary["count"], batch_count):
        n = min(batch_count, primary["count"] - start_index)
        # Skip the unused tail of the last entry in the batch. A wide entry can
        # exceed the read budget by itself, but its last required field ends at byte 48.
        size = (n - 1) * entry_bytes + 48
        at = primary["entries_start"] + start_index * entry_bytes
        data = os.pread(fd, size, at)
        if len(data) != size:
            raise ValueError("Truncated GPT partition entry")
        for local_index in range(n):
            index = start_index + local_index
            type_guid, unique_guid, first, last = struct.unpack_from(
                "<16s16sQQ", data, local_index * entry_bytes)
            # Per UEFI, an entry with a zero type GUID is unused.
            if type_guid == empty_guid:
                continue
            if unique_guid == empty_guid or unique_guid in seen_guids:
                raise ValueError("GPT active partition has zero or duplicate unique GUID")
            seen_guids.add(unique_guid)
            if not primary["first"] <= first <= last <= primary["last"]:
                raise ValueError("GPT active partition outside usable range or reversed")
            extents.append((first, last, index + 1))
    extents.sort()
    for previous, current in zip(extents, extents[1:]):
        if previous[1] >= current[0]:
            raise ValueError(f"GPT active partitions {previous[2]} and {current[2]} overlap")


def require_gpt_mirror(disk_fd, disk_size):
    """Reject an inconsistent primary/backup GPT, including recoverable damage."""
    if disk_size % SECTOR_BYTES or disk_size < 6 * SECTOR_BYTES:
        raise ValueError("Invalid 512-byte-sector GPT disk size")
    disk_sectors = disk_size // SECTOR_BYTES
    primary = _copy(disk_fd, disk_sectors, 1)
    backup = _copy(disk_fd, disk_sectors, disk_sectors - 1)
    if primary["alternate"] != backup["current"] or backup["alternate"] != 1:
        raise ValueError("GPT primary and backup locations disagree")
    if (primary["entries_end"] > primary["first"] * SECTOR_BYTES or
            backup["entries_start"] < (backup["last"] + 1) * SECTOR_BYTES):
        raise ValueError("GPT copies do not occupy distinct primary/backup entry regions")
    for field in ("first", "last", "guid", "revision", "header_size",
                  "count", "entry_size", "entries_crc", "entries_digest"):
        if primary[field] != backup[field]:
            raise ValueError(f"GPT primary and backup {field} disagree")
    _active_entries(disk_fd, primary)
