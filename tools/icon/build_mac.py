"""Build Swoopdl's original Mac symbolic SVG icons (Python standard library only).

These are independent drawings, not Apple artwork or exported SF Symbols. All
symbols share rounded terminals, a 24-unit grid, and optical weights for 16/24 px.
Run from any directory: python tools/icon/build_mac.py
"""

from pathlib import Path
import math
import re

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "crates/app/assets/themes/mac/icons"


def path(d):
    return f'<path d="{d}"/>'


def rect(x, y, width, height, radius=1.5):
    return f'<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}"/>'


def circle(x, y, radius):
    return f'<circle cx="{x}" cy="{y}" r="{radius}"/>'


def filled(d):
    return f'<path d="{d}" fill="#ffffff" stroke="none"/>'


FOLDER = path("M3 7V5.5A1.5 1.5 0 0 1 4.5 4H9l2 2h8.5A1.5 1.5 0 0 1 21 7.5v11a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5V7h18")
DOCUMENT = path("M13.5 3H6a1.5 1.5 0 0 0-1.5 1.5v15A1.5 1.5 0 0 0 6 21h12a1.5 1.5 0 0 0 1.5-1.5V9l-6-6v4.5A1.5 1.5 0 0 0 15 9h4.5")
TRASH = path("M4.5 6h15M9 6V4.5A1.5 1.5 0 0 1 10.5 3h3A1.5 1.5 0 0 1 15 4.5V6M6.5 6l.7 13.1a2 2 0 0 0 2 1.9h5.6a2 2 0 0 0 2-1.9L17.5 6M10 10v7M14 10v7")
MAGNET = path("M5 9v5a7 7 0 0 0 14 0V9M5 9V4h4v5H5m10 0V4h4v5h-4M9 9v5a3 3 0 0 0 6 0V9")
CHECK = path("m8 12 2.6 2.6L16 9")
CLOCK = circle(12, 12, 8.5) + path("M12 7v5l3.5 2")
PLAY = filled("m10 7 7 5-7 5Z")
QUEUE = path("M4 5h16M4 10h9M4 15h6M4 20h6")


def gear():
    points = []
    for tooth in range(8):
        for offset, radius in [(-0.35, 7), (-0.19, 9), (0.19, 9), (0.35, 7)]:
            angle = tooth * math.tau / 8 + offset
            points.append(f"{12 + math.cos(angle) * radius:.3f},{12 + math.sin(angle) * radius:.3f}")
    return '<polygon points="' + " ".join(points) + '"/>' + circle(12, 12, 3)


