"""Compile the tracked console descriptor and feed it deterministic UEFI mode records."""
import hashlib,json,subprocess,sys,tempfile
from pathlib import Path
src,inc=map(lambda x:Path(x).resolve(),sys.argv[1:3])
here=Path(__file__).resolve().parent
p=src/'examples/firmware-owned-uefi/loader.c'
source=p.read_text()
def between(first,stop):
    start=source.index(first)
    return source[start:source.index(stop,start)]
sections=[
    between('struct __attribute__((packed)) mb_range', '_Static_assert(sizeof(GRANT)'),
    between('static void copy_bytes(', '#ifdef PCP_USB_VM'),
    between('struct __attribute__((packed)) mb_framebuffer', '#endif'),
]
with tempfile.TemporaryDirectory() as tmp:
    wd=Path(tmp)
    (wd/'source.inc').write_text('#define MAX_DESCRIPTORS 512\n'+'\n'.join(sections))
    exe=wd/'describe'
    cmd=['cc','-std=c11','-Wall','-Wextra','-Werror','-DGNU_EFI_USE_MS_ABI','-I'+str(inc),'-I'+str(inc/'x86_64'),'-I'+str(wd),str(here/'host.c'),'-o',str(exe)]
    subprocess.run(cmd,check=True)
    result=subprocess.check_output([exe],text=True)
print(result,end='')
