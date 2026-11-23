#!/usr/bin/env python3
"""Native program-name acceptance with same-row SHA-256 binding.

Source-pinned #436 geometry at 1280x720: name begins at (cell 10, row 9+4*i),
occupies cells 10..27, and SHA-256 begins at cell 37 of that same row.
At 800x600 the plain name-only list starts at (cell 3, row 5+i).
Trailing blanks may be removed by the optical decoder; an extra word, a
different digest cell, or a changed layout fails closed.
"""
WIDE_ROW, WIDE_STRIDE, WIDE_COL = 9, 4, 10
WIDE_NAME_END, WIDE_DIGEST_COL = 28, 37
PLAIN_ROW, PLAIN_COL = 5, 3


def _label(row, col, end, name):
    # The native decoder removes trailing blanks; spaces *inside* a loader
    # name are still significant. A prefix plus " old" is not the product.
    # Trailing-space loader names are optically indistinguishable from padding;
    # their identity must be established by the actual loader metadata.
    return (len(row) >= col + len(name)
            and row[col:end].rstrip(" ") == name)


def wide_named_row(rows, slot, name, digest):
    at = WIDE_ROW + WIDE_STRIDE * slot
    if at >= len(rows) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        return False
    row = rows[at]
    expected = "SHA-256 " + digest[:16] + "..."
    return (_label(row, WIDE_COL, WIDE_NAME_END, name)
            and row[WIDE_DIGEST_COL:WIDE_DIGEST_COL + len(expected)] == expected)


def plain_named_row(rows, slot, name):
    at = PLAIN_ROW + slot
    return at < len(rows) and _label(rows[at], PLAIN_COL, None, name)


if __name__ == "__main__":
    import unittest
    D1, D2 = "0123456789abcdef" * 4, "fdecba9876543210" * 4

    def wide(names=("Tailorbird", "Surfbird"), hashes=(D1, D2)):
        rows = [" " * 90 for _ in range(30)]
        for i, (name, digest) in enumerate(zip(names, hashes)):
            line = list(" " * 90)
            line[10:10 + len(name)] = name
            line[28:33] = "1 KiB"
            line[37:64] = "SHA-256 " + digest[:16] + "..."
            rows[9 + 4*i] = "".join(line)
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

        def test_trimmed_native_rows(self):
            # The real optical decoder removes trailing spaces from each row.
            # A selected 800px row reads "  ?Tailorbird" and the next one
            # reads "   Surfbird", so the exact name may end the string.
            self.assertTrue(plain_named_row([""] * 5 + ["  ?Tailorbird", "   Surfbird"], 0, "Tailorbird"))
            self.assertTrue(plain_named_row([""] * 5 + ["  ?Tailorbird", "   Surfbird"], 1, "Surfbird"))
            self.assertFalse(plain_named_row([""] * 5 + ["  ?TailorbirdX"], 0, "Tailorbird"))

        def test_wrong_or_invalid_digest(self):
            self.assertFalse(wide_named_row(wide(), 0, "Tailorbird", D2))
            self.assertFalse(wide_named_row(wide(), 0, "Tailorbird", "0" * 63))

        def test_space_suffix_is_not_the_name(self):
            for bad in ("Tailorbird old", "Tailorbird extra", "Tailorbird-old", "X Tailorbird"):
                self.assertFalse(wide_named_row(wide((bad, "Surfbird")), 0, "Tailorbird", D1))
                self.assertFalse(plain_named_row(plain((bad, "Surfbird")), 0, "Tailorbird"))

        def test_digest_must_be_in_source_column_37(self):
            rows = wide()
            real = "SHA-256 " + D1[:16] + "..."
            other = "SHA-256 " + D2[:16] + "..."
            self.assertEqual(rows[9][37:64], real)
            self.assertTrue(wide_named_row(rows, 0, "Tailorbird", D1))
            displaced = rows.copy()
            line = list(displaced[9])
            line[37:64] = " " * len(real)
            line[70:97] = real
            displaced[9] = "".join(line)
            self.assertFalse(wide_named_row(displaced, 0, "Tailorbird", D1))
            echo = displaced.copy()
            line = list(echo[9])
            line[37:64] = other
            echo[9] = "".join(line)
            self.assertFalse(wide_named_row(echo, 0, "Tailorbird", D1))
            self.assertTrue(wide_named_row(wide(), 1, "Surfbird", D2))

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
