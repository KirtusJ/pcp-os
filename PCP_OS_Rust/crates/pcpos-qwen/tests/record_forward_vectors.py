"""Convert pinned-runtime dumps of the exported tiny models into test vectors.
Usage: python record_forward_vectors.py QWEN2_REFERENCE_DIR QWEN3_REFERENCE_DIR
"""
import json,pathlib,struct,sys
out=pathlib.Path(__file__).parent/'data'
for arch,folder in zip(['qwen2','qwen3'],sys.argv[1:],strict=True):
    p=pathlib.Path(folder);r=json.loads((p/'result.json').read_text())
    assert r['tokens']==[3,7,5,2] and r['vocabulary']==16
    assert r['reference']=='llama.cpp b12387 / a956a852c9c8bc1fb07206364838ddef299178c0'
    lines=[r['model_sha256']]
    for i,token in enumerate(r['tokens']):
        raw=(p/f'{i}.f32').read_bytes();assert len(raw)==64
        lines.append(str(token)+' '+' '.join(f'{n:08x}' for n in struct.unpack('<16I',raw)))
    (out/f'{arch}-forward.txt').write_text('\n'.join(lines)+'\n')
