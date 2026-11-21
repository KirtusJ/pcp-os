"""Create a new directory containing one empty PCP OS VM data disk and its volume ID."""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import uuid


def create(directory):
    """Require a new directory so an existing disk cannot be reformatted."""
    volume_id = uuid.uuid4()
    image = bytearray(259 * 512)
    marker = b"PCPOS USER STORE 2\n"
    image[:len(marker)] = marker
    image[32:48] = volume_id.bytes
    header = b"PCPSLOT1" + struct.pack("<QI", 1, 0) + hashlib.sha256(b"").digest()
    image[512:596] = header + hashlib.sha256(header).digest()

    directory.mkdir(mode=0o700)
    with (directory / "data.raw").open("xb") as disk:
        disk.write(image)
        disk.flush()
        os.fsync(disk.fileno())
    with (directory / "volume-id.txt").open("x", encoding="ascii") as identity:
        identity.write(volume_id.hex + "\n")
        identity.flush()
        os.fsync(identity.fileno())
    return volume_id.hex


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("new_directory", type=Path)
    print(create(parser.parse_args().new_directory))
