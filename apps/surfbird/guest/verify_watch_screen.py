"""Compare the documented title and address with actual QEMU pixel regions."""

from framebuffer import check_line, load_glyphs, raster

BG = 0x10252C


def check_screens(work, title, canonical):
    glyphs = load_glyphs()
    front, address, back = [
        raster(work / name) for name in ("title.ppm", "address.ppm", "back.ppm")
    ]
    if front != back or front == address:
        raise ValueError("back failed to restore the full screen")

    y = 119
    while title:
        stop = len(title) if len(title) <= 58 else title.rfind(" ", 0, 59)
        if stop <= 0:
            stop = 58
        check_line(front, glyphs, 24, y, title[:stop], 0xF4F6F7, BG)
        title = title[stop:].lstrip()
        y += 28
    check_line(address, glyphs, 24, 122, canonical, 0xB9E4E2, BG)
