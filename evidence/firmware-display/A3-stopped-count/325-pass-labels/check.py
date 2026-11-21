import json,runpy,sys
from pathlib import Path
src,guests=(Path(s).resolve() for s in sys.argv[1:3])
api=runpy.run_path(str(src/'PCP_OS_Rust/crates/pcpos-kernel/tests/replay_probe.py'))
check,inventory=api['coverage_problems'],api['pass_inventory']
def counts(kind,mode):
    return inventory((guests/(kind+'-'+mode+'-checked')/'serial.txt').read_text())
out={}
for mode in ('normal','opt-in'):
    a,b=counts('default',mode),counts('demo',mode)
    assert not check(a,b)
    controls=[]
    for i,name in enumerate(api['KEY_CHECKS']):
        changed=a.copy();changed[name]-=1;changed[api['KEY_CHECKS'][(i+1)%6]]+=1
        assert changed.total()==a.total() and check(changed,b)
        controls.append('replace normal pass: '+name)
    other=next(name for name in b if name not in api['REPLAY_INDEPENDENT'])
    for name in api['REPLAY_INDEPENDENT']:
        ca,cb=a.copy(),b.copy()
        for copy in (ca,cb):copy[name]-=1;copy[other]+=1
        assert ca.total()==a.total() and cb.total()==b.total()
        assert check(ca,cb)
        controls.append('shared substitution in both runs: '+name)
    for key in api['KEY_CHECKS']:
        changed=b.copy();changed[other]-=1;changed[key]+=1
        assert changed.total()==b.total() and check(a,changed)
        controls.append('substitute limited pass: '+key)
    out[mode]={'intact_accepted':True,'normal':a.total(),'limited':b.total(),'refused':controls}
print(json.dumps(out,indent=2))
