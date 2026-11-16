#!/usr/bin/env python3
"""Independent zlib-backed, synthetic GPT v1 512-sector fixtures.

Creates files ONLY in a newly created test directory. No physical disk,
PCI/virtio driver, firmware boot, GPT repair or installation is attempted.
The source bytes are generated separately from the Rust verifier.
"""
import argparse
import struct
import zlib
from pathlib import Path

CAP = 512
SECTOR = 512
ARRAY_SIZE = 128 * 128
D_GUID = bytes(range(1,17))
P_GUID = bytes(range(17,33))
T_GUID = bytes(range(33,49))
OTHER_GUID = bytes(range(49,65))
OTHER_TYPE = bytes(range(65,81))

def write_u32(buf, p, value):
    struct.pack_into("<I",buf,p,value)

def write_u64(buf, p, value):
    struct.pack_into("<Q",buf,p,value)

def crc(buf):
    return zlib.crc32(buf) & 0xffffffff

def header(my, other, array_lba, entries, disk_guid=D_GUID):
    h=bytearray(SECTOR)
    h[0:8]=b"EFI PART"
    write_u32(h,8,0x00010000)
    write_u32(h,12,92)
    write_u64(h,24,my)
    write_u64(h,32,other)
    write_u64(h,40,34)
    write_u64(h,48,CAP-34)
    h[56:72]=disk_guid
    write_u64(h,72,array_lba)
    write_u32(h,80,128)
    write_u32(h,84,128)
    write_u32(h,88,crc(entries))
    write_u32(h,16,crc(h[:92]))
    return h

def partition(arr, index, kind, guid, start, end):
    p=index*128
    arr[p:p+128]=bytes(128)
    arr[p:p+16]=kind
    arr[p+16:p+32]=guid
    write_u64(arr,p+32,start)
    write_u64(arr,p+40,end)

def valid():
    whole=bytearray(CAP*SECTOR)
    whole[446+4]=0xee
    write_u32(whole,446+8,1)
    write_u32(whole,446+12,CAP-1)
    whole[510:512]=b"\x55\xaa"
    arr=bytearray(ARRAY_SIZE)
    partition(arr,0,T_GUID,P_GUID,40,100)
    whole[2*SECTOR:34*SECTOR]=arr
    whole[(CAP-33)*SECTOR:(CAP-1)*SECTOR]=arr
    whole[SECTOR:2*SECTOR]=header(1,CAP-1,2,arr)
    whole[(CAP-1)*SECTOR:CAP*SECTOR]=header(CAP-1,1,CAP-33,arr)
    return whole

def refresh_header_crc(b, side):
    off=(1 if side=="primary" else CAP-1)*SECTOR
    write_u32(b,off+16,0)
    write_u32(b,off+16,crc(b[off:off+92]))

def edit_entries(b, func, update_header_crc=True):
    out=bytearray(b)
    arr=bytearray(out[2*SECTOR:34*SECTOR])
    func(arr)
    out[2*SECTOR:34*SECTOR]=arr
    out[(CAP-33)*SECTOR:(CAP-1)*SECTOR]=arr
    if update_header_crc:
        for side in ("primary","backup"):
            off=(1 if side=="primary" else CAP-1)*SECTOR
            write_u32(out,off+88,crc(arr))
            refresh_header_crc(out,side)
    return out

def create():
    v=valid()
    outputs={"valid":v, "clone":bytearray(v)}
    x=bytearray(v); x[SECTOR+16]^=1;outputs["primary_crc_bad"]=x
    x=bytearray(v);x[(CAP-1)*SECTOR+16]^=1;outputs["backup_crc_bad"]=x
    x=bytearray(v);x[510]^=1;outputs["protective_mbr_bad"]=x
    x=bytearray(v);x[SECTOR+92]=1;outputs["primary_reserved_bad"]=x
    x=bytearray(v);x[2*SECTOR+60]^=1;outputs["primary_array_mismatch"]=x
    outputs["both_arrays_wrong_crc"]=edit_entries(v,lambda a:a.__setitem__(60,a[60]^1),False)
    outputs["overlap"]=edit_entries(v,lambda a:partition(a,1,T_GUID,OTHER_GUID,90,150))
    outputs["duplicate_guid"]=edit_entries(v,lambda a:partition(a,1,T_GUID,P_GUID,120,150))
    outputs["out_of_range"]=edit_entries(v,lambda a:write_u64(a,32,30))
    outputs["large_lba_out_of_range"]=edit_entries(v,lambda a:write_u64(a,40,(1<<32)+1))
    outputs["missing_partition"]=edit_entries(v,lambda a:a.__setitem__(slice(0,128),bytes(128)))
    outputs["wrong_type"]=edit_entries(v,lambda a:a.__setitem__(slice(0,16),OTHER_TYPE))
    x=bytearray(v)
    for side in ("primary","backup"):
        off=(1 if side=="primary" else CAP-1)*SECTOR
        x[off+56:off+72]=OTHER_GUID
        refresh_header_crc(x,side)
    outputs["wrong_disk_guid"]=x
    x=bytearray(v);x[(CAP-1)*SECTOR+56:(CAP-1)*SECTOR+72]=OTHER_GUID
    refresh_header_crc(x,"backup");outputs["mismatched_header_guids"]=x
    x=bytearray(v)
    for side in ("primary","backup"):
        off=(1 if side=="primary" else CAP-1)*SECTOR
        write_u32(x,off+80,256)
        refresh_header_crc(x,side)
    outputs["unsupported_entries"]=x
    outputs["truncated_last_sector"]=v[:-SECTOR]
    return outputs

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out",type=Path,required=True)
    a=p.parse_args()
    a.out.mkdir(parents=True,exist_ok=False)
    result=create()
    for key,bytes_ in result.items():
        with (a.out/(key+".raw")).open("xb") as f:
            f.write(bytes_)
    print("synthetic_GPT_cases",len(result),"bytes_per_full_disk",len(result["valid"]))
    print("independent_GPT_header_CRC",hex(struct.unpack_from("<I",result["valid"],SECTOR+16)[0]))
    print("independent_GPT_entries_CRC",hex(struct.unpack_from("<I",result["valid"],SECTOR+88)[0]))
    print("No physical disk, virtio PCI driver, hardware identity, ISO or repair")
if __name__=="__main__":
    main()
