"""Independent Qwen2 host graph: GGML weight decoding, NumPy float64 matrix products.
This deliberately avoids llama.cpp's runtime activation quantization, to isolate
the scalar Rust contract from its optimized quantized matrix path.
"""
import os,sys
os.environ['OPENBLAS_NUM_THREADS']='1'

import numpy as np
import ctypes as c,hashlib,json,mmap,pathlib,struct,time
dlls,path,profile,out=map(pathlib.Path,sys.argv[1:5])
p=json.loads(profile.read_text())
assert p['metadata']['general.architecture']=='qwen2'
with path.open('rb') as f:assert hashlib.file_digest(f,'sha256').hexdigest()==p['sha256']
dllpath=dlls/'ggml-base.dll'
assert hashlib.sha256(dllpath.read_bytes()).hexdigest()=='3e0ceab7d54a173b349d05968b589aff9fb84e75ea8acb578322afaec2ab4cfb'
lib=c.CDLL(str(dllpath))
dec={}
for kind,name in [(6,'q5_0'),(8,'q8_0'),(12,'q4_K'),(14,'q6_K')]:
 f=getattr(lib,'dequantize_row_'+name);f.argtypes=[c.c_void_p,c.c_void_p,c.c_int64];f.restype=None;dec[kind]=f
file=path.open('rb');weights=mmap.mmap(file.fileno(),0,access=mmap.ACCESS_READ)
table={t['name']:t for t in p['tensors']}
def rows(name,first,count):
 t=table[name];cols=t['shape'][0];kind=t['ggml_type'];block,size={0:(1,4),6:(32,22),8:(32,34),12:(256,144),14:(256,210)}[kind]
 stride=cols//block*size;start=p['tensor_data_offset']+t['offset']+first*stride;raw=weights[start:start+count*stride]
 if kind==0:return np.frombuffer(raw,dtype='<f4').reshape(count,cols).copy()
 out=np.empty((count,cols),np.float32);dec[kind](c.c_char_p(raw),out.ctypes.data,count*cols);return out
def vec(name):return rows(name,0,1)[0]
def mm(name,x):
 t=table[name];out=np.empty(t['shape'][1],np.float32);v=x.astype(np.float64)
 for i in range(0,len(out),256):
  n=min(256,len(out)-i);out[i:i+n]=(rows(name,i,n).astype(np.float64)@v).astype(np.float32)
 return out
meta=p['metadata'];layers=meta['qwen2.block_count'];dim=meta['qwen2.embedding_length'];heads=meta['qwen2.attention.head_count'];kvheads=meta['qwen2.attention.head_count_kv'];width=dim//heads
eps=np.float32(meta['qwen2.attention.layer_norm_rms_epsilon']);base=np.float32(meta['qwen2.rope.freq_base']);tokens=list(map(int,sys.argv[5:]));assert 0<len(tokens)<=32
keys=np.zeros((layers,len(tokens),kvheads,width),np.float32);values=keys.copy()
freq=np.power(base,-np.arange(0,width,2,dtype=np.float32)/np.float32(width)).astype(np.float32)
def norm(x,name):
 scale=np.float32(1/np.sqrt(np.mean(x.astype(np.float64)**2)+float(eps)));return ((x*scale)*vec(name)).astype(np.float32)
def rope(x,pos):
 x=x.reshape(-1,width).copy();angle=(np.float32(pos)*freq).astype(np.float32);s=np.sin(angle);c0=np.cos(angle);a=x[:,:width//2].copy();b=x[:,width//2:].copy();x[:,:width//2]=a*c0-b*s;x[:,width//2:]=a*s+b*c0;return x
out.mkdir(exist_ok=False);start=time.monotonic()
for pos,token in enumerate(tokens):
 x=rows('token_embd.weight',token,1)[0]
 for layer in range(layers):
  pre=f'blk.{layer}.';z=norm(x,pre+'attn_norm.weight');q=mm(pre+'attn_q.weight',z)+vec(pre+'attn_q.bias');k=mm(pre+'attn_k.weight',z)+vec(pre+'attn_k.bias');v=mm(pre+'attn_v.weight',z)+vec(pre+'attn_v.bias')
  q=rope(q,pos);keys[layer,pos]=rope(k,pos);values[layer,pos]=v.reshape(kvheads,width)
  att=np.empty((heads,width),np.float32)
  for h in range(heads):
   kh=h//(heads//kvheads);score=(keys[layer,:pos+1,kh].astype(np.float64)@q[h].astype(np.float64)/np.sqrt(width)).astype(np.float32)
   prob=np.exp(score-np.max(score)).astype(np.float32);att[h]=(prob.astype(np.float64)@values[layer,:pos+1,kh].astype(np.float64)/np.sum(prob,dtype=np.float64)).astype(np.float32)
  x=(x+mm(pre+'attn_output.weight',att.flatten())).astype(np.float32);z=norm(x,pre+'ffn_norm.weight');gate=mm(pre+'ffn_gate.weight',z);up=mm(pre+'ffn_up.weight',z)
  sigmoid=np.empty_like(gate);positive=gate>=0;sigmoid[positive]=1/(1+np.exp(-gate[positive]));e=np.exp(gate[~positive]);sigmoid[~positive]=e/(1+e)
  x=(x+mm(pre+'ffn_down.weight',(gate*sigmoid)*up)).astype(np.float32)
 logits=mm('output.weight',norm(x,'output_norm.weight'));(out/f'{pos}.f32').write_bytes(logits.astype('<f4').tobytes());print('position',pos,'top',np.argmax(logits),flush=True)
result={'reference':'independent NumPy graph + pinned GGML scalar dequantization','numpy':np.__version__,'model_sha256':p['sha256'],'tokens':tokens,'seconds':time.monotonic()-start}
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));weights.close();file.close()
