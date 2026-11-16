"""Independent reference raster and adverse controls for the fixed preview."""
import ast
import hashlib
from pathlib import Path
import tempfile
import unittest

import screen


BACKGROUND = (24, 54, 79)
FOREGROUND = (242, 247, 252)
AMBER = (227, 162, 75)
TAG_GLYPHS = {
    "O": [14, 17, 17, 17, 17, 17, 14], "U": [17, 17, 17, 17, 17, 17, 14],
    "T": [31, 4, 4, 4, 4, 4, 4], "P": [30, 17, 17, 30, 16, 16, 16],
    "L": [16, 16, 16, 16, 16, 16, 31],
}


def put_pixel(pixels, x, y, color):
    start = 3 * (y * screen.WIDTH + x)
    pixels[start:start + 3] = bytes(color)


def put_row(pixels, y, kind, message, glyphs):
    if kind:
        for index, char in enumerate(kind):
            for row, bits in enumerate(TAG_GLYPHS[char]):
                for col in range(5):
                    if bits & (16 >> col):
                        put_pixel(pixels, 336 + 6*index + col, y + row, AMBER)
    for index, char in enumerate(message):
        glyph = glyphs[ord(char) - 32]
        for row in range(10):
            for col in range(5):
                pos = row * 20 + col * 2
                coverage = sum(glyph[pos + k] for k in (0, 1, 10, 11))
                color = [a + (b-a)*coverage//1020 for a, b in zip(BACKGROUND, FOREGROUND)]
                put_pixel(pixels, 422 + 6*index + col, y + row, color)


class ScreenOutput(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[2]
        font = root / "PCP_OS_Rust/crates/pcpos-kernel/src/font.rs"
        source = font.read_bytes()
        if hashlib.sha256(source).hexdigest() != "37ee1cdb046e1ee7c597a7e8f61c307934a31ef7443cfb944a22b284cac189a5":
            raise ValueError("reference font differs from the fixed preview")
        body = source.decode().split("pub static GLYPHS:", 1)[1].split(" = ", 1)[1]
        cls.glyphs = ast.literal_eval(body.strip().removesuffix(";"))

    def test_recompute_and_distinguish_output(self):
        def blank():
            return bytearray(bytes(BACKGROUND) * screen.WIDTH * screen.HEIGHT)

        for text in ("original", "replacement", "changed"):
            pixels = blank()
            put_row(pixels, 246, "OUTPUT", text, self.glyphs)
            self.assertEqual(screen.matching_row(pixels, text), 246)
            self.assertEqual(screen.crop_digest(pixels, 422, 246, 504), screen.OUTPUT_FIELDS[text])
        for label, text in [("", "original"), ("OUTPUT", "print original"), ("OUTPUT", "Original"),
                            ("OUTPUT", "originalx"), ("OUTPUT", "replacement"), ("OUTPUT", "")]:
            with self.subTest(label=label, text=text):
                pixels = blank()
                put_row(pixels, 246, label, text, self.glyphs)
                self.assertIsNone(screen.matching_row(pixels, "original"))
        for conflict in ("changed", "replacement", "original"):
            with self.subTest(conflict=conflict):
                pixels = blank()
                put_row(pixels, 190, "OUTPUT", "original", self.glyphs)
                put_row(pixels, 218, "OUTPUT", conflict, self.glyphs)
                self.assertIsNone(screen.matching_row(pixels, "original"))
        pixels = blank()
        put_row(pixels, 246, "OUTPUT", "original", self.glyphs)
        put_pixel(pixels, 925, 255, AMBER)
        self.assertIsNone(screen.matching_row(pixels, "original"))

    def test_refusal_requires_the_tool_tag_and_complete_message_on_one_row(self):
        for kind, message, accepted in [
            ("TOOL", "Request denied", True),
            ("OUTPUT", "Request denied", False),
            ("TOOL", "Request denied!", False),
            ("TOOL", "request denied", False),
        ]:
            with self.subTest(kind=kind, message=message):
                pixels = bytearray(bytes(BACKGROUND) * screen.WIDTH * screen.HEIGHT)
                put_row(pixels, 246, kind, message, self.glyphs)
                self.assertEqual(screen.matching_row(pixels, "Request denied", tag=screen.TOOL_TAG),
                                 246 if accepted else None)
        pixels = bytearray(bytes(BACKGROUND) * screen.WIDTH * screen.HEIGHT)
        put_row(pixels, 218, "TOOL", "", self.glyphs)
        put_row(pixels, 246, "", "Request denied", self.glyphs)
        self.assertIsNone(screen.matching_row(pixels, "Request denied", tag=screen.TOOL_TAG))

    def test_checks_entire_event_label_before_message(self):
        for tag, name, text in [(screen.OUTPUT_TAG, "OUTPUT", "original"),
                                (screen.TOOL_TAG, "TOOL", "Request denied")]:
            pixels = bytearray(bytes(BACKGROUND) * screen.WIDTH * screen.HEIGHT)
            put_row(pixels, 218, name, text, self.glyphs)
            self.assertEqual(screen.matching_row(pixels, text, tag=tag), 218)
            for x in (372, 408, 421):
                with self.subTest(label=name, x=x):
                    altered = pixels.copy()
                    put_pixel(altered, x, 220, AMBER)
                    self.assertIsNone(screen.matching_row(altered, text, tag=tag))

    def test_source_echo_precedes_genuine_installed_output(self):
        """A source echo must not suppress the separately rendered program result."""
        pixels = bytearray(bytes(BACKGROUND) * screen.WIDTH * screen.HEIGHT)
        put_row(pixels, 218, "OUTPUT", "print replacement", self.glyphs)
        put_row(pixels, 330, "OUTPUT", "replacement", self.glyphs)
        self.assertEqual(screen.matching_row(pixels, "replacement"), 330)
        # A second independently known result remains a conflicting outcome.
        put_row(pixels, 358, "OUTPUT", "changed", self.glyphs)
        self.assertIsNone(screen.matching_row(pixels, "replacement"))

    def test_frame_boundaries(self):
        header = b"P6\n960 540\n255\n"
        data = bytes(3 * screen.WIDTH * screen.HEIGHT)
        with tempfile.TemporaryDirectory() as tmp:
            frame = Path(tmp) / "fixture.ppm"
            for incomplete in (b"", header[:8], header + data[:-1]):
                frame.write_bytes(incomplete)
                self.assertIsNone(screen.read_frame(frame))
            for invalid in (b"P6\n540 960\n255\n" + data, header + data + b"x"):
                frame.write_bytes(invalid)
                with self.assertRaises(ValueError):
                    screen.read_frame(frame)
            frame.write_bytes(header + data)
            self.assertEqual(screen.read_frame(frame), data)


if __name__ == "__main__":
    unittest.main()
