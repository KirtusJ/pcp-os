#!/usr/bin/env python3
"""Bounded arithmetic audit of PCP OS native launcher rows (not a QEMU raster).

Exact source anchors are required before the model is considered applicable.
Any layout code change invalidates this source-only witness and requires
fresh frame captures. The test describes a visual overlap hazard, not a
verified guest screenshot, not a security error and not a fixed layout.
"""
import argparse
import hashlib
import json
from pathlib import Path


def menu_positions(programs, ethernet_grants=0):
    """Directly follow session.rs draw_menu's explicit row arithmetic."""
    if not (0 <= ethernet_grants <= programs):
        raise ValueError("invalid granted program count")
    row = 9  # PROGRAMS heading
    row += 1  # session.rs displays the first selectable program at row 10
    program_rows = []
    for i in range(programs):
        program_rows.append({"index": i, "selected_row": row,
                             "description_row": row + 1})
        row += 2
        if i < ethernet_grants:
            row += 1
    row += 1
    files_heading = row
    row += 1
    file_rows = list(range(row, row + min(8, programs)))
    row += len(file_rows)
    if programs == 0:
        row += 1
    row += 1
    machine_heading = row
    row += 1
    memory_row = row
    row += 1
    display_row = row
    store_row = row + 1
    message_row = row + 3  # display increments once, then session.rs increments twice
    return {
        "program_rows": program_rows,
        "files_heading": files_heading,
        "file_rows": file_rows,
        "machine_heading": machine_heading,
        "memory_row": memory_row,
        "display_row": display_row,
        "store_row": store_row,
        "message_first_row": message_row,
    }


def inspect(session, ui, font):
    required_session = [
        "let mut row = 9;",
        "for (i, p) in programs.iter().enumerate() {",
        "at(4, row + 1,",
        "row += 2;",
        "if ethernet_grant(start.boot.exact_command_line(), &p.digest) {",
        'at(4, row, "Ethernet grant:',
        "row += 1;",
        'at(2, row, "FILES FROM THE LOADER",',
        "for f in files.iter().take(8)",
        'at(2, row, "MACHINE",',
        'at(4, row, &format!("memory for programs:',
        'at(4, row, &format!("display:',
        'at(4, row, &format!("store:',
        'ui::fill(0, (rows - 2) * ui::CELL_H',
    ]
    required_ui = [
        "pub const CELL_W: u32 = CELL_WIDTH as u32;",
        "pub const CELL_H: u32 = CELL_HEIGHT as u32;",
    ]
    required_font = [
        "pub const CELL_WIDTH: usize = 10;",
        "pub const CELL_HEIGHT: usize = 20;",
    ]
    absent = [name for name in required_session if name not in session]
    absent += [name for name in required_ui if name not in ui]
    absent += [name for name in required_font if name not in font]
    if absent:
        raise RuntimeError("layout source changed, model invalid: " + repr(absent))
    evaluated = {}
    for width, height in [(800, 600), (1024, 768)]:
        rows = height // 20
        footer_first = rows - 2
        entries = []
        for programs in range(1, 12):
            geom = menu_positions(programs, ethernet_grants=1)
            occluded = []
            for label in ("files_heading", "machine_heading", "memory_row",
                          "display_row", "store_row", "message_first_row"):
                if geom[label] >= footer_first:
                    occluded.append(label)
            if geom["file_rows"] and any(v >= footer_first for v in geom["file_rows"]):
                occluded.append("file_rows")
            if any(v["selected_row"] >= footer_first for v in geom["program_rows"]):
                occluded.append("program_focus")
            entries.append({
                "programs": programs, "files": programs,
                "one_program_has_ethernet_grant": True,
                "overlap_or_offscreen": occluded,
                "store_row": geom["store_row"],
                "message_first_row": geom["message_first_row"],
                "last_selected_row": geom["program_rows"][-1]["selected_row"],
            })
        first_store = next((e for e in entries if e["store_row"] >= footer_first), None)
        first_focus = next((e for e in entries if "program_focus" in e["overlap_or_offscreen"]), None)
        evaluated[f"{width}x{height}"] = {
            "rows": rows,
            "footer_first_row": footer_first,
            "first_store_overlap_case": first_store,
            "first_obscured_focus_case": first_focus,
            "all_examined_cases": entries,
        }
    assert evaluated["800x600"]["first_store_overlap_case"]["programs"] == 4
    assert evaluated["1024x768"]["first_store_overlap_case"]["programs"] == 7
    assert menu_positions(1, ethernet_grants=1)["message_first_row"] == 22
    assert menu_positions(1, ethernet_grants=1)["store_row"] == 20
    assert menu_positions(4, ethernet_grants=1)["store_row"] == 29
    assert menu_positions(7, ethernet_grants=1)["display_row"] == 37
    return evaluated


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--session", type=Path, required=True)
    p.add_argument("--ui", type=Path, required=True)
    p.add_argument("--font", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    source = args.session.read_text(encoding="utf-8")
    ui = args.ui.read_text(encoding="utf-8")
    font = args.font.read_text(encoding="utf-8")
    result = {
        "source_scope": "pinned native launcher row arithmetic only",
        "not_native_frame_capture": True,
        "not_display_rendering_confirmation": True,
        "not_network_gui": True,
        "model": "direct translation of fixed draw_menu row positions",
        "session_file_sha256": hashlib.sha256(source.encode("utf-8")).hexdigest(),
        "ui_file_sha256": hashlib.sha256(ui.encode("utf-8")).hexdigest(),
        "font_file_sha256": hashlib.sha256(font.encode("utf-8")).hexdigest(),
    }
    try:
        result["cases"] = inspect(source, ui, font)
        result["status"] = "SOURCE_MODEL_OVERLAP_REPRODUCED"
    except Exception as exc:
        result["status"] = "MODEL_NOT_APPLICABLE_OR_FAILED"
        result["error"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2, sort_keys=True)+"\n",
                               encoding="utf-8")
    print(json.dumps({key: {"store_overlap_at":value["first_store_overlap_case"]["programs"],
                             "focus_hidden_at": value["first_obscured_focus_case"]["programs"]
                             if value["first_obscured_focus_case"] else None}
                      for key, value in result["cases"].items()}, sort_keys=True))


if __name__ == "__main__":
    main()
