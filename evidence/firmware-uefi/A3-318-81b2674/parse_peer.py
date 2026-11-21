"""Read the unmodified launcher parser against B3's archived USB serial and pixel dimensions."""
from pathlib import Path
import hashlib,json,runpy,sys,tarfile
root=Path(__file__).resolve().parent
reader=runpy.run_path(str(root/'src/examples/tester-cd/check.py'))['launcher_boot_profile']
file=root/sys.argv[1]
assert hashlib.sha256(file.read_bytes()).hexdigest()==sys.argv[2]
records={}
with tarfile.open(file,'r|xz') as archive:
    for entry in archive:
        if not entry.isfile() or not entry.name.startswith('candidate/'):
            continue
        path=Path(entry.name)
        if path.name not in {'kernel-display.ppm','serial.txt'}:
            continue
        record=records.setdefault(str(path.parent),{})
        f=archive.extractfile(entry)
        if path.name.endswith('ppm'):
            assert f.readline().strip()==b'P6'
            record['pixels']=list(map(int,f.readline().split()))
            continue
        raw=f.read()
        text=raw.decode('utf-8',errors='replace')
        row=next(r for r in text.splitlines() if r.startswith('display: '))
        command,width,sound=reader(text)
        record.update(display=row,width=width,sound=sound,serial_sha256=hashlib.sha256(raw).hexdigest())
for trial in records.values():
    if 'pixels' in trial:
        assert trial['pixels'][0]==trial['width']
assert {800,1280,1920}=={t['width'] for t in records.values()}
(root/'usb-parser-readback.json').write_text(json.dumps({'b3_evidence_sha256':sys.argv[2],'trials':records},indent=2)+'\n')
print([(k,v['width']) for k,v in records.items()])
