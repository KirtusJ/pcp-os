#!/usr/bin/env python3
"""Read-only ELF section comparator for bounded PCP OS diagnostic builds.

Compares *the bytes represented by each ELF section*, distinguishing allocated
load-image sections from non-allocated symbol/debug/metadata tables. Never
classify a difference as harmless solely because its section is non-allocated.
No access to installed, live, or private guest state.
"""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
import struct
import sys

SHF_ALLOC=2
SHT_NOBITS=8


def sections(path):
    data=Path(path).read_bytes()
    if data[:4]!=b'\x7fELF' or len(data)<64:
        raise ValueError("invalid ELF file")
    if data[5]!=1 or data[4] not in (1,2):
        raise ValueError("only little-endian ELF32/64 supported")
    is64=data[4]==2
    if is64:
        shoff=struct.unpack_from("<Q",data,40)[0]
        entsize, count, namesidx=struct.unpack_from("<HHH",data,58)
        fmt="<IIQQQQIIQQ"
    else:
        shoff=struct.unpack_from("<I",data,32)[0]
        entsize, count, namesidx=struct.unpack_from("<HHH",data,46)
        fmt="<IIIIIIIIII"
    n=struct.calcsize(fmt)
    if count==0 or namesidx>=count or entsize<n:
        raise ValueError("unsupported or malformed section table")
    if shoff + count*entsize > len(data):
        raise ValueError("truncated section table")
    table=[]
    for i in range(count):
        v=struct.unpack_from(fmt,data,shoff+i*entsize)
        # all class formats have name, type, flags, addr, offset, size at first 6.
        nameidx,kind,flags,addr,offset,size=v[:6]
        if kind!=SHT_NOBITS and offset+size>len(data):
            raise ValueError("section data extends beyond ELF")
        table.append(dict(index=i,name_offset=nameidx,type=kind,flags=flags,address=addr,
                          offset=offset,size=size))
    st=table[namesidx]
    if st["type"]==SHT_NOBITS:
        raise ValueError("string table has no data")
    names=data[st["offset"]:st["offset"]+st["size"]]
    out=[]
    for section in table:
        index=section["name_offset"]
        if index>=len(names):
            raise ValueError("invalid section name offset")
        end=names.find(b'\0',index)
        if end<0:
            raise ValueError("unterminated section name")
        name=names[index:end].decode("utf-8")
        payload=b'' if section["type"]==SHT_NOBITS else data[section["offset"]:section["offset"]+section["size"]]
        out.append({**{k:v for k,v in section.items() if k!="name_offset"},
                    "name":name,"allocated":bool(section["flags"]&SHF_ALLOC),
                    "payload_sha256":None if section["type"]==SHT_NOBITS else hashlib.sha256(payload).hexdigest()})
    return {"elf_class":64 if is64 else 32,"file_size":len(data),
            "file_sha256":hashlib.sha256(data).hexdigest(),"sections":out}


def compare(a,b):
    if a["elf_class"]!=b["elf_class"]:
        raise ValueError("ELF architectures differ")
    sa,sb=a["sections"],b["sections"]
    if [(x["index"],x["name"]) for x in sa]!=[(x["index"],x["name"]) for x in sb]:
        raise ValueError("section-name/index topology differs; cannot compare by index")
    differences=[]
    for x,y in zip(sa,sb):
        changed=[k for k in ("type","flags","address","offset","size","payload_sha256") if x[k]!=y[k]]
        if changed:
            differences.append({"name":x["name"],"index":x["index"],"allocated":x["allocated"] or y["allocated"],
                                "changed_fields":changed,
                                "source_size":x["size"],"candidate_size":y["size"],
                                "source_digest":x["payload_sha256"],
                                "candidate_digest":y["payload_sha256"]})
    return {"source_elf_sha256":a["file_sha256"],"candidate_elf_sha256":b["file_sha256"],
            "source_elf_size":a["file_size"],"candidate_elf_size":b["file_size"],
            "section_count":len(sa),"differences":differences,
            "allocated_section_payloads_identical":all(x["payload_sha256"]==y["payload_sha256"]
                                                        for x,y in zip(sa,sb) if x["allocated"] or y["allocated"]),
            "all_section_payloads_identical":all(x["payload_sha256"]==y["payload_sha256"]
                                                for x,y in zip(sa,sb)),
            "whole_elf_bytes_identical":a["file_sha256"]==b["file_sha256"],
            "runtime_equivalence_or_released_image_authenticated":False}


def main():
    if len(sys.argv)!=5:
        raise SystemExit("usage: compare_elf_sections.py BASE_A BASE_B CANDIDATE OUTPUT_JSON")
    base1,base2,cand=[sections(x) for x in sys.argv[1:4]]
    if base1["file_sha256"]!=base2["file_sha256"]:
        raise SystemExit("A/A builds disagree: no causal attribution")
    result=compare(base1,cand)
    result.update({"a_a_full_ELF_equal":True,
                   "source_revision":"74c4d4a9934ad68e1b5be380971bf3377a464c6d",
                   "candidate_revision":"f1b7430a9c1aaabc5587f19b16e0c1e76646ef81",
                   "scope":"exact same runner/stage compiled binaries, not installed production"})
    Path(sys.argv[4]).write_text(json.dumps(result,indent=2,sort_keys=True)+"\n")
    print(json.dumps(result,sort_keys=True))
    return 0


if __name__=="__main__":
    raise SystemExit(main())
