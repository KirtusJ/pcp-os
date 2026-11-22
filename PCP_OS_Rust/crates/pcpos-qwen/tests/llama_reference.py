"""Host QA only. Dump raw logits from the pinned b12387 Windows CPU release.
Usage: python llama_reference.py DLL_DIRECTORY MODEL.gguf NEW_OUTPUT_DIR TOKEN...
FFI layouts follow include/llama.h at a956a852c9c8bc1fb07206364838ddef299178c0.
"""
import ctypes as c, hashlib, json, os, pathlib, struct, sys, time
P=c.c_void_p; I=c.c_int32; U=c.c_uint32; F=c.c_float; B=c.c_bool; Z=c.c_size_t
class ModelParams(c.Structure):
    _fields_=[('devices',P),('tensor_buft_overrides',P),('n_gpu_layers',I),('split_mode',I),
        ('load_mode',I),('lazy_mode',I),('main_gpu',I),('tensor_split',P),('progress_callback',P),
        ('progress_callback_user_data',P),('kv_overrides',P),('vocab_only',B),('check_tensors',B),
        ('use_extra_bufts',B),('no_host',B),('no_alloc',B),('load_mtp',B)]
class ContextParams(c.Structure):
    _fields_=[(n,U) for n in ['n_ctx','n_batch','n_ubatch','n_seq_max','n_rs_seq','n_outputs_max','n_outputs_max_per_seq']]+[
        (n,I) for n in ['n_threads','n_threads_batch','ctx_type','rope_scaling_type','pooling_type','attention_type','flash_attn_type']]+[
        (n,F) for n in ['rope_freq_base','rope_freq_scale','yarn_ext_factor','yarn_attn_factor','yarn_beta_fast','yarn_beta_slow']]+[
        ('yarn_orig_ctx',U),('cb_eval',P),('cb_eval_user_data',P),('type_k',I),('type_v',I),
        ('moe_cache_size',Z),('moe_cache_cpu_share',F),('abort_callback',P),('abort_callback_data',P)]+[
        (n,B) for n in ['embeddings','offload_kqv','no_perf','op_offload','swa_full','kv_unified']]+[
        ('samplers',P),('n_samplers',Z),('ctx_other',P)]
class Batch(c.Structure):
    _fields_=[('n_tokens',I),('token',c.POINTER(I)),('embd',P),('pos',P),('n_seq_id',P),('seq_id',P),('logits',P)]

def main():
    dlls,model,out=map(pathlib.Path,sys.argv[1:4]);tokens=list(map(int,sys.argv[4:]));assert 0<len(tokens)<=32
    runtime=json.loads((pathlib.Path(__file__).parent/'data/reference-runtime.json').read_text())
    hashes=runtime['dll_sha256']
    assert {p.name for p in dlls.glob('*.dll')}==set(hashes),'unexpected or missing DLL'
    for name,want in hashes.items():assert hashlib.sha256((dlls/name).read_bytes()).hexdigest()==want,name
    out.mkdir(exist_ok=False)
    directory=os.add_dll_directory(str(dlls.resolve()))
    ggml=c.CDLL(str((dlls/'ggml.dll').resolve()))
    ggml.ggml_backend_load_all_from_path.argtypes=[c.c_char_p]
    ggml.ggml_backend_load_all_from_path(str(dlls.resolve()).encode())
    lib=c.CDLL(str((dlls/'llama.dll').resolve()))
    def fn(name,ret,args):
        f=getattr(lib,name);f.restype=ret;f.argtypes=args;return f
    fn('llama_backend_init',None,[])()
    mp=fn('llama_model_default_params',ModelParams,[])();mp.n_gpu_layers=0;mp.use_extra_bufts=False
    m=fn('llama_model_load_from_file',P,[c.c_char_p,ModelParams])(str(model.resolve()).encode(),mp)
    assert m,'reference refused model'
    ctx=None
    try:
        cp=fn('llama_context_default_params',ContextParams,[])()
        cp.n_ctx=32;cp.n_batch=1;cp.n_ubatch=1;cp.n_seq_max=1;cp.n_threads=2;cp.n_threads_batch=2
        cp.type_k=0;cp.type_v=0;cp.flash_attn_type=0;cp.offload_kqv=False;cp.op_offload=False
        ctx=fn('llama_init_from_model',P,[P,ContextParams])(m,cp);assert ctx,'reference context refused'
        vocab=fn('llama_model_get_vocab',P,[P])(m)
        count=fn('llama_vocab_n_tokens',I,[P])(vocab)
        get_batch=fn('llama_batch_get_one',Batch,[c.POINTER(I),I]);decode=fn('llama_decode',I,[P,Batch])
        logits=fn('llama_get_logits_ith',c.POINTER(F),[P,I])
        rows=[];started=time.monotonic()
        for pos,token in enumerate(tokens):
            assert 0<=token<count
            data=(I*1)(token);assert decode(ctx,get_batch(data,1))==0
            raw=c.string_at(logits(ctx,-1),count*4);(out/f'{pos}.f32').write_bytes(raw)
            values=struct.unpack('<'+'f'*count,raw)
            rows.append({'position':pos,'token':token,'top':max(range(count),key=values.__getitem__)})
        result={'reference':'llama.cpp b12387 / a956a852c9c8bc1fb07206364838ddef299178c0',
            'dll_sha256':hashes,'model':str(model),'model_bytes':model.stat().st_size,
            'tokens':tokens,'vocabulary':count,'kv':'F32','flash_attention':False,
            'seconds':time.monotonic()-started,'rows':rows}
        with model.open('rb') as f:result['model_sha256']=hashlib.file_digest(f,'sha256').hexdigest()
        (out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    finally:
        if ctx:fn('llama_free',None,[P])(ctx)
        fn('llama_model_free',None,[P])(m)
        directory.close()
if __name__=='__main__':main()
