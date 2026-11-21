"""Replay #324's scale boundary evidence. Supply an exact source checkout and the records directory."""
import hashlib
import json
import runpy
import sys
from pathlib import Path
import numpy as np
from PIL import Image
from read import font, rows

source, root = (Path(s).resolve() for s in sys.argv[1:])
stock = runpy.run_path(str(source/"examples/tester-cd/check_small_screens.py"))
hash_bytes = lambda b: hashlib.sha256(b).hexdigest()
out = {}
dims = {"1280x944":(1,128),"1280x960":(2,64),"1904x1080":(2,95),"1920x1080":(2,96)}
for size, (scale, cols) in dims.items():
    base = root/size
    m = json.loads((base/"result.json").read_text())
    assert m["source"]=="17f6b30fda94fd217823bc0437e905d420aa1cc4"
    assert m["completed"] and m["exit"]==0 and m["columns"]==cols
    serial=(base/"serial.txt").read_text()
    assert f"desktop: {size} at {scale}x UI scale" in serial and "ALL SELF-TESTS PASS." in serial
    n=sum(line.startswith("  pass ") for line in serial.splitlines())
    assert n==m["final_checks"]==72
    table=font(source, scale)
    decoded={}
    for digest in set(m["frames"].values()):
        with Image.open(base/(digest+".png")) as f:
            assert f.size==tuple(int(v) for v in size.split("x"))
            pixels=np.asarray(f.convert("RGB"))
        assert hash_bytes(pixels.tobytes())==digest
        decoded[digest]=rows(pixels,table,scale)
    for label,digest in m["frames"].items():
        assert decoded[digest]==(base/(label+".txt")).read_text().splitlines()
    frames=m["frames"]
    for key in "drpvkf":
        assert frames["home-"+key]==frames["home"] and frames["programs-"+key]==frames["selected"]
    assert frames["selected"]==frames["returned"]==frames["cancelled"]
    assert not stock["power_problems"](decoded[frames["power"]])
    detail_pages=[item for item in frames if item.startswith("details-")]
    facts={"rdesk.elf":("53 KiB","a7684bf04234a61b408f2d050ecd71cd27a8fa3d80a2e571be46e45342e7b37c"),
           "surf.elf":("117 KiB","943d2622cfe28d6c4220635d7a28bedf325fede584254639b3165dd767a17ec3")}
    if cols<96:
        parts=[]
        for label in detail_pages:
            page=decoded[frames[label]]
            assert page[1].strip()=="DETAILS"
            height=int(size.split("x")[1])//(20*scale)
            content=[s.strip() for s in page[3:3+height-6]]
            overlap=max((k for k in range(1,min(len(parts),len(content))+1) if parts[-k:]==content[:k]),default=0)
            if parts: assert overlap
            parts.extend(content[overlap:])
        assert parts==(base/"sections.txt").read_text().splitlines()
        assert not stock["details_problems"](parts,facts)
        for heading,section in stock["details_sections"](parts).items():
            for name,(_,digest) in facts.items():
                beg=parts.index(heading)+1+section.index(name)+3
                j=beg
                while not "".join(parts[beg:j+1])==digest:
                    j+=1
                    assert j<len(parts)
                damaged=parts[:]
                damaged[j]=damaged[j][:-1]
                errors=stock["details_problems"](damaged,facts)
                assert len(errors)==1 and name in errors[0] and heading in errors[0]
    else:
        assert any("FILES FROM THE LOADER" in s for s in decoded[frames["details-0"]])
        assert any("REGISTER" in s for s in decoded[frames["plain-register"]])
    out[size]={"scale":scale,"columns":cols,"route":m["route"],"checks":n,
               "unique_frames":len(decoded),"unchanged_letter_inputs":12,
               "digits_independently_erased_and_rejected":4 if cols<96 else 0}
print(json.dumps(out,indent=2))
