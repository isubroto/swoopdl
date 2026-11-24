"""Build every app icon size from the approved Swoopdl S-and-arrow mark.

Keep the complete mark at every size, including the taskbar and browser toolbar.
Preserve the source's transparency and square canvas.

Writes crates/app/assets/icons/app/sdl-<size>.png, SDL.ico (all sizes) and the
browser extension's icons. Run from the repository root: python -I tools/icon/build_icon.py
"""

from pathlib import Path
import base64
import re

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "art" / "Swoopdl.png"
APP = ROOT / "crates" / "app" / "assets" / "icons" / "app"
EXT = ROOT / "extension" / "icons"
SIZES = [16, 20, 24, 32, 48, 64, 128, 256, 512]


def main() -> None:
    src = Image.open(SRC).convert("RGBA")
    if src.width != src.height or not src.getbbox():
        raise ValueError("The approved icon must have a nonempty square canvas")
    icons = {}
    for size in SIZES:
        icons[size] = src.resize((size, size), Image.Resampling.LANCZOS)
        icons[size].save(APP / f"sdl-{size}.png")
    big = icons[256]
    big.save(
        APP / "sdl.ico",
        sizes=[(s, s) for s in SIZES if s <= 256],
        append_images=[icons[s] for s in SIZES if s < 256],
    )
    for size in (16, 32, 48, 64, 128):
        icons[size].save(EXT / f"icon-{size}.png")
    icons[512].save(ROOT / "docs" / "images" / "logo.png")
    # Content scripts embed the toolbar mark so pages cannot block an extension URL.
    video = ROOT / "extension" / "video.js"
    data = base64.b64encode((EXT / "icon-48.png").read_bytes()).decode("ascii")
    text, count = re.subn(r'const LOGO = "data:image/png;base64,[^"]+";',
                          f'const LOGO = "data:image/png;base64,{data}";',
                          video.read_text(encoding="utf-8"))
    if count != 1:
        raise ValueError("Expected exactly one embedded video button logo")
    video.write_text(text, encoding="utf-8")
    print("wrote", ", ".join(str(s) for s in SIZES), "+ sdl.ico + extension icons")


if __name__ == "__main__":
    main()
