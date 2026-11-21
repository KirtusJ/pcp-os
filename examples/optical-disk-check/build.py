#!/usr/bin/env python3
"""Build two local BIOS CD experiments from one pinned PCP OS optical image."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import time
from unittest.mock import patch

import pycdlib


ORIGINAL = '93ed3b599ad7be0e1e88a64813b1f2b5299d51507df2edca691aaf6189d10d15'
KEYBOARD_01 = '09e61116cc295f45a91061253d3913301ef867d790ee08678bd0fb25c1a0a00d'
# Limine 10.8.5 binary release, 7a9013f305de0bea1f6310e76e7baba30499fef0
LOADER = {
    'limine-bios.sys': '34dd9fb6e706d78e920f9e3a403addcab66647ebe78d282d6c11eb6f1ffa5788',
    'limine-bios-cd.bin': '913b56f2a70679049ea2f18c5b5e5902ce488131d76a036ca363c7beb4d8f657',
    'LICENSE': 'f174b8ff78f6f8982a1fc992668f8486eb8cf598f6938923c22e9bb50f8338a8',
}
CD_BOOT = 'limine-bios-cd.bin'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_root(path):
    image = pycdlib.PyCdlib()
    image.open(str(path))
    try:
        files = {}
        for record in image.list_children(iso_path='/'):
            if record.is_dot() or record.is_dotdot():
                continue
            iso_name = '/' + record.file_identifier().decode('ascii')
            rr_name = record.rock_ridge.name().decode('ascii')
            stream = io.BytesIO()
            image.get_file_from_iso_fp(stream, iso_path=iso_name)
            files[rr_name] = (iso_name, stream.getvalue())
        return files
    finally:
        image.close()


def make_iso(path, files):
    image = pycdlib.PyCdlib()
    image.new(interchange_level=3, vol_ident='PCP_OS', rock_ridge='1.09')
    try:
        for name, (iso_name, data) in sorted(files.items()):
            if name != 'boot.cat':
                image.add_fp(io.BytesIO(data), len(data), iso_path=iso_name, rr_name=name)
        image.add_eltorito(files[CD_BOOT][0], bootcatfile=files['boot.cat'][0],
                           rr_bootcatname='boot.cat', boot_load_size=4, boot_info_table=True)
        image.write(str(path))
    finally:
        image.close()
    # Optical drives may satisfy a larger read around the last file. Match the
    # conventional 300 KiB end padding used when mastering optical images.
    with path.open('ab') as out:
        out.write(bytes(150 * 2048))
    readback = read_root(path)
    if readback.keys() != files.keys():
        raise ValueError('ISO filename set changed')
    for name, (iso_name, expected) in files.items():
        actual_name, actual = readback[name]
        if iso_name != actual_name:
            raise ValueError(f'{name}: ISO filename changed')
        if name == 'boot.cat':
            continue  # The catalog is generated for this layout.
        if name == CD_BOOT:
            # El Torito Boot Info Table is filled during image generation.
            expected, actual = expected[:8] + expected[64:], actual[:8] + actual[64:]
        if actual != expected:
            raise ValueError(f'{name}: payload read-back failed')
    print(f'{digest(path.read_bytes())}  {path.name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('iso', type=Path)
    parser.add_argument('upstream', type=Path, help='pinned Limine binary release directory')
    parser.add_argument('output', type=Path, help='new directory')
    args = parser.parse_args()
    source_digest = digest(args.iso.read_bytes()) if args.iso.is_file() else None
    if source_digest not in (ORIGINAL, KEYBOARD_01):
        parser.error('wrong input ISO')
    files = read_root(args.iso)
    label = b'Experimental BIOS disk-write test candidate.'
    if source_digest == KEYBOARD_01:
        # The keyboard CD uses relaxed primary ISO identifiers. Keep its Rock Ridge names.
        files = {name: (primary.upper().replace('-', '_'), data)
                 for name, (primary, data) in files.items()}
        label = b'Experimental BIOS keyboard and disk-write test candidate.'
    replacements = {}
    for name, expected in LOADER.items():
        data = (args.upstream / name).read_bytes()
        if digest(data) != expected:
            parser.error(f'wrong upstream input: {name}')
        replacements['limine.txt' if name == 'LICENSE' else name] = data
    readme = files['readme.txt'][1]
    if readme.count(b'Loader: Limine v9.2.3.') != 1:
        parser.error('unexpected loader attribution')
    replacements['readme.txt'] = readme.replace(b'Loader: Limine v9.2.3.',
        label + b' Loader: Limine v10.8.5.')
    args.output.mkdir()
    os.environ['TZ'] = 'UTC'
    time.tzset()
    # pycdlib uses the current wall clock for volume and directory metadata.
    with patch('time.time', return_value=1704067200):
        make_iso(args.output / 'old-loader-control.iso', files)
        for name, data in replacements.items():
            files[name] = (files[name][0], data)
        make_iso(args.output / 'no-write-candidate.iso', files)


if __name__ == '__main__':
    main()
