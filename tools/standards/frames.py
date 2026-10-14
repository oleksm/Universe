"""Built frames (SFO 14 assemblies) and the structural stock they are cut from (SFO 13). One rule for the registry, the
studio and the game.

    python3 tools/standards/frames.py --stock    # mill lengths and sheet sizes on frame stock; end fittings and bolts, with their recipes
    python3 tools/standards/frames.py --check    # every assembly's derived groups against its members, nodes and decks
    python3 tools/standards/frames.py --write    # write them (fittings, bill, physical, making) into the assemblies

The designer places members, nodes and decks; the rest follows:
- A member end within 1 mm of a node meets it. It is welded where the member is of the node's material and not pinned;
  any other end meeting a node, and any pinned end, is bolted: a forged END fitting the size of the member (the
  smallest at least as wide) and one bolt, the smallest ISO size at least a quarter of the member's width, 4 diameters
  long (the MERO joint).
- A member longer than its stock's mill length is spliced (ceil(L / mill) - 1 splices): steel is butt-welded, counting
  nothing; anything else takes a 0.3 m sleeve of its own stock at each.
- A deck rests on every member whose top (axis plus half its width) lies within 20 mm under it and whose run crosses
  it; each such member holds it with a clip every 0.5 m of the run, two at least: a 60 mm strip of 6061 plate 5 mm bent
  round half the member with two 40 mm tabs, and two M10 bolts.
- The bill: members packed into mill lengths (longest first, first fit), each deck laid on whole sheets the cheaper way
  round, fittings by the piece (clips' plate with 15% cut away).
- Physical: the mass of members, nodes, decks and fittings; length, width and height the extent in x, z and y.
"""
import glob, math, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import edit, q  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
S = os.path.join(ROOT, "standards/SFO/metadata")
STEELS = {"material.high-strength-steel-4340", "material.structural-steel-a36", "material.stainless-steel-304"}
ISO = [10, 12, 16, 20, 24, 30, 36, 42, 48, 56, 64, 72, 80, 90, 100]      # metric bolt sizes, mm
END_EXTRA = [400]                                                       # box widths a pinned box end needs an END for
TUBE_MILL, BOX_MILL = 6.0, 12.0
SHEETS = {"metal plate": (6.0, 2.0), "metal sheet": (2.5, 1.25), "composite": (3.0, 1.5)}
CLIP_STRIP, CLIP_TABS, CLIP_GAUGE, CLIP_EVERY = 0.06, 0.08, 0.005, 0.5
SLEEVE, NODE_TOL, DECK_TOL = 0.3, 0.001, 0.02
STEEL = "material.high-strength-steel-4340"
STOCK_NOTE_TUBE = "Mill lengths: 6 m for tube and bar, 12 m for box section, the commonest mill and extrusion lengths on Earth; chosen, a frame longer than its member's length is spliced (tools/standards/frames.py)."


def load(kind):
    out = {}
    for f in sorted(glob.glob(os.path.join(S, kind, "*.yaml"))):
        d = yaml.safe_load(open(f, encoding="utf-8")) or {}
        out[d["identity"]["key"]] = (f, d)
    return out


def materials():
    return {d["identity"]["key"]: d for f, d in load("materials").values()}


def density(stock, mats):
    return mats[stock["made_from"][0]["item"]]["mass"]["density"]


def area(stock):
    """m2 of a member's section."""
    z, form = stock.get("size") or {}, stock["identity"]["form"]
    if form == "box":
        w, h, t = z["width"], z["height"], z["wall"]
        return w * h - (w - 2 * t) * (h - 2 * t)
    d = z["diameter"]
    t = min(z.get("wall", d / 2), d / 2)
    return math.pi / 4 * (d * d - (d - 2 * t) ** 2)


def width(stock):
    z = stock.get("size") or {}
    return max(z.get("width", 0), z.get("height", 0)) if stock["identity"]["form"] == "box" else z["diameter"]


def node_mass(stock, mats):
    z, rho = stock["size"], density(stock, mats)
    d, w = z["diameter"], z.get("wall")
    return 4 * math.pi * (d / 2) ** 2 * w * rho * (1 - w / d) if w else math.pi / 6 * d ** 3 * rho


def per_area(stock, mats):
    sw = stock.get("sandwich")
    if sw:
        return sw.get("mass_per_area") or 2 * sw["faces"]["thickness"] * mats[sw["faces"]["material"]]["mass"]["density"] + sw["core"]["depth"] * sw["core"]["density"] + sw["bond"]
    return stock["size"]["thickness"] * density(stock, mats)


def bolt_for(w):
    return next(d for d in ISO if d >= w * 1000 / 4)


