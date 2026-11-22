"""Boot refusal controls on private copies of an assembled FAT volume using OVMF."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def change_volume(volume, work, case):
    def m(command, *args):
        subprocess.run([command, "-i", volume, *args], check=True, stdout=subprocess.DEVNULL)
    base = "::/PCPOS/"
    scratch = work/"control"
    if case == "short":
        scratch.write_bytes(b"short")
        m("mcopy", "-o", scratch, base+"shard-00000.bin")
    elif case == "missing":
        m("mdel", base+"shard-00000.bin")
    elif case == "extra":
        scratch.write_bytes(b"extra")
        m("mcopy", scratch, base+"shard-99999.bin")
    elif case == "order":
        m("mren", base+"shard-00001.bin", base+"saved.bin")
        m("mren", base+"shard-00000.bin", base+"shard-00001.bin")
        m("mren", base+"saved.bin", base+"shard-00000.bin")
    elif case in ("kernel","manifest","truncated-manifest","worker"):
        name = {"kernel":"KERNEL.ELF","manifest":"model.manifest","truncated-manifest":"model.manifest","worker":"WORKER.ELF"}[case]
        m("mcopy", base+name, scratch)
        data = bytearray(scratch.read_bytes())
        if case == "truncated-manifest":
            data.pop()
        else:
            data[-1] ^= 1
        scratch.write_bytes(data)
        m("mcopy","-o",scratch,base+name)


def trial(args, case, expected):
    with tempfile.TemporaryDirectory(prefix="pcpos-efi-control-",dir=args.work) as tmp:
        work = Path(tmp)
        volume = args.volume
        if case != "memory":
            volume = work/"boot.fat"
            shutil.copyfile(args.volume, volume)
            change_volume(volume, work, case)
        variables = work/"vars.fd"
        shutil.copyfile(args.vars, variables)
        serial = work/"serial.txt"
        cmd=[args.qemu,"-machine","q35","-accel","tcg","-cpu","max","-m","512M" if case=="memory" else args.ram,
             "-no-reboot","-nic","none","-display","none","-monitor","none","-serial","file:"+str(serial),
             "-drive",f"if=pflash,format=raw,unit=0,readonly=on,file={args.code}",
             "-drive",f"if=pflash,format=raw,unit=1,file={variables}",
             "-device","qemu-xhci,id=xhci","-drive",f"if=none,id=stick,readonly=on,format=raw,file={volume}",
             "-device","usb-storage,bus=xhci.0,drive=stick,bootindex=1"]
        proc=subprocess.Popen(cmd,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        try:
            deadline=time.monotonic()+120
            while time.monotonic()<deadline:
                log=serial.read_text(errors="replace") if serial.exists() else ""
                if "Loader resources released." in log: break
                if proc.poll() is not None: break
                time.sleep(.15)
            if expected not in log or "Loader resources released." not in log or "Model verified. Starting PCP OS." in log:
                raise RuntimeError(f"{case}: unexpected firmware output: {log[-800:]}")
            if case=="memory" and ("Model memory required:" not in log or "Verified " in log):
                raise RuntimeError("Expected a capacity report before model-payload reads")
            if args.logs:
                args.logs.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(serial,args.logs/(case+".txt"))
            print(case,expected,flush=True)
        finally:
            if proc.poll() is None: proc.terminate()
            try: proc.wait(timeout=5)
            except subprocess.TimeoutExpired: proc.kill();proc.wait()


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("volume",type=Path,help="unpartitioned FAT volume; the order case needs two full-size shards")
    parser.add_argument("--code",type=Path,required=True); parser.add_argument("--vars",type=Path,required=True)
    parser.add_argument("--work",type=Path);parser.add_argument("--logs",type=Path)
    parser.add_argument("--ram",default="4096M");parser.add_argument("--qemu",default="qemu-system-x86_64")
    a=parser.parse_args()
    for case,expected in [
        ("memory","There is not enough usable high memory"),("kernel","file does not match the SHA-256"),
        ("worker","file does not match the SHA-256"),("manifest","file does not match the SHA-256"),
        ("truncated-manifest","could not be read to its expected length"),("missing","missing a model shard"),
        ("short","wrong length"),("extra","extra or duplicate model shard"),
        ("order","does not match its ordered manifest digest")]:
        trial(a,case,expected)
