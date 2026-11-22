"""Check assembly refusal and pin binding using the four files of a known-good parts directory."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
from build import FILES, FIXTURE, header

def digest(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("parts",type=Path);parser.add_argument("receipt",type=Path)
    args=parser.parse_args()
    with tempfile.TemporaryDirectory() as temporary:
        d=Path(temporary)
        for name in FILES: shutil.copyfile(args.parts/name,d/name)
        receipt=json.loads(args.receipt.read_bytes());lock=d/"receipt.json"
        def write():
            lock.write_text(json.dumps(receipt));return digest(lock)
        def refuses(pin,part):
            try: header(d,lock,pin)
            except ValueError as e:
                if part in str(e): return
                raise
            raise AssertionError("accepted "+part)
        pin=write();assert "MEASUREMENT_TEXT" in header(d,lock,pin)
        refuses("0"*64,"independent pin")
        receipt["kernel_features"].append("model-memory-fixture");refuses(write(),"fixture")
        receipt["kernel_features"].remove("model-memory-fixture")
        receipt["worker_manifest_sha256"]="0"*64;refuses(write(),"worker build manifest pin")
        receipt["worker_manifest_sha256"]=receipt["files"]["model.manifest"]
        kernel=d/"KERNEL.ELF"
        with kernel.open("ab") as f:f.write(FIXTURE)
        refuses(write(),"input SHA-256 differs")
        receipt["files"]["KERNEL.ELF"]=digest(kernel);refuses(write(),"fixture banner")
    print("PASS independent receipt pin, feature refusal, worker pin, file digest and re-pinned fixture binary")

if __name__=="__main__":main()