def end_mass(d):
    """kg of a forged END fitting for a member d across: a cone d long from d to its bolt, bored for it; about 0.12 rho d^3."""
    return round(0.12 * 7850 * d ** 3, 3)


def bolt_mass(d_mm):
    d = d_mm / 1000
    return round(math.pi / 4 * d * d * 4 * d * 7850 * 1.3, 4)        # shank 4 d long, and 30% for the head and nut


# ---- stock

def sheet_size(stock, mats):
    form, mat = stock["identity"]["form"], stock["made_from"][0]["item"]
    if form == "sandwich" or mats[mat]["identity"]["class"] == "composite":
        return SHEETS["composite"]
    return SHEETS["metal plate" if form == "plate" else "metal sheet"]


def add_size(text, lines, note):
    """Add `lines` (already indented) to a record's size group, and a basis entry with `note`."""
    if "\nsize:\n" not in text:
        return text
    head, rest = text.split("\nsize:\n", 1)
    body = []
    rest_lines = rest.split("\n")
    while rest_lines and rest_lines[0].startswith("  "):
        body.append(rest_lines.pop(0))
    text = head + "\nsize:\n" + "\n".join(body + lines) + "\n" + "\n".join(rest_lines)
    of = "[" + ", ".join("size." + l.split(":")[0].strip() for l in lines) + "]"
    return text.replace("\nbasis:\n", f"\nbasis:\n  - {{ of: {of}, tier: invented, review: true, note: {q(note)} }}\n", 1)


def stock():
    mats = materials()
    added = 0
    diameters = set()
    for key, (f, d) in load("mill-stock").items():
        idn, z = d["identity"], d.get("size") or {}
        form, code = idn["form"], idn["code"]
        if form in ("tube", "bar"):
            diameters.add(round(z["diameter"], 3))
        if code.endswith(("-BLANK", "-PANEL")) or "length" in z:
            continue
        if form in ("tube", "bar", "box"):
            mill = BOX_MILL if form == "box" else TUBE_MILL
            edit(f, lambda t: add_size(t, [f"  length: {mill}"], STOCK_NOTE_TUBE))
            added += 1
        elif form in ("plate", "sheet", "sandwich"):
            L, W = sheet_size(d, mats)
            note = f"Sold in sheets of {L} by {W} m: 6 by 2 m for metal plate, 2.5 by 1.25 m for sheet, 3 by 1.5 m for composite and sandwich panel (an autoclave's bed); chosen. A deck is laid out on whole sheets (tools/standards/frames.py)."
            lines = [f"  length: {L}"] + ([] if form == "box" else [f"  width: {W}"])
            edit(f, lambda t: add_size(t, lines, note))
            added += 1
    # (The END fittings, one for each tube and bar width and the boxes' that need one; their bolts; and the deck clips' M10.)
    ends = sorted(diameters | {w / 1000 for w in END_EXTRA})
    bolts = sorted({bolt_for(d) for d in ends} | {10})
    made = []
    for d in ends:
        mm = round(d * 1000)
        code, key = f"ST4340-END-{mm}", f"stock.st4340-end-{mm}"
        made.append((key, "A forged END fitting pressed between dies, bored and machined true."))
        write_new(code, f"""# yaml-language-server: $schema=../../schema/mill-stock.schema.yaml
identity:
  key: {key}
  code: {code}
  name: "4340 end fitting {mm} mm"
  traded_as: market.metals
  form: forging
  temper: "quenched and tempered"
package: package.crate-heavy
tint: [0.44, 0.45, 0.47]
made_from:
  - item: {STEEL}
size:
  diameter: {d}
  length: {d}
physical:
  mass: {end_mass(d)}
basis:
  - {{ of: [size, physical.mass], tier: invented, review: true, note: "The cone welded or bonded into a {mm} mm member's end, through which one M{bolt_for(d)} bolt holds it to a node (the MERO joint): as long as the member is wide, about 0.12 rho d^3 with its bore. Chosen (tools/standards/frames.py), not drawn." }}
""")
    for b in bolts:
        L = 4 * b / 1000
        code, key = f"ST4340-BOLT-M{b}", f"stock.st4340-bolt-m{b}"
        made.append((key, "Bar cut to length, its head upset hot and its thread rolled; the nut likewise."))
        write_new(code, f"""# yaml-language-server: $schema=../../schema/mill-stock.schema.yaml
identity:
  key: {key}
  code: {code}
  name: "4340 bolt M{b} x {round(L * 1000)} with nut"
  traded_as: market.metals
  form: fastener
  temper: "quenched and tempered"
package: package.crate-heavy
tint: [0.44, 0.45, 0.47]
made_from:
  - item: {STEEL}
size:
  diameter: {b / 1000}
  length: {L}
physical:
  mass: {bolt_mass(b)}
basis:
  - {{ of: [size, physical.mass], tier: invented, review: true, note: "An ISO metric hex bolt and nut, its shank 4 diameters long, and 30% of the shank's weight for the head and the nut. One length to a size (tools/standards/frames.py)." }}
""")
    fp = os.path.join(S, "modules/forging-press.yaml")
    have = open(fp, encoding="utf-8").read()
    new = "".join(f"""  - makes: {key}
    does: "{does}"
    inputs:
      - {{ item: good.hot-ingot-4340, quantity: 1.2 }}
    outputs:
      - {{ item: stock.st4340-scrap, quantity: 0.2 }}
    rate: 1.38888888888889
    power: 6000000
""" for key, does in made if f"makes: {key}\n" not in have)
    if new:
        edit(fp, lambda t: t.replace("\nlife:", "\n" + new.rstrip("\n") + "\nlife:", 1))
    print(f"{added} stock records sized; {len(ends)} end fittings, {len(bolts)} bolts")


