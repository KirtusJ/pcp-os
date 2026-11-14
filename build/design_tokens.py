#!/usr/bin/env python3
"""Derive the PCP design tokens: three edition themes x seven module profiles, checked, not assumed.

    python3 design_tokens.py <out dir>

Inputs are the brand colours already in use on the covers (edition themes and module plates). For every
module and every edition field this computes the module's accent for that field: same hue, lightness
moved only as far as needed to reach the contrast floor. Output: Design Tokens Community Group format
(pcp.tokens.json), a CSS custom-property sheet, a terminal table, and an audit of every pairing.
Standard library only.
"""
import json, math, sys
from pathlib import Path

EDITIONS = {   # field, text, accent (sig), heading ink on white, table header, soft text
 "personal":   dict(name="Personal Edition",   field="#F6F0E4", text="#1B2733", accent="#A25600", ink="#8A4A00", th="#5E3608", soft="#4A5560"),
 "commercial": dict(name="Commercial Edition", field="#0F6B67", text="#F4FBF9", accent="#A6F0E4", ink="#0C5F5B", th="#0B4744", soft="#D5EEEA"),
 "enterprise": dict(name="Enterprise Edition", field="#16151F", text="#F3F2F8", accent="#B7A4F7", ink="#4B3A96", th="#2A2358", soft="#C9C6D8"),
}
MODULES = {    # plate background, text on the plate, the hue source (ink), wing mark, and why this hue
 "core":              dict(name="Core",              plate="#10283F", on="#F2F7FC", hue="#187C78", mark="plain wing",
    why="Navy ground with the reference bird's teal. The neutral base every other profile sits on; it claims no domain."),
 "mathematics":       dict(name="Mathematics",       plate="#701A32", on="#F2F7FC", hue="#7D2945", mark="two concentric arcs",
    why="Burgundy: a deep, low-lightness red, apart from the alarm reds used for errors. Reads as ink and binding, for derivation and proof."),
 "science":           dict(name="Science",           plate="#A5E7F5", on="#103844", hue="#006B86", mark="ring with a centre dot",
    why="Sky blue, the lightest chromatic plate: open field, observation. Blue stays distinct under red-green colour-vision deficiency, which shifts reds and greens and leaves the blue-yellow axis."),
 "engineering":       dict(name="Engineering",       plate="#D05D29", on="#211008", hue="#9A3E16", mark="two stacked chevrons",
    why="Orange, the hue safety signage uses for warning (ANSI Z535 pairs orange with its WARNING signal word). Fits work where a failure has consequences."),
 "cryptography":      dict(name="Cryptography",      plate="#34214F", on="#F2F7FC", hue="#68429B", mark="keyhole",
    why="Deep violet: the darkest chromatic plate, at the far end of the spectrum from orange. Concealment and keys."),
 "statistics":        dict(name="Statistics",        plate="#FFFFFF", on="#152137", hue="#152137", mark="three rising bars",
    why="White plate, ink-blue marks: the empty plot area. Deliberately achromatic so that data colours placed on it are not biased by a tint."),
 "quality_assurance": dict(name="Quality Assurance", plate="#B5D94B", on="#152137", hue="#4E6B12", mark="check mark",
    why="Lime green: the conventional pass hue, shifted toward yellow so it stays apart from the Commercial teal and survives red-green deficiency by lightness."),
}
TEXT_FLOOR, UI_FLOOR = 4.5, 3.0   # WCAG 2.2: 1.4.3 body text (AA), 1.4.11 non-text contrast

def rgb(h): return tuple(int(h[i:i+2], 16) / 255 for i in (1, 3, 5))
def hexs(c): return "#%02X%02X%02X" % tuple(max(0, min(255, round(x * 255))) for x in c)
def lin(u): return u / 12.92 if u <= 0.04045 else ((u + 0.055) / 1.055) ** 2.4
def gam(u): return 12.92 * u if u <= 0.0031308 else 1.055 * (max(u, 0) ** (1 / 2.4)) - 0.055
def lum(h):
    r, g, b = (lin(x) for x in rgb(h)); return 0.2126 * r + 0.7152 * g + 0.0722 * b
