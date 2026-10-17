"""Buildings as layouts (the owner, 2026-10-17: a building is a layout laid of a frame, panels, fit-out units and service
kit, not one model; frame + panel kit chosen). For each building record (now deprecated, replaced_by its layout):

- a frame of the kit's standard struts (SFO 13) on the 3 m grid: columns at every grid node, beams along every cell
  edge at each floor and the roof, one diagonal in every other outside wall face; sections sized by the loads;
- panels (building_kit.py) on its faces: the front, the sides and back, each storey's floor, the roof, partitions and
  stairs;
- fit-out units (furnished 6 m bays, written here) on bays of plant-mount.bay-s1;
- a ground slab and edge footings.

    python3 tools/standards/building_layouts.py      # write the fit-out units and the 25 building layouts

Site: a world with air at about 1 g (the building records' own). What each building holds comes from its record's
floor, storeys and staff; the mix of bays a building type has is chosen (invented), said in each layout's basis.
Services (power, water, drains) are not laid yet: the kit has no low-voltage cable or small pipe; each layout says so.
"""
import glob, math, os, sys
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import frames  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata")
HEAD = "# Written by tools/standards/building_layouts.py: change it there.\n"
G = 9.80665
CELL, BAY = 3.0, 6.0
STEEL, SS, AL, PE, GLASS, SILICA, RUB, CONC = ("material.structural-steel-a36", "material.stainless-steel-304", "material.aluminium-alloy-6061",
                                               "material.polyethylene-uhmw", "material.soda-lime-glass", "material.fused-silica", "material.epdm-rubber", "good.concrete")


def r(x, nd=1):
    return round(float(x), nd)


def load(path):
    return yaml.safe_load(open(path, encoding="utf-8"))


PANELS = {os.path.basename(f)[:-5]: load(f) for f in glob.glob(os.path.join(M, "panels", "*.yaml"))}


def pmass(slug):
    return PANELS[slug]["physical"]["mass"]