def write_new(code, text):
    path = os.path.join(S, "mill-stock", code + ".yaml")
    if not os.path.exists(path):
        open(path, "w", encoding="utf-8").write(text)


# ---- assemblies

def near(p, q_):
    return math.dist(p, q_) <= NODE_TOL


def pack(lengths, mill):
    """Mill lengths needed: pieces longer than a mill length are cut from whole ones first; then longest first, first fit."""
    bins, n = [], 0
    for L in sorted(lengths, reverse=True):
        while L > mill + 1e-9:
            n += 1
            L -= mill
        for i, room in enumerate(bins):
            if room >= L - 1e-9:
                bins[i] -= L
                break
        else:
            bins.append(mill - L)
    return n + len(bins)


def derive(rec, stocks, mats):
    """The derived groups of an assembly record: fittings, bill, physical."""
    S_ = lambda k: stocks[k][1]
    nodes = rec.get("nodes") or []
    fit, bill, mass = {}, {}, 0.0
    lo, hi = [math.inf] * 3, [-math.inf] * 3

    def add(kind, key, count, kg):
        c, m = fit.get((kind, key), (0, 0.0))
        fit[(kind, key)] = (c + count, m + kg)

    def ext(p, r):
        for i in range(3):
            lo[i], hi[i] = min(lo[i], p[i] - r), max(hi[i], p[i] + r)

    lengths = {}
    for m in rec.get("members") or []:
        st = S_(m["stock"])
        rho, a_, w = density(st, mats), area(st), width(st)
        L = math.dist(m["a"], m["b"])
        mass += L * a_ * rho
        lengths.setdefault(m["stock"], []).append(L)
        ext(m["a"], w / 2)
        ext(m["b"], w / 2)
        mat = st["made_from"][0]["item"]
        for end, pinned in zip((m["a"], m["b"]), m.get("pinned") or [False, False]):
            node = next((n for n in nodes if near(n["at"], end)), None)
            welded = node is not None and not pinned and S_(node["stock"])["made_from"][0]["item"] == mat
            if pinned or (node is not None and not welded):
                e = min((k for k, (f, d) in stocks.items() if ".st4340-end-" in k and d["size"]["diameter"] >= w - 1e-9), key=lambda k: stocks[k][1]["size"]["diameter"])
                b = f"stock.st4340-bolt-m{bolt_for(w)}"
                add("end", e, 1, S_(e)["physical"]["mass"])
                add("bolt", b, 1, S_(b)["physical"]["mass"])
        mill = (st.get("size") or {}).get("length", TUBE_MILL)
        splices = max(0, math.ceil(L / mill - 1e-9) - 1)
        if splices and mat not in STEELS:
            add("splice", m["stock"], splices, splices * SLEEVE * a_ * rho)
    for key, ls in lengths.items():
        st = S_(key)
        mill = (st.get("size") or {}).get("length", TUBE_MILL)
        n = pack(ls, mill)
        bill[key] = (n, n * mill * area(st) * density(st, mats))
    for n in nodes:
        st = S_(n["stock"])
        kg = node_mass(st, mats)
        mass += kg
        ext(n["at"], st["size"]["diameter"] / 2)
        c, m_ = bill.get(n["stock"], (0, 0.0))
        bill[n["stock"]] = (c + 1, m_ + kg)
    for dk in rec.get("decks") or []:
        st = S_(dk["stock"])
        (x0, z0), (x1, z1) = dk["lo"], dk["hi"]
        dx, dz = abs(x1 - x0), abs(z1 - z0)
        kpa = per_area(st, mats)
        mass += dx * dz * kpa
        th = (st.get("size") or {}).get("thickness", 0.0)
        ext([min(x0, x1), dk["y"], min(z0, z1)], 0)
        ext([max(x0, x1), dk["y"] + th, max(z0, z1)], 0)
        z = st.get("size") or {}
        L, W = z.get("length", 3.0), z.get("width", 1.5)
        sheets = min(math.ceil(dx / L - 1e-9) * math.ceil(dz / W - 1e-9), math.ceil(dx / W - 1e-9) * math.ceil(dz / L - 1e-9))
        c, m_ = bill.get(dk["stock"], (0, 0.0))
        bill[dk["stock"]] = (c + sheets, m_ + sheets * L * W * kpa)
        for m in rec.get("members") or []:
            w = width(S_(m["stock"]))
            ya, yb = m["a"][1] + w / 2, m["b"][1] + w / 2
            if not (abs(ya - dk["y"]) <= DECK_TOL and abs(yb - dk["y"]) <= DECK_TOL):
                continue
            run = clipped(m["a"], m["b"], min(x0, x1), max(x0, x1), min(z0, z1), max(z0, z1))
            if run <= 0:
                continue
            clips = max(2, math.ceil(run / CLIP_EVERY - 1e-9))
            plate = "stock.al6061-pl-5"
            kg = clips * CLIP_STRIP * (math.pi * w / 2 + CLIP_TABS) * CLIP_GAUGE * density(S_(plate), mats)
            add("clip", plate, clips, kg)
            add("bolt", "stock.st4340-bolt-m10", 2 * clips, 2 * clips * S_("stock.st4340-bolt-m10")["physical"]["mass"])
    fittings = []
    for (kind, key), (c, kg) in sorted(fit.items()):
        mass += kg
        fittings.append({"kind": kind, "stock": key, "count": c, "mass": round(kg, 2)})
        if kind == "splice":
            continue
        b = bill.get(key, (0, 0.0))
        bill[key] = (b[0] + c, b[1] + (kg * 1.15 if kind == "clip" else kg))
    phys = {"mass": round(mass, 1), "length": round(hi[0] - lo[0], 3), "width": round(hi[2] - lo[2], 3), "height": round(hi[1] - lo[1], 3)} if lo[0] < math.inf else {}
    return fittings, [{"stock": k, "pieces": n, "mass": round(kg, 1)} for k, (n, kg) in sorted(bill.items())], phys