def contrast(a, b):
    x, y = sorted((lum(a), lum(b)), reverse=True); return (x + 0.05) / (y + 0.05)
def oklab(h):
    r, g, b = (lin(x) for x in rgb(h))
    l = (0.4122214708*r + 0.5363325363*g + 0.0514459929*b) ** (1/3)
    m = (0.2119034982*r + 0.6806995451*g + 0.1073969566*b) ** (1/3)
    s = (0.0883024619*r + 0.2817188376*g + 0.6299787005*b) ** (1/3)
    return (0.2104542553*l + 0.7936177850*m - 0.0040720468*s, 1.9779984951*l - 2.4285922050*m + 0.4505937099*s, 0.0259040371*l + 0.7827717662*m - 0.8086757660*s)
def from_oklab(L, a, b):
    l = (L + 0.3963377774*a + 0.2158037573*b) ** 3; m = (L - 0.1055613458*a - 0.0638541728*b) ** 3; s = (L - 0.0894841775*a - 1.2914855480*b) ** 3
    return (4.0767416621*l - 3.3077115913*m + 0.2309699292*s, -1.2684380046*l + 2.6097574011*m - 0.3413193965*s, -0.0041960863*l - 0.7034186147*m + 1.7076147010*s)
def oklch(h):
    L, a, b = oklab(h); return L, math.hypot(a, b), math.degrees(math.atan2(b, a)) % 360
def from_oklch(L, C, H):
    """In-gamut sRGB for this lightness and hue: chroma is reduced until the colour fits (CSS Color 4 gamut mapping, simplified)."""
    for k in range(0, 101):
        c = C * (1 - k / 100)
        r = from_oklab(L, c * math.cos(math.radians(H)), c * math.sin(math.radians(H)))
        if all(-0.0005 <= x <= 1.0005 for x in r):
            return hexs(tuple(gam(x) for x in r))
    return hexs((L, L, L))
def de(a, b):   # distance in OKLab, times 100
    return 100 * math.dist(oklab(a), oklab(b))
# Machado, Oliveira and Fernandes (2009), severity 1.0, applied in linear RGB
CVD = {"protanopia": ((0.152286, 1.052583, -0.204868), (0.114503, 0.786281, 0.099216), (-0.003882, -0.048116, 1.051998)),
       "deuteranopia": ((0.367322, 0.860646, -0.227968), (0.280085, 0.672501, 0.047413), (-0.011820, 0.042940, 0.968881)),
       "tritanopia": ((1.255528, -0.076749, -0.178779), (-0.078411, 0.930809, 0.147602), (0.004733, 0.691367, 0.303900))}
def sim(h, kind):
    v = [lin(x) for x in rgb(h)]
    return hexs(tuple(gam(max(0, min(1, sum(m * x for m, x in zip(row, v))))) for row in CVD[kind]))

def accent_for(hue_hex, field, floor):
    """The module's hue on this field: keep hue and chroma, move lightness the least distance that reaches the floor."""
    L0, C, H = oklch(hue_hex)
    best = None
    for i in range(0, 1001):
        L = i / 1000
        c = from_oklch(L, C, H)
        if contrast(c, field) >= floor and (best is None or abs(L - L0) < abs(best[0] - L0)):
            best = (L, c)
    return best[1]

