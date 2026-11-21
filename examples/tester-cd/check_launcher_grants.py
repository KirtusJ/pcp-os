"""Replay the chosen card and two host-only RGB controls on each native tester-CD capture."""
import argparse
from collections import Counter
import json
from pathlib import Path
import runpy


HERE = Path(__file__).resolve().parent
OPTICAL = runpy.run_path(str(HERE.parent / 'optical-disk-check/check.py'))
SCREEN = OPTICAL['SCREEN']


def replace_text(frame, line, column, original, replacement, shapes):
    """Redraw only these text cells, with the original plate background and kernel font."""
    magic, size, depth, pixels = frame.split(b'\n', 3)
    width = int(size.split()[0])
    changed = bytearray(pixels)
    for i, char in enumerate(replacement.ljust(len(original))):
        offsets = [3 * ((line * 20 + y) * width + (column + i) * 10 + x)
                   for y in range(20) for x in range(10)]
        colors = Counter(pixels[start:start + 3] for start in offsets)
        ground = colors.most_common(1)[0][0]
        # The OCR reads the font's positive coverage. This uses solid white ink
        # for a forged word, so it also decodes over originally blank cells.
        for start, ink in zip(offsets, shapes[char]):
            changed[start:start + 3] = b'\xff\xff\xff' if ink else ground
    return b'\n'.join((magic, size, depth, changed))


def check_capture(path, program, digest, glyphs, shapes):
    frame = path.read_bytes()
    rows = SCREEN['read_screen'](frame, glyphs)
    permissions = OPTICAL['launcher_grants'](sound=True)

    def verdicts(view, raw):
        return (SCREEN['graphical_selection'](view, program, permissions, digest),
                OPTICAL['selected_program'](view, raw, program, permissions, digest))

    if not rows or verdicts(rows, frame) != (True, True):
        raise ValueError(f'{path}: original selection or permission disclosure failed')
    entry = next(i for i, row in enumerate(rows) if '> ' + program + ' ' in row)
    grants = 'may use: ' + ', '.join(permissions)
    line, column = entry + 1, rows[entry].index('> ' + program + ' ') + 2
    if not rows[line].startswith(grants, column):
        raise ValueError(f'{path}: expected a complete single-line disclosure on the native tester CD')
    prefix = 'may use: display'
    controls = [('erased suffix', column + len(prefix), grants[len(prefix):], ''),
                ('forged console-only policy', column, grants, 'may use: console only')]
    for name, start, original, replacement in controls:
        changed = replace_text(frame, line, start, original, replacement, shapes)
        view = SCREEN['read_screen'](changed, glyphs)
        expected = rows.copy()
        expected[line] = (rows[line][:start] + replacement.ljust(len(original)) + rows[line][start + len(original):]).rstrip()
        if view != expected or verdicts(view, changed) != (False, False):
            raise ValueError(f'{path}: {name} did not produce the expected reading and rejection')
    print(f'{path.parent.name}/{path.name}: original accepted; erased suffix and forged grants rejected by both checkers')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('receipt', type=Path)
    parser.add_argument('evidence', type=Path)
    args = parser.parse_args()
    receipt = json.loads(args.receipt.read_text())
    files = receipt.get('on_disc', receipt.get('files', {}))
    captures = [('launcher.ppm', 'surf.elf')]
    if 'rdesk.elf' in files:
        captures.append(('launcher-after-queue.ppm', 'rdesk.elf'))
    glyphs = SCREEN['kernel_glyphs']()
    shapes = {char: shape for shape, char in glyphs.items()}
    for machine in ('pc', 'q35'):
        for clock in ('ordinary', 'counted'):
            directory = args.evidence / f'{machine}-{clock}'
            for capture, program in captures:
                check_capture(directory / capture, program, files[program]['sha256'], glyphs, shapes)


if __name__ == '__main__':
    main()
