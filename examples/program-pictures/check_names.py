#!/usr/bin/env python3
"""Native program-name acceptance with same-row SHA-256 binding.

Source-pinned #436 geometry at 1280x720: name begins at (cell 10, row 9+4*i),
and the first 16 digest digits occupy that same row. At 800x600 the plain
name-only list starts at (cell 3, row 5+i). A changed layout fails closed.
"""
WIDE_ROW, WIDE_STRIDE, WIDE_COL = 9, 4, 10
PLAIN_ROW, PLAIN_COL = 5, 3


def _label(row, col, name):
    end = col + len(name)
    return len(row) > end and row[col:end] == name and row[end] == " "


def wide_named_row(rows, slot, name, digest):
    at = WIDE_ROW + WIDE_STRIDE * slot
    if at >= len(rows) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        return False
    row = rows[at]
    return (_label(row, WIDE_COL, name)
            and ("SHA-256 " + digest[:16] + "...") in row[WIDE_COL + len(name) + 1:])


def plain_named_row(rows, slot, name):
    at = PLAIN_ROW + slot
    return at < len(rows) and _label(rows[at], PLAIN_COL, name)


if __name__ == "__main__":
    import unittest
    D1, D2 = "0123456789abcdef" * 4, "fdecba9876543210" * 4

    def wide(names=("Tailorbird", "Surfbird"), hashes=(D1, D2)):
        rows = [" " * 90 for _ in range(30)]
        for i, (name, digest) in enumerate(zip(names, hashes)):
            rows[9 + 4*i] = " " * 10 + name + " " * (30-len(name)) + "SHA-256 " + digest[:16] + "..."
        return rows

    def plain(names=("Tailorbird", "Surfbird")):
        rows = [" " * 80 for _ in range(30)]
        for i, name in enumerate(names):
            rows[5+i] = " " * 3 + name + " " * 10
        return rows

    class Controls(unittest.TestCase):
        def test_positive(self):
            self.assertTrue(wide_named_row(wide(), 0, "Tailorbird", D1))
            self.assertTrue(wide_named_row(wide(), 1, "Surfbird", D2))
            self.assertTrue(plain_named_row(plain(), 0, "Tailorbird"))
            self.assertTrue(plain_named_row(plain(), 1, "Surfbird"))

        def test_prefix_suffix_lowercase(self):
            for bad in ("TailorbirdX", "XTailorbird", "Tailorbird.elf", "tailorbird"):
                self.assertFalse(wide_named_row(wide((bad, "Surfbird")), 0, "Tailorbird", D1))
                self.assertFalse(plain_named_row(plain((bad, "Surfbird")), 0, "Tailorbird"))

        def test_wrong_or_invalid_digest(self):
            self.assertFalse(wide_named_row(wide(), 0, "Tailorbird", D2))
            self.assertFalse(wide_named_row(wide(), 0, "Tailorbird", "0" * 63))

        def test_footer_only_and_split_digest(self):
            rows = wide()
            rows[9] = " " * 90
            rows[25] = " " * 10 + "Tailorbird SHA-256 " + D1[:16] + "..."
            self.assertFalse(wide_named_row(rows, 0, "Tailorbird", D1))
            rows = wide()
            rows[9] = " " * 10 + "Tailorbird "
            rows[25] = "SHA-256 " + D1[:16] + "..."
            self.assertFalse(wide_named_row(rows, 0, "Tailorbird", D1))
            rows = plain()
            rows[5] = " " * 80
            rows[28] = " " * 3 + "Tailorbird "
            self.assertFalse(plain_named_row(rows, 0, "Tailorbird"))

        def test_control_names_not_promoted(self):
            names = ("TAILORBIRD", "tailorbird.elf", "surfbirds")
            hashes = (D1, D2, D1)
            rows = wide(names, hashes)
            for i, (name, digest) in enumerate(zip(names, hashes)):
                self.assertTrue(wide_named_row(rows, i, name, digest))
                self.assertFalse(wide_named_row(rows, i, "Tailorbird", digest))
                self.assertFalse(wide_named_row(rows, i, "Surfbird", digest))

    unittest.main()