def build():
    tokens = {"$description": "Parrot Council Protocol (PCP) design tokens. Edition carries lightness and the page; module carries hue. Every colour is paired with a word or a mark: colour is never the only code.",
              "edition": {}, "module": {}, "workspace": {}}
    audit = {"floors": {"text": TEXT_FLOOR, "non_text": UI_FLOOR, "standard": "WCAG 2.2 success criteria 1.4.3 and 1.4.11; 1.4.1 (colour is not the only means)"}, "edition": {}, "module_plate": {}, "workspace": {}, "separation": {}}
    col = lambda v, d=None: ({"$type": "color", "$value": v, "$description": d} if d else {"$type": "color", "$value": v})
    for ek, e in EDITIONS.items():
        tokens["edition"][ek] = {"$description": e["name"], "field": col(e["field"]), "text": col(e["text"]), "accent": col(e["accent"]), "ink": col(e["ink"], "headings on a white page"),
                                 "table-header": col(e["th"]), "soft": col(e["soft"], "secondary text on the field")}
        audit["edition"][ek] = {"text_on_field": round(contrast(e["text"], e["field"]), 2), "accent_on_field": round(contrast(e["accent"], e["field"]), 2),
                                "soft_on_field": round(contrast(e["soft"], e["field"]), 2), "ink_on_white": round(contrast(e["ink"], "#FFFFFF"), 2), "white_on_table_header": round(contrast("#FFFFFF", e["th"]), 2)}
    for mk, m in MODULES.items():
        tokens["module"][mk] = {"$description": m["why"], "name": {"$type": "string", "$value": m["name"]}, "mark": {"$type": "string", "$value": m["mark"]},
                                "plate": col(m["plate"]), "on-plate": col(m["on"]), "hue": col(m["hue"], "the module's hue source; accents are derived from it per field")}
        audit["module_plate"][mk] = {"on_plate": round(contrast(m["on"], m["plate"]), 2)}
        tokens["workspace"][mk] = {}
        audit["workspace"][mk] = {}
        for ek, e in EDITIONS.items():
            a = accent_for(m["hue"], e["field"], TEXT_FLOOR)
            tokens["workspace"][mk][ek] = {"$description": "%s workspace, %s" % (m["name"], e["name"]),
                "field": {"$type": "color", "$value": "{edition.%s.field}" % ek}, "text": {"$type": "color", "$value": "{edition.%s.text}" % ek},
                "accent": col(a, "the module hue at the lightness that reaches %.1f:1 on this field" % TEXT_FLOOR),
                "plate": {"$type": "color", "$value": "{module.%s.plate}" % mk}, "on-plate": {"$type": "color", "$value": "{module.%s.on-plate}" % mk}}
            audit["workspace"][mk][ek] = {"accent": a, "accent_on_field": round(contrast(a, e["field"]), 2), "plate_against_field": round(contrast(m["plate"], e["field"]), 2)}
    # how far apart the things that must be told apart are, with and without simulated colour-vision deficiency
    def pairs(cols):
        ks = list(cols); out = {}
        for kind in (None, *CVD):
            best = min(((de(sim(cols[a], kind) if kind else cols[a], sim(cols[b], kind) if kind else cols[b]), a, b) for i, a in enumerate(ks) for b in ks[i+1:]))
            out[kind or "normal"] = {"closest_pair": [best[1], best[2]], "distance": round(best[0], 1)}
        return out
    audit["separation"]["edition_fields"] = pairs({k: e["field"] for k, e in EDITIONS.items()})
    audit["separation"]["module_plates"] = pairs({k: m["plate"] for k, m in MODULES.items()})
    for ek in EDITIONS:
        audit["separation"]["workspace_accents_on_" + ek] = pairs({mk: audit["workspace"][mk][ek]["accent"] for mk in MODULES})
    audit["separation"]["note"] = "Distance is Euclidean in OKLab times 100; about 2 is a just-noticeable difference. Deficiency is simulated (Machado et al. 2009, full severity). A simulation is not a test with people."
    fails = [("edition", k, n, v) for k, d in audit["edition"].items() for n, v in d.items() if v < TEXT_FLOOR]
    fails += [("plate", k, "on_plate", d["on_plate"]) for k, d in audit["module_plate"].items() if d["on_plate"] < TEXT_FLOOR]
    fails += [("workspace", mk, ek, d["accent_on_field"]) for mk, x in audit["workspace"].items() for ek, d in x.items() if d["accent_on_field"] < TEXT_FLOOR]
    audit["below_floor"] = fails
    return tokens, audit