# ------------------------------------------------------------------ fit-out units: furnished 6 m bays
FITOUT = {  # slug: (name, description, [(what, item, kg, note)], people it serves or seats, staff it needs)
    "sales-floor": ("Sales floor bay", "A 6 m bay of shelving gondolas and their aisles: stock laid out for people to take.", [("Gondolas", STEEL, 820, "six double-sided runs 1.8 m high"), ("Shelf edges and signs", PE, 80, "")], 12, 0),
    "counter": ("Counter bay", "A serving counter with its tills, back shelving and a queue's room.", [("Counter and back shelving", STEEL, 426.9, "hollow frame and 2 mm cabinet and shelf sheet: blender's model (2026-10-17)"), ("Worktop", PE, 96.4, "24 mm: blender's model"), ("Tills", AL, 40, "two")], 6, 2),
    "store": ("Store bay", "Pallet racking three levels high and its forklift aisle: the stock room.", [("Racking", STEEL, 1250, "two runs, three beam levels")], 0, 0),
    "dining": ("Dining bay", "Four tables of four and their chairs: a tavern's, an inn's or a canteen's room to eat.", [("Tables", AL, 160, "four"), ("Chairs", AL, 170, "sixteen"), ("Tops and seats", PE, 90, "")], 16, 0),
    "kitchen": ("Kitchen bay", "A commercial kitchen line: ranges, ovens, fryers, a cold room door, sinks and the hood over them.", [("Line, sinks and benches", SS, 1450, ""), ("Hood and duct", SS, 250, "")], 0, 3),
    "office": ("Office bay", "Six workstations with their desks, chairs and cabinets.", [("Desks and cabinets", STEEL, 420, "six"), ("Chairs", AL, 90, "six"), ("Tops and screens", PE, 90, "")], 6, 0),
    "reception": ("Reception bay", "A front desk and waiting chairs: where people are met.", [("Desk", STEEL, 220, ""), ("Waiting chairs", AL, 160, "twelve"), ("Tops and seats", PE, 60, "")], 12, 1),
    "ward": ("Ward bay", "Four beds with their lockers, screens and the gases and call points at their heads.", [("Beds", STEEL, 480, "four, powered"), ("Lockers and screens", AL, 160, ""), ("Bed head trunking", AL, 90, "")], 4, 1),
    "treatment": ("Treatment bay", "Two treatment rooms' couches, trolleys, sinks and cabinets.", [("Couches and trolleys", STEEL, 380, ""), ("Cabinets and sinks", SS, 260, ""), ("Lamps and screens", AL, 60, "")], 4, 2),
    "classroom": ("Classroom bay", "Sixteen desks and chairs, a teacher's desk and a board.", [("Desks and chairs", STEEL, 380, "sixteen of each"), ("Teacher's desk and board", AL, 90, ""), ("Tops and seats", PE, 70, "")], 16, 1),
    "washroom": ("Washroom bay", "Four toilet cubicles, four basins and a shower: a building's washroom.", [("Cubicles", STEEL, 240, ""), ("Sanitaryware", SILICA, 300, "vitreous china, as fused silica here"), ("Taps, pipes and drains", SS, 90, "")], 0, 0),
    "flat": ("Flat bay", "A furnished 36 m2 flat: a bed, a kitchenette, a shower room and a table.", [("Kitchenette and shower", SS, 420, ""), ("Bed, table and storage", STEEL, 520, ""), ("Sanitaryware", SILICA, 120, "vitreous china, as fused silica here"), ("Tops and soft goods", PE, 180, "")], 2, 0),
    "dorm": ("Dormitory bay", "Three double bunks and six lockers.", [("Bunks", STEEL, 360, "three doubles"), ("Lockers", STEEL, 240, "six"), ("Mattresses", PE, 120, "")], 6, 0),
    "cell": ("Cell bay", "Two locked cells, each a bed, a toilet and a basin, caged all round in bars, and the passage outside them.", [("Bars, rails, doors and beds", STEEL, 1548, "20 mm solid bars at about 140 mm, 60 x 60 x 4 mm rails and posts: blender's model (2026-10-17)"), ("Sanitaryware and hardware", SS, 224.7, "stainless, vandal-proof: blender's model")], 2, 0),
    "workshop": ("Workshop bay", "A workbench, a vice, tool racks and a pillar drill: where things are mended.", [("Bench, racks and drill", STEEL, 1200, ""), ("Tools", "material.high-strength-steel-4340", 300, "")], 0, 2),
    "seating": ("Seating bay", "Tiered seating, three rows of twelve, and its stair.", [("Tiers and frames", STEEL, 2200, ""), ("Seats", PE, 300, "thirty-six")], 36, 0),
    "stage": ("Stage bay", "A raised stage deck with its rigging bar and lights.", [("Deck and frame", STEEL, 2400, "1 m high"), ("Rigging and lights", AL, 600, "")], 0, 2),
    "garage": ("Vehicle bay", "An open bay a vehicle stands in, its floor drained and its charge point.", [("Charge point and drain", STEEL, 180, "")], 0, 0),
    "court": ("Court bay", "A bench, a dock, a witness box and seats: a court's room.", [("Bench, dock and box", STEEL, 494.9, "hollow frames, 3 to 4 mm sheet: blender's model (2026-10-17)"), ("Seats", AL, 212.1, "twenty-four: blender's model")], 24, 0),
    "press-room": ("Press room bay", "A small web press, its folder and its paper stand.", [("Press, folder and stand", STEEL, 4200, ""), ("Rollers and plates", AL, 300, "")], 0, 3),
    "garden": ("Garden bay", "A 6 m square of planting, paths and a bench.", [("Bench and edging", STEEL, 160, ""), ("Paths", CONC, 2400, "50 mm of paving on the bay's half")], 8, 0),
}


EXTENT = {"cell": (6.0, 4.0), "counter": (5.4, 2.375), "court": (5.67, 5.63)}       # m the fitted unit covers, where it is less than its 6 m bay (the rest is the bay's free floor): blender's models


def fitout_records():
    for slug, (name, desc, parts, people, staff) in FITOUT.items():
        mass = sum(p[2] for p in parts)
        height = {"store": 4.5, "seating": 2.4, "stage": 3.0, "press-room": 3.0, "court": 1.2}.get(slug, 2.2)
        length, width = EXTENT.get(slug, (BAY, BAY))
        yield slug, {
            "identity": {"key": f"unit.fitout-{slug}", "name": name, "description": desc},
            "size_class": 1, "fits": "plant-mount.bay-s1",
            "physical": {"length": length, "width": width, "height": height, "mass": r(mass)},
            "built_of": [{"what": w, "item": i, "mass": float(m), **({"note": n} if n else {})} for w, i, m, n in parts],
            "basis": [{"of": ["built_of", "physical"], "tier": "invented", "review": True,
                       "note": f"A furnished 6 m bay (its size is what is fitted; the bay, plant-mount.bay-s1, is what it is given): {people} people served or seated, {staff} at work in it; masses from memory of such furniture, to be sourced."}]}


