#!/usr/bin/env python3
"""Source-only controls; a real native guest screendump is still required."""
import tempfile
import unittest
from pathlib import Path
import check_register as c

HEAD = "0123456789abcdef" * 4
TAIL_CHANGED = HEAD[:47] + "f" * 17


def fixture_font():
    # Deliberately distinct glyphs ensure the synthetic row is not all background.
    return [[((code * 19 + x * 5 + y * 11) % 256) for y in range(20) for x in range(10)] for code in range(95)]


def ppm(width, height, content, *, col=2, row=2, prefix="HEAD ", clipped=False):
    pixels = bytearray(b"\x10\x28\x3f" * (width * height))
    lines = c.expected_text(prefix, content, fixture_font(), b"\x41\xba\xd7", b"\x10\x28\x3f")
    for y, line in enumerate(lines):
        for x in range(len(line) // 3):
            px = col*10+x
            if px >= width:
                if clipped:
                    continue
                raise ValueError("synthetic text exceeds native frame")
            offset = ((row * 20+y)*width+px)*3
            pixels[offset:offset+3] = line[x*3:x*3+3]
    return f"P6\n{width} {height}\n255\n".encode() + pixels


class RegisterPixelChecks(unittest.TestCase):
    def check(self, width, height):
        with tempfile.TemporaryDirectory() as tmp:
            frame = Path(tmp) / "frame.ppm"
            frame.write_bytes(ppm(width, height, HEAD))
            good = c.check_scanout(frame, HEAD, fixture_font())
            self.assertTrue(good["accepted"])
            self.assertEqual(good["mismatching_pixels"], 0)
            bad = c.check_scanout(frame, TAIL_CHANGED, fixture_font())
            self.assertFalse(bad["accepted"])
            self.assertGreater(bad["mismatching_pixels"], 0)
            self.assertGreaterEqual(bad["first_mismatch"][0], 20+10*(5+47))
            return frame

    def test_800x600_full_head_and_changed_tail(self):
        self.check(800, 600)

    def test_1024x768_full_head_and_changed_tail(self):
        self.check(1024, 768)

    def test_1920x1080_full_head_and_changed_tail(self):
        self.check(1920, 1080)

    def test_old_117_cell_row_cannot_fit_smaller_displays(self):
        for width, height in [(800, 600), (1024, 768)]:
            with tempfile.TemporaryDirectory() as tmp:
                frame = Path(tmp) / "old.ppm"
                frame.write_bytes(ppm(width, height, HEAD, prefix="THE KERNEL'S REGISTER: 48 events, chain intact, head ", row=1, clipped=True))
                with self.assertRaisesRegex(ValueError, "does not fit"):
                    c.check_scanout(frame, HEAD, fixture_font(), prefix="THE KERNEL'S REGISTER: 48 events, chain intact, head ", row=1)

    def test_last_cell_corrupted(self):
        with tempfile.TemporaryDirectory() as tmp:
            frame = Path(tmp) / "frame.ppm"
            data = bytearray(ppm(800, 600, HEAD))
            off = data.index(b"\n255\n") + 5
            px = 20+(5+63)*10
            at = off+3*(40*800+px)
            data[at] ^= 255
            frame.write_bytes(data)
            self.assertFalse(c.check_scanout(frame, HEAD, fixture_font())["accepted"])

    def test_wrong_claimed_hash_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            frame = Path(tmp) / "frame.ppm"
            frame.write_bytes(ppm(800, 600, HEAD))
            for h in (HEAD.upper(), HEAD[:-1], HEAD+"0", "q"*64):
                with self.assertRaises(ValueError):
                    c.check_scanout(frame, h, fixture_font())

    def test_reject_bad_ppm_size(self):
        with tempfile.TemporaryDirectory() as tmp:
            frame = Path(tmp) / "frame.ppm"
            frame.write_bytes(b"P6\n800 600\n255\n" + b"0"*9)
            with self.assertRaisesRegex(ValueError, "pixel count"):
                c.check_scanout(frame, HEAD, fixture_font())


if __name__ == "__main__":
    unittest.main()
