"""Adverse sparse-bytes GPT fixtures: no kernel, disk, OS, QEMU or file I/O."""
from dataclasses import replace
import hashlib
import struct
import unittest
import zlib

from gpt_metadata import GPTState, SparseSnapshot, inspect_gpt

BLOCK_ENTRIES = 128
ENTRY_SIZE = 128
DISK_GUID = bytes(range(1, 17))
EFI_SYSTEM_TYPE = bytes.fromhex('28732ac11ff8d211ba4b00a0c93ec93b')  # GUID wire order


def _entry(start, end, guid_seed):
    b = bytearray(ENTRY_SIZE)
    b[:16] = EFI_SYSTEM_TYPE
    b[16:32] = bytes([guid_seed]) * 16
    struct.pack_into('<QQ', b, 32, start, end)
    return bytes(b)


def _header(block_bytes, my, alt, first, last, arr_lba, arr_crc,
            count=BLOCK_ENTRIES, entry_size=ENTRY_SIZE, disk_guid=DISK_GUID):
    x = bytearray(block_bytes)
    x[:8] = b'EFI PART'
    struct.pack_into('<IIII', x, 8, 0x10000, 92, 0, 0)
    struct.pack_into('<QQQQ', x, 24, my, alt, first, last)
    x[56:72] = disk_guid
    struct.pack_into('<QIII', x, 72, arr_lba, count, entry_size, arr_crc)
    struct.pack_into('<I', x, 16, zlib.crc32(x[:92]))
    return bytes(x)


def factory(lba_bytes=512, total_lba=512, entries=None):
    """Create sparse synthetic GPT metadata, not a disk image."""
    if entries is None:
        entries = [(64, 91, 0x17)]
    array = bytearray(BLOCK_ENTRIES * ENTRY_SIZE)
    for i, (start, end, seed) in enumerate(entries):
        array[i * ENTRY_SIZE:(i+1)*ENTRY_SIZE] = _entry(start, end, seed)
    arr_crc = zlib.crc32(array)
    nblocks = len(array) // lba_bytes
    backup_array = total_lba - nblocks - 1
    first = nblocks + 2
    last = backup_array - 1
    blocks = {}
    for i in range(nblocks):
        part = bytes(array[i * lba_bytes:(i+1)*lba_bytes])
        blocks[2+i] = part
        blocks[backup_array+i] = part
    blocks[1] = _header(lba_bytes, 1, total_lba-1, first, last, 2, arr_crc)
    blocks[total_lba - 1] = _header(lba_bytes, total_lba-1, 1,
                                    first, last, backup_array, arr_crc)
    mbr = bytearray(lba_bytes)
    mbr[446] = 0
    mbr[447:450] = b"\x00\x02\x00"
    mbr[450] = 0xee
    struct.pack_into('<II', mbr, 454, 1, min(total_lba-1, 0xffffffff))
    mbr[510:512] = b'\x55\xaa'
    blocks[0] = bytes(mbr)
    return SparseSnapshot(lba_bytes, total_lba, blocks)


def change_block(s, lba, offset, data):
    d = dict(s.blocks)
    blk = bytearray(d[lba])
    blk[offset:offset+len(data)] = data
    d[lba] = bytes(blk)
    return replace(s, blocks=d)


def header_edit(s, lba, offset, fmt, value):
    d = dict(s.blocks)
    x = bytearray(d[lba])
    struct.pack_into(fmt, x, offset, value)
    struct.pack_into('<I', x, 16, 0)
    struct.pack_into('<I', x, 16, zlib.crc32(x[:struct.unpack_from('<I', x, 12)[0]]))
    d[lba] = bytes(x)
    return replace(s, blocks=d)


