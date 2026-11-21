"""Read successive real narrow Details pages with the working checker."""
import hashlib,json,re,runpy,sys
from pathlib import Path
source,guest=map(lambda p:Path(p).resolve(),sys.argv[1:3])
api=runpy.run_path(str(source/'examples/tester-cd/check_small_screens.py'))
font=api['CHECK']['kernel_glyphs']()
r=json.loads((guest/'result.json').read_text())
pages=[api['PINS']['read']((guest/(h+'.ppm')).read_bytes(),font) for h in r['details_pages']]
facts={'alpha.elf':('117 KiB',r['facts']['alpha.elf']),'bravo.elf':('53 KiB',r['facts']['bravo.elf'])}
lines=api['merged_lines'](pages)
positions={app:[n for n,line in enumerate(lines) if line==app] for app in facts}
def sections(lines):
    return (lines[lines.index('PROGRAMS')+1:lines.index('FILES FROM THE LOADER')],lines[lines.index('FILES FROM THE LOADER')+1:lines.index('MACHINE')])
by_section=[api['details_problems'](s,facts) for s in sections(lines)]
actual=api['details_problems'](lines,facts)
print(json.dumps({'positions':positions,'checker_problems_on_original':actual,'problems_scoped_to_each_section':by_section,'pages':len(pages)},indent=2))
assert all(len(index)==2 for index in positions.values()) and len(actual)==2 and by_section==[[],[]]
