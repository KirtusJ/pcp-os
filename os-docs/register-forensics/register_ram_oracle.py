#!/usr/bin/env python3
"""Independent, read-only candidate register scan of a paused QEMU guest RAM image.

This tool deliberately does not call pcpos, import the native raster checker, or
trust a head extracted from pixels as its oracle. It validates chain bytes
independently, then *compares* their head and event count to the supplied screen.

A RAM scan identifies candidate byte sequences, not the live Record pointer.
Heap copies, complete alternate ledgers and device capture provenance are
remaining limitations. No promotion, host-authority or kernel-validity claim.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import mmap
import os
from pathlib import Path
import re

MARKER = b'{"actor":'
ZERO = "0" * 64
FIELDS = frozenset(("actor", "auth", "body", "kind", "prev", "seq", "ts"))
HEX = re.compile(r"[0-9a-f]{64}\Z")
TIMESTAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")
MAX_LINE = 65536
MAX_EVENTS = 4096


def quote(s: str) -> str:
    """Match pcpos-core Value::write_str, including the DEL character."""
    special = {'"': r'\"', "\\": r'\\', "\n": r'\n', "\r": r'\r',
               "\t": r'\t', "\b": r'\b', "\f": r'\f'}
    pieces = ['"']
    for c in s:
        if c in special:
            pieces.append(special[c])
        elif ord(c) < 32 or ord(c) == 127:
            pieces.append('\\u%04x' % ord(c))
        elif ord(c) < 128:
            pieces.append(c)
        else:
            u = ord(c)
            if u <= 0xffff:
                pieces.append('\\u%04x' % u)
            else:
                u -= 0x10000
                pieces.append('\\u%04x\\u%04x' % (0xd800 + (u >> 10), 0xdc00 + (u & 1023)))
    return ''.join(pieces) + '"'


def canonical(value, depth=0) -> str:
    if depth > 32:
        raise ValueError('nesting depth exceeds Rust limit')
    if value is None:
        return 'null'
    if value is True:
        return 'true'
    if value is False:
        return 'false'
    if type(value) is int:
        if not -(1 << 63) <= value < (1 << 63):
            raise ValueError('non-i64 integer')
        return str(value)
    if isinstance(value, str):
        return quote(value)
    if isinstance(value, list):
        return '[' + ','.join(canonical(v, depth+1) for v in value) + ']'
    if isinstance(value, dict):
        return '{' + ','.join(quote(k) + ':' + canonical(value[k], depth+1)
                              for k in sorted(value)) + '}'
    raise ValueError('unsupported type')


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate key')
        result[key] = value
    return result


def invalid_number(_):
    raise ValueError('non-integer/constant')


def event_from_line(line: bytes):
    if not 0 < len(line) <= MAX_LINE:
        return None
    try:
        obj = json.loads(line, object_pairs_hook=unique_pairs,
                         parse_float=invalid_number, parse_constant=invalid_number)
        if type(obj) is not dict or obj.keys() != FIELDS:
            return None
        if not all(isinstance(obj[k], str) for k in ('actor','auth','kind','prev','ts')):
            return None
        if obj['auth'] not in ('HOST', 'DECLARED'):
            return None
        if type(obj['body']) is not dict or type(obj['seq']) is not int:
            return None
        if not HEX.fullmatch(obj['prev']) or not TIMESTAMP.fullmatch(obj['ts']):
            return None
        if line.decode('ascii') != canonical(obj):
            return None
        return obj
    except (ValueError, TypeError, UnicodeError, RecursionError):
        return None


def candidate_chain(memory, offset: int, max_events=MAX_EVENTS, workspace_label='kernel'):
    cursor = offset
    previous = ZERO
    count = 0
    last_head = ZERO
    while count < max_events:
        end = memory.find(b'\n', cursor, min(len(memory), cursor + MAX_LINE + 2))
        if end < 0:
            break
        line = memory[cursor:end]
        obj = event_from_line(line)
        if obj is None or obj['seq'] != count or obj['prev'] != previous:
            break
        if count == 0:
            if obj['kind'] != 'WORKSPACE_CREATED' or obj['body'].get('label') != workspace_label:
                break
        last_head = hashlib.sha256(line).hexdigest()
        previous = last_head
        count += 1
        cursor = end + 1
    if count == 0:
        return None
    return {'start': offset, 'end': cursor, 'events': count, 'head': last_head,
            'chain_bytes_sha256': hashlib.sha256(memory[offset:cursor]).hexdigest()}


def find_candidates(memory, expected_events: int, max_events=MAX_EVENTS, workspace_label='kernel'):
    """Find complete canonical candidates without using the optical head to select them."""
    candidates = []
    position = 0
    while True:
        position = memory.find(MARKER, position)
        if position < 0:
            break
        result = candidate_chain(memory, position, max_events, workspace_label)
        if result and result['events'] >= expected_events:
            candidates.append(result)
        position += 1
    return candidates


def compare(memory, visible_head, visible_events, workspace_label='kernel'):
    """Return an evidence-limited result; never select by visible digest."""
    if not HEX.fullmatch(visible_head) or visible_events <= 0 or visible_events > MAX_EVENTS:
        raise ValueError('expected exactly 64 lowercase hex chars and bounded positive event count')
    candidates = find_candidates(memory, visible_events, workspace_label=workspace_label)
    if not candidates:
        return {'accepted': False, 'reason': 'no matching-length canonical kernel candidate',
                'candidate_count': 0}
    count = max(x['events'] for x in candidates)
    leaders = [x for x in candidates if x['events'] == count]
    distinct = {(x['events'], x['head'], x['chain_bytes_sha256']) for x in leaders}
    if len(distinct) != 1:
        return {'accepted': False, 'reason': 'ambiguous longest chains',
                'candidate_count': len(candidates), 'longest_events': count,
                'distinct_longest_chains': len(distinct)}
    selected = leaders[0]
    agree = count == visible_events and selected['head'] == visible_head
    return {'accepted': agree,
            'reason': ('matching independent RAM-candidate bytes' if agree else 'RAM candidate differs from pixels/count'),
            'candidate_count': len(candidates), 'longest_events': count,
            'independent_head': selected['head'],
            'chain_bytes_sha256': selected['chain_bytes_sha256'],
            'offset': selected['start'], 'visible_events': visible_events,
            'visible_head': visible_head,
            'live_record_pointer_authenticated': False,
            'actual_boot_refusal_tested': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ram', type=Path, help='raw, guest-physical-memory dump from stopped VM')
    parser.add_argument('--visible-head', required=True, help='full optically decoded 64-digit SHA256')
    parser.add_argument('--visible-events', type=int, required=True)
    parser.add_argument('--workspace-label', default='kernel')
    args = parser.parse_args()
    with args.ram.open('rb') as fp:
        if fp.seek(0, os.SEEK_END) == 0:
            raise SystemExit('empty RAM dump')
        fp.seek(0)
        with mmap.mmap(fp.fileno(), 0, access=mmap.ACCESS_READ) as memory:
            result = compare(memory, args.visible_head, args.visible_events, args.workspace_label)
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0 if result['accepted'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
