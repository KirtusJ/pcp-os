"""Controls for binding a decoded launcher name to its visible selection and grants."""
from pathlib import Path
import runpy
import unittest

optical = runpy.run_path(str(Path(__file__).with_name('check.py')))
selected_program = optical['selected_program']
launcher_grants = optical['launcher_grants']


class LauncherSelection(unittest.TestCase):
    def test_graphical_grants_belong_to_selected_entry(self):
        prefix = '0123456789abcdef'
        digest = prefix * 4
        permissions = launcher_grants(sound=True)
        entry = '  > surf.elf                          SHA-256 ' + prefix + '...'
        grants = '    may use: display, keyboard, sound, loader files, store'
        neighbor = ['    rdesk.elf', grants]
        def chosen(lines, program='surf.elf', sha=digest):
            return selected_program([entry, *lines, '', *neighbor], b'', program, permissions, sha)

        self.assertTrue(chosen([grants]))
        self.assertTrue(chosen([grants, '    pinned to the desktop']))
        self.assertFalse(chosen([grants], 'rdesk.elf'))
        self.assertFalse(chosen([grants], 'surf'))
        self.assertFalse(chosen([grants], sha='f' * 64))
        for wrong in ['', '    may use: display', '    may use: console only', grants.replace('sound, ', ''),
                      grants.replace('store', 'store, store'), grants.replace('store', 'console only')]:
            with self.subTest(wrong=wrong):
                self.assertFalse(chosen([wrong]))
        self.assertFalse(chosen([grants, '             second serial port']))

    def test_active_image_grants_and_wrapped_lines(self):
        digest, other = 'a' * 64, 'b' * 64
        features = ('game-link', 'guest-ethernet')
        command = 'net_image=' + digest + ' link_image=' + digest
        def expected(cmd=command, sha=digest):
            return launcher_grants(sound=False, command_line=cmd, digest=sha, features=features)

        base = ('display', 'keyboard', 'loader files', 'store')
        self.assertEqual(expected(), base + ('raw Ethernet frames', 'second serial port'))
        self.assertEqual(expected(sha=other), base)
        self.assertEqual(expected(command + ' grant=none'), ('console only',))
        self.assertEqual(expected(command + ' net_image=' + digest), base + ('second serial port',))
        self.assertEqual(launcher_grants(sound=True, command_line=command, digest=digest),
                         ('display', 'keyboard', 'sound', 'loader files', 'store'))
        entry = '  > surf.elf                      SHA-256 ' + digest[:16] + '...'
        first = '    may use: display, keyboard, loader files, store,'
        last = '             raw Ethernet frames, second serial port'
        def chosen(lines, permissions):
            return selected_program([entry, *lines, ''], b'', 'surf.elf', permissions, digest)

        self.assertTrue(chosen([first, last], expected()))
        self.assertFalse(chosen([first], expected()))
        self.assertFalse(chosen([first, last[1:]], expected()))
        self.assertFalse(chosen([first, last], expected(sha=other)))
        self.assertFalse(chosen([first, last.replace('raw Ethernet frames', 'second serial port')], expected()))
        self.assertTrue(chosen(['    may use: console only'], expected(command + ' grant=none')))

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
                permissions = launcher_grants(sound=False, command_line='grant=none')
                self.assertTrue(selected_program(rows, frame, expected, permissions))
                self.assertFalse(selected_program(rows, frame, expected, launcher_grants(sound=False)))
                rows[-5] = '  Access: second serial port'
                self.assertFalse(selected_program(rows, frame, expected, permissions))
                rows[-5] = ''
                self.assertFalse(selected_program(rows, frame, programs[1 - selected], permissions))
                # The decoded '?' alone must not stand in for a full teal strip.
                pixels[offset:offset + 3] = b'\x00\x00\x00'
                self.assertFalse(selected_program(rows, header + pixels, expected, permissions))


if __name__ == '__main__':
    unittest.main()
