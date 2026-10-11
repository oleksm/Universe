"""Logos as PNG beside their SVG (MakerHouse/logos/<name>.png, 256 px), so the game reads an image, not SVG
(docs/packing-contract.md, 4). Rerun after a logo changes; needs rsvg-convert.

    python3 tools/standards/logos.py
"""
import glob, os, subprocess

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
for svg in sorted(glob.glob(os.path.join(ROOT, "standards/MakerHouse/logos/*.svg"))):
    png = svg[:-4] + ".png"
    subprocess.run(["rsvg-convert", "-w", "256", "-h", "256", "-o", png, svg], check=True)
    print(os.path.relpath(png, ROOT))
