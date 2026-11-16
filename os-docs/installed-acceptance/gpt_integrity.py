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
        "last": last_usable, "guid": disk_guid,
        "count": count, "entry_size": entry_size,
        "entries_crc": entries_crc, "entries_digest": digest.digest(),
    }


def require_gpt_mirror(disk_fd, disk_size):
    """Reject an inconsistent primary/backup GPT, including recoverable damage."""
    if disk_size % SECTOR_BYTES or disk_size < 6 * SECTOR_BYTES:
        raise ValueError("Invalid 512-byte-sector GPT disk size")
    disk_sectors = disk_size // SECTOR_BYTES
    primary = _copy(disk_fd, disk_sectors, 1)
    backup = _copy(disk_fd, disk_sectors, disk_sectors - 1)
    if primary["alternate"] != backup["current"] or backup["alternate"] != 1:
        raise ValueError("GPT primary and backup locations disagree")
    for field in ("first", "last", "guid", "count", "entry_size",
                  "entries_crc", "entries_digest"):
        if primary[field] != backup[field]:
            raise ValueError(f"GPT primary and backup {field} disagree")
