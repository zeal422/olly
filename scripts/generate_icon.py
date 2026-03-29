#!/usr/bin/env python3
"""
Generate a clean Windows app icon (.ico) from olly-logo.PNG.

Usage:
    python scripts/generate_icon.py
"""

from collections import deque
from pathlib import Path
import colorsys

from PIL import Image


ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "olly-logo.PNG"
OUT_DIR = ROOT / "assets" / "icon"
PNG_OUT = OUT_DIR / "olly-icon.png"
ICO_OUT = OUT_DIR / "olly.ico"


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    img = Image.open(SRC).convert("RGBA")
    # Crop around the llama/cloud mark (exclude the wordmark for taskbar clarity).
    crop = img.crop((150, 20, 710, 360))
    w, h = crop.size
    px = crop.load()

    # Keep saturated logo colors and drop low-saturation paper background.
    mask = [[False] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            r, g, b, _ = px[x, y]
            _, s, v = colorsys.rgb_to_hsv(r / 255.0, g / 255.0, b / 255.0)
            if s >= 0.18 and v >= 0.20:
                mask[y][x] = True

    # Keep only large connected components to remove tiny text remnants/noise.
    visited = [[False] * w for _ in range(h)]
    components = []
    for y in range(h):
        for x in range(w):
            if not mask[y][x] or visited[y][x]:
                continue

            q = deque([(x, y)])
            visited[y][x] = True
            pixels = []
            min_y = y

            while q:
                cx, cy = q.popleft()
                pixels.append((cx, cy))
                if cy < min_y:
                    min_y = cy

                for nx, ny in (
                    (cx + 1, cy),
                    (cx - 1, cy),
                    (cx, cy + 1),
                    (cx, cy - 1),
                    (cx + 1, cy + 1),
                    (cx - 1, cy - 1),
                    (cx + 1, cy - 1),
                    (cx - 1, cy + 1),
                ):
                    if (
                        0 <= nx < w
                        and 0 <= ny < h
                        and mask[ny][nx]
                        and not visited[ny][nx]
                    ):
                        visited[ny][nx] = True
                        q.append((nx, ny))

            components.append((len(pixels), min_y, pixels))

    keep = set()
    y_cutoff = int(h * 0.78)
    for area, min_y, pixels in components:
        if area >= 450 and min_y < y_cutoff:
            keep.update(pixels)

    for y in range(h):
        for x in range(w):
            r, g, b, _ = px[x, y]
            px[x, y] = (r, g, b, 255 if (x, y) in keep else 0)

    alpha = crop.split()[3]
    bbox = alpha.getbbox()
    mark = crop.crop(bbox) if bbox else crop

    size = 1024
    padding = 115
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ratio = min(
        (size - 2 * padding) / mark.width,
        (size - 2 * padding) / mark.height,
    )
    mark = mark.resize(
        (max(1, int(mark.width * ratio)), max(1, int(mark.height * ratio))),
        Image.Resampling.LANCZOS,
    )
    canvas.paste(
        mark,
        ((size - mark.width) // 2, (size - mark.height) // 2),
        mark,
    )

    canvas.save(PNG_OUT)
    canvas.save(
        ICO_OUT,
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print(f"Generated {PNG_OUT}")
    print(f"Generated {ICO_OUT}")


if __name__ == "__main__":
    main()
