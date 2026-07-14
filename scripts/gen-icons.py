# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""Regenerate all GUI icon assets from the logo renders in artifacts/.

The renders are AI images with NO alpha channel, so every asset is derived
by scripted masking — never hand-edit the outputs.

Sources:
  artifacts/logo-icon-light.png  light squircle render on white   -> app icon (light)
  artifacts/logo-icon-dark.png  dark squircle render on near-black -> Dock dark variant
  artifacts/logo-mono-glyph.png  mono glyph on baked checkerboard -> menu-bar template

Outputs (crates/openwith-gui/src-tauri/icons/):
  icon-dark.png      1024 dark master (runtime Dock swap, set_dock_icon_dark)
  tray-template.png  64x44 black+alpha template image (glyph ~34px tall)
  icon.icns          hand-built: >=128px slots keep the full render; <=64px
                     slots use a legibility variant (header dots inpainted
                     away, no drop shadow, glyph at ~92% of canvas, unsharp)

Plus a 1024 light master at <scratch>/app-icon-1024.png for the PNG set:
run  (cd crates/openwith-gui && npm run tauri icon <that file>)
BEFORE this script — `tauri icon` clobbers icon.icns with a naive one, and
this script rebuilds it (and the PNG masters `tauri icon` doesn't touch).

Usage:  uv run scripts/gen-icons.py
"""

import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

REPO = Path(__file__).resolve().parent.parent
ART = REPO / "artifacts"
ICONS = REPO / "crates/openwith-gui/src-tauri/icons"

CANVAS, ARTS = 1024, 824  # Apple grid: 824px art centered on a 1024 canvas


def differs(p, q, t):
    return max(abs(a - b) for a, b in zip(p, q)) > t


def cut_squircle(path: Path, thresh: int) -> Image.Image:
    """Locate the rendered squircle on its solid background (scan the center
    row/column for pixels that differ from the corner color) and crop it."""
    src = Image.open(path).convert("RGB")
    w, h = src.size
    bg = src.getpixel((5, 5))
    row = [x for x in range(w) if differs(src.getpixel((x, h // 2)), bg, thresh)]
    col = [y for y in range(h) if differs(src.getpixel((w // 2, y)), bg, thresh)]
    left, top, side = row[0], col[0], row[-1] - row[0] + 1
    print(f"{path.name}: left={left} top={top} side={side}")
    return src.crop((left, top, left + side, top + side))


def squircle_mask(size: int) -> Image.Image:
    """Rounded-rect alpha mask (macOS-style radius), supersampled for clean
    edges, inset 1px so no background fringe survives at the corners."""
    ss = 4
    m = Image.new("L", (size * ss,) * 2, 0)
    ImageDraw.Draw(m).rounded_rectangle(
        (ss, ss, size * ss - 1 - ss, size * ss - 1 - ss),
        radius=int(size * ss * 0.225),
        fill=255,
    )
    return m.resize((size, size), Image.LANCZOS)


def masked_art(art: Image.Image, size: int) -> Image.Image:
    out = art.resize((size, size), Image.LANCZOS).convert("RGBA")
    out.putalpha(squircle_mask(size))
    return out


def master_1024(art: Image.Image) -> Image.Image:
    """824px masked art centered on a transparent 1024 canvas with a soft
    Apple-template-style drop shadow."""
    icon = masked_art(art, ARTS)
    off = (CANVAS - ARTS) // 2
    sh = Image.new("L", (CANVAS, CANVAS), 0)
    sh.paste(icon.getchannel("A"), (off, off + 12))
    sh = sh.filter(ImageFilter.GaussianBlur(16)).point(lambda a: int(a * 0.30))
    shadow = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    shadow.putalpha(sh)
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    canvas.alpha_composite(shadow)
    canvas.alpha_composite(icon, (off, off))
    return canvas


def remove_header_dots(art: Image.Image) -> Image.Image:
    """Inpaint the three window-header dots for the small icns slots (at
    <=64px they are sub-pixel noise). A dot pixel is white-ish with blue
    visible in all four directions; fill from the nearest blue in its row."""
    px = art.load()
    sz = art.size[0]

    def is_blue(p):
        return p[2] > 140 and p[2] - p[0] > 50

    def is_whiteish(p):
        return min(p) > 170 and max(p) - min(p) < 55

    def blue_dir(x, y, dx, dy):
        for d in (18, 26, 34, 44):
            nx, ny = x + dx * d, y + dy * d
            if 0 <= nx < sz and 0 <= ny < sz and is_blue(px[nx, ny]):
                return True
        return False

    cand = {
        (x, y)
        for y in range(sz // 2)
        for x in range(sz // 2, sz)
        if is_whiteish(px[x, y])
        and all(blue_dir(x, y, *d) for d in ((1, 0), (-1, 0), (0, 1), (0, -1)))
    }
    print(f"header dots: {len(cand)} px")
    dilated = {(x + dx, y + dy) for x, y in cand for dx in range(-4, 5) for dy in range(-4, 5)}

    def nearest_blue(x, y):
        for d in range(6, 90):
            for nx in (x + d, x - d):
                if 0 <= nx < sz and is_blue(px[nx, y]):
                    return px[nx, y]
        return None

    out = art.copy()
    opx = out.load()
    for x, y in dilated:
        if 0 <= x < sz and 0 <= y < sz and (c := nearest_blue(x, y)):
            opx[x, y] = c
    return out


def small_rep(dotless: Image.Image, size: int) -> Image.Image:
    """Legibility variant for <=64px: no shadow, art at ~92% of the canvas,
    slight unsharp mask."""
    margin = max(1, round(size * 0.04))
    art = masked_art(dotless, size - 2 * margin)
    art = art.filter(ImageFilter.UnsharpMask(radius=1, percent=60, threshold=2))
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.alpha_composite(art, (margin, margin))
    return canvas


def build_icns(light_art: Image.Image, master: Image.Image) -> None:
    dotless = remove_header_dots(light_art)
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "icon.iconset"
        iconset.mkdir()
        reps = {
            "icon_16x16.png": small_rep(dotless, 16),
            "icon_16x16@2x.png": small_rep(dotless, 32),
            "icon_32x32.png": small_rep(dotless, 32),
            "icon_32x32@2x.png": small_rep(dotless, 64),
            "icon_128x128.png": master.resize((128, 128), Image.LANCZOS),
            "icon_128x128@2x.png": master.resize((256, 256), Image.LANCZOS),
            "icon_256x256.png": master.resize((256, 256), Image.LANCZOS),
            "icon_256x256@2x.png": master.resize((512, 512), Image.LANCZOS),
            "icon_512x512.png": master.resize((512, 512), Image.LANCZOS),
            "icon_512x512@2x.png": master,
        }
        for name, img in reps.items():
            img.save(iconset / name)
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(ICONS / "icon.icns")],
            check=True,
        )
    print("icon.icns rebuilt")


def build_tray_template() -> None:
    """logo-mono-glyph.png -> 64x44 template image. The checkerboard is baked-in pixels,
    so alpha comes from a luminance ramp (dark glyph -> opaque black)."""
    g = Image.open(ART / "logo-mono-glyph.png").convert("L")
    alpha = g.point(lambda l: 0 if l >= 200 else 255 if l <= 110 else int(255 * (200 - l) / 90))
    glyph = alpha.crop(alpha.getbbox())
    tw, th, maxw, maxh = 64, 44, 62, 34
    scale = min(maxw / glyph.width, maxh / glyph.height)
    nw, nh = round(glyph.width * scale), round(glyph.height * scale)
    glyph = glyph.resize((nw, nh), Image.LANCZOS)
    tray = Image.new("RGBA", (tw, th), (0, 0, 0, 0))
    black = Image.new("RGBA", (nw, nh), (0, 0, 0, 255))
    black.putalpha(glyph)
    tray.alpha_composite(black, ((tw - nw) // 2, (th - nh) // 2))
    tray.save(ICONS / "tray-template.png")
    print(f"tray-template.png rebuilt ({nw}x{nh} glyph on {tw}x{th})")


def main() -> int:
    light_art = cut_squircle(ART / "logo-icon-light.png", thresh=10)
    dark_art = cut_squircle(ART / "logo-icon-dark.png", thresh=12)

    light_master = master_1024(light_art)
    out = Path(tempfile.gettempdir()) / "openwith-app-icon-1024.png"
    light_master.save(out)
    print(f"light master -> {out}")
    print(f"  (PNG set: cd crates/openwith-gui && npm run tauri icon {out})")

    master_1024(dark_art).save(ICONS / "icon-dark.png")
    print("icon-dark.png rebuilt")

    build_icns(light_art, light_master)
    build_tray_template()
    return 0


if __name__ == "__main__":
    sys.exit(main())
