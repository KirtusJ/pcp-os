#!/usr/bin/env python3
"""Independent native PCP OS two-program visible-name collision witness.

Run after building the pinned #441/#436 kernel:
    python3 -B examples/program-pictures/audit_collisions.py <kernel.elf32> <new-output-dir>

This *diagnoses* an unresolved presentation decision. A successful run means that
real Q35 guest frames reproduce the conditional visual ambiguity, not that the
ambiguity is acceptable or that any installed image has it. Keep all original
frames, decoded rows, serial logs and input hashes. No model or service is used.
"""
import hashlib
import json
import sys
from pathlib import Path
from check import boot, program
from check_names import wide_named_row, plain_named_row


def sha(blob):
    return hashlib.sha256(blob).hexdigest()


def make_files(first, second):
    # Two real initrd modules, same length but distinct bytes and digests.
    return [(first, program(("native-pair:first:" + first).encode())),
            (second, program(("native-pair:second:" + second).encode()))]


def witness(kernel, output, label, files, size):
    rows, frame = boot(kernel, output, label, size, files, "")
    # A second admission with a different loader name must not be inferred
    # from label pixels alone: retain names, source bytes, sha256 and the
    # 1280px's distinct row-bound digest as a separate check.
    digests = [sha(contents) for _, contents in files]
    names = [name for name, _ in files]
    shown = files[0][0].title()
    if size == "1280x720":
        accepted = [wide_named_row(rows, i, shown, digests[i]) for i in range(2)]
        labels = [rows[9 + 4*i][10:28].rstrip(" ") for i in range(2)]
        # Unlike at 800px, a separate 16-digit digest lets a reviewer
        # distinguish the two cards even though the headings collide.
        digest_distinguished = digests[0] != digests[1] and all(accepted)
    else:
        accepted = [plain_named_row(rows, i, shown) for i in range(2)]
        labels = [rows[5 + i][3:].rstrip(" ") for i in range(2)]
        digest_distinguished = False
    return {
        "tag": label, "screen": size,
        "loader_names": names, "module_sha256": digests,
        "labels": labels, "same_rendered_label": labels[0] == labels[1] == shown,
        "row_predicates": accepted, "digest_bound_at_wide_screen": digest_distinguished,
        "frame_sha256": sha(frame), "frame_bytes": len(frame),
        "rows": [rows[9], rows[13]] if size == "1280x720" else [rows[5], rows[6]],
    }


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    kernel = Path(sys.argv[1]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    observations = []
    for raw, product in (("tailorbird", "Tailorbird"), ("surfbird", "Surfbird")):
        files = make_files(raw, product)
        for screen, tag in (("800x600", "plain"), ("1280x720", "wide")):
            observations.append(witness(kernel, output, raw + "-" + tag, files, screen))

    # Adverse case: uppercase is NOT renamed. The same checker must not
    # invent a collision merely because two loader names share a stem.
    control_files = make_files("tailorbird", "TAILORBIRD")
    control_rows, control_frame = boot(kernel, output, "control-uppercase", "800x600", control_files, "")
    control = {
        "screen": "800x600", "raw": [x for x, _ in control_files],
        "digests": [sha(b) for _, b in control_files],
        "labels": [control_rows[5+i][3:].rstrip(" ") for i in range(2)],
        "frame_sha256": sha(control_frame),
        "name_checks": [plain_named_row(control_rows, 0, "Tailorbird"),
                        plain_named_row(control_rows, 1, "TAILORBIRD")],
        "false_collided": control_rows[5][3:].rstrip(" ") == control_rows[6][3:].rstrip(" "),
    }
    record = {
        "scope": "model-free native Q35/TCG list frames; not installed F6/8B or a real task effect",
        "source": "#436 source + D2 #441 checker, no product changes",
        "observations": observations, "uppercase_control": control,
        "kernel_sha256": sha(kernel.read_bytes()),
    }
    (output / "collision-witness.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    good = (all(x["same_rendered_label"] and all(x["row_predicates"]) for x in observations)
            and all(x["digest_bound_at_wide_screen"] for x in observations if x["screen"] == "1280x720")
            and all(control["name_checks"]) and not control["false_collided"]
            and control["labels"] == ["Tailorbird", "TAILORBIRD"])
    for item in observations:
        print("NATIVE_PAIR", item["tag"], item["screen"], "raw=", item["loader_names"],
              "labels=", item["labels"], "sha_prefixes=",
              [d[:16] for d in item["module_sha256"]], "frame=", item["frame_sha256"])
    print("NATIVE_CONTROL", control["labels"], control["name_checks"], control["frame_sha256"])
    if not good:
        sys.exit("FAIL: native two-module collision/control fixture did not match source-pinned expectation; inspect retained frames")
    print("WITNESS: genuine native 800px labels collide for distinct loader filenames;")
    print("         wide-screen row SHA distinguishes the modules. PRODUCT DISAMBIGUATION UNRESOLVED.")
    print("         Success here is evidence of the ambiguity, NOT display/release acceptance.")


if __name__ == "__main__":
    main()
