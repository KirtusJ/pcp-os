"""Verify a GitHub cohort artifact. Arguments: ZIP, expected ZIP SHA-256, pinned model.bin, expected rounds."""
from pathlib import Path
import hashlib, io, json, re, sys, zipfile
import numpy as np

def predict_all(weights):
    data=np.fromfile(weights,dtype='<f4',offset=28)
    mat={}
    cur=0
    for name,shape in [('emb',(512,64)),('att',(5,64)),('q',(5,64,64)),('k',(5,32,64)),('v',(5,32,64)),('o',(5,64,64)),('ffn',(5,64)),('w1',(5,172,64)),('w2',(5,64,172)),('w3',(5,172,64)),('final',(64,))]:
        size=int(np.prod(shape))
        mat[name]=data[cur:cur+size].reshape(shape).astype(float)
        cur+=size
    def norm(x,g):return x*g/np.sqrt(np.mean(x*x)+1e-5)
    def rot(x,p):
        y=x.reshape(-1,4,2)
        angle=p*np.array([1.,.1,.01,.001])
        c,s=np.cos(angle),np.sin(angle)
        return np.stack((y[:,:,0]*c-y[:,:,1]*s,y[:,:,0]*s+y[:,:,1]*c),axis=-1).reshape(-1,8)
    def tokens(prompt):
        seq=[1,300+prompt]; extra=[]; cache=[([],[]) for _ in range(5)]
        for pos in range(len(seq)+2):
            tok=seq[pos] if pos<len(seq) else extra[-1]
            x=mat['emb'][tok].copy()
            for i,(K,V) in enumerate(cache):
                y=norm(x,mat['att'][i])
                q=rot(mat['q'][i]@y,pos)
                K.append(rot(mat['k'][i]@y,pos))
                V.append((mat['v'][i]@y).reshape(4,8))
                keys=np.repeat(np.stack(K),2,axis=1)
                values=np.repeat(np.stack(V),2,axis=1)
                scores=np.sum(keys*q,axis=-1)/np.sqrt(8)
                att=np.exp(scores-scores.max(axis=0));att/=att.sum(axis=0)
                x+=mat['o'][i]@np.sum(values*att[:,:,None],axis=0).reshape(64)
                y=norm(x,mat['ffn'][i]);z=mat['w1'][i]@y
                x+=mat['w2'][i]@(z/(1+np.exp(-z))*(mat['w3'][i]@y))
            if pos>=len(seq)-1:
                extra.append(int(np.argmax(mat['emb']@norm(x,mat['final']))))
        return extra
    return [tokens(prompt) for prompt in range(16)]

weights = Path(sys.argv[3])
assert hashlib.sha256(weights.read_bytes()).hexdigest() == 'b0a507e7ad0f626624f17112325e66691f9076d622e1d3274d103d00299f2696'
oracle=predict_all(weights)
parents=[0,0,1,1,1,2,2,2,3,3,4,4,4,5,5,5,6,6,7,8,8,9,10,11,12]
digest=lambda data:hashlib.sha256(data).hexdigest()
archive=Path(sys.argv[1]).read_bytes()
assert digest(archive)==sys.argv[2]
with zipfile.ZipFile(io.BytesIO(archive)) as z:
    def read(name):
        matches=[n for n in z.namelist() if n.split('/')[-1]==name]
        assert len(matches)==1,(name,matches)
        return z.read(matches[0])
    ref=read('oracle.json')
    assert json.loads(ref)==oracle
    events=[json.loads(line) for line in read('events.jsonl').splitlines()]
    inputs=next(e for e in events if e['event']=='inputs')
    assert inputs['oracle_sha256']==digest(ref)
    history={}
    admissions=[e for e in events if e['event']=='admitted']
    ids={a['node']:a for a in admissions}
    assert len(admissions)==len(ids)==24 and len({e['uuid'] for e in ids.values()})==24
    assert not any(e['event'] in ('interrupted','failed') for e in events)
    rounds=[e for e in events if e['event']=='readback']
    assert len(rounds)==int(sys.argv[4])
    for count,e in enumerate(rounds,1):
        record=e['record']
        assert record['cycle']==count and record['repairs']==0
        assert digest((json.dumps(record,indent=2)+'\n').encode())==e['sha256']
        assert sorted(r['node'] for r in record['nodes'])==list(range(1,25))
        for r in record['nodes']:
            n=r['node']; a=ids[n]; parent=parents[n]
            assert a['parent']==r['parent']==parent and a['uuid']==r['uuid']
            incoming=ids[parent]['ticket'] if parent else inputs['root_input']
            assert a['input']==incoming
            salt=inputs['salt']
            ticket=(incoming*31+n*13+salt+1)%1000003
            children=[child for child in range(2,25) if parents[child]==n]
            assert a['ticket']==ticket and a['children']==children
            assert r['serial_file']==f"{n}-{a['attempt']}.serial"
            serial=read(r['serial_file'])
            size=r['serial_bytes']
            assert 0<size<=len(serial)
            raw=serial[:size]; assert digest(raw)==r['serial_sha256']
            lines=raw.decode().replace('\r','').splitlines()
            assert '  FAIL' not in '\n'.join(lines) and any(inputs['measurement']+' (measured; equal to the value the loader named)' in line for line in lines)
            launch=[i for i,l in enumerate(lines) if re.fullmatch(r'starting resident\.elf \(\d+ bytes, SHA-256 '+inputs['worker_sha256']+r'\) with: '+f'{n} {incoming} {salt}',l)]
            assert len(launch)==1
            prediction=lambda prompt:'LM tokens '+' '.join(map(str,oracle[prompt]))
            expected=[f'NODE {n} PARENT {parent} INPUT {incoming} TICKET {ticket}',prediction((n+salt+incoming)%16),'GATE 1',*[f'SPAWN {c} PARENT {n} TICKET {ticket}' for c in children],f'READY {n}']
            hist=history.setdefault(n,[]); prompt=r['prompt']
            assert (not hist or prompt != hist[-1]) and oracle[prompt]==r['tokens']
            hist.append(prompt)
            for k in hist:expected.extend([f'CHALLENGE {k}',prediction(k),f'READY {n}'])
            assert lines[launch[0]+1:]==expected,(n,count)
    assert json.loads(read('readback.json'))==rounds[-1]['record']
    print(json.dumps(dict(root=ids[1]['uuid'],nodes=len(ids),edges=23,reference_tokens=48,rounds=[dict(cycle=r['cycle'],time=r['record']['time'],sha256=r['sha256']) for r in rounds],worker=inputs['worker_sha256'],kernel=inputs['kernel_sha256'],archive=digest(archive),last_event=events[-1]['event'])))

