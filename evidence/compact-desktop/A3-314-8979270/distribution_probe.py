from pathlib import Path
import hashlib,json,runpy,subprocess
root=Path(__file__).resolve().parent
checkout=root.parent/'a3-mapper318/distribution'
def rev(path):return subprocess.check_output(['git','-C',path,'rev-parse','HEAD'],text=True).strip()
path=checkout/'audit/public-live-cd/desktop_screens.py'
screens=runpy.run_path(str(path))['DesktopScreens'](root/'src',{'rdesk.elf':hashlib.sha256((root/'rdesk.elf').read_bytes()).hexdigest()})
out={}
for mode in ['320x200','320x220','320x240','320x260','320x280','640x480','800x600']:
    folder=root/mode
    frame=(folder/'power.ppm').read_bytes()
    rows=json.loads((folder/'decoded.json').read_text())['power']
    text=' '.join(r.strip() for r in rows if r.strip())
    out[mode]=dict(accepted=screens.matches(frame,'power'),screen_sha256=hashlib.sha256(frame).hexdigest(),warning_complete='Files of this session that were not saved by a program are lost.' in text,recorded_words=text)
report=dict(checker_commit=rev(checkout),os_commit=rev(root/'src'),source='audit/public-live-cd/desktop_screens.py',checker_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),cases=out)
(root/'distribution-power-readback.json').write_text(json.dumps(report,indent=2)+'\n')
print([(m,v['accepted'],v['warning_complete']) for m,v in out.items()])