# ------------------------------------------------------------------ what each building type is laid of
MIX = {  # building: (storey height m, {bay: share of the bays}, front, back door kind, partitions a bay)
    "shop": (3.0, {"sales-floor": 0.6, "counter": 0.1, "store": 0.2, "washroom": 0.1}, "shopfront", "door", 0.5),
    "tavern": (3.0, {"dining": 0.6, "counter": 0.1, "kitchen": 0.2, "washroom": 0.1}, "shopfront", "door", 0.5),
    "inn": (3.0, {"dining": 0.2, "kitchen": 0.1, "counter": 0.05, "flat": 0.55, "washroom": 0.1}, "double-door", "door", 1.5),
    "bank": (3.0, {"counter": 0.2, "reception": 0.1, "office": 0.6, "washroom": 0.1}, "shopfront", "door", 1.0),
    "offices": (3.0, {"office": 0.75, "reception": 0.05, "washroom": 0.1, "kitchen": 0.1}, "double-door", "door", 1.0),
    "clinic": (3.0, {"reception": 0.15, "treatment": 0.55, "ward": 0.15, "washroom": 0.15}, "shopfront", "door", 1.5),
    "hospital": (4.5, {"ward": 0.45, "treatment": 0.3, "reception": 0.05, "kitchen": 0.05, "washroom": 0.1, "office": 0.05}, "shopfront", "roller", 1.5),
    "care-home": (3.0, {"ward": 0.5, "dining": 0.15, "kitchen": 0.05, "reception": 0.05, "washroom": 0.15, "office": 0.1}, "double-door", "door", 1.5),
    "school": (3.0, {"classroom": 0.65, "office": 0.1, "dining": 0.1, "kitchen": 0.05, "washroom": 0.1}, "double-door", "door", 1.0),
    "college": (3.0, {"classroom": 0.55, "office": 0.15, "workshop": 0.1, "dining": 0.05, "kitchen": 0.05, "washroom": 0.1}, "double-door", "door", 1.0),
    "flight-school": (4.5, {"classroom": 0.3, "workshop": 0.2, "garage": 0.35, "office": 0.05, "washroom": 0.1}, "double-door", "roller", 0.5),
    "courthouse": (4.5, {"court": 0.4, "office": 0.35, "reception": 0.1, "cell": 0.05, "washroom": 0.1}, "double-door", "door", 1.0),
    "gaol": (3.0, {"cell": 0.7, "office": 0.1, "dining": 0.05, "kitchen": 0.05, "washroom": 0.1}, "door", "door", 1.0),
    "watch-house": (3.0, {"office": 0.4, "reception": 0.15, "cell": 0.2, "garage": 0.15, "washroom": 0.1}, "double-door", "roller", 1.0),
    "press": (4.5, {"press-room": 0.4, "office": 0.35, "store": 0.15, "washroom": 0.1}, "double-door", "roller", 0.5),
    "studio": (4.5, {"stage": 0.3, "office": 0.3, "workshop": 0.2, "store": 0.1, "washroom": 0.1}, "double-door", "roller", 0.5),
    "theatre": (6.0, {"seating": 0.55, "stage": 0.2, "reception": 0.1, "counter": 0.05, "washroom": 0.1}, "shopfront", "roller", 0.3),
    "arena": (6.0, {"seating": 0.6, "stage": 0.2, "counter": 0.05, "reception": 0.05, "washroom": 0.1}, "double-door", "roller", 0.2),
    "market-hall": (6.0, {"sales-floor": 0.55, "counter": 0.2, "store": 0.15, "washroom": 0.1}, "shopfront", "roller", 0.2),
    "repair-shop": (4.5, {"workshop": 0.6, "garage": 0.2, "store": 0.1, "reception": 0.05, "washroom": 0.05}, "double-door", "roller", 0.3),
    "builders-yard": (6.0, {"store": 0.35, "workshop": 0.25, "garage": 0.3, "office": 0.05, "washroom": 0.05}, "double-door", "roller", 0.2),
    "depot": (6.0, {"store": 0.7, "garage": 0.2, "office": 0.05, "washroom": 0.05}, "double-door", "roller", 0.1),
    "bunkhouse": (3.0, {"dorm": 0.75, "washroom": 0.15, "dining": 0.05, "kitchen": 0.05}, "double-door", "door", 1.5),
    "dwelling-block": (3.0, {"flat": 0.9, "washroom": 0.0, "reception": 0.02, "store": 0.08}, "double-door", "door", 1.5),
    "park": (None, {"garden": 1.0}, None, None, 0.0),
}


