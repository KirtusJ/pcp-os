"""Conservative GPT metadata inspection of *caller-supplied sparse bytes*.

This module does not open paths, devices, disks, or physical media. It accepts an
untrusted in-memory mapping of LBA -> exact sector bytes. Results cannot grant
an installer write, authenticate a medium, establish firmware interoperability
or establish durability. It deliberately supports only a bounded GPT profile.
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import struct
import zlib
from typing import Mapping

MAX_U64 = (1 << 64) - 1
MIN_ENTRY_ARRAY_BYTES = 16 * 1024
MAX_PROFILE_ENTRY_ARRAY_BYTES = 1024 * 1024
SUPPORTED_LBA_BYTES = frozenset((512, 4096))
SIGNATURE = b"EFI PART"
ZERO_GUID = bytes(16)


class GPTState(str, Enum):
    HOLD = "HOLD"
    METADATA_CONSISTENT_ONLY = "METADATA_CONSISTENT_ONLY"


@dataclass(frozen=True)
class SparseSnapshot:
    """No implicit reads: missing sectors do not default to zero."""
    logical_block_bytes: int
    total_lba: int  # number of LBAs, not the last LBA index
    blocks: Mapping[int, bytes]


@dataclass(frozen=True)
class GPTResult:
    state: GPTState
    reason: str
    partitions_observed: int = 0


def _hold(reason: str) -> GPTResult:
    return GPTResult(GPTState.HOLD, reason)


class _Invalid(Exception):
    def __init__(self, reason: str) -> None:
        self.reason = reason


def _require(ok: bool, reason: str) -> None:
    if not ok:
        raise _Invalid(reason)


def _read(s: SparseSnapshot, lba: int) -> bytes:
    _require(0 <= lba < s.total_lba, "out-of-range-lba")
    try:
        data = s.blocks[lba]
    except KeyError:
        raise _Invalid("missing-snapshot-sector") from None
    _require(type(data) is bytes and len(data) == s.logical_block_bytes,
             "invalid-snapshot-sector")
    return data


@dataclass(frozen=True)
class _Header:
    current: int
    alternate: int
    first: int
    last: int
    guid: bytes
    entries_at: int
    entries_count: int
    entry_size: int
    entries_crc: int
    entries_blocks: int


def _parse_header(s: SparseSnapshot, lba: int) -> _Header:
    block = _read(s, lba)
    _require(block[:8] == SIGNATURE, "missing-gpt-signature")
    revision, size, expected_crc, reserved = struct.unpack_from("<IIII", block, 8)
    _require(revision == 0x00010000, "unsupported-gpt-revision")
    _require(92 <= size <= s.logical_block_bytes, "invalid-header-size")
    _require(reserved == 0 and not any(block[size:]),
             "nonzero-header-reserved")
    check = bytearray(block[:size])
    check[16:20] = b"\x00" * 4
    _require(zlib.crc32(check) == expected_crc, "bad-header-crc")
    current, alternate, first, last = struct.unpack_from("<QQQQ", block, 24)
    guid = block[56:72]
    entries_at, entries_count, entry_size, entries_crc = struct.unpack_from(
        "<QIII", block, 72)
    _require(current == lba and alternate != current and alternate < s.total_lba,
             "bad-header-location")
    _require(guid != ZERO_GUID, "missing-disk-guid")
    _require(entry_size >= 128 and entry_size <= MAX_PROFILE_ENTRY_ARRAY_BYTES
             and entry_size & (entry_size - 1) == 0,
             "unsupported-entry-size")
    _require(entries_count > 0 and entries_count <= MAX_U64 // entry_size,
             "invalid-entry-count")
    total_bytes = entries_count * entry_size
    _require(MIN_ENTRY_ARRAY_BYTES <= total_bytes <= MAX_PROFILE_ENTRY_ARRAY_BYTES,
             "entry-array-outside-bounded-profile")
    needed_blocks = (total_bytes + s.logical_block_bytes - 1) // s.logical_block_bytes
    _require(entries_at < s.total_lba and needed_blocks <= s.total_lba - entries_at,
             "entry-array-out-of-bounds")
    _require(first <= last and last < s.total_lba, "invalid-usable-lba-range")
    return _Header(current, alternate, first, last, guid, entries_at,
                   entries_count, entry_size, entries_crc, needed_blocks)


def _read_array(s: SparseSnapshot, h: _Header) -> bytes:
    raw = b"".join(_read(s, h.entries_at + n) for n in range(h.entries_blocks))
    exact_length = h.entries_count * h.entry_size
    _require(not any(raw[exact_length:]), "nonzero-entry-array-padding")
    array = raw[:exact_length]
    _require(zlib.crc32(array) == h.entries_crc, "bad-entry-array-crc")
    return array


def _check_partitions(array: bytes, h: _Header) -> int:
    spans: list[tuple[int, int]] = []
    guids: set[bytes] = set()
    for i in range(h.entries_count):
        entry = array[i * h.entry_size:(i + 1) * h.entry_size]
        if entry[:16] == ZERO_GUID:
            # Conservative first-tier policy: unused records must be all-zero.
            _require(not any(entry), "nonzero-unused-entry")
            continue
        unique = entry[16:32]
        begin, end = struct.unpack_from("<QQ", entry, 32)
        _require(unique != ZERO_GUID and unique not in guids,
                 "missing-or-duplicate-partition-guid")
        guids.add(unique)
        _require(h.first <= begin <= end <= h.last, "partition-out-of-usable-range")
        _require(not any(entry[128:]), "nonzero-partition-reserved")
        spans.append((begin, end))
    spans.sort()
    for left, right in zip(spans, spans[1:]):
        _require(left[1] < right[0], "overlapping-partitions")
    return len(spans)


def _check_protective_mbr(s: SparseSnapshot) -> None:
    mbr = _read(s, 0)
    _require(mbr[510:512] == b"\x55\xaa", "bad-protective-mbr-signature")
    entry = mbr[446:462]
    _require(entry[0] == 0 and entry[4] == 0xee,
             "bad-protective-mbr-partition-type")
    _require(entry[1:4] == b"\x00\x02\x00",
             "bad-protective-mbr-start-chs")
    _require(not any(mbr[440:446]), "nonzero-protective-mbr-reserved")
    start, size = struct.unpack_from("<II", entry, 8)
    _require(start == 1 and size == min(s.total_lba - 1, 0xffff_ffff),
             "bad-protective-mbr-size")
    _require(not any(mbr[462:510]) and not any(mbr[512:]),
             "hybrid-or-nonzero-mbr-reserved")


def inspect_gpt(s: SparseSnapshot) -> GPTResult:
    """Return a limited metadata conclusion, never filesystem or disk acceptance.

    This is a *pure byte model* for sparse, bounded, externally constructed
    fixtures. No authenticity, independence, write-attempt monitoring, physical
    hardware, boot or recovery outcome is inferred from metadata consistency.
    """
    try:
        _require(type(s.logical_block_bytes) is int and
                 s.logical_block_bytes in SUPPORTED_LBA_BYTES,
                 "unsupported-gpt-lba-byte-size")
        _require(type(s.total_lba) is int and
                 100 <= s.total_lba <= MAX_U64 and
                 s.total_lba <= MAX_U64 // s.logical_block_bytes,
                 "invalid-or-overflowed-capacity")
        _check_protective_mbr(s)
        last_lba = s.total_lba - 1
        p = _parse_header(s, 1)
        b = _parse_header(s, last_lba)
        _require(p.current == 1 and p.alternate == last_lba and
                 b.current == last_lba and b.alternate == 1,
                 "nonreciprocal-gpt-header-lbas")
        _require((p.first, p.last, p.guid, p.entries_count, p.entry_size,
                  p.entries_crc, p.entries_blocks) ==
                 (b.first, b.last, b.guid, b.entries_count, b.entry_size,
                  b.entries_crc, b.entries_blocks), "primary-backup-header-mismatch")
        _require(2 <= p.entries_at and
                 p.entries_at + p.entries_blocks <= p.first,
                 "primary-array-overlaps-usable-or-header")
        _require(b.entries_at > b.last and
                 b.entries_at + b.entries_blocks <= b.current,
                 "backup-array-overlaps-usable-or-header")
        _require(p.last < b.entries_at and p.first > p.current,
                 "usable-range-collides-with-metadata")
        a = _read_array(s, p)
        a2 = _read_array(s, b)
        _require(a == a2, "primary-backup-array-content-mismatch")
        used = _check_partitions(a, p)
        return GPTResult(GPTState.METADATA_CONSISTENT_ONLY,
                         "metadata-crc-location-bounds-and-pair-consistent", used)
    except _Invalid as e:
        return _hold(e.reason)
