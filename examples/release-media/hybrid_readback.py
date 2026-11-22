"""Read back the single-ESP GPT / El Torito layout emitted by model-media-iso."""
import hashlib
import struct
import uuid
import zlib

SECTOR = 512
BLOCK = 2048
ESP_TYPE = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b").bytes_le


def read_at(stream, offset, length):
    stream.seek(offset)
    data = stream.read(length)
    if len(data) != length:
        raise ValueError("short read from the finished ISO")
    return data


def gpt_header(stream, lba, last_lba):
    block = read_at(stream, lba * SECTOR, SECTOR)
    signature, revision, size, checksum = struct.unpack_from("<8sIII", block)
    if signature != b"EFI PART" or revision != 0x10000 or not 92 <= size <= SECTOR:
        raise ValueError("invalid GPT header")
    if zlib.crc32(block[:16] + bytes(4) + block[20:size]) != checksum:
        raise ValueError("GPT header checksum differs")
    current, other, first, last = struct.unpack_from("<QQQQ", block, 24)
    table, count, stride, table_crc = struct.unpack_from("<QIII", block, 72)
    if current != lba or other != (last_lba if lba == 1 else 1):
        raise ValueError("GPT headers do not point to one another")
    if not 1 < first <= last < last_lba or stride != 128 or not count:
        raise ValueError("unexpected GPT partition-table geometry")
    table_end = table + (count * stride + SECTOR - 1) // SECTOR
    reserved_start, reserved_end = (2, first) if lba == 1 else (last + 1, last_lba)
    if table < reserved_start or table_end > reserved_end:
        raise ValueError("GPT table extends outside its reserved sectors")
    return table, count, table_crc, first, last, block[56:72]


def efi_partition(stream, image_size):
    last_lba = image_size // SECTOR - 1
    primary = gpt_header(stream, 1, last_lba)
    backup = gpt_header(stream, last_lba, last_lba)
    if primary[1:] != backup[1:]:
        raise ValueError("GPT headers describe different partition tables")
    table, count, expected_crc, first, last, _ = primary
    checksum = 0
    esp = None
    for index in range(0, count, 256):
        length = min(256, count - index) * 128
        entries = read_at(stream, table * SECTOR + index * 128, length)
        if entries != read_at(stream, backup[0] * SECTOR + index * 128, length):
            raise ValueError("primary and backup GPT partition entries differ")
        checksum = zlib.crc32(entries, checksum)
        for offset in range(0, length, 128):
            if entries[offset:offset + 16] == ESP_TYPE:
                start, end = struct.unpack_from("<QQ", entries, offset + 32)
                if esp is not None or not first <= start <= end <= last:
                    raise ValueError("ambiguous or out-of-range EFI System Partition")
                esp = (start * SECTOR, (end - start + 1) * SECTOR)
    if checksum != expected_crc or esp is None:
        raise ValueError("GPT partition checksum differs or the EFI partition is missing")
    return esp


def optical_entry(stream, image_size):
    catalog = None
    joliet = False
    for lba in range(16, image_size // BLOCK):
        desc = read_at(stream, lba * BLOCK, BLOCK)
        if desc[1:7] != b"CD001\x01":
            raise ValueError("invalid ISO volume descriptor")
        if desc[0] == 255:
            break
        if desc[0] == 2 and desc[88:91] in (b"%/@", b"%/C", b"%/E"):
            joliet = True
        if desc[0] == 0 and desc[7:39].rstrip(b"\x00 ") == b"EL TORITO SPECIFICATION":
            if catalog is not None:
                raise ValueError("multiple El Torito boot records")
            catalog = struct.unpack_from("<I", desc, 71)[0]
    else:
        raise ValueError("unterminated ISO volume descriptors")
    if not joliet or catalog is None:
        raise ValueError("missing Joliet descriptor or El Torito boot record")
    entry = read_at(stream, catalog * BLOCK, 64)
    if entry[:2] != b"\x01\xef" or entry[30:34] != b"\x55\xaa\x88\x00" or sum(struct.unpack("<16H", entry[:32])) % 65536:
        raise ValueError("invalid EFI no-emulation boot catalog entry")
    sectors, block = struct.unpack_from("<HI", entry, 38)
    return block * BLOCK, sectors


def verify_embedded_fat(iso, fat_bytes, fat_sha256):
    """Bind the on-image bytes accessible through both firmware entry points."""
    size = iso.stat().st_size
    if size % SECTOR or size < 18 * BLOCK:
        raise ValueError("invalid hybrid image size")
    with iso.open("rb") as stream:
        offset, length = efi_partition(stream, size)
        optical_offset, sectors = optical_entry(stream, size)
        expected_sectors = length // SECTOR
        # The stock master uses zero when the 16-bit optical sector count cannot fit.
        count_field = expected_sectors if expected_sectors <= 0xffff else 0
        if optical_offset != offset or sectors != count_field or length != fat_bytes:
            raise ValueError("optical and GPT entries do not identify the verified FAT")
        stream.seek(offset)
        checksum = hashlib.sha256()
        remaining = length
        while remaining:
            block = stream.read(min(4 << 20, remaining))
            if not block:
                raise ValueError("truncated EFI System Partition")
            checksum.update(block)
            remaining -= len(block)
        if checksum.hexdigest() != fat_sha256:
            raise ValueError("finished ISO's embedded FAT differs from the verified FAT")
    return offset, length