def counts(n, mix):
    """n bays shared by the mix: largest remainders, every named share at least one where n allows."""
    raw = {k: n * v for k, v in mix.items() if v > 0}
    out = {k: int(v) for k, v in raw.items()}
    for k in raw:
        if out[k] == 0 and sum(out.values()) < n:
            out[k] = 1
    rest = sorted(raw, key=lambda k: raw[k] - int(raw[k]), reverse=True)
    i = 0
    while sum(out.values()) < n and rest:
        out[rest[i % len(rest)]] += 1
        i += 1
    while sum(out.values()) > n:
        k = max(out, key=out.get)
        out[k] -= 1
    return {k: v for k, v in out.items() if v}


def tube(section):
    st = load(os.path.join(M, "mill-stock", section.upper() + ".yaml"))
    return st


def lightest_beam(moment, margin=1.5):
    """The kit's lightest st4340 tube section whose bending stress at `moment` stays under 470 MPa with the margin."""
    best = None
    for f in os.listdir(os.path.join(M, "mill-stock")):
        if f.startswith("ST4340-TB-") and f.endswith("-S3000.yaml"):
            st = load(os.path.join(M, "mill-stock", f))
            sec = tube(st["kit"]["section"].split(".", 1)[1])
            d, t = sec["size"]["diameter"], sec["size"]["wall"]
            Z = math.pi * (d ** 4 - (d - 2 * t) ** 4) / (32 * d)
            if margin * moment / Z <= 470e6 and (best is None or st["physical"]["mass"] < best[0]):
                best = (st["physical"]["mass"], f[:-11].lower())
    return best[1] if best else "st4340-tb-250x20"


def strut(section, diag=False):
    key = f"stock.{section}-{'d' if diag else 's'}3000"
    return key, load(os.path.join(M, "mill-stock", key.split(".", 1)[1].upper() + ".yaml"))


