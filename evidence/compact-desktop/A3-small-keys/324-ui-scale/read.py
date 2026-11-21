"""Read cells from a PCP kernel frame using the font selected at startup."""
import ast
from pathlib import Path
import numpy as np

def font(source, scale):
    directory = Path(source) / "PCP_OS_Rust/crates/pcpos-kernel/src"
    if scale == 2:
        alpha = np.frombuffer((directory / "font-2x.bin").read_bytes(), dtype=np.uint8)
    else:
        source_text = (directory / "font.rs").read_text()
        alpha = np.asarray(ast.literal_eval("[" + source_text.split(" = [", 1)[1].split("];", 1)[0] + "]"), dtype=np.uint8)
    masks = {}
    for index, glyph in enumerate(alpha.reshape(95, -1)):
        for threshold in range(4):
            mask = np.packbits(glyph > threshold).tobytes()
            if mask in masks:
                assert masks[mask] == chr(32 + index)
            masks[mask] = chr(32 + index)
    return masks

def rows(pixels, masks, scale):
    height, width = pixels.shape[:2]
    cell_w, cell_h = 10*scale, 20*scale
    height -= height%cell_h
    width -= width%cell_w
    cells = pixels[:height, :width].reshape(height//cell_h, cell_h, width//cell_w, cell_w, 3).transpose(0,2,1,3,4)
    ink = (cells != cells[:,:,0:1,0:1]).any(4)
    bits = np.packbits(ink.reshape(cells.shape[0],cells.shape[1],-1),axis=2)
    return ["".join(masks.get(cell.tobytes(),"?") for cell in row).rstrip() for row in bits]
