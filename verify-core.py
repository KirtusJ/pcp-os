import sys,json,hashlib,re,zipfile
from pathlib import Path
from collections import Counter
z=zipfile.ZipFile(sys.argv[1])
def get(name):
    matches=[p for p in z.namelist() if p.endswith('/'+name) or p==name]
    assert len(matches)==1, (name,matches)
    return z.read(matches[0])
data=json.loads(get('fixture/inputs.json'))
assert data['manifest']=='7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707'
assert data['hashes']['Core']=='649211ff2d765e06b5d5eb31dc80af6f97aedea242f2dcb5b77b3fa270ef34d6'
events=[json.loads(row) for row in get('cohort/events.jsonl').splitlines()]
inputs=events[0]
for key,kind in [('worker_sha256','worker'),('kernel_sha256','kernel'),('oracle_sha256','oracle')]:
    assert inputs[key]==data['hashes'][kind]
assert data['hashes']['worker']!=data['hashes']['control']
rows=[e for e in events if e['event']=='admitted']
assert len(rows)==24
for row in rows:
    raw=get(f"cohort/{row['node']}-{row['attempt']}.serial").decode().splitlines()
    modules=[line for line in raw if line.startswith('file from the loader: core-spec.md')]
    assert len(modules)==1 and re.fullmatch(r'file from the loader: core-spec\.md \(138793 bytes at 0x[0-9a-f]+\)',modules[0]),modules
for name,code in [('absent',2),('changed',1)]:
    lines=get(f'controls/{name}.serial').decode().splitlines()
    launch=[s for s in lines if s.startswith('starting resident.elf')]
    assert len(launch)==1 and ('SHA-256 '+data['hashes']['worker']+') with: 1 7 0') in launch[0]
    assert [e for s in lines if (m:=re.match(r'^resident\.elf ended FAILED: exit (\d+)\b',s)) for e in [int(m[1])]]==[code]
    assert not any(s.startswith(('NODE ','LM tokens ','READY ')) for s in lines)
print(json.dumps({'module_admissions':len(rows),'core':data['hashes']['Core'],'bytes':138793,'worker':data['hashes']['worker'],'negative_exits':[1,2],'fixture':data['fixture']}))
