from pathlib import Path
import hashlib, json, os, runpy, struct, subprocess, tempfile, uuid
p=Path('service/gate395-b3')
img=p/'coder-gate.iso'
expected=json.loads((p/'coder-inventory.json').read_bytes())
model='1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32'
def digest(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()
def verify(root):
    actual={f.relative_to(root).as_posix() for f in root.rglob('*') if f.is_file()}
    if actual!=set(expected): raise ValueError('path inventory differs')
    if {d.relative_to(root).as_posix() for d in root.rglob('*') if d.is_dir()}!={'EFI','EFI/BOOT','PCPOS'}:
        raise ValueError('directory inventory differs')
    whole=hashlib.sha256()
    for name in sorted(expected):
        f=root/name
        if f.stat().st_size!=expected[name]['bytes'] or digest(f)!=expected[name]['sha256']: raise ValueError(name)
        if name.startswith('PCPOS/shard-'):
            with f.open('rb') as stream:
                while data:=stream.read(1<<20): whole.update(data)
    if whole.hexdigest()!=model:raise ValueError('model differs')
    return len(actual)
def read_esp(f):
    runpy.run_path(str(p/'source/os-docs/installed-acceptance/gpt_integrity.py'))['require_gpt_mirror'](f.fileno(),img.stat().st_size)
    f.seek(512);hdr=f.read(512)
    tab,num,size=struct.unpack_from('<QII',hdr,72)
    f.seek(tab*512);entries=[f.read(size) for _ in range(num)]
    esp=[e for e in entries if e[:16]==uuid.UUID('C12A7328-F81F-11D2-BA4B-00A0C93EC93B').bytes_le]
    if len(esp)!=1: raise ValueError('expected a unique ESP')
    lo,hi=struct.unpack_from('<QQ',esp[0],32)
    lba=None
    for n in range(16,80):
        f.seek(n*2048);q=f.read(2048)
        if q[0]==0 and q[1:6]==b'CD001' and q[7:30]==b'EL TORITO SPECIFICATION':
            lba=struct.unpack_from('<I',q,71)[0];break
        if q[0]==255: break
    if not lba: raise ValueError('boot record missing')
    f.seek(lba*2048);cat=f.read(2048)
    if cat[0:2]!=b'\x01\xef' or sum(struct.unpack('<16H',cat[:32]))%65536 or cat[30:34]!=b'\x55\xaa\x88\x00':
        raise ValueError('invalid boot catalog')
    boot_lba=struct.unpack_from('<I',cat,40)[0]
    if boot_lba*2048!=lo*512: raise ValueError('optical and USB boot locations differ')
    return lo*512,(hi-lo+1)*512
with img.open('rb') as f:
    offset,esp_bytes=read_esp(f)
with tempfile.TemporaryDirectory(dir=p) as t:
    tree=Path(t)/'joliet'
    subprocess.run(['xorriso','-read_fs','norock','-osirrox','on','-indev',img,'-extract','/',tree],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    joliet=verify(tree)
with tempfile.TemporaryDirectory(dir=p) as t:
    subprocess.run(['mcopy','-sm','-i',str(img)+'@@'+str(offset),'::/*',t],check=True)
    fat=verify(Path(t))
report=dict(candidate_source='09533036136c5a8748eb2246507585d77f3387aa',iso_bytes=img.stat().st_size,iso_sha256=digest(img),esp_start_bytes=offset,esp_bytes=esp_bytes,primary_and_backup_gpt_verified=True,el_torito_and_gpt_esp_agree=True,joliet_files=joliet,embedded_esp_files=fat,model_bytes=491400064,shards=8,model_sha256=model,approved_inventory_sha256=digest(p/'coder-inventory.json'),readback='Independently reopened the finished ISO using xorriso -read_fs norock and mtools at the verified ESP offset. Verified the exact files, directories, lengths, SHA-256 and ordered model in each namespace.')
(p/'written-coder.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
print(json.dumps(report))
