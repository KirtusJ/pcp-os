#!/usr/bin/env python3
"""Synthetic regular-file GPT detector cross-check. NOT an installer permission gate.

Only opens files created in TemporaryDirectory, then calls the system blkid
on those exact regular files. No user input, real devices, mounts, loops,
PCP-OS kernel, privileged instructions or networking.
Recognition by blkid is NOT certification of GPT cross-copy consistency.
"""
import struct
import zlib
import hashlib
import subprocess
import tempfile
import shutil
import json
from pathlib import Path

SIZE=4096
BS=512
LAST=SIZE-1
P_ARRAY=2
B_ARRAY=SIZE-33
FIRST=34
LAST_USABLE=B_ARRAY-1
ESP_TYPE=bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")
GUID_A=bytes.fromhex("0123456789abcdef0123456789abcdef")
GUID_B=bytes.fromhex("fedcba9876543210fedcba9876543210")
DISK_A=bytes.fromhex("00112233445566778899aabbccddeeff")
DISK_B=bytes.fromhex("ffeeddccbbaa99887766554433221100")

def array(guid=GUID_A):
    data=bytearray(128*128)
    data[:16]=ESP_TYPE
    data[16:32]=guid
    struct.pack_into("<QQQ",data,32,2048,3071,0)
    data[56:56+len("PCPOS-SYNTH".encode("utf-16le"))]="PCPOS-SYNTH".encode("utf-16le")
    return bytes(data)

def header(my,other,arr_lba,arr,disk_guid):
    b=bytearray(BS)
    b[:8]=b"EFI PART"
    struct.pack_into("<II",b,8,0x10000,92)
    struct.pack_into("<QQQQ",b,24,my,other,FIRST,LAST_USABLE)
    b[56:72]=disk_guid
    struct.pack_into("<QIII",b,72,arr_lba,128,128,zlib.crc32(arr))
    struct.pack_into("<I",b,16,zlib.crc32(b[:92]))
    return bytes(b)

def pmbr(slot=0,hybrid=False):
    b=bytearray(BS)
    off=446+slot*16
    b[off+1:off+4]=b"\x00\x02\x00"
    b[off+4]=0xee
    # Correctly representable end CHS for this declared 255/63 geometry.
    track,sector_index=divmod(LAST,63)
    cylinder,head=divmod(track,255)
    assert cylinder < 1024
    b[off+5:off+8]=bytes((head,(sector_index+1)|((cylinder>>2)&0xc0),cylinder&0xff))
    struct.pack_into("<II",b,off+8,1,LAST)
    if hybrid:b[446+((slot+1)%4)*16+4]=0x83
    b[510:512]=b"\x55\xaa"
    return bytes(b)

def crc_ok(h):
    b=bytearray(h[:92])
    b[16:20]=bytes(4)
    return zlib.crc32(b)==struct.unpack_from("<I",h,16)[0]

def cases():
    a=array()
    mutations=[
       ("valid_first_slot",a,DISK_A,DISK_A,1,0,False,False,False),
       ("valid_fourth_slot",a,DISK_A,DISK_A,1,3,False,False,False),
       ("different_disk_guid_both_crcs_valid",a,DISK_A,DISK_B,1,0,False,False,False),
       ("different_partition_arrays_both_crcs_valid",array(GUID_B),DISK_A,DISK_A,1,0,False,False,False),
       ("wrong_backup_alternate_lba_both_crcs_valid",a,DISK_A,DISK_A,5,0,False,False,False),
       ("hybrid_mbr_valid_gpt",a,DISK_A,DISK_A,1,0,True,False,False),
       ("primary_crc_bad_backup_valid",a,DISK_A,DISK_A,1,0,False,True,False),
       ("both_header_crcs_bad",a,DISK_A,DISK_A,1,0,False,True,True),
    ]
    for name,ba,pg,bg,balt,slot,hybrid,cp,cb in mutations:
        p=bytearray(header(1,LAST,P_ARRAY,a,pg))
        b=bytearray(header(LAST,balt,B_ARRAY,ba,bg))
        if cp:p[16]^=0x80
        if cb:b[16]^=0x80
        yield name,(pmbr(slot,hybrid),bytes(p),a,ba,bytes(b))

def run():
    exe=shutil.which("blkid")
    if not exe:raise RuntimeError("blkid unavailable; cannot run independent cross-probe")
    vers=subprocess.run((exe,"-V"),capture_output=True,text=True,check=False)
    output={"scope":"GPT recognition only, never conformance or device authorization",
            "version":(vers.stdout+vers.stderr).strip(),"cases":[]}
    with tempfile.TemporaryDirectory(prefix="pcpos-gpt-cross-") as d:
        for i,(name,items) in enumerate(cases()):
            m,p,pa,ba,b=items
            path=Path(d)/f"{i:02d}.img"
            with path.open("wb") as f:
                f.truncate(SIZE*BS)
                for offset,bytes_ in ((0,m),(BS,p),(P_ARRAY*BS,pa),
                                      (B_ARRAY*BS,ba),(LAST*BS,b)):
                    f.seek(offset)
                    f.write(bytes_)
            r=subprocess.run((exe,"-p","-o","export",str(path)),
                             capture_output=True,text=True,check=False)
            fields=dict(x.split("=",1) for x in r.stdout.splitlines() if "=" in x)
            output["cases"].append({
                "case":name,"rc":r.returncode,"pttype":fields.get("PTTYPE"),
                "primary_crc_ok":crc_ok(p),"backup_crc_ok":crc_ok(b),
                "arrays_equal":pa==ba,"disk_guid_equal":p[56:72]==b[56:72],
                "backup_alternate_lba_ok":struct.unpack_from("<Q",b,32)[0]==1,
                "image_sha256":hashlib.sha256(path.read_bytes()).hexdigest()
            })
    rows={x["case"]:x for x in output["cases"]}
    for name in ("valid_first_slot","valid_fourth_slot"):
        assert rows[name]["primary_crc_ok"] and rows[name]["backup_crc_ok"]
        assert rows[name]["arrays_equal"] and rows[name]["disk_guid_equal"]
        # The external oracle must recognize the intended positive controls.
        # A globally failing tool must not count as a successful probe.
        # No invalid-case PTTYPE is asserted as product policy.
        assert rows[name]["rc"] == 0 and rows[name]["pttype"] == "gpt", (
            "blkid failed to recognize the synthetic positive GPT: " + name
        )
    assert not rows["different_disk_guid_both_crcs_valid"]["disk_guid_equal"]
    assert rows["different_disk_guid_both_crcs_valid"]["backup_crc_ok"]
    assert not rows["different_partition_arrays_both_crcs_valid"]["arrays_equal"]
    assert rows["different_partition_arrays_both_crcs_valid"]["backup_crc_ok"]
    assert not rows["wrong_backup_alternate_lba_both_crcs_valid"]["backup_alternate_lba_ok"]
    assert not rows["primary_crc_bad_backup_valid"]["primary_crc_ok"]
    assert not rows["both_header_crcs_bad"]["backup_crc_ok"]
    print(json.dumps(output,sort_keys=True,indent=2))

if __name__=="__main__":
    run()