def layout(slug, b):
    sh, mix, front, back, part_per_bay = MIX[slug]
    ph = b["physical"]
    storeys = int(b.get("storeys") or 1)
    floor = float(b.get("floor") or ph["length"] * ph["width"])
    plan = floor / storeys
    nbx = max(1, round(ph["length"] / BAY))
    nby = max(1, round(plan / (BAY * BAY * nbx)))
    L, W = nbx * BAY, nby * BAY
    bays_n = nbx * nby * storeys
    name = b["identity"]["name"]
    mixc = counts(bays_n, mix)
    bays, placed = [], []
    order = [k for k in mixc for _ in range(mixc[k])]
    i = 0
    for s in range(storeys):
        for jy in range(nby):
            for ix in range(nbx):
                if i >= len(order):
                    break
                k = order[i]
                i += 1
                bays.append({"id": f"{k[:3].upper()}{i}", "mount": "plant-mount.bay-s1", "unit": f"unit.fitout-{k}",
                             "at": [r(ix * BAY + BAY / 2 - L / 2, 2), r(jy * BAY + BAY / 2 - W / 2, 2), r(s * (sh or 0), 2)]})
    units = {k: load(os.path.join(M, "units", f"fitout-{k}.yaml")) for k in mixc}
    fit_mass = sum(units[k]["physical"]["mass"] * n for k, n in mixc.items())
    people = sum(FITOUT[k][3] * n for k, n in mixc.items())
    rec = {"identity": {"key": f"layout.building-{slug}", "name": f"{name}, world with air",
                        "description": f"{b['identity']['description']} Laid of the kit: a strut frame on the 3 m grid, panels, and {bays_n} furnished 6 m bays ({', '.join(f'{n} {FITOUT[k][0].lower().replace(' bay', '')}' for k, n in mixc.items())})."},
           "stands_for": b["identity"]["key"], "site": {"atmosphere": "air", "gravity": [8.0, 11.0]}}
    if sh is None:          # (a park: ground, no frame)
        ground = [{"what": "Paths and beds", "item": CONC, "volume": r(L * W * 0.05 * 0.5), "mass": r(L * W * 0.05 * 0.5 * 2400), "note": "paving on half the park's ground, 50 mm"}]
        total = fit_mass + ground[0]["mass"]
        rec.update({"physical": {"length": L, "width": W, "height": 2.2, "mass": r(total)}, "bays": bays, "ground": ground,
                    "totals": {"mass": r(total), "against_module": f"{b['identity']['key']} (deprecated): one box {ph['length']:g} x {ph['width']:g} m. This layout: {L:g} x {W:g} m of gardens."},
                    "staff": b.get("staff") or [],
                    "basis": [{"of": ["bays"], "tier": "invented", "review": True, "note": "A park is its ground and gardens: no frame or panels."}]})
        return rec
    H = sh * storeys
    nx, ny, rows = round(L / CELL), round(W / CELL), round(sh / CELL * 2) / 2      # (rows: panel rows a storey, 3 m panels, a 1.5 m row as 3 x 1.5)
    nz = round(H / CELL)
    # Panels: the perimeter faces a storey, by side
    full_rows, half_row = int(rows), rows - int(rows) > 0
    per_side = {"front": nx, "back": nx, "side": 2 * ny}
    panels = {}

    def add(k, n):
        if n > 0:
            panels[k] = panels.get(k, 0) + n
    for s in range(storeys):
        for side, n in per_side.items():
            faces = n * full_rows
            special = 0
            if s == 0 and side == "front" and front:
                k = {"shopfront": "wall-shopfront-3x3", "double-door": "wall-double-door-3x3", "door": "wall-door-3x3"}[front]
                nd = max(1, nx // 6)
                add(k, nd)
                special += nd
                if front == "shopfront":
                    add("wall-glazed-3x3", n - nd)
                    special = n
            if s == 0 and side == "back" and back:
                k = {"roller": "wall-roller-3x3", "door": "wall-door-3x3"}[back]
                nd = max(1, nx // 8)
                add(k, nd)
                special += nd
            rest = faces - special
            win = (n - (special if special <= n else n)) // 2 if side != "back" else 0
            add("wall-window-3x3", win)
            add("wall-insulated-3x3", rest - win)
            if half_row:
                add("wall-insulated-3x1p5", n)
    plan_cells = nx * ny
    stairs = 0 if storeys == 1 else (storeys - 1) * max(1, round(plan / 600))
    add("floor-3x3", plan_cells * (storeys - 1) - 2 * stairs)
    add("stair-3m", stairs)
    add("roof-3x3", plan_cells)
    nparts = round(part_per_bay * bays_n * 2)
    add("partition-3x3", nparts * full_rows - round(part_per_bay * bays_n))
    add("partition-door-3x3", round(part_per_bay * bays_n))
    panel_mass = sum(pmass(k) * n for k, n in panels.items())
    # Frame: columns at every grid node, beams on every cell edge at each floor and the roof, half the outside faces braced
    nodes_plan = (nx + 1) * (ny + 1)
    columns = nodes_plan * nz
    levels = storeys
    beams = (nx * (ny + 1) + ny * (nx + 1)) * levels
    braces = (2 * (nx + ny) * nz) // 2
    nodes = nodes_plan * (nz + 1)
    dead = (pmass("floor-3x3") / 9.0 + 25.0) * G         # Pa: a floor panel and services, fittings and partitions (25 kg/m2, invented)
    live = 5000.0 if slug not in ("dwelling-block", "bunkhouse", "inn", "care-home") else 2000.0
    roof = (pmass("roof-3x3") / 9.0) * G + 1000.0
    col_force = CELL * CELL * ((storeys - 1) * (dead + live) + roof) + CELL * sh * storeys * (pmass("wall-insulated-3x3") / 9.0) * G
    beam_moment = (dead + live) * CELL * CELL ** 2 / 8
    sec_col = frames.lightest_section(col_force)
    sec_beam = lightest_beam(beam_moment)
    sec_brace = frames.lightest_section(0.25 * col_force, with_diagonal=True)
    members, fmass = [], 0.0
    for sec, n, where, diag in ((sec_col, columns, "columns", False), (sec_beam, beams, "beams", False), (sec_brace, braces, "bracing", True)):
        key, st = strut(sec, diag)
        members.append({"item": key, "count": int(n), "where": where})
        fmass += n * st["physical"]["mass"]
    node_key = strut(sec_col)[1]["kit"]["node"]
    node = load(os.path.join(M, "mill-stock", node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": int(nodes), "where": "nodes"})
    fmass += nodes * frames.node_mass(node, frames.materials())
    slab = L * W * 0.15
    strip = 2 * (L + W) * 0.6 * 0.6
    ground = [{"what": "Ground slab", "item": CONC, "volume": r(slab), "mass": r(slab * 2400), "note": f"0.15 m over the {L:g} x {W:g} m plan"},
              {"what": "Edge footings", "item": CONC, "volume": r(strip), "mass": r(strip * 2400), "note": "0.6 x 0.6 m strips under the outside walls"}]
    if storeys > 4:
        pads = nodes_plan * (0.3 * storeys) ** 2 * 0.6
        ground.append({"what": "Column pads", "item": CONC, "volume": r(pads), "mass": r(pads * 2400), "note": f"a pad under each column, {0.3 * storeys:.1f} m square, 0.6 m deep (sized by the storeys, invented)"})
    gmass = sum(g["mass"] for g in ground)
    total = fit_mass + panel_mass + fmass + gmass
    old = b["physical"]["mass"]
    rec.update({
        "physical": {"length": L, "width": W, "height": r(H + 0.15, 2), "mass": r(total)},
        "bays": bays,
        "frame": {"box": [-L / 2, -W / 2, 0.0, L / 2, W / 2, r(H, 2)], "cell": CELL,
                  "shape": f"A skeleton frame on the 3 m grid: {nx} x {ny} cells, {storeys} storey(s) of {sh:g} m; columns at every node, beams on every cell edge at each floor and the roof, one diagonal in every other outside face. Panels on its faces (frame.panels).",
                  "members": members, "mass": r(fmass),
                  "panels": [{"panel": f"panel.{k}", "count": int(n)} for k, n in sorted(panels.items())],
                  "panels_mass": r(panel_mass),
                  "sized": f"At 1 g: floors {dead:.0f} Pa dead and {live:.0f} Pa imposed, the roof {roof:.0f} Pa; a ground column carries {col_force / 1e3:.0f} kN ({sec_col} takes it with half again to spare); a 3 m beam bends with {beam_moment / 1e3:.1f} kN m ({sec_beam}, under 470 MPa with half again to spare). Wind, the tall halls' columns between floors and the footings' loads not yet checked."},
        "ground": ground,
        "totals": {"mass": r(total), "against_module": f"{b['identity']['key']} (deprecated): one box {ph['length']:g} x {ph['width']:g} x {ph['height']:g} m of {old / 1e3:,.0f} t, from the average of 538 concrete buildings. This layout: {L:g} x {W:g} x {H:g} m, {total / 1e3:,.0f} t ({fmass / 1e3:,.0f} t frame, {panel_mass / 1e3:,.0f} t panels, {fit_mass / 1e3:,.0f} t fit-out, {gmass / 1e3:,.0f} t ground), for {people} people served or seated at once. Services (power, water, drains) not laid: the kit has no low-voltage cable or small pipe yet."},
        "staff": b.get("staff") or [],
        "basis": [{"of": ["bays"], "tier": "invented", "review": True, "note": f"The mix of bays for a {name.lower()} is chosen ({', '.join(f'{k} {v:g}' for k, v in mix.items() if v)}), shared over the record's floor ({floor:g} m2 in {storeys} storeys, laid as {nbx} x {nby} bays of 6 m)."},
                  {"of": ["frame", "ground"], "tier": "derived", "review": True, "note": "Counted from the grid (tools/standards/building_layouts.py); sections by the loads in frame.sized; masses from the strut, node and panel records."}]})
    return rec


KEEP = ("visual", "model_requests")      # (what the model installer and the asset tools write into a record: kept across a rerun)


def write(folder, slug, d, schema):
    path = os.path.join(M, folder, slug + ".yaml")
    if os.path.exists(path):
        old = yaml.safe_load(open(path, encoding="utf-8")) or {}
        d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
    text = f"# yaml-language-server: $schema=../../schema/{schema}.schema.yaml\n" + HEAD + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
    if not os.path.exists(path) or open(path).read() != text:
        open(path, "w").write(text)
        return 1
    return 0


def main():
    n = 0
    for slug, d in fitout_records():
        n += write("units", f"fitout-{slug}", d, "unit")
    rows = []
    for f in sorted(glob.glob(os.path.join(M, "buildings", "*.yaml"))):
        slug = os.path.basename(f)[:-5]
        b = load(f)
        d = layout(slug, b)
        n += write("layouts", f"building-{slug}", d, "layout")
        rows.append(f"  {slug:<15} {d['physical']['length']:>5g} x {d['physical']['width']:<4g} x {d['physical']['height']:<6g} {d['physical']['mass'] / 1e3:>9,.0f} t  (record {b['physical']['mass'] / 1e3:>9,.0f} t)")
    print("\n".join(rows))
    print(f"{len(FITOUT)} fit-out units, {len(rows)} building layouts; {n} written")


if __name__ == "__main__":
    main()
