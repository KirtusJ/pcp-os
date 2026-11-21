"""Read the four English lines drawn by the OVMF text console in the 64 MiB refusal guest."""
import hashlib

def line_shapes(frame):
    """Hash ink shapes independently of screen position, background and foreground colour."""
    magic, size, depth, pixels = frame.split(b'\n', 3)
    width, height = map(int, size.split())
    if magic != b'P6' or depth != b'255' or len(pixels) != width * height * 3:
        raise ValueError('incomplete firmware framebuffer')
    background = pixels[:3]
    blank = background * width
    shapes, band = [], []
    for y in range(height + 1):
        row = pixels[y*len(blank):(y+1)*len(blank)] if y < height else blank
        if row != blank:
            band.append(bytes(row[i:i+3] != background for i in range(0, len(row), 3)))
        elif band:
            left = min(row.index(1) for row in band)
            right = max(row.rindex(1) for row in band) + 1
            mask = b'\n'.join(row[left:right] for row in band)
            shapes.append(hashlib.sha256(mask).hexdigest())
            band = []
    return shapes

# OVMF narrow-font ink in the original, visually read 1280x800 screens.
# Keeping a hash of each complete line detects a missing word or clipped glyph.
COMMON = [
    'cb8af903a55c5fc4d09f4cfd614fc758a9e924da0eafc69b88d757fc9995b7e2',  # PCP OS could not start.
    'fa145265de04de85c9ac55c0b390a8137775a8ca026e31177881c285340aa1ed',  # Firmware could not reserve...
    '933d1bd8bec38980c959a4e056ca18dcb0eb871dbee0e279a9aa6818a49615a6',  # You can switch off...
]
ACTIONS = {
    'Press F10 to request power off.': '0f9481659cb77f98f99c253ac291fc18d76d070074f5644e7043bd71a182425b',
    'Switch off? Enter switches off. Esc cancels.': '40d31c820cc88030b60043b68089c72b01de9a3f0a78fe1b03cc9c47341285f9',
}

def matches(frame, action):
    return line_shapes(frame)[:4] == [*COMMON, ACTIONS[action]]
