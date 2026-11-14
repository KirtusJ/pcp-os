use super::*;
use crate::journal::{Journal, Note, Page};
use bellweather::manifest_text::{CASES, ENDINGS};

const RED: u32 = 0xaa4c3d;

fn paper(c: &mut Canvas<'_>, x: usize, y: usize, w: usize, h: usize) {
    c.rect(x + 3, y + 3, w, h, 0x0d252c);
    c.rect(x, y, w, h, PALE);
    c.rect(x + 2, y + 2, w - 4, h - 4, PAPER);
    c.rect(x + 2, y + 2, w - 4, 2, RULE);
    c.rect(x + 9, y + 8, 1, h - 16, 0xe8d6b9);
}
/// Visit word-wrapped slices without allocating. Original punctuation and intentional
/// paragraph breaks survive in the printed dossier.
fn lines(s: &str, width: usize, mut visit: impl FnMut(&str)) {
    for raw in s.split('\n') {
        let mut p = raw.trim();
        if p.is_empty() {
            visit("");
            continue;
        }
        while p.len() > width {
            let end = p
                .get(..width + 1)
                .and_then(|v| v.rfind(' '))
                .filter(|&v| v > 0)
                .unwrap_or(width);
            if !p.is_char_boundary(end) {
                visit(p);
                break;
            }
            visit(p[..end].trim_end());
            p = p[end..].trim_start();
        }
        if !p.is_empty() && p.len() <= width {
            visit(p);
        }
    }
}
struct Copy<'a> {
    parts: [&'a str; 3],
    width: usize,
}
impl Copy<'_> {
    fn visit(&self, mut f: impl FnMut(&str)) {
        let mut first = true;
        for &s in &self.parts {
            if s.is_empty() {
                continue;
            }
            if !first {
                f("");
            }
            first = false;
            lines(s, self.width, |v| f(v));
        }
    }
    fn display(&self, c: &mut Canvas<'_>, j: &mut Journal, x: usize, y: usize, rows: usize) {
        let mut count = 0usize;
        self.visit(|_| count += 1);
        j.shown_lines = count.saturating_sub(rows);
        j.offset = j.offset.min(j.shown_lines);
        let mut line = 0;
        self.visit(|s| {
            if line >= j.offset && line < j.offset + rows {
                c.label(x, y + (line - j.offset) * 15, 1, INK, format_args!("{s}"));
            }
            line += 1;
        });
        if count > rows {
            c.label(
                x,
                y + rows * 15 + 1,
                1,
                RED,
                format_args!(
                    "UP / DOWN   lines {}-{} of {count}",
                    j.offset + 1,
                    (j.offset + rows).min(count)
                ),
            );
        }
    }
}
fn file_icon(c: &mut Canvas<'_>, x: usize, y: usize, sealed: bool) {
    c.rect(x, y, 18, 15, BRASS);
    c.rect(x + 1, y - 3, 7, 4, BRASS);
    c.rect(x + 2, y + 2, 14, 11, PALE);
    c.rect(x + 3, y + 5, 11, 1, RULE);
    c.rect(x + 3, y + 8, 8, 1, RULE);
    if sealed {
        c.rect(x + 12, y + 8, 4, 4, RED);
    }
}
pub(super) fn draw(c: &mut Canvas<'_>, g: &Economy, j: &mut Journal) {
    c.rect(0, 0, W, H, NIGHT);
    c.rect(0, 0, W, 2, BRASS);
    if j.page == Page::Wire {
        super::wire::draw(c, g, j);
        return;
    }
    c.label(39, 12, 2, PAPER, format_args!("The Second Manifest"));
    file_icon(c, 11, 14, true);
    c.label(
        421,
        16,
        1,
        FOAM,
        format_args!("HARBOR ARCHIVE   /   FIELD COPY"),
    );
    let complete = g.manifest.solved as usize;
    for (i, case) in CASES.iter().enumerate() {
        let x = 12 + i * 206;
        let selected = i == j.case;
        let access = i <= complete;
        let filed = i < complete;
        c.rect(x, 43, 202, 35, if selected { TEAL } else { INK });
        if selected {
            c.rect(x, 43, 202, 2, BRASS);
        }
        let state = if filed {
            "FILED"
        } else if access {
            "OPEN"
        } else {
            "SEALED"
        };
        c.label(
            x + 9,
            50,
            1,
            if selected { BRASS } else { FOAM },
            format_args!("FILE 0{}  /  {state}", i + 1),
        );
        if access {
            c.label(x + 9, 65, 1, PAPER, format_args!("{}", case.title));
        } else {
            c.label(x + 9, 65, 1, MUTED, format_args!("Awaiting prior finding"));
        }
    }
    if matches!(j.page, Page::Decision | Page::Confirm | Page::Ending) {
        end_page(c, g, j);
        return;
    }
    let case = &CASES[j.case];
    let filed = j.case < complete;
    let tagged = if filed {
        Journal::proved(g, j.case)
    } else {
        j.pins[j.case]
    };
    c.rect(12, 88, 189, 245, INK);
    c.label(22, 100, 1, BRASS, format_args!("EVIDENCE INDEX"));
    for (n, lead) in case.leads.iter().enumerate() {
        let y = 119 + 47 * n;
        let selected = j.page == Page::Lead && j.lead == n;
        c.rect(15, y, 183, 44, if selected { TEAL } else { NIGHT });
        c.label(
            22,
            y + 7,
            1,
            if tagged & (1 << n) != 0 { BRASS } else { FOAM },
            format_args!(
                "0{}   {}",
                n + 1,
                if tagged & (1 << n) != 0 {
                    "TAGGED"
                } else if g.manifest.read[j.case] & (1 << n) != 0 {
                    "READ"
                } else {
                    "UNREAD"
                }
            ),
        );
        let mut row = 0;
        lines(lead.title, 27, |line| {
            if row < 2 {
                c.label(22, y + 22 + row * 10, 1, PAPER, format_args!("{line}"));
            }
            row += 1;
        });
    }
    c.label(
        22,
        317,
        1,
        FOAM,
        format_args!("{} / 2 tagged", tagged.count_ones()),
    );
    paper(c, 209, 88, 417, 245);
    let (kicker, title, parts) = if j.note == Note::Hint {
        ("FIELD NOTES", "A question to follow", [case.hint, "", ""])
    } else {
        match j.page {
            Page::Scene => ("CASE NOTES", case.title, [case.scene, case.question, ""]),
            Page::Lead => {
                let l = &case.leads[j.lead];
                (l.source, l.title, [l.body, "", ""])
            }
            Page::Finding => ("CORROBORATED FINDING", case.title, [case.reveal, "", ""]),
            _ => unreachable!(),
        }
    };
    c.label(225, 100, 1, RED, format_args!("{kicker}"));
    let size = if title.len() <= 32 { 2 } else { 1 };
    c.label(225, 119, size, INK, format_args!("{title}"));
    c.rect(225, 143, 385, 1, RULE);
    Copy { parts, width: 63 }.display(c, j, 225, 156, 10);
    c.rect(12, 344, 616, 40, INK);
    c.rect(22, 354, 4, 15, BRASS);
    let message = match j.note {
        Note::Wrong => {
            "These records do not corroborate the finding. Change a tag or press H for a hint."
        }
        Note::NeedTwo => "Inspect two leads and tag each with Space, then press Enter.",
        Note::PinsFull => {
            "Two leads are tagged. Open a tagged record and press Space to remove its tag."
        }
        Note::Hint => "Press H to put the field notes away.",
        Note::None if j.page == Page::Finding => {
            "Finding filed. Press N to continue, or 1-4 to inspect these records again."
        }
        Note::None if filed => {
            "Filed evidence stays readable. Enter shows the corroborated finding."
        }
        Note::None if j.page == Page::Lead => {
            "Space tags or untags this record. Enter checks the pair."
        }
        Note::None => "Open a numbered lead to inspect it.",
    };
    let mut n = 0;
    lines(message, 94, |w| {
        c.label(32, 354 + 12 * n, 1, PAPER, format_args!("{w}"));
        n += 1;
    });
    c.key(10, 398, "1-4", "Leads");
    c.key(80, 398, "SPACE", "Tag");
    c.key(150, 398, "ENTER", "Check");
    c.key(238, 398, "H", "Hint");
    c.key(290, 398, "N", "Continue");
    c.key(371, 398, "LEFT/RIGHT", "Files");
    c.key(505, 398, "ESC", "Back");
    c.key(576, 398, "J", "Exit");
}
fn end_page(c: &mut Canvas<'_>, g: &Economy, j: &mut Journal) {
    let signed = g.manifest.ending as usize > 0;
    let pick = if signed {
        g.manifest.ending as usize - 1
    } else {
        j.choice
    };
    let ending = &ENDINGS[pick];
    for (n, option) in ENDINGS.iter().enumerate() {
        let x = 12 + 312 * n;
        let active = n == pick;
        c.rect(x, 89, 304, 48, if active { TEAL } else { INK });
        if active {
            c.rect(x, 89, 304, 2, BRASS);
        }
        c.label(
            x + 9,
            98,
            1,
            FOAM,
            format_args!(
                "{}   {}",
                n + 1,
                if signed && active {
                    "FILED"
                } else {
                    "CONCLUSION"
                }
            ),
        );
        c.label(x + 9, 117, 1, PAPER, format_args!("{}", option.title));
    }
    paper(c, 12, 147, 613, 187);
    let (header, parts) = match j.page {
        Page::Confirm => (
            "SIGN YOUR DECISION",
            [
                ending.benefit,
                "This is the final entry. Enter signs it. Esc returns to the two options.",
                "",
            ],
        ),
        Page::Ending => ("ARCHIVED CONCLUSION", [ending.epilogue, ending.benefit, ""]),
        _ => (
            "CHOOSE THE FINAL ENTRY",
            [
                ending.benefit,
                "Press 1 or 2 to inspect the other option. Enter reviews the selected decision.",
                "",
            ],
        ),
    };
    c.label(29, 159, 1, RED, format_args!("{header}"));
    let size = if ending.title.len() < 46 { 2 } else { 1 };
    c.label(29, 179, size, INK, format_args!("{}", ending.title));
    c.rect(29, 203, 575, 1, RULE);
    Copy { parts, width: 94 }.display(c, j, 29, 216, 6);
    c.rect(12, 345, 616, 38, INK);
    c.label(
        24,
        361,
        1,
        FOAM,
        format_args!(
            "{}",
            if signed {
                "The archive is open. Read a file, or listen to the traffic on Circuit Eight."
            } else {
                "Both outcomes carry into future charters. Only one can be signed."
            }
        ),
    );
    if signed {
        c.key(12, 398, "1-3", "Read files");
        c.key(185, 398, "E", "Listen on Circuit Eight");
    } else if j.page == Page::Confirm {
        c.key(12, 398, "ENTER", "Sign");
    } else {
        c.key(12, 398, "1-2", "Choose");
        c.key(115, 398, "ENTER", "Review");
    }
    c.key(461, 398, "ESC", "Back");
    c.key(565, 398, "J", "Exit");
}