def css(tokens, audit):
    out = ["/* PCP design tokens as CSS custom properties. Generated from pcp.tokens.json; do not edit. */",
           "/* Use: <html data-pcp-edition=\"personal\" data-pcp-module=\"science\">. Edition sets the page; module sets the accent. */"]
    for ek, e in EDITIONS.items():
        out.append('[data-pcp-edition="%s"] { --pcp-field: %s; --pcp-text: %s; --pcp-accent: %s; --pcp-soft: %s; --pcp-ink: %s; --pcp-table-header: %s; }' % (ek, e["field"], e["text"], e["accent"], e["soft"], e["ink"], e["th"]))
    for mk, m in MODULES.items():
        out.append('[data-pcp-module="%s"] { --pcp-plate: %s; --pcp-on-plate: %s; }' % (mk, m["plate"], m["on"]))
        for ek in EDITIONS:
            out.append('[data-pcp-edition="%s"][data-pcp-module="%s"] { --pcp-accent: %s; }' % (ek, mk, audit["workspace"][mk][ek]["accent"]))
    return "\n".join(out) + "\n"

def main():
    out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    tokens, audit = build()
    (out / "pcp.tokens.json").write_text(json.dumps(tokens, indent=1) + "\n")
    (out / "pcp.tokens.audit.json").write_text(json.dumps(audit, indent=1) + "\n")
    (out / "pcp.tokens.css").write_text(css(tokens, audit))
    # Claude Code terminal themes: one per module for a dark and a light terminal. The terminal's page is the
    # user's, so only the module shows here; the accent is the module hue at the contrast floor on that base.
    th = out / "themes"; th.mkdir(exist_ok=True)
    audit["terminal_themes"] = {}
    for mk, m in MODULES.items():
        for base, ground in (("dark", "#1E1E1E"), ("light", "#FFFFFF")):
            a = accent_for(m["hue"], ground, TEXT_FLOOR)
            slug = "pcp-%s%s" % (mk.replace("_", "-"), "" if base == "dark" else "-light")
            (th / (slug + ".json")).write_text(json.dumps({"name": "PCP %s%s" % (m["name"], "" if base == "dark" else " (light)"), "base": base,
                "overrides": {"claude": a, "promptBorder": a, "briefLabelClaude": a}}, indent=1) + "\n")
            audit["terminal_themes"][slug] = {"accent": a, "assumed_ground": ground, "contrast": round(contrast(a, ground), 2)}
    (out / "pcp.tokens.audit.json").write_text(json.dumps(audit, indent=1) + "\n")
    rows = ["module\tplate\ton_plate\tansi_background\tansi_text"]
    for mk, m in MODULES.items():
        r, g, b = (round(x * 255) for x in rgb(m["plate"])); fr, fg, fb = (round(x * 255) for x in rgb(m["on"]))
        rows.append("%s\t%s\t%s\t48;2;%d;%d;%d\t38;2;%d;%d;%d" % (mk, m["plate"], m["on"], r, g, b, fr, fg, fb))
    (out / "pcp.terminal.tsv").write_text("\n".join(rows) + "\n")
    print(json.dumps({"below_floor": audit["below_floor"], "separation": audit["separation"], "workspace": {mk: {ek: (d["accent"], d["accent_on_field"]) for ek, d in x.items()} for mk, x in audit["workspace"].items()}, "plates": audit["module_plate"], "edition": audit["edition"]}, indent=1))
    return 1 if audit["below_floor"] else 0

if __name__ == "__main__":
    sys.exit(main())