def symbols():
    return {
        "tb-add-url": path("m9 14-1 1a3.5 3.5 0 0 1-5-5l4-4a3.5 3.5 0 0 1 5 0M10 11l1-1a3.5 3.5 0 0 1 5 0M7 13l4-4M17 14v7M13.5 17.5h7"),
        "tb-add-torrent": MAGNET,
        "tb-resume": circle(12, 12, 9) + PLAY,
        "tb-stop": circle(12, 12, 9) + rect(8, 8, 8, 8, 1),
        "tb-stop-all": rect(3.5, 3.5, 13, 13, 3) + path("M8 20.5h9.5a3 3 0 0 0 3-3V8") + rect(7.5, 7.5, 5, 5, 0.5),
        "tb-delete": TRASH,
        "tb-delete-completed": path("M4 6h13M8 6V4h5v2M6 6l.8 13a2 2 0 0 0 2 2H12M10 10v6M16 10v2m-2 5 2.5 2.5L21 15"),
        "tb-options": gear(),
        "tb-scheduler": rect(3, 5, 18, 16, 3) + path("M7.5 3v4M16.5 3v4M3 10h18") + circle(12, 15.5, 3) + path("M12 13.5v2l1.5 1"),
        "tb-start-queue": QUEUE + filled("m15 13 6 4-6 4Z"),
        "tb-stop-queue": QUEUE + rect(14, 14, 7, 7, 1),
        "cat-all": path("M4 8 2.5 17v2A1.5 1.5 0 0 0 4 20.5h16a1.5 1.5 0 0 0 1.5-1.5v-2L20 8M3 16h5l1.5 2h5l1.5-2h5M12 3v9m-3-3 3 3 3-3"),
        "cat-compressed": rect(4, 4, 16, 17, 3) + path("M3 4h18M12 4v9m-1-7h2m-2 3h2") + rect(10, 13, 4, 4, 1),
        "cat-documents": rect(7, 3, 13, 16, 2) + path("M4 6v13a2 2 0 0 0 2 2h11M10.5 8h6M10.5 12h6M10.5 16h3"),
        "cat-music": path("M10 17V6l10-2v11M10 9l10-2") + '<ellipse cx="7" cy="17.5" rx="3" ry="2.5"/><ellipse cx="17" cy="15.5" rx="3" ry="2.5"/>',
        "cat-programs": rect(3, 3, 18, 18, 4) + path("M3 8h18M7 5.5h.01M10 5.5h.01M8 12l-2 2 2 2M12 17h5"),
        "cat-video": rect(3, 5, 18, 14, 3) + filled("m10 8 6 4-6 4Z"),
        "cat-torrents": MAGNET,
        "cat-unfinished": path("M20 8a9 9 0 1 0 .7 5M12 6v10m-4-4 4 4 4-4"),
        "cat-finished": circle(12, 12, 9) + CHECK,
        "cat-queues": rect(4, 3, 16, 4, 1.5) + rect(4, 10, 16, 4, 1.5) + rect(4, 17, 16, 4, 1.5),
        "cat-folder": FOLDER,
        "cat-folder-open": path("M3 17V5.5A1.5 1.5 0 0 1 4.5 4H9l2 2h7.5A1.5 1.5 0 0 1 20 7.5V9M3 20h15a2 2 0 0 0 1.9-1.4l2-7A1.3 1.3 0 0 0 20.6 10H7a1.5 1.5 0 0 0-1.4 1L3 20Z"),
        "misc-emblem-ok": circle(12, 12, 8.5) + CHECK,
        "misc-up": path("M12 20V4m-6 6 6-6 6 6"),
        "misc-down": path("M12 4v16m-6-6 6 6 6-6"),
        "misc-remove": circle(12, 12, 8.5) + path("m9 9 6 6m0-6-6 6"),
        "ft-generic": DOCUMENT,
        "ft-archive": DOCUMENT + path("M12 10v5m-1-3h2") + rect(10.5, 15, 3, 3, 0.5),
        "ft-audio": DOCUMENT + path("M12 17v-5l4-1v5") + '<ellipse cx="10.5" cy="17" rx="1.5" ry="1"/><ellipse cx="14.5" cy="16" rx="1.5" ry="1"/>',
        "ft-video": DOCUMENT + filled("m10 12 5 3-5 3Z"),
        "ft-image": DOCUMENT + circle(9, 12, 1) + path("m7.5 18 3-3 2 2 2-3 2 4"),
        "ft-document": DOCUMENT + path("M8 12h8M8 15h8M8 18h5"),
        "ft-pdf": DOCUMENT + path("M7.5 18v-5h1a1.2 1.2 0 0 1 0 2.4h-1m4 2.6v-5h.5a2.5 2.5 0 0 1 0 5h-.5m5 0v-5H19m-2.5 2.5H18"),
        "ft-spreadsheet": DOCUMENT + rect(8, 12, 8, 6, 0.5) + path("M8 15h8M11 12v6"),
        "ft-presentation": DOCUMENT + path("M8 12h8v5H8v-5m4 5v2m-2 0h4") + path("m10 15 1.5-1.5 2 1"),
        "ft-text": DOCUMENT + path("M8 12h8M8 15h8M8 18h8"),
        "ft-code": DOCUMENT + path("m9.5 12-2 3 2 3m5-6 2 3-2 3m-2.5-6-1 6"),
        "ft-executable": DOCUMENT + rect(8, 12, 8, 6, 1) + path("M8 14h8m-6 2h.01m3 0h1"),
        "ft-disk-image": DOCUMENT + circle(12, 15, 3.5) + circle(12, 15, 0.75),
        "ft-font": DOCUMENT + path("m8 18 4-7 4 7m-6.5-2.5h5"),
        "ft-torrent": DOCUMENT + path("M9 12v3a3 3 0 0 0 6 0v-3M9 14h1.5m3 0H15"),
        "st-complete": circle(12, 12, 9) + CHECK,
        "st-downloading": circle(12, 12, 9) + path("M12 7v10m-4-4 4 4 4-4"),
        "st-paused": circle(12, 12, 9) + path("M9 8v8M15 8v8"),
        "st-queued": CLOCK,
        "st-error": path("m10.5 4-8 14a1.5 1.5 0 0 0 1.3 2.2h16.4a1.5 1.5 0 0 0 1.3-2.2l-8-14a1.7 1.7 0 0 0-3 0ZM12 9v4.5M12 17h.01"),
        "st-seeding": circle(12, 12, 9) + path("M12 17V7m-4 4 4-4 4 4"),
        "ui-minimize": path("M6 12h12"),
        "ui-maximize": path("M5 10V5h5m4 14h5v-5M5 5l5 5m4 4 5 5"),
        "ui-restore": rect(7, 4, 13, 13, 2) + path("M4 8v10a2 2 0 0 0 2 2h10"),
        "ui-close": path("m6 6 12 12M18 6 6 18"),
        "ui-to-tray": path("M4 16v3a1.5 1.5 0 0 0 1.5 1.5h13A1.5 1.5 0 0 0 20 19v-3M12 3v12m-4-4 4 4 4-4"),
    }


def main():
    drawings = symbols()
    source = (ROOT / "crates/app/src/themeicons.rs").read_text("utf-8")
    needs = set(re.findall(r'"((?:tb|cat|misc|ft|st|ui)-[a-z-]+)"', source.split("pub const NEEDS:", 1)[1].split("];", 1)[0]))
    if needs != drawings.keys():
        raise ValueError(f"Missing: {needs - drawings.keys()}; extra: {drawings.keys() - needs}")
    for size, weight in [(16, 1.9), (24, 1.65)]:
        folder = OUT / str(size)
        folder.mkdir(parents=True, exist_ok=True)
        for name, body in drawings.items():
            svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="{weight}" stroke-linecap="round" stroke-linejoin="round">\n{body}\n</svg>\n'
            (folder / f"{name}.svg").write_text(svg, encoding="utf-8", newline="\n")
    print(f"Built {len(drawings)} Mac symbols at 16 and 24 px -> {OUT}")


if __name__ == "__main__":
    main()