class GPTMetadataTests(unittest.TestCase):
    def ok(self, s, expected_count=1):
        r = inspect_gpt(s)
        self.assertEqual(r.state, GPTState.METADATA_CONSISTENT_ONLY, r)
        self.assertEqual(r.partitions_observed, expected_count)
        self.assertNotIn('SAFE', r.state.value)

    def held(self, s, expected_reason):
        r = inspect_gpt(s)
        self.assertEqual(r.state, GPTState.HOLD, r)
        self.assertEqual(r.reason, expected_reason, r)

    def test_positive_512_lba(self):
        self.ok(factory())

    def test_positive_4096_lba(self):
        self.ok(factory(lba_bytes=4096))

    def test_sparse_large_lba_no_giant_allocation(self):
        self.ok(factory(total_lba=8_000_000))

    def test_empty_partition_table_is_allowed(self):
        self.ok(factory(entries=[]), 0)

    def test_two_separated_partitions_allowed(self):
        self.ok(factory(entries=[(64, 81, 1), (82, 91, 2)]), 2)

    def test_missing_backup_header_is_not_silently_repaired(self):
        s = factory()
        d = dict(s.blocks)
        del d[s.total_lba-1]
        self.held(replace(s, blocks=d), 'missing-snapshot-sector')

    def test_header_crc_corrupted(self):
        self.held(change_block(factory(), 1, 80, b'X'), 'bad-header-crc')

    def test_backup_header_crc_corrupted(self):
        s = factory()
        self.held(change_block(s, s.total_lba-1, 80, b'X'), 'bad-header-crc')

    def test_partition_entry_crc_corrupted(self):
        self.held(change_block(factory(), 2, 0, b'X'), 'bad-entry-array-crc')

    def test_backup_entry_array_crc_corrupted(self):
        s = factory()
        backup_array_first = s.total_lba - 1 - (16*1024 // s.logical_block_bytes)
        self.held(change_block(s, backup_array_first, 0, b'X'), 'bad-entry-array-crc')

    def test_recipient_lba_mismatch_even_when_header_crc_repaired(self):
        s = factory()
        self.held(header_edit(s, 1, 32, '<Q', 400), 'nonreciprocal-gpt-header-lbas')

    def test_backup_guid_mismatch_even_when_crc_repaired(self):
        s = factory()
        lba = s.total_lba - 1
        d = dict(s.blocks)
        x = bytearray(d[lba])
        x[56:72] = bytes([0xfa])*16
        struct.pack_into('<I', x, 16, 0)
        struct.pack_into('<I', x, 16, zlib.crc32(x[:92]))
        d[lba] = bytes(x)
        self.held(replace(s, blocks=d), 'primary-backup-header-mismatch')

    def test_stale_backup_on_expanded_disk(self):
        s = factory()
        d = dict(s.blocks)
        d[2*s.total_lba-1] = d[s.total_lba-1]
        self.held(replace(s, total_lba=2*s.total_lba, blocks=d), 'bad-protective-mbr-size')

    def test_explicit_primary_usable_range_overlap(self):
        s = factory()
        self.held(header_edit(s, 1, 40, '<Q', 2), 'primary-backup-header-mismatch')

    def test_protective_mbr_signature_missing(self):
        self.held(change_block(factory(), 0, 510, b'\x00\x00'), 'bad-protective-mbr-signature')

    def test_protective_mbr_bad_start_chs_holds(self):
        self.held(change_block(factory(), 0, 447, b'\x00\x00\x00'),
                  'bad-protective-mbr-start-chs')

    def test_protective_mbr_nonzero_reserved_holds(self):
        self.held(change_block(factory(), 0, 440, b'\x01'),
                  'nonzero-protective-mbr-reserved')

    def test_backup_array_boundary_intrusion_with_recalculated_crc(self):
        s = factory()
        last_usable = struct.unpack_from('<Q', s.blocks[s.total_lba-1], 48)[0]
        new = header_edit(s, s.total_lba-1, 72, '<Q', last_usable)
        self.held(new, 'backup-array-overlaps-usable-or-header')

    def test_backup_array_diverges_with_each_crc_repaired(self):
        s = factory()
        blocks = dict(s.blocks)
        start = s.total_lba - 1 - 32
        x = bytearray(blocks[start])
        x[1] ^= 1
        blocks[start] = bytes(x)
        new_crc = zlib.crc32(b''.join(blocks[start+i] for i in range(32)))
        s2 = replace(s, blocks=blocks)
        s2 = header_edit(s2, s.total_lba-1, 88, '<I', new_crc)
        self.held(s2, 'primary-backup-header-mismatch')

    def test_hybrid_mbr_is_held(self):
        self.held(change_block(factory(), 0, 466, b'\x83'), 'hybrid-or-nonzero-mbr-reserved')

    def test_overlapping_partitions_with_valid_crcs_still_held(self):
        s = factory(entries=[(64, 85, 1), (82, 92, 2)])
        self.held(s, 'overlapping-partitions')

    def test_duplicate_partition_guid_with_valid_crcs_still_held(self):
        s = factory(entries=[(64, 85, 1), (86, 92, 1)])
        self.held(s, 'missing-or-duplicate-partition-guid')

    def test_partition_outside_usable_range_with_good_crcs(self):
        s = factory(entries=[(64, 900, 1)])
        self.held(s, 'partition-out-of-usable-range')

    def test_zero_partition_guid_even_if_type_nonzero(self):
        s = factory(entries=[(64, 91, 0)])
        self.held(s, 'missing-or-duplicate-partition-guid')

    def test_unsupported_2048_gpt_block_size_not_misclassified_as_optical(self):
        s = factory()
        self.held(replace(s, logical_block_bytes=2048), 'unsupported-gpt-lba-byte-size')

    def test_missing_sparse_array_sector_holds_not_zero_fills(self):
        s = factory()
        d = dict(s.blocks)
        del d[3]
        self.held(replace(s, blocks=d), 'missing-snapshot-sector')

    def test_bad_sector_length_does_not_claim_inspection(self):
        s = factory()
        d = dict(s.blocks)
        d[2] = d[2][:511]
        self.held(replace(s, blocks=d), 'invalid-snapshot-sector')

    def test_overflowed_capacity_holds(self):
        s = factory()
        self.held(replace(s, total_lba=(1 << 64)), 'invalid-or-overflowed-capacity')

    def test_header_reserved_byte_with_crc_unchanged_holds(self):
        self.held(change_block(factory(), 1, 93, b'\x7f'), 'nonzero-header-reserved')

    def test_good_crc_is_not_media_authentication(self):
        original = factory()
        copied = SparseSnapshot(original.logical_block_bytes, original.total_lba,
                                dict(original.blocks))
        self.ok(copied)  # clone cannot be differentiated using only bytes


class SimulatedWriteOracleTests(unittest.TestCase):
    def test_idempotent_write_attempt_is_not_visible_in_final_hash(self):
        class MemoryOnlyRecorder:
            def __init__(self):
                self.blocks = {0: b'toy test sector'}
                self.attempts = []
            def write(self, lba, payload):
                self.attempts.append((lba, payload))
                self.blocks[lba] = payload
        rec = MemoryOnlyRecorder()
        before = hashlib.sha256(rec.blocks[0]).hexdigest()
        rec.write(0, b'toy test sector')
        after = hashlib.sha256(rec.blocks[0]).hexdigest()
        self.assertEqual(before, after)
        self.assertEqual(len(rec.attempts), 1)

    def test_undo_write_restores_bytes_but_attempts_remain(self):
        attempts = []
        b = b'initial'
        before = hashlib.sha256(b).hexdigest()
        for payload in (b'mutated', b'initial'):
            attempts.append(payload)
            b = payload
        self.assertEqual(hashlib.sha256(b).hexdigest(), before)
        self.assertEqual(len(attempts), 2)


if __name__ == '__main__':
    unittest.main()
