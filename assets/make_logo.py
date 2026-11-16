"""Cut the rounded logo tile out of the painted-checkerboard source image.

Usage: python -I assets/make_logo.py SRC DST [UI_DST [UI_SIZE]]
  e.g. python -I assets/make_logo.py assets/logo-source.jpg assets/logo.png app/src/assets/logo-128.png 128
"""
import sys
from collections import deque

from PIL import Image, ImageDraw

OUT_SIZE = 1024
RADIUS_FRACTION = 0.18  # corner radius as a share of the tile side
CHECKER_TOLERANCE = 25  # channel distance from the checker colours
INSET = 3  # source pixels trimmed off each edge to avoid checker fringe
SCALE = 4  # supersampling factor for the anti-aliased mask


def is_background(px):
    """True for the light grey / white checkerboard (and its faint shadow)."""
    r, g, b = px
    return min(r, g, b) > 125 and max(r, g, b) - min(r, g, b) < CHECKER_TOLERANCE


def find_tile_bbox(img):
    """Bounding box of the connected non-checker region containing the centre."""
    w, h = img.size
    px = img.load()
    start = (w // 2, h // 2)
    seen = bytearray(w * h)
    queue = deque([start])
    seen[start[1] * w + start[0]] = 1
    x0, y0, x1, y1 = w, h, 0, 0
    while queue:
        x, y = queue.popleft()
        x0, y0, x1, y1 = min(x0, x), min(y0, y), max(x1, x), max(y1, y)
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if 0 <= nx < w and 0 <= ny < h and not seen[ny * w + nx]:
                seen[ny * w + nx] = 1
                if not is_background(px[nx, ny]):
                    queue.append((nx, ny))
    return x0, y0, x1 + 1, y1 + 1


def main(src, dst, ui_dst=None, ui_size=128):
    img = Image.open(src).convert("RGB")
    x0, y0, x1, y1 = find_tile_bbox(img)
    # The drop shadow below the tile also passes the colour test, so the
    # height is unreliable; the tile is square, so trust the width and the top.
    inset = INSET
    if abs((y1 - y0) / (x1 - x0) - 1) > 0.10:
        sys.exit(f"detected tile is not roughly square: {x1 - x0}x{y1 - y0}")
    side = (x1 - x0) - 2 * inset
    left, top = x0 + inset, y0 + inset
    tile = img.crop((left, top, left + side, top + side))
    print("tile bbox", (x0, y0, x1, y1), "side", side)

    big = OUT_SIZE * SCALE
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, big - 1, big - 1), radius=int(big * RADIUS_FRACTION), fill=255
    )
    mask = mask.resize((OUT_SIZE, OUT_SIZE), Image.LANCZOS)

    out = tile.resize((OUT_SIZE, OUT_SIZE), Image.LANCZOS).convert("RGBA")
    out.putalpha(mask)
    out.save(dst)
    if ui_dst:
        out.resize((ui_size, ui_size), Image.LANCZOS).save(ui_dst, optimize=True)


if __name__ == "__main__":
    args = sys.argv[1:]
    main(args[0], args[1], args[2] if len(args) > 2 else None, int(args[3]) if len(args) > 3 else 128)
