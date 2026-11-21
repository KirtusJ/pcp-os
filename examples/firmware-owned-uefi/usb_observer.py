"""Host observations for a disposable Q35/OVMF VM. The caller controls the guest and its USB image."""
import hashlib
from pathlib import Path
import shutil
import struct
import uuid


AUTH_STORE = uuid.UUID('aaf32c78-947b-439a-a180-2e144ec37792')
NV_VOLUME = uuid.UUID('fff12b8d-7696-4c8b-a985-2747075b4f50')
COUNTERS = ('rd_operations', 'rd_bytes', 'wr_operations', 'wr_bytes', 'failed_wr_operations',
            'invalid_wr_operations', 'flush_operations', 'unmap_operations', 'unmap_bytes',
            'failed_unmap_operations', 'invalid_unmap_operations')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def variables(raw):
    """Active variables in the pinned x64 OVMF authenticated store, per EDK II VariableFormat.h."""
    if raw[40:44] != b'_FVH' or raw[16:32] != NV_VOLUME.bytes_le:
        raise ValueError('expected an EDK II system NV data firmware volume')
    header = struct.unpack_from('<H', raw, 48)[0]
    size, formatted, healthy = struct.unpack_from('<IBB', raw, header + 16)
    end = header + size
    if (raw[header:header + 16] != AUTH_STORE.bytes_le or (formatted, healthy) != (0x5a, 0xfe)
            or size < 28 or end > len(raw)):
        raise ValueError('expected a healthy OVMF authenticated variable store')
    pos = (header + 28 + 3) & ~3
    active, transitioning = {}, {}
    while pos < end and raw[pos:pos + 2] != b'\xff\xff':
        if pos + 60 > end or raw[pos:pos + 2] != b'\xaa\x55':
            raise ValueError(f'invalid variable header at {pos}')
        state, attributes = struct.unpack_from('<BxI', raw, pos + 2)
        name_size, data_size = struct.unpack_from('<II', raw, pos + 36)
        name_at = pos + 60
        data_at = name_at + name_size
        next_pos = data_at + data_size
        if name_size < 2 or name_size % 2 or next_pos > end or raw[data_at - 2:data_at] != b'\0\0':
            raise ValueError(f'invalid variable sizes at {pos}')
        if state in (0x3f, 0x3e):  # VAR_ADDED or a last value in a deletion transition
            guid = uuid.UUID(bytes_le=raw[pos + 44:pos + 60])
            name = raw[name_at:data_at - 2].decode('utf-16le')
            table = active if state == 0x3f else transitioning
            table[f'{guid}/{name}'] = {'attributes': attributes, 'bytes': data_size,
                                      'sha256': sha(raw[data_at:next_pos])}
        pos = (next_pos + 3) & ~3
    if any(byte != 0xff for byte in raw[pos:end]):
        raise ValueError('unparsed data in the free variable-store area')
    return transitioning | active


def create(directory, vars_template, make_gpt_disk):
    """Create regular files in a new trial directory; supply optical-disk-check's make_gpt_disk."""
    directory = Path(directory).resolve()
    disk, store = directory / 'internal.raw', directory / 'vars.fd'
    baseline, _ = make_gpt_disk(512)
    with disk.open('xb') as out:
        out.write(baseline)
    with Path(vars_template).open('rb') as source, store.open('xb') as out:
        shutil.copyfileobj(source, out)
    return ['-drive', f'if=none,id=sentinel,format=raw,cache=writethrough,file={disk}',
            '-device', 'nvme,id=internal,serial=PCP-USB-INTERNAL,drive=sentinel',
            '-drive', f'if=pflash,unit=1,id=varstore,format=raw,cache=writethrough,file={store}']


def capture(rpc, directory, stage):
    """Snapshot after -S, QMP stop or guest shutdown with -no-shutdown."""
    directory = Path(directory).resolve()
    status = rpc('query-status')['status']
    if status not in ('prelaunch', 'paused', 'shutdown'):
        raise ValueError('pause vCPUs before capturing the NVMe and OVMF stores')
    blocks = {row['device']: row for row in rpc('query-block') if row.get('device')}
    stats = {row['device']: row['stats'] for row in rpc('query-blockstats') if row.get('device')}
    counters = {}
    for device, name in (('sentinel', 'internal.raw'), ('varstore', 'vars.fd')):
        media = blocks[device]['inserted']
        if media['ro'] or media['drv'] != 'raw' or Path(media['file']).resolve() != directory / name:
            raise ValueError(f'{device} has the wrong media attachment')
        counters[device] = {k: stats[device][k] for k in COUNTERS}
    raw = (directory / 'vars.fd').read_bytes()
    snapshot = directory / f'{stage}-vars.fd'
    with snapshot.open('xb') as out:
        out.write(raw)
    return {'stage': stage, 'qemu_status': status, 'counters': counters,
            'nvme_sha256': sha((directory / 'internal.raw').read_bytes()),
            'vars_sha256': sha(raw), 'vars_snapshot': snapshot.name, 'variables': variables(raw)}


def compare(directory, before, after):
    """Report observable changes during an interval, including firmware writes during boot."""
    old = (directory / before['vars_snapshot']).read_bytes()
    new = (directory / after['vars_snapshot']).read_bytes()
    if len(old) != len(new) or sha(old) != before['vars_sha256'] or sha(new) != after['vars_sha256']:
        raise ValueError('variable-store evidence does not match snapshots')
    left, right = before['variables'], after['variables']
    counts = {device: {key: value - before['counters'][device][key] for key, value in values.items()}
              for device, values in after['counters'].items()}
    return {'from': before['stage'], 'to': after['stage'],
            'nvme_bytes_preserved': before['nvme_sha256'] == after['nvme_sha256'], 'counter_deltas': counts,
            'vars_bytes_changed': sum(a != b for a, b in zip(old, new)),
            'variables_added': sorted(right.keys() - left.keys()), 'variables_removed': sorted(left.keys() - right.keys()),
            'variables_modified': sorted(key for key in left.keys() & right.keys() if left[key] != right[key])}
