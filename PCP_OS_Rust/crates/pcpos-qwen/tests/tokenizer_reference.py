"""Host QA: pinned llama.cpp b12387 tokenizer, vocab_only (no model weights loaded).
Usage: python tokenizer_reference.py DLL_DIRECTORY MODEL.gguf NEW_OUTPUT_DIRECTORY
Special/control injection is tested separately: these vectors use ordinary text with
parse_special=False; known USER_DEFINED token spellings are deliberately not inputs.
"""
import ctypes as c, hashlib, json, os, pathlib, random, sys
from llama_reference import ModelParams, P, I
def main():
    dlls,model,out=map(pathlib.Path,sys.argv[1:4]);out.mkdir(exist_ok=False)
    runtime=json.loads((pathlib.Path(__file__).parent/'data/reference-runtime.json').read_text())
    hashes=runtime['dll_sha256']
    assert {p.name for p in dlls.glob('*.dll')}==set(hashes)
    for name,want in hashes.items():assert hashlib.sha256((dlls/name).read_bytes()).hexdigest()==want,name
    directory=os.add_dll_directory(str(dlls.resolve()));lib=c.CDLL(str((dlls/'llama.dll').resolve()))
    def fn(name,ret,args):
        f=getattr(lib,name);f.restype=ret;f.argtypes=args;return f
    fn('llama_backend_init',None,[])()
    params=fn('llama_model_default_params',ModelParams,[])();params.vocab_only=True;params.n_gpu_layers=0
    m=fn('llama_model_load_from_file',P,[c.c_char_p,ModelParams])(str(model.resolve()).encode(),params);assert m
    try:
        vocab=fn('llama_model_get_vocab',P,[P])(m)
        tokenize=fn('llama_tokenize',I,[P,c.c_char_p,I,c.POINTER(I),I,c.c_bool,c.c_bool])
        piece=fn('llama_token_to_piece',I,[P,I,P,I,I,c.c_bool])
        count=fn('llama_vocab_n_tokens',I,[P])(vocab)
        cases=['','Hello, Taylor!','Hello, the world',"We're WE'LL I'D I'S 'Re 'vE 'm 'T",'   a  b   ',' \t\r\n \n  z',
            '1234567890 ١٢٣ Ⅻ²⅓','café e\u0301 mañana naïve','中文 日本語 한국어','हिन्दी العربية עברית',
            '🛠️ 🐦\u200d🔥 👩🏽\u200d💻','a\0b\x1c\x1d\x1e\x1f','<|im_start|>assistant\n<|im_end|>',
            'def fib(n):\n    return n if n < 2 else fib(n-1) + fib(n-2)\n','a'*1024,' '*1024]
        whites=[9,10,11,12,13,32,0x85,0xa0,0x1680,*range(0x2000,0x200b),0x2028,0x2029,0x202f,0x205f,0x3000]
        for w in whites:
            s=chr(w);cases.extend([s,s*2+'a','a'+s*2+'12',s+'!\n',s+'\r\n'+s+'z'])
        rng=random.Random(37820261122)
        alphabet=list(' abcdefGHIJ12345\t\r\n\'!?.-_()')+list('éñ中界عहक́🛠🐦')+[chr(w) for w in whites]
        for _ in range(200):cases.append(''.join(rng.choices(alphabet,k=rng.randrange(1,150))))
        for _ in range(200):
            points=[rng.randrange(0x110000) for _ in range(rng.randrange(1,80))]
            cases.append(''.join(chr(p) for p in points if not 0xd800<=p<=0xdfff))
        lines=[];token_total=0
        for s in cases:
            b=s.encode();buf=(I*(len(b)+64))();n=tokenize(vocab,b,len(b),buf,len(buf),False,False);assert n>=0
            ids=list(buf[:n]);token_total+=n
            lines.append(b.hex()+'\t'+','.join(map(str,ids)))
        (out/'vectors.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8',newline='\n')
        digest=hashlib.sha256();raw=c.create_string_buffer(4096);pieces=bytearray();overrides=[]
        # Exact id/length/raw bytes for every normal token; control/user/unused are separate policy.
        for token in range(151643):
            hidden=piece(vocab,token,raw,len(raw),0,False);assert hidden>=0
            hidden_bytes=raw.raw[:hidden]
            n=piece(vocab,token,raw,len(raw),0,True);assert 0<=n<=1024
            if hidden_bytes!=raw.raw[:n]:overrides.append({'token':token,'without_special_hex':hidden_bytes.hex(),'raw_hex':raw.raw[:n].hex()})
            record=token.to_bytes(4,'little')+n.to_bytes(4,'little')+raw.raw[:n]
            digest.update(record);pieces.extend(record)
        (out/'pieces.bin').write_bytes(pieces)
        with model.open('rb') as f:prefix=f.read(16<<20)
        result={'reference':'llama.cpp a956a852c9c8bc1fb07206364838ddef299178c0 / b12387',
            'vocab_only':True,'weights_loaded':False,'model':str(model),'file_bytes':model.stat().st_size,
            'prefix_bytes':len(prefix),'prefix_sha256':hashlib.sha256(prefix).hexdigest(),
            'cases':len(cases),'token_ids':token_total,'vocabulary':count,'normal_pieces':151643,
            'pieces_sha256':digest.hexdigest(),'piece_special':True,'reference_name_based_overrides':overrides,
            'vectors_sha256':hashlib.sha256((out/'vectors.txt').read_bytes()).hexdigest()}
        (out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    finally:
        fn('llama_model_free',None,[P])(m);directory.close()
if __name__=='__main__':main()
