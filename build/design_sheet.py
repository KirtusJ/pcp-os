#!/usr/bin/env python3
"""Write the design sheet: every edition theme, module profile and workspace on one self-contained page.

    python3 design_sheet.py <design dir>      (reads pcp.tokens.json and pcp.tokens.audit.json there)
"""
import json, sys, html
from pathlib import Path
D = Path(sys.argv[1])
T = json.loads((D / "pcp.tokens.json").read_text()); A = json.loads((D / "pcp.tokens.audit.json").read_text())
ed, mod = T["edition"], T["module"]
e = html.escape
out = ["""<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>PCP design profiles</title>
<style>
:root{--ink:#18222D;--soft:#5B6B7A;--line:#DDE3EA;--page:#FFFFFF}
*{box-sizing:border-box}body{margin:0;background:var(--page);color:var(--ink);font:15px/1.5 Inter,system-ui,-apple-system,"Segoe UI",sans-serif}
main{max-width:1080px;margin:0 auto;padding:32px 16px 64px}
h1{font-size:28px;line-height:1.2;margin:0 0 4px}h2{font-size:19px;margin:40px 0 6px}p{margin:0 0 12px;max-width:70ch}.sub{color:var(--soft)}
code{font:13px ui-monospace,"DejaVu Sans Mono",Menlo,monospace}
.row{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:12px}
.card{border-radius:10px;padding:16px;min-height:132px;border:1px solid var(--line)}
.card b{display:block;font-size:17px}.card small{display:block;opacity:.9}
.plates{display:grid;grid-template-columns:repeat(auto-fit,minmax(138px,1fr));gap:12px}
.plate{border-radius:14px;padding:14px;min-height:118px;border:1px solid var(--line);display:flex;flex-direction:column;justify-content:space-between}
.plate b{font-size:16px}.plate small{font-size:12px}
.scroll{overflow-x:auto}table{border-collapse:collapse;width:100%;min-width:720px}th,td{padding:0;text-align:left;vertical-align:top}
th{font-size:12px;color:var(--soft);font-weight:600;padding:6px 8px}
.ws{padding:10px 12px;border-bottom:1px solid rgba(128,128,128,.25)}
.ws .name{font-weight:700}.ws .bar{height:4px;border-radius:2px;margin:6px 0 4px}.ws small{font-size:12px}
.tag{display:inline-block;border-radius:6px;padding:1px 7px;font-size:12px;font-weight:600}
.note{border-left:3px solid var(--line);padding:2px 0 2px 12px;color:var(--soft)}
</style></head><body><main>
<h1>Parrot Council Protocol &middot; design profiles</h1>
<p class="sub">Evidence before agreement. &nbsp;Edition sets the page. Module sets the hue. A workspace is a module on an edition's page.</p>
<p>Generated from <code>pcp.tokens.json</code>. Every figure on this page is computed from the colours shown, by the build that wrote it.</p>
<h2>The three editions</h2>
<p>Told apart by lightness first, hue second, and by name always. Lightness survives greyscale printing and colour-vision deficiency; hue does not.</p><div class="row">"""]
for k, v in ed.items():
    a = A["edition"][k]
    out.append('<div class="card" style="background:%s;color:%s"><b>%s</b><small>field <code>%s</code> &middot; text <code>%s</code></small><p style="color:%s;margin:10px 0 0;font-weight:600">Accent %s</p><small>text %s:1 &middot; accent %s:1 &middot; secondary %s:1</small></div>'
               % (v["field"]["$value"], v["text"]["$value"], e(v["$description"]), v["field"]["$value"], v["text"]["$value"], v["accent"]["$value"], v["accent"]["$value"], a["text_on_field"], a["accent_on_field"], a["soft_on_field"]))
out.append('</div><h2>The seven modules</h2><p>One plate each, the same in every edition, with a wing mark so that the plate is never read by colour alone.</p><div class="plates">')
for k, v in mod.items():
    out.append('<div class="plate" style="background:%s;color:%s"><b>%s</b><small>%s<br><code>%s</code> &middot; %s:1</small></div>'
               % (v["plate"]["$value"], v["on-plate"]["$value"], e(v["name"]["$value"]), e(v["mark"]["$value"]), v["plate"]["$value"], A["module_plate"][k]["on_plate"]))
out.append('</div><h2>Why each hue</h2><div class="scroll"><table style="min-width:560px">')
for k, v in mod.items():
    out.append('<tr><td style="padding:8px 10px 8px 0;white-space:nowrap"><span class="tag" style="background:%s;color:%s">%s</span></td><td style="padding:8px 0;border-bottom:1px solid var(--line)">%s</td></tr>'
               % (v["plate"]["$value"], v["on-plate"]["$value"], e(v["name"]["$value"]), e(v["$description"])))
out.append('</table></div><h2>The 21 workspaces</h2><p>The module\'s hue, moved in lightness only as far as needed to reach 4.5 to 1 on that edition\'s field. The figure under each is the measured contrast of the accent against the field.</p><div class="scroll"><table><tr><th>Module</th>')
for k, v in ed.items():
    out.append("<th>%s</th>" % e(v["$description"]))
out.append("</tr>")
for mk, mv in mod.items():
    out.append('<tr><td style="padding:10px 12px 10px 0;white-space:nowrap"><span class="tag" style="background:%s;color:%s">%s</span></td>' % (mv["plate"]["$value"], mv["on-plate"]["$value"], e(mv["name"]["$value"])))
    for ek, ev in ed.items():
        w = A["workspace"][mk][ek]
        out.append('<td><div class="ws" style="background:%s;color:%s"><span class="name" style="color:%s">%s workspace</span><div class="bar" style="background:%s"></div><small>accent <code>%s</code> &middot; %s:1</small></div></td>'
                   % (ev["field"]["$value"], ev["text"]["$value"], w["accent"], e(mv["name"]["$value"]), w["accent"], w["accent"], w["accent_on_field"]))
    out.append("</tr>")
sep = A["separation"]
out.append('</table></div><h2>What the measurements say, including what they do not support</h2>')
out.append("<p>Every text pairing above is at or over 4.5 to 1 (WCAG 2.2, 1.4.3). Edition fields stay at least %s units apart in OKLab under every simulated colour-vision deficiency.</p>" % min(v["distance"] for v in sep["edition_fields"].values()))
worst = min((v["distance"], k, n, v["closest_pair"]) for k, d in sep.items() if k.startswith("workspace_accents") for n, v in d.items())
out.append('<p class="note">Hue alone does not separate every pair of modules for every viewer. The closest pair of workspace accents is %s and %s (%s, %s): %s units, where about 2 is just noticeable. Module plates are closest for %s and %s under deuteranopia (%s units). This is why each module also has a name and a wing mark, and why no colour is ever the only code (WCAG 1.4.1). These are computed figures. Deficiency is simulated; no test with people, print proof or accessibility audit was performed.</p>'
           % (worst[3][0].replace("_", " "), worst[3][1].replace("_", " "), worst[1].replace("workspace_accents_on_", "") + " field", worst[2], worst[0],
              sep["module_plates"]["deuteranopia"]["closest_pair"][0], sep["module_plates"]["deuteranopia"]["closest_pair"][1], sep["module_plates"]["deuteranopia"]["distance"]))
out.append("<p>Colour decides nothing in a review. A workspace's colour repeats what its name already says.</p></main></body></html>")
(D / "pcp.design.html").write_text("\n".join(out) + "\n")
print("written", (D / "pcp.design.html").stat().st_size)
