"""Replay the retained disk and audiovisual observations of the first source-built tester CD."""
from hashlib import sha256
import json
from pathlib import Path
import runpy
import tarfile
from tempfile import TemporaryDirectory

HERE = Path(__file__).resolve().parent
ARCHIVE = HERE / 'source-bios-160b2d50.tar.xz'
ARCHIVE_SHA = 'e58ba7fe3a4e3b932bcc496f9017c271af44d505c93ba0454362091e6e54a899'
ISO_SHA = '160b2d5057ae435dbdce48c47697e0a269c175333a60ab977968cda77a881409'
KERNEL = '82b96a61b5e949caa63abb9e262f0c7e135de443c8a8576344687aa84f2d6cde'
disk = runpy.run_path(str(HERE.parent / 'check.py'))
media = runpy.run_path(str(HERE.parents[2] / 'apps/surfbird/guest/replay_local_media.py'))


def require(ok, detail):
    if not ok:
        raise ValueError(detail)


def read_json(path):
    return json.loads(path.read_bytes())


def check_session(directory, *, stopped):
    lines = (directory / 'serial.log').read_text().splitlines()
    startup = next(i for i, line in enumerate(lines) if line.startswith('starting surf.elf'))
    require(sum(line.startswith('  pass ') for line in lines[:startup]) == 71, directory)
    require(any(KERNEL + ' (measured; equal to the value the loader named)' in line for line in lines), directory)
    require(any('surf.elf ended DONE: exit 0' in line for line in lines), directory)
    require(any(line.startswith('ALL SELF-TESTS PASS.') for line in lines), directory)
    if stopped:
        require(any(line.startswith('surf.elf ended STOPPED') for line in lines), directory)
    for name in (['launcher', 'after-playback', 'after-stop', 'register', 'returned']
                 if stopped else ['launcher', 'returned']):
        regions = (disk['REGISTER_KEYS'],) if name == 'register' else (disk['VERIFIED'], disk['LAUNCHER_KEYS'])
        require(disk['matches_frame']((directory / (name + '.ppm')).read_bytes(), regions), directory / name)


def check_disk(root, controller, machine, sector):
    normal = root / controller / (machine + '-session')
    control = root / controller / (machine + '-write-control')
    fixture, marker = disk['make_disk']() if controller == 'ide' else disk['make_gpt_disk'](sector)
    before = sha256(fixture).hexdigest()
    changed = bytearray(fixture)
    changed[marker:marker + sector] = b'\x5a' * sector
    session = read_json(normal / 'result.json')
    control_receipt = read_json(control / 'result.json')
    for path, receipt, expected_after in [(normal, session, before), (control, control_receipt, sha256(changed).hexdigest())]:
        require(receipt['case'] == path.name, path)
        argv = receipt['argv']
        require(argv[argv.index('-machine') + 1] == machine, path)
        require(argv[argv.index('-nic') + 1] == 'none' and '-icount' not in argv, path)
        require(receipt['iso_sha256'] == ISO_SHA and receipt['kernel_measurement'] == KERNEL, path)
        require(receipt['logical_sector_bytes'] == sector and receipt['before_sha256'] == before, path)
        require(receipt['after_sha256'] == expected_after, path)
    fields = ('wr_operations', 'wr_bytes', 'wr_highest_offset', 'failed_wr_operations', 'invalid_wr_operations')
    for stage in ('before_cpu_start', 'at_launcher', 'before_poweroff', 'at_end'):
        require(all(session[stage][field] == 0 for field in fields), (normal, stage))
    require(session['at_end']['rd_bytes'] > 0, normal)
    require(any(e['event'] == 'SHUTDOWN' and e['data'].get('guest') is True for e in session['events']), normal)
    check_session(normal, stopped=True)


def check_playback(root, build):
    scratch = root / 'generated'
    scratch.mkdir()
    capture = root / 'av-q35'
    frames = media['make_media'](scratch, 0)
    for name in ('clip.ppm', 'clip.wav'):
        require(sha256((scratch / name).read_bytes()).hexdigest() == build['files'][name]['sha256'], name)
    indices = [media['screenshot_frame'](capture / f'frame-{i}.ppm', frames) for i in range(len(frames))]
    recorded = read_json(capture / 'result.json')
    require(recorded['iso_sha256'] == ISO_SHA, capture)
    require(indices == recorded['screen_indices'] == list(range(32)), 'frame ordering')
    tones = media['captured_audio'](capture / 'capture.wav', 0)
    require(tones == recorded['captured_audio'], 'audio observations')
    check_session(capture, stopped=False)


def main():
    require(sha256(ARCHIVE.read_bytes()).hexdigest() == ARCHIVE_SHA, 'archive digest')
    with TemporaryDirectory() as temp:
        root = Path(temp)
        with tarfile.open(ARCHIVE, 'r:xz') as archive:
            archive.extractall(root, filter='data')
        build = read_json(root / 'build/receipt.json')
        require(build['iso_sha256'] == ISO_SHA and build['kernel_measurement'] == KERNEL, 'build receipt')
        for machine in ('pc', 'q35'):
            check_disk(root, 'ide', machine, 512)
        for sector in (512, 4096):
            check_disk(root, f'nvme-{sector}', 'q35', sector)
        check_playback(root, build)
    print('Verified four disk sessions, four write controls, 32 video frames and four stereo tones.')


if __name__ == '__main__':
    main()
