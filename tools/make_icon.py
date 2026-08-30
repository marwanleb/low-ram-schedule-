"""Generate the app icon: a pixel wave in the app's own palette.

Drawn on a 32x32 grid and scaled with nearest-neighbour, so it stays pixel art
at every size rather than turning into a blurry gradient. Re-run to regenerate:

    python tools/make_icon.py
"""
from PIL import Image
import os

S = 32
GROUND = (12, 14, 18, 255)      # --page
DEEP = (18, 36, 44, 255)
CYAN = (127, 208, 236, 255)     # --cyan
CYAN_D = (63, 138, 168, 255)
FOAM = (223, 240, 250, 255)
GOLD = (245, 185, 66, 255)      # --gold
SAND = (255, 207, 149, 255)     # --sand

img = Image.new("RGBA", (S, S), GROUND)
px = img.load()

def rect(x0, y0, x1, y1, c):
    for y in range(max(0, y0), min(S, y1 + 1)):
        for x in range(max(0, x0), min(S, x1 + 1)):
            px[x, y] = c

# Notched corners, matching the app's clip-path.
for x, y in [(0, 0), (1, 0), (0, 1), (S-1, S-1), (S-2, S-1), (S-1, S-2)]:
    px[x, y] = (0, 0, 0, 0)

# A low sun, top right. Kept small so the wave is the subject.
rect(23, 5, 26, 8, GOLD)
px[22, 6] = SAND; px[22, 7] = SAND
px[27, 6] = SAND; px[27, 7] = SAND

# One wave, one full period across the icon. A short period reads as mountains
# at 32px, which is what the first attempt did.
import math
def crest(mid, amp, phase):
    return [round(mid + amp * math.sin((x / S) * 2 * math.pi + phase)) for x in range(S)]

back = crest(15, 3, 0.6)
front = crest(21, 3.5, 2.4)

# Back swell: a flat mass so the silhouette is a wave, not a line.
for x in range(S):
    rect(x, back[x], x, S - 1, CYAN_D)
    px[x, back[x]] = CYAN

# Front swell over it, brighter, with foam along the crest.
for x in range(S):
    y = front[x]
    rect(x, y, x, S - 1, CYAN)
    px[x, y] = FOAM
    if y + 1 < S:
        px[x, y + 1] = FOAM if x % 5 == 0 else CYAN
    # Trough shading, so the front mass has some depth.
    rect(x, min(S - 1, y + 4), x, S - 1, CYAN_D)

out = os.path.join(os.path.dirname(__file__), "..", "app", "src-tauri", "icons")
os.makedirs(out, exist_ok=True)

sizes = {
    "32x32.png": 32, "128x128.png": 128, "128x128@2x.png": 256,
    "icon.png": 512, "Square30x30Logo.png": 30, "Square44x44Logo.png": 44,
    "Square71x71Logo.png": 71, "Square89x89Logo.png": 89,
    "Square107x107Logo.png": 107, "Square142x142Logo.png": 142,
    "Square150x150Logo.png": 150, "Square284x284Logo.png": 284,
    "Square310x310Logo.png": 310, "StoreLogo.png": 50,
}
for name, size in sizes.items():
    img.resize((size, size), Image.NEAREST).save(os.path.join(out, name))

# Windows .ico wants several sizes in one file.
img.resize((256, 256), Image.NEAREST).save(
    os.path.join(out, "icon.ico"),
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
print("wrote", len(sizes) + 1, "icon files to", os.path.normpath(out))
