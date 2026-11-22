"""Read processor enumeration and firmware tables in a private, read-only preloaded UEFI guest."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import time

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

out = Path(sys.argv[1]).resolve()
out.mkdir()
fw = Path(os.environ['B3_ROOT'])/'runtime/usr/share/OVMF'
vars_file = out/'vars.fd'
shutil.copyfile(fw/'OVMF_VARS_4M.fd', vars_file)
serial = out/'serial.txt'
args = [os.environ['B3_ROOT']+'/hostbin/qemu-system-x86_64', '-machine','q35,max-ram-below-4g=512M','-accel','tcg',
        '-cpu','max','-smp', '6,sockets=2,cores=3,threads=1', '-m','1536M','-no-reboot','-nic','none','-display','none','-vga','std',
        '-monitor','none','-serial','file:'+str(serial),'-qmp','stdio',
        '-drive',f'if=pflash,unit=0,format=raw,readonly=on,file={fw}/OVMF_CODE_4M.fd',
        '-drive',f'if=pflash,unit=1,format=raw,file={vars_file}',
        '-device','qemu-xhci,id=xhci','-drive',f'if=none,format=raw,readonly=on,file={Path("paired.fat").resolve()},id=stick',
        '-device','usb-storage,bus=xhci.0,drive=stick,bootindex=1']
(out/'command.json').write_text(json.dumps(args,indent=2)+'\n')
with (out/'stderr.txt').open('w') as err, subprocess.Popen(args,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True) as vm:
    try:
        def rpc(cmd, **kwargs):
            vm.stdin.write(json.dumps({'execute':cmd,'arguments':kwargs})+'\n')
            vm.stdin.flush()
            while line := vm.stdout.readline():
                reply=json.loads(line)
                if 'return' in reply: return reply['return']
                require('error' not in reply,str(reply))
            raise RuntimeError('closed QMP: '+cmd)
        hello=json.loads(vm.stdout.readline())['QMP']['version']
        rpc('qmp_capabilities')
        cpus=rpc('query-cpus-fast')
        ids=[rpc('qom-get',path=c['qom-path'],property='apic-id') for c in cpus]
        blocks=[(b['device'],b['inserted']['ro']) for b in rpc('query-block') if 'inserted' in b]
        require(('stick',True) in blocks,'FAT is not read-only')
        print('QMP APIC IDs',ids,flush=True)
        while True:
            s=serial.read_text(errors='replace') if serial.exists() else ''
            require(vm.poll() is None, 'premature exit')
            if 'leaves no memory behind' in s or 'it is in SAFE' in s: break
            time.sleep(.2)
        rpc('stop')
        ptrs=re.findall(r' acpi_rsdp=0x([0-9a-f]{16})(?=\s|$)',s)
        require(len(ptrs)==1, 'expected one canonical RSDP')
        root=int(ptrs[0],16)
        def capture(addr,size):
            require(0 < addr < (1 << 32) and 0 < size <= (1 << 20), 'unexpected ACPI range')
            path=out/f'acpi-{addr:016x}-{size}.bin'
            rpc('pmemsave',val=addr,size=size,filename=str(path))
            b=path.read_bytes()
            require(len(b)==size,'short read')
            return b
        rsdp=capture(root,36)
        require(rsdp[:8]==b'RSD PTR ' and rsdp[15]>=2 and sum(rsdp[:20])%256==sum(rsdp)%256==0,'RSDP mismatch')
        xsdt=struct.unpack_from('<Q',rsdp,24)[0]
        def table(addr,signature):
            h=capture(addr,36)
            require(h[:4]==signature,'unexpected table')
            b=capture(addr,struct.unpack_from('<I',h,4)[0])
            require(sum(b)%256==0,'checksum mismatch')
            return b
        xt=table(xsdt,b'XSDT')
        for entry, in struct.iter_unpack('<Q',xt[36:]):
            if capture(entry,4)==b'APIC':
                madt=table(entry,b'APIC')
                break
        else: raise RuntimeError('no MADT')
        apics=[]
        at=44
        while at<len(madt):
            kind,n=madt[at:at+2]
            require(n>=2 and at+n<=len(madt),'invalid MADT entry size')
            if kind==0 and int.from_bytes(madt[at+4:at+8],'little')&1:
                apics.append(madt[at+3])
            if kind==9 and int.from_bytes(madt[at+8:at+12],'little')&1:
                apics.append(int.from_bytes(madt[at+4:at+8],'little'))
            at+=n
        reports=[l for l in s.splitlines() if l.startswith('processors:')]
        lines=[l for l in s.splitlines() if l.startswith('  pass ')]
        result=dict(root=root,xsdt=xsdt,madt=entry,qmp_apic_ids=ids,firmware_apic_ids=apics,qemu=hello,blocks=blocks,
                    kernel_processor_report=reports,pass_lines=len(lines),kernel_measurement_lines=
                    [l for l in s.splitlines() if l.startswith('running image')],serial_sha256=hashlib.sha256(serial.read_bytes()).hexdigest())
        (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
        require(sorted(ids)==sorted(apics)==[0,1,2,4,5,6], 'identity mismatch')
        require(len(reports)==1 and '6 usable (at the address the loader named)' in reports[0], str(reports))
        print(json.dumps(result),flush=True)
        rpc('quit')
        vm.wait(timeout=10)
    finally:
        if vm.poll() is None:
            vm.terminate()
            vm.wait(timeout=10)
