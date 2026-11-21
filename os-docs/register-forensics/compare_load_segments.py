#!/usr/bin/env python3
"""Compare the actual file-backed ELF PT_LOAD bytes and load-map contracts.

This is an independent, read-only discriminator for two *built* PCP OS kernel
ELF files under the same compiler and source path. It is not a bootloader proof,
a release safety certificate, or evidence of an independently running model.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import struct
import sys

PT_LOAD=1


def map_segments(path):
    data=Path(path).read_bytes()
    if data[:4]!=b'\x7fELF' or len(data)<64 or data[5]!=1 or data[4] not in (1,2):
        raise ValueError("not a supported ELF32/ELF64 little-endian image")
    is64=data[4]==2
    if is64:
        entry,phoff=struct.unpack_from("<QQ",data,24)
        phsize,phnum=struct.unpack_from("<HH",data,54)
        fmt="<IIQQQQQQ"
    else:
        entry,phoff=struct.unpack_from("<II",data,24)
        phsize,phnum=struct.unpack_from("<HH",data,42)
        fmt="<IIIIIIII"
    size=struct.calcsize(fmt)
    if not 0<phnum<=64 or phsize<size or phoff+phnum*phsize>len(data):
        raise ValueError("invalid or unsupported ELF program-header table")
    descriptors=[]
    for index in range(phnum):
        values=struct.unpack_from(fmt,data,phoff+index*phsize)
        if is64:
            kind,flags,offset,vaddr,paddr,filesz,memsz,align=values
        else:
            kind,offset,vaddr,paddr,filesz,memsz,flags,align=values
        if filesz>memsz and kind==PT_LOAD:
            raise ValueError("load segment file size exceeds memory size")
        if offset+filesz>len(data):
            raise ValueError("load program header exceeds file")
        payload=data[offset:offset+filesz]
        descriptors.append({
            "index":index,"kind":kind,"flags":flags,"offset":offset,
            "vaddr":vaddr,"paddr":paddr,"file_size":filesz,
            "mem_size":memsz,"align":align,
            "file_payload_sha256":hashlib.sha256(payload).hexdigest()
        })
    load=[x for x in descriptors if x["kind"]==PT_LOAD]
    if not load:
        raise ValueError("no ELF PT_LOAD programs")
    return {"class":64 if is64 else 32,"entry":entry,"ELF_sha256":hashlib.sha256(data).hexdigest(),
            "ELF_byte_length":len(data),"program_headers":descriptors,"load_count":len(load)}


def compare(a,b):
    if a["class"]!=b["class"]:
        raise ValueError("different ELF classes")
    lhs,rhs=a["program_headers"],b["program_headers"]
    if [x["kind"] for x in lhs]!=[x["kind"] for x in rhs]:
        raise ValueError("different program-header topology")
    keys=("kind","flags","vaddr","paddr","file_size","mem_size","align","file_payload_sha256")
    physical_keys=keys+("offset",)
    loadpairs=[(x,y) for x,y in zip(lhs,rhs) if x["kind"]==PT_LOAD]
    result={
       "ELF_class":a["class"],"PT_LOAD_count":len(loadpairs),
       "entry_point_identical":a["entry"]==b["entry"],
       "PT_LOAD_memory_contracts_and_payloads_identical":bool(loadpairs) and
                all(all(x[k]==y[k] for k in keys) for x,y in loadpairs),
       "PT_LOAD_offsets_identical":all(x["offset"]==y["offset"] for x,y in loadpairs),
       "every_program_header_content_and_offset_identical":all(
                all(x[k]==y[k] for k in physical_keys) for x,y in zip(lhs,rhs)),
       "source_ELF_sha256":a["ELF_sha256"],"candidate_ELF_sha256":b["ELF_sha256"],
       "whole_ELF_identical":a["ELF_sha256"]==b["ELF_sha256"],
       "load_pairs":[{"index":x["index"],"source":x,"candidate":y} for x,y in loadpairs],
       "scope":"actual ELF file-backed PT_LOAD descriptors and file payloads only",
       "runtime_equivalence_or_ISO_publication_established":False
    }
    return result


def main():
    if len(sys.argv)!=5:
        raise SystemExit("usage: compare_load_segments.py BASE_A BASE_B CANDIDATE OUTPUT")
    a,b,c=(map_segments(x) for x in sys.argv[1:4])
    if a["ELF_sha256"]!=b["ELF_sha256"]:
        raise SystemExit("A/A reproducibility failed; no cross-source inference")
    result=compare(a,c)
    result["A_A_complete_ELF_identical"]=True
    result["source_revision"]="74c4d4a9934ad68e1b5be380971bf3377a464c6d"
    result["candidate_revision"]="f1b7430a9c1aaabc5587f19b16e0c1e76646ef81"
    Path(sys.argv[4]).write_text(json.dumps(result,sort_keys=True,indent=2)+"\n")
    print(json.dumps(result,sort_keys=True))
    return 0


if __name__=="__main__":
    raise SystemExit(main())
