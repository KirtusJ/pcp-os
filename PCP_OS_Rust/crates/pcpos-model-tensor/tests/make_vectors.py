"""Regenerate independent decoder vectors from the pinned GGML reference library.

Host helper only. No model download, inference or generated-output execution.
Usage: python make_vectors.py /path/to/ggml-base.dll
The library must be the official llama.cpp b12387 Windows x64 CPU reference.
The generator refuses an unexpected library hash, recorded with the vectors.
"""
import ctypes
import hashlib
import json
import random
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
library = Path(sys.argv[1]).resolve()
pin = ROOT / 'data/reference.json'
expected = json.loads(pin.read_text())['library_sha256']
actual = hashlib.sha256(library.read_bytes()).hexdigest()
if actual != expected:
    raise SystemExit('GGML reference library hash mismatch')
ggml = ctypes.CDLL(str(library))
randomizer = random.Random(0xC3_20261122)
lines = ['# type columns encoded_hex expected_little_endian_f32_hex; independent GGML b12387']
for kind, suffix, values, size, scales in [
    (6, 'q5_0', 32, 22, [0]), (8, 'q8_0', 32, 34, [0]),
    (12, 'q4_K', 256, 144, [0, 2]), (14, 'q6_K', 256, 210, [208])
]:
    function = getattr(ggml, 'dequantize_row_' + suffix)
    function.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_int64]
    function.restype = None
    for case in range(6):
        raw = bytearray(randomizer.randrange(256) for _ in range(size * 2))
        if case < 4:
            raw[:] = bytes(([0, 255, i % 256, 0xAA][case] for i in range(len(raw))))
        for block in range(2):
            for at in scales:
                scale = [0, 0x3C00, 0xB800, 0x0400,
                         randomizer.randrange(0x7C00) | (block << 15),
                         0x7BFF if block == 0 else 0x8001][case]
                struct.pack_into('<H', raw, block * size + at, scale)
        encoded = ctypes.create_string_buffer(bytes(raw))
        output = (ctypes.c_float * (values * 2))()
        function(encoded, output, values * 2)
        decoded = bytes(output)
        lines.append(f'{kind} {values*2} {raw.hex()} {decoded.hex()}')

halves = [0, 0x8000, 1, 0x8001, 0x3FF, 0x400, 0x3C00, 0xBC00, 0x3555, 0xB555, 0x7BFF, 0xFBFF]
f = ggml.ggml_fp16_to_fp32
f.argtypes = [ctypes.c_uint16]
f.restype = ctypes.c_float
raw = b''.join(struct.pack('<H', h) for h in halves)
out = b''.join(struct.pack('<f', f(h)) for h in halves)
lines.append(f'1 {len(halves)} {raw.hex()} {out.hex()}')
bfloats = [0, 0x8000, 1, 0x8001, 0x7F, 0x80, 0x3F80, 0xBF80, 0x3EAA, 0xBEAA, 0x7F7F, 0xFF7F]
raw = b''.join(struct.pack('<H', b) for b in bfloats)
out = b''.join(struct.pack('<I', b << 16) for b in bfloats)
lines.append(f'30 {len(bfloats)} {raw.hex()} {out.hex()}')
floats = [0, 0x80000000, 1, 0x80000001, 0x7FFFFF, 0x800000, 0x3F800000, 0xBF800000, 0x7F7FFFFF, 0xFF7FFFFF]
raw = b''.join(struct.pack('<I', b) for b in floats)
lines.append(f'0 {len(floats)} {raw.hex()} {raw.hex()}')
(ROOT/'data/ggml-b12387.txt').write_text('\n'.join(lines)+'\n', encoding='ascii')
print(json.dumps({'fixture_rows': len(lines)-1, 'library_sha256': actual,
                  'vectors_sha256': hashlib.sha256((ROOT/'data/ggml-b12387.txt').read_bytes()).hexdigest()}))
