from pathlib import Path
from tempfile import TemporaryDirectory
import hashlib
import json
import os
import struct
import subprocess
import uuid
import zlib

root = Path.cwd()
lab = root/'service/qwen3-release'
iso = lab/'PCPOS-Qwen3-8B.iso'
fat = lab/'worker24-qwen3-8b.fat'
files = json.loads((lab/'expected-files.json').read_text())
base = json.loads((lab/'fat-readback.json').read_text())
def digest(p):
    with p.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()
def sha_sorted_shards(parts):
    whole = hashlib.sha256()
    for i in range(75):
        with (parts/f'shard-{i:05}.bin').open('rb') as f:
            while data := f.read(4 << 20):
                whole.update(data)
    return whole.hexdigest()
with iso.open('rb') as disc:
    def read(offset, count):
        disc.seek(offset)
        data = disc.read(count)
        assert len(data) == count
        return data
    def gpt(lba):
        header = read(lba*512,512)
        assert header[:8] == b'EFI PART'
        size, crc = struct.unpack_from('<II',header,12)
        check = bytearray(header[:size])
        check[16:20] = bytes(4)
        assert 92 <= size <= 512 and zlib.crc32(check) == crc
        current, alt = struct.unpack_from('<QQ',header,24)
        addr, n, stride, want = struct.unpack_from('<QIII',header,72)
        assert current == lba and n*stride <= 1 << 20 and stride >= 128
        table = read(addr*512,n*stride)
        assert zlib.crc32(table) == want
        esp_type = uuid.UUID('C12A7328-F81F-11D2-BA4B-00A0C93EC93B').bytes_le
        spans = [struct.unpack_from('<QQ',table,i+32) for i in range(0,len(table),stride) if table[i:i+16] == esp_type]
        assert len(spans) == 1
        return alt, spans[0]
    backup, esp = gpt(1)
    primary, backup_esp = gpt(backup)
    assert primary == 1 and esp == backup_esp and (backup+1)*512 == iso.stat().st_size
    low, high = esp
    esp_bytes = (high-low+1)*512
    assert esp_bytes == fat.stat().st_size
    catalog = None
    for i in range(16,80):
        d = read(i*2048,2048)
        assert d[1:6] == b'CD001'
        if d[0] == 0 and d[7:30].startswith(b'EL TORITO SPECIFICATION'):
            catalog = struct.unpack_from('<I',d,71)[0]
        if d[0] == 255:
            break
    assert catalog
    page = read(catalog*2048,2048)
    assert page[0:2] == b'\x01\xef' and page[30:34] == b'\x55\xaa\x88\x00'
    assert sum(struct.unpack('<16H',page[:32])) % 65536 == 0
    sectors, boot_lba = struct.unpack_from('<HI',page,38)
    assert boot_lba*2048 == low*512
    disc.seek(low*512)
    with fat.open('rb') as src:
        written = hashlib.sha256()
        remaining = esp_bytes
        while remaining:
            chunk = disc.read(min(4<<20, remaining))
            assert len(chunk) > 0 and chunk == src.read(len(chunk))
            written.update(chunk)
            remaining -= len(chunk)
    assert written.hexdigest() == base['sha256']
with TemporaryDirectory(dir=lab) as tmp:
    tree = Path(tmp)/'tree'
    subprocess.run([root/'runtime/usr/bin/xorriso','-read_fs','norock','-osirrox','on','-indev',iso,'-extract','/',tree],check=True)
    actual = {p.relative_to(tree).as_posix() for p in tree.rglob('*') if p.is_file()}
    assert actual == set(files), actual ^ set(files)
    for name, info in files.items():
        assert (tree/name).stat().st_size == info['bytes'] and digest(tree/name) == info['sha256'], name
    model = sha_sorted_shards(tree/'PCPOS')
    assert model == base['ordered_model_sha256']
summary = dict(iso_bytes=iso.stat().st_size,iso_sha256=digest(iso),fat_bytes=esp_bytes,
        fat_sha256=written.hexdigest(),esp_start_byte=low*512,primary_and_backup_gpt_crc=True,
        identical_esp_and_fat=True,el_torito_lba=boot_lba,el_torito_sector_count=sectors,
        joliet_files=len(files),verified_shards=75,model_sha256=model,model_bytes=5027783488,
        final_shard_bytes=files['PCPOS/shard-00074.bin']['bytes'],
        kernel_source='41e35d1f167dd9c2bd043380cd454fdfa41db6e3',loader_source='44b77452784d2654386b8818c769d10f3403bb41',
        worker_source='9c14937c413d7fcd63878b9870ae0862d760c69c',worker_ci_artifact=14752508424,
        worker_zip_sha256=digest(root/'service/worker24-dual.zip'),
        publisher_commit='7c41481f57cb95916b40956ab2f0b139b296d974',component_receipt_sha256=digest(lab/'efi-inputs.json'),
        efi_build_record_sha256=digest(lab/'efi-build.json'))
(lab/'image-readback.json').write_text(json.dumps(summary,indent=2,sort_keys=True)+'\n')
print(json.dumps(summary,indent=2))
