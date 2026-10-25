"""The reference renderer of a package label (docs/formats/freefall-label-1.md): who, what, where, when and a barcode,
drawn to a 2:1 PNG the game (or anyone) can match. The game prints the same layout onto a package model's `label`
plates at runtime; this draws it for previews and for checking.

    python3 tools/standards/label_render.py --sample out.png                       # a sample label
    python3 tools/standards/label_render.py label.json out.png [--px=1024]          # a label (the `label` shape, JSON)

Needs PIL; logos (SVG) are rasterised with rsvg-convert, and a brand with no logo prints its ticker in its colours.
"""
import io, json, os, subprocess, sys
import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
T = os.path.join(ROOT, "standards")

C128 = ("212222 222122 222221 121223 121322 131222 122213 122312 132212 221213 221312 231212 112232 122132 122231 113222 "
        "123122 123221 223211 221132 221231 213212 223112 312131 311222 321122 321221 312212 322112 322211 212123 212321 "
        "232121 111323 131123 131321 112313 132113 132311 211313 231113 231311 112133 112331 132131 113123 113321 133121 "
        "313121 211331 231131 213113 213311 213131 311123 311321 331121 312113 312311 332111 314111 221411 431111 111224 "
        "111422 121124 121421 141122 141221 112214 112412 122114 122411 142112 142211 241211 221114 413111 241112 134111 "
        "111242 121142 121241 114212 124112 124211 411212 421112 421211 212141 214121 412121 111143 111341 131141 114113 "
        "114311 411113 411311 113141 114131 311141 411131 211412 211214 211232 2331112").split()


def code128_modules(text):
    """Code 128 set B: the bars and spaces as a list of (is_bar, width) in modules, quiet zones excluded."""
    vals = [104] + [ord(c) - 32 for c in text]
    if any(not 0 <= v - 0 <= 95 for v in vals[1:]):
        raise ValueError("Code 128 set B takes printable ASCII")
    check = (vals[0] + sum(i * v for i, v in enumerate(vals[1:], 1))) % 103
    out = []
    for v in vals + [check, 106]:
        for k, w in enumerate(C128[v]):
            out.append((k % 2 == 0, int(w)))
    return out


def find(folder, key):
    for f in __import__("glob").glob(os.path.join(T, folder, "**", "*.yaml"), recursive=True):
        d = yaml.safe_load(open(f)) or {}
        if (d.get("identity") or {}).get("key") == key:
            return d
    return {}


def when(s):
    """World time as the game shows it: day and clock."""
    s = int(s); return f"DAY {s // 86400} {s % 86400 // 3600:02d}:{s % 3600 // 60:02d}"


def render(label, out, px=1024):
    from PIL import Image, ImageDraw, ImageFont
    W, H = px, px // 2
    org = find("MakerHouse", label["brand"]); brand = org.get("brand") or {}
    stock = find("SFO/metadata", label["product"]); pkg = find("SFO/metadata/packages", label["package"])
    fac = find("LocalAdministration", label["made_at"])
    rgb = lambda c: tuple(int(round(v * 255)) for v in c)
    prim, acc = rgb(brand.get("primary", [0.2, 0.2, 0.2])), rgb(brand.get("accent", [0.85, 0.85, 0.85]))
    img = Image.new("RGB", (W, H), (250, 250, 246)); d = ImageDraw.Draw(img)
    font = lambda size, bold=False: ImageFont.truetype("/usr/share/fonts/noto/NotoSansMono-" + ("Bold" if bold else "Regular") + ".ttf", size)
    band = H // 4
    d.rectangle([0, 0, W, band], fill=prim)
    logo = brand.get("logo")
    if logo and os.path.exists(os.path.join(T, logo)):
        pngb = subprocess.run(["rsvg-convert", "-w", str(band - 12), "-h", str(band - 12), os.path.join(T, logo)], capture_output=True).stdout
        img.paste(Image.open(io.BytesIO(pngb)).convert("RGBA"), (8, 6), Image.open(io.BytesIO(pngb)).convert("RGBA"))
    else:
        d.rectangle([8, 6, band - 6, band - 6], fill=acc); d.text((band // 2, band // 2), org.get("ticker", "?"), fill=prim, font=font(band // 4, True), anchor="mm")
    d.text((band + 10, band // 2), brand.get("name", org.get("identity", {}).get("name", label["brand"])).upper(), fill=(255, 255, 255), font=font(band // 3, True), anchor="lm")
    y = band + 10; f1, f2 = font(H // 14, True), font(H // 20)
    d.text((14, y), (stock.get("identity") or {}).get("name", label["product"]), fill=(20, 20, 20), font=f1); y += H // 11
    lines = [f"LOT {label['lot']}   SERIAL {label['serial']}",
             f"MADE AT {(fac.get('identity') or {}).get('name', label['made_at'])}",
             f"MADE {when(label['made_on'])}   PACKED {when(label['packed_on'])}",
             f"NET {label['net_mass']:.0f} kg   GROSS {label['gross_mass']:.0f} kg   {(pkg.get('identity') or {}).get('name', label['package']).upper()}"]
    keeps = pkg.get("keeps"); stack = pkg.get("stack")
    lines.append(f"STACK {stack} HIGH" + (f"   KEEP {keeps.upper()}" if keeps else ""))
    for ln in lines:
        d.text((14, y), ln, fill=(40, 40, 40), font=f2); y += H // 17
    mods = code128_modules(label["serial"]); total = sum(w for _, w in mods) + 20
    unit = max(1, (W - 28) // total); x = 14 + 10 * unit; top, bot = H - H // 5, H - H // 16
    for bar, w in mods:
        if bar:
            d.rectangle([x, top, x + w * unit - 1, bot], fill=(0, 0, 0))
        x += w * unit
    d.text((W // 2, H - 4), label["serial"], fill=(0, 0, 0), font=font(H // 22), anchor="mb")
    img.save(out)
    return out


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("--")]
    px = int(next((a.split("=")[1] for a in argv if a.startswith("--px=")), 1024))
    if "--sample" in argv:
        label = {"brand": "org.brightwater-dairy", "product": "stock.cheese-bulk", "lot": "BWD-004471", "serial": "BWD-004471-0007",
                 "made_at": "facility.treistun.port-eikir.eikir-farms", "made_on": 3_456_000, "packed_on": 3_470_400,
                 "net_mass": 820, "gross_mass": 910, "package": "package.cold-crate", "barcode": "code128"}
        print(render(label, args[0] if args else "label-sample.png", px)); return
    if len(args) < 2:
        sys.exit(__doc__)
    print(render(json.load(open(args[0])), args[1], px))


if __name__ == "__main__":
    main(sys.argv)
