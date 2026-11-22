import hashlib,json,os,subprocess
from pathlib import Path
p=Path("kernel-stock/PCP_OS_Rust/crates/pcpos-kernel")
kernel=p/"target/pcpos-kernel.elf32"
measure=subprocess.check_output(["python3",p/"tools/measure.py",kernel],text=True).strip()
qemu=os.environ["B3_ROOT"]+"/hostbin/qemu-system-x86_64"
for n,topo in [(1,"1"),(2,"2"),(4,"4"),(8,"8"),(6,"6,sockets=2,cores=3,threads=1")]:
    args=[qemu,"-machine","q35","-accel","tcg","-cpu","max","-smp",topo,"-m","256M",
          "-nic","none","-display","none","-monitor","none","-serial","stdio","-no-reboot",
          "-device","isa-debug-exit,iobase=0xf4,iosize=0x04","-kernel",str(kernel),"-append","image="+measure+" then=off"]
    result=subprocess.run(args,capture_output=True,timeout=120)
    path=Path(f"evidence/bios-{n}.serial")
    path.write_bytes(result.stdout)
    lines=result.stdout.decode().splitlines()
    observed=[l for l in lines if l.startswith("processors: ")]
    passes=sum(l.startswith("  pass ") for l in lines)
    if result.returncode!=33 or passes!=72 or len(observed)!=1 or f"lists {n} usable (found where a BIOS puts it)" not in observed[0] or not any(measure+" (measured; equal" in l for l in lines):
        raise RuntimeError((n,result.returncode,passes,observed,lines[-2:]))
    print(n,observed[0],passes,result.returncode,hashlib.sha256(result.stdout).hexdigest(),flush=True)
