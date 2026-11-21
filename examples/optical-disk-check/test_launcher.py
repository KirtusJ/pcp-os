"""Controls for binding a decoded launcher name to its visible selection and grants."""
from pathlib import Path
import runpy
import unittest

selected_program = runpy.run_path(str(Path(__file__).with_name('check.py')))['selected_program']


class LauncherSelection(unittest.TestCase):
    def test_graphical_grants_belong_to_selected_entry(self):
        rows = ['  > surf.elf    118 KiB', '    may use: display, keyboard',
                '    rdesk.elf    53 KiB', '    may use: console only']
        self.assertTrue(selected_program(rows, b'', 'surf.elf'))
        self.assertFalse(selected_program(rows, b'', 'rdesk.elf'))
        self.assertFalse(selected_program(rows, b'', 'surf'))
        rows[1] = ''
        self.assertFalse(selected_program(rows, b'', 'surf.elf'))

    def test_compact_strip_follows_selected_filename(self):
        width, height = 640, 480
        header = f'P6\n{width} {height}\n255\n'.encode()
        programs = ['queue.elf', 'second.elf']
        for selected, expected in enumerate(programs):
            with self.subTest(selected=expected):
                rows = [''] * (height // 20)
                pixels = bytearray(width * height * 3)
                for i, name in enumerate(programs):
                    rows[5 + i] = ('  ?' if i == selected else '   ') + name
                rows[-7] = '  Access: console only'
                rows[-2] = '  Up/Down choose   Enter open'
                start_y = (5 + selected) * 20
                for y in range(start_y, start_y + 20):
                    offset = (y * width + 20) * 3
                    pixels[offset:offset + 9] = bytes.fromhex('41bad7') * 3
                frame = header + pixels
                self.assertTrue(selected_program(rows, frame, expected))
                self.assertFalse(selected_program(rows, frame, programs[1 - selected]))
                # The decoded '?' alone must not stand in for a full teal strip.
                pixels[offset:offset + 3] = b'\x00\x00\x00'
                self.assertFalse(selected_program(rows, header + pixels, expected))


if __name__ == '__main__':
    unittest.main()
