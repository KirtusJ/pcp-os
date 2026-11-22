//! The small pictures of the programs this system knows by sight, for the list of programs.
//!
//! A picture is shown for a program only when the loader's line, taken whole, names that program's
//! SHA-256 as the picture's: `taylor_image=<digest>` or `surfbird_image=<digest>`. A program's name
//! decides nothing: a file that calls itself tailorbird and is other bytes has no picture. Where the
//! loader's line was cut, nothing it says is relied on and no picture is shown.
//!
//! Each picture is sixteen by sixteen squares, drawn by filling squares, as Tailorbird draws Taylor
//! on its own screen. The kernel decodes no image. The colours are the brand's: Taylor's are those
//! of `tailorbird.svg` and of Tailorbird's palette (`HARD_HAT`, `LEAF`, `CREAM`); Surfbird's are
//! taken from `surfbird.svg`.

use crate::dev;

/// Squares across and down.
pub const SIZE: u32 = 16;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Picture {
    Taylor,
    Surfbird,
}

// Taylor's head, facing left: yellow hard hat (H), cream face (W), eye (K), bill (S), leaf-green back (G).
const TAYLOR: [&[u8; 16]; 16] = [
    b"................",
    b".....HHHHHH.....",
    b"....HHHHHHHH....",
    b"...HHHHHHHHHH...",
    b"...HHHHHHHHHH...",
    b".HHHHHHHHHHHHH..",
    b"....WWWWWWWW....",
    b"...WWKWWWWWWW...",
    b"SSSWWWWWWWWWW...",
    b".SSSWWWWWWWWW...",
    b"....WWWWWWWW....",
    b".....WWWWWW.....",
    b"....GGGGGGGG....",
    b"...GGGGGGGGGG...",
    b"..GGGGGGGGGGGG..",
    b"..GGGGGGGGGGGG..",
];

// Surfbird's parrot on its board, facing right: crest and wing (C), silver body (L), eye (K), beak,
// tail and legs (D), board (T) with its light stripe (W) and bright nose (E).
const SURFBIRD: [&[u8; 16]; 16] = [
    b"................",
    b".......CCCC.....",
    b"......CCLLLL....",
    b".....CCLLKLLD...",
    b".....CLLLLLDDD..",
    b"....CCLLLLL.D...",
    b"...CCCLLLLL.....",
    b"..CCCCLLLLL.....",
    b".DCCCCLLLL......",
    b"..DD..LLLL......",
    b"......D..D......",
    b"E....DD.DD....T.",
    b"EETTTTTTTTTTTTT.",
    b".EWWWWWWWWWWWW..",
    b"..TTTTTTTTTTT...",
    b"................",
];

fn colour(picture: Picture, square: u8) -> Option<u32> {
    Some(match (picture, square) {
        (Picture::Taylor, b'H') => 0xF2C230,
        (Picture::Taylor, b'W') => 0xF1F4E4,
        (Picture::Taylor, b'K') => 0x0C2033,
        (Picture::Taylor, b'S') => 0x9FB4C8,
        (Picture::Taylor, b'G') => 0x7FAE5C,
        (Picture::Surfbird, b'C') => 0x36B9BF,
        (Picture::Surfbird, b'L') => 0xD3DEE3,
        (Picture::Surfbird, b'K') => 0x102630,
        (Picture::Surfbird, b'D') => 0x869DA5,
        (Picture::Surfbird, b'T') => 0x087F91,
        (Picture::Surfbird, b'E') => 0x53E5E4,
        (Picture::Surfbird, b'W') => 0xD0E3E8,
        _ => return None,
    })
}

/// The picture the loader's line gives to the program with this digest, if any. `line` is the
/// loader's line only where it was taken whole; `None` (a cut line) gives no picture. The first
/// option of each name is the one read, as for every other option on the line.
pub fn of(line: Option<&str>, digest: &str) -> Option<Picture> {
    let line = line?;
    let named = |key: &str| line.split_whitespace().find_map(|w| w.strip_prefix(key).and_then(|r| r.strip_prefix('='))) == Some(digest);
    if named("taylor_image") {
        Some(Picture::Taylor)
    } else if named("surfbird_image") {
        Some(Picture::Surfbird)
    } else {
        None
    }
}

/// Draw the picture with its top left corner at (x, y), each square `cell` pixels across. Squares
/// that are not part of the figure are left as they were.
pub fn draw(picture: Picture, x: u32, y: u32, cell: u32) {
    let map = match picture {
        Picture::Taylor => &TAYLOR,
        Picture::Surfbird => &SURFBIRD,
    };
    for (row, line) in map.iter().enumerate() {
        for (column, square) in line.iter().enumerate() {
            if let Some(ink) = colour(picture, *square) {
                dev::display_fill(x + column as u32 * cell, y + row as u32 * cell, cell, cell, ink);
            }
        }
    }
}
