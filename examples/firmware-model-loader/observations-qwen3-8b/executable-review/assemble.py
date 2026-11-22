from pathlib import Path
from tempfile import TemporaryDirectory
from zipfile import ZipFile
import hashlib
import json
import os
import shutil
import struct
import subprocess
import time

root = Path.cwd()
lab = root / 'service/qwen3-release'
env = dict(os.environ, TZ='UTC', LC_ALL='C', SOURCE_DATE_EPOCH='1794614400',
           PATH=f"{root}/runtime/usr/bin:{root}/runtime/usr/sbin:" + os.environ['PATH'])
model = lab / 'Qwen3-8B-Q4_K_M.gguf'
packed = lab / 'packed'
model_sha = 'd98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785'
manifest_sha = '2c02ff6f00a2ee802ce09648dfbc56dda38a64b009635f0aeb383f09e1af67dc'
def digest(p):
    with p.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()
def run(*cmd, **kwargs):
    subprocess.run(cmd, env=env, check=True, **kwargs)
def record(name, obj):
    (lab / name).write_text(json.dumps(obj, indent=2, sort_keys=True) + '\n')
def stamp(text):
    print(time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()), text, flush=True)

if not packed.exists():
    if digest(model) != model_sha:
        raise ValueError('publisher digest mismatch')
    run('cargo','run','--locked','--release','-p','pcpos-model-media','--example','pack','--',
        model, model_sha, packed, cwd=root/'service/tb22/pcp-os/PCP_OS_Rust')
manifest = (packed/'model.manifest').read_bytes()
assert manifest == (lab/'pins/model.manifest').read_bytes()
assert manifest[:8] == b'PCPMDL01' and len(manifest) == 2456
assert hashlib.sha256(manifest).hexdigest() == manifest_sha and manifest[24:56].hex() == model_sha
total, stride, n = struct.unpack_from('<QII', manifest, 8)
assert (total, stride, n) == (5027783488, 67108864, 75)
files = {}
inputs = json.loads((lab/'efi-inputs.json').read_bytes())['files']
for name, sha in inputs.items():
    path = lab/'pins'/name
    assert digest(path) == sha
    files['PCPOS/'+name] = (path, sha)
files['EFI/BOOT/BOOTX64.EFI'] = (lab/'BOOTX64.EFI', digest(lab/'BOOTX64.EFI'))
for i in range(n):
    name = f'shard-{i:05}.bin'
    path = packed/name
    expected = manifest[56+32*i:88+32*i].hex()
    assert path.stat().st_size == min(total-i*stride, stride)
    assert digest(path) == expected
    files['PCPOS/'+name] = (path, expected)
assert len(files) == 80
record('expected-files.json', {name:{'bytes':p.stat().st_size,'sha256':sha} for name,(p,sha) in files.items()})
stamp('independent packer manifest and 75 source shard digests match')
volume = lab/'worker24-qwen3-8b.fat'
if not volume.exists():
    partial = lab/'volume-in-progress.fat'
    with partial.open('wb') as blank:
        blank.truncate(5010 << 20)
    run('mkfs.fat','--invariant','-F','32','-n','PCPOS',partial)
    for folder in ('EFI','EFI/BOOT','PCPOS'):
        run('mmd','-i',partial,'::/'+folder)
    for name,(p,_) in sorted(files.items()):
        run('mcopy','-i',partial,p,'::/'+name)
    os.replace(partial, volume)
whole = hashlib.sha256()
with TemporaryDirectory(dir=lab) as tmp:
    recovered = Path(tmp)/'reopened'
    for name,(p,sha) in sorted(files.items()):
        run('mcopy','-o','-i',volume,'::/'+name,recovered)
        if recovered.stat().st_size != p.stat().st_size or digest(recovered) != sha:
            raise ValueError('FAT readback differs: ' + name)
        if name.startswith('PCPOS/shard-'):
            with recovered.open('rb') as f:
                while chunk := f.read(4<<20):
                    whole.update(chunk)
assert whole.hexdigest() == model_sha
record('fat-readback.json', {'bytes':volume.stat().st_size,'sha256':digest(volume),
        'verified_files':len(files), 'verified_shards':n,'ordered_model_sha256':whole.hexdigest(),
        'tail_shard_bytes':(packed/'shard-00074.bin').stat().st_size})
stamp('written FAT independently reopened; ordered model, shards and five launch files verified')
# The original is a locally downloaded staging copy. Both verified FAT and packed
# shards retain every source byte and can reconstitute it in order.
if model.exists() and shutil.disk_usage(lab).free < (23 << 30):
    model.unlink()
if shutil.disk_usage(lab).free < (20 << 30):
    raise OSError('space needed for the stock ISO master; verified FAT retained')
iso = lab/'PCPOS-Qwen3-8B.iso'
helper = root/'service/r382/examples/model-media-iso/build.py'
if not iso.exists():
    run('python',helper,volume,iso)
stamp('stock hybrid ISO master complete')
