"""Pixel checks for the fixed 960 by 540 Tailorbird desktop preview."""
import hashlib


WIDTH, HEIGHT = 960, 540
# Hash the full event label from x=336 up to the message at x=422.
OUTPUT_TAG = "962b9508d0b68a00e9e68d203ef4db74e3b619bc3a2400cf3b6cc455c22cd090"
TOOL_TAG = "c3e4f85964b80006baf6e937d438b5b523c160fabf1d382e060a0b02df38e7f4"
DENIED_FIELD = "f6d265e07f535d5da7621bc4ce48742647d54169d8a58ec34c199f7c7066899f"
OUTPUT_FIELDS = {
    "original": "f6a9bfc01e3f12cd61970050998353bdbe8e2c835942e7773a98b53c085ce1bf",
    "replacement": "15763dd7a5f6214ab080dfc22eef11357f48f762c67dc47e97700162faa057be",
    "changed": "bb8d181d93f7472d918f10fb49ccd90e0ea204692e46c8c9f428e81ad3f0fe8a",
}


def read_frame(path):
    """Return complete QEMU P6 pixels, or None while a frame is being written."""
    with path.open("rb") as capture:
        header = [capture.readline(64) for _ in range(3)]
        if any(not line.endswith(b"\n") for line in header):
            return None
        if [line.strip() for line in header] != [b"P6", b"960 540", b"255"]:
            raise ValueError(f"{path.name}: unsupported capture header {header!r}")
        size = WIDTH * HEIGHT * 3
        pixels = capture.read(size + 1)
        if len(pixels) > size:
            raise ValueError(f"{path.name}: trailing capture bytes")
        return pixels if len(pixels) == size else None


def crop_digest(pixels, x, y, width):
    digest = hashlib.sha256()
    for row in range(y, y + 10):
        start = (row * WIDTH + x) * 3
        digest.update(pixels[start:start + width * 3])
    return digest.hexdigest()


def matching_row(pixels, expected, *, tag=OUTPUT_TAG):
    """Require a whole output field and tag on one row, with no conflicting test output."""
    if tag == OUTPUT_TAG:
        fields = OUTPUT_FIELDS
    elif tag == TOOL_TAG and expected == "Request denied":
        fields = {expected: DENIED_FIELD}
    else:
        raise ValueError("unsupported task row")
    matches = []
    for index in range(8):
        y = 190 + 28 * index
        if crop_digest(pixels, 336, y, 422 - 336) == tag:
            field = crop_digest(pixels, 422, y, 504)
            for word, digest in fields.items():
                if field == digest:
                    matches.append((word, y))
    if len(matches) == 1 and matches[0][0] == expected:
        return matches[0][1]
    return None