def clipped(a, b, x0, x1, z0, z1):
    """m of the run a-b (in x and z) inside the rectangle (Liang-Barsky)."""
    t0, t1 = 0.0, 1.0
    dx, dz = b[0] - a[0], b[2] - a[2]
    for p, q_ in ((-dx, a[0] - x0), (dx, x1 - a[0]), (-dz, a[2] - z0), (dz, z1 - a[2])):
        if abs(p) < 1e-12:
            if q_ < 0:
                return 0.0
        else:
            t = q_ / p
            if p < 0:
                t0 = max(t0, t)
            else:
                t1 = min(t1, t)
    return max(0.0, t1 - t0) * math.hypot(dx, dz)


ORDER = ["identity", "nodes", "members", "decks", "fittings", "bill", "physical", "making", "revision", "basis"]


def dump(rec):
    flow = lambda v: yaml.safe_dump(v, default_flow_style=True, sort_keys=False, width=1 << 20).strip()
    out = ["# yaml-language-server: $schema=../../schema/assembly.schema.yaml"]
    for k in ORDER:
        if k not in rec:
            continue
        v = rec[k]
        if isinstance(v, list):
            out.append(f"{k}:")
            out += [f"  - {flow(x)}" for x in v]
        else:
            out.append(yaml.safe_dump({k: v}, sort_keys=False, width=1 << 20).rstrip())
    return "\n".join(out) + "\n"


def assemblies(write):
    stocks, mats = load("mill-stock"), materials()
    bad = 0
    for key, (f, rec) in load("assemblies").items():
        fittings, bill, phys = derive(rec, stocks, mats)
        want = {**rec, "fittings": fittings, "bill": bill, "physical": {**(rec.get("physical") or {}), **phys}}
        want.setdefault("making", {"module": "module.welding-bay"})
        if any(want.get(k) != rec.get(k) for k in ("fittings", "bill", "physical", "making")):
            if write:
                open(f, "w", encoding="utf-8").write(dump(want))
                print(f"wrote {os.path.relpath(f, ROOT)}")
            else:
                bad += 1
                print(f"{os.path.relpath(f, ROOT)}: derived groups out of date (run frames.py --write)")
    return bad


if __name__ == "__main__":
    if "--stock" in sys.argv:
        stock()
    if "--write" in sys.argv or "--check" in sys.argv:
        sys.exit(1 if assemblies("--write" in sys.argv) else 0)
