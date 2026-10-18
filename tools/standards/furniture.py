"""Furniture and rooms (the owner, 2026-10-17: one model a piece, a table, a chair, a bed; the layout places it as
many times as a room needs, never a furnished set as one model).

- FURNITURE: single pieces, written as units (`unit.furniture-<name>`), each modelled once;
- ARRANGEMENTS: how a 6 m room is furnished, as placements of those pieces (`arrangement.<name>`); building
  layouts place arrangements in their rooms (building_layouts.py).

Masses: the table and chair from blender's dining set (2026-10-17), the counter, court pieces and cell from its bay
models; the rest from memory, to be sourced.
"""
import math, os

STEEL, SS, AL, PE, SILICA, CONC, CI = ("material.structural-steel-a36", "material.stainless-steel-304", "material.aluminium-alloy-6061",
                                       "material.polyethylene-uhmw", "material.fused-silica", "good.concrete", "material.cast-iron-grey")
BLENDER = "blender's model (2026-10-17)"

FURNITURE = {  # name: (title, description, (length, width, height), [(what, item, kg)], source)
    "table": ("Table", "A table for four, 1.1 m square.", (1.1, 1.1, 0.75), [("Frame", AL, 9.9), ("Top", PE, 22.7)], "blender's dining set: four tables 39.7 kg of aluminium, 20 mm tops"),
    "chair": ("Chair", "A plain chair with a seat and back.", (0.45, 0.5, 0.85), [("Frame", AL, 2.6), ("Seat and back", PE, 3.5)], "blender's dining set: sixteen chairs 41.2 kg of aluminium, 10 mm seats"),
    "desk": ("Desk", "A desk 1.6 m by 0.8 m.", (1.6, 0.8, 0.75), [("Frame", STEEL, 22.0), ("Top", PE, 13.0)], None),
    "office-chair": ("Office chair", "A swivel chair on castors.", (0.65, 0.571, 1.0), [("Base, spindle and frame", STEEL, 14.2), ("Seat and back", PE, 5.9)], "blender's model (2026-10-17): hollow 25 and 30 mm frame, solid hub and spindle, five castors"),
    "pupil-desk": ("Pupil desk", "A single pupil's desk.", (0.7, 0.5, 0.75), [("Frame", STEEL, 9.0), ("Top", PE, 4.5)], "of the order of blender's classroom: 300 kg of steel for sixteen sets and the teacher's"),
    "cabinet": ("Cabinet", "A steel filing or storage cabinet.", (0.8, 0.45, 1.2), [("Carcass and drawers", STEEL, 40.0)], None),
    "board": ("Board", "A wall board for a classroom or a briefing room.", (2.4, 0.1, 1.2), [("Board and frame", AL, 25.0)], None),
    "bed": ("Bed", "A single bed with its mattress.", (2.1, 1.0, 0.6), [("Frame", STEEL, 35.0), ("Mattress", PE, 15.0)], None),
    "bed-ward": ("Ward bed", "A powered hospital bed with rails.", (2.2, 1.0, 0.7), [("Frame, rails and motors", STEEL, 105.0), ("Mattress", PE, 18.0)], None),
    "bunk": ("Bunk", "A double bunk with its two mattresses.", (2.0, 0.9, 1.7), [("Frame", STEEL, 60.0), ("Mattresses", PE, 30.0)], None),
    "locker": ("Locker", "A full-height steel locker.", (0.3, 0.5, 1.8), [("Carcass", STEEL, 25.0)], None),
    "shelf": ("Shelf section", "A double-sided gondola shelf section, 1.2 m long.", (1.2, 1.0, 1.8), [("Uprights, base and shelves", STEEL, 70.0)], None),
    "counter": ("Counter section", "A serving counter section 1.8 m long with its worktop.", (1.8, 0.8, 1.05), [("Carcass", STEEL, 85.0), ("Worktop", PE, 32.0)], "blender's counter bay: 5.4 m of counter and back shelving 427 kg of steel, 96 kg of worktop"),
    "back-shelf": ("Back shelf", "Shelving behind a counter, 1.8 m long.", (1.8, 0.45, 2.2), [("Frame and shelves", STEEL, 57.0)], "blender's counter bay"),
    "till": ("Till", "A till with its drawer and screen.", (0.5, 0.409, 0.4), [("Case, drawer and stand", AL, 7.7), ("Screen", "material.soda-lime-glass", 0.2), ("Boards, printer and supply", PE, 3.0)], "blender's model (2026-10-17) for the case, drawer, stand and screen, 7.9 kg; the electronics inside it 3 kg (invented, as polymer here)"),
    "rack": ("Pallet rack bay", "One bay of pallet racking, 2.7 m long, three beam levels.", (2.7, 1.1, 4.5), [("Uprights and beams", STEEL, 210.0)], None),
    "range": ("Range", "A commercial cooking range with its oven.", (1.2, 0.9, 0.9), [("Range and oven", SS, 250.0)], None),
    "kitchen-bench": ("Kitchen bench", "A stainless kitchen bench with a shelf under it.", (1.8, 0.7, 0.9), [("Bench", SS, 60.0)], None),
    "sink": ("Sink unit", "A double stainless sink on its stand.", (1.2, 0.7, 0.9), [("Sink and stand", SS, 31.5)], "blender's model (2026-10-17): two 1.2 mm bowls and top, a 30 x 30 x 1.5 mm stand"),
    "hood": ("Kitchen hood", "An extract hood over a cooking line.", (3.0, 1.2, 0.6), [("Hood and filters", SS, 120.0)], None),
    "fridge": ("Fridge", "An upright commercial fridge.", (0.7, 0.8, 2.0), [("Cabinet", SS, 110.0)], None),
    "wc": ("Toilet", "A toilet pan with its cistern and seat.", (0.4, 0.7, 0.8), [("Pan and cistern", SILICA, 34.7), ("Seat and lid", PE, 1.3)], "blender's model (2026-10-17): hollow bowl and pedestal, 8 mm cistern, 10 mm lid"),
    "cubicle": ("Toilet cubicle", "A cubicle's three walls and door round one toilet.", (0.9, 1.5, 2.0), [("Panels, door and posts", STEEL, 60.0)], None),
    "basin": ("Basin", "A wash basin with its tap.", (0.6, 0.45, 0.85), [("Basin", SILICA, 15.0), ("Tap and trap", SS, 2.0)], None),
    "shower": ("Shower", "A shower tray, screen and mixer.", (0.9, 0.9, 2.1), [("Tray, screen and mixer", SS, 50.0)], None),
    "reception-desk": ("Reception desk", "A front desk 2.4 m long.", (2.4, 0.8, 1.1), [("Carcass", STEEL, 120.0), ("Top", PE, 30.0)], None),
    "couch": ("Treatment couch", "An adjustable treatment couch.", (1.9, 0.7, 0.75), [("Frame", STEEL, 70.0), ("Pad", PE, 10.0)], None),
    "trolley": ("Trolley", "A stainless instrument trolley on four castors.", (0.8, 0.5, 0.9), [("Frame, trays and yokes", SS, 21.7), ("Castor wheels", "material.epdm-rubber", 2.0)], "blender's model (2026-10-17): 25 x 25 x 1.5 mm tube, two 1.5 mm trays; its four wheels, drawn as solid proxies, as about 2 kg of rubber-tyred wheels (invented)"),
    "cell-front": ("Cell front", "A barred cell front 3 m wide with its locked door: solid 20 mm bars about 140 mm apart.", (3.0, 0.1, 2.2), [("Bars, rails and door", STEEL, 330.0)], "blender's cell bay: a full cage of bars 1,548 kg of steel for two cells"),
    "cell-wall": ("Cell wall", "A barred wall between or behind cells, 4 m long.", (4.0, 0.1, 2.2), [("Bars and rails", STEEL, 300.0)], "blender's cell bay"),
    "steel-wc": ("Steel toilet", "A stainless vandal-proof toilet and basin in one.", (0.6, 0.6, 0.9), [("Toilet and basin", SS, 55.0)], "blender's cell bay: 225 kg of stainless for two cells"),
    "seat-row": ("Seating tier", "A tier of six seats 3.6 m long on its 0.9 m step.", (3.6, 0.9, 0.5), [("Tier frame", STEEL, 180.0), ("Seats", PE, 50.0)], None),
    "stage-deck": ("Stage deck", "A stage deck module 2 m by 1 m, 1 m high.", (2.0, 1.0, 1.0), [("Deck and legs", STEEL, 120.0)], None),
    "workbench": ("Workbench", "A steel workbench with a vice.", (2.0, 0.8, 0.9), [("Bench and vice", STEEL, 120.0)], None),
    "tool-rack": ("Tool rack", "A wall rack of tools.", (2.0, 0.4, 2.0), [("Rack", STEEL, 63.6), ("Tools", "material.high-strength-steel-4340", 67.7)], "blender's model (2026-10-17): 40 x 40 x 2 mm frame, backboard and tray; spanners, sledges and bars"),
    "drill": ("Pillar drill", "A pillar drill.", (0.6, 0.505, 1.8), [("Base, column, table and head", CI, 228.7)], "blender's model (2026-10-17): cast base, 120 x 12 mm column, 35 mm table; of the order of a floor-standing drill's 150 to 300 kg"),
    "charge-point": ("Charge point", "A vehicle charge point on its post.", (0.3, 0.3, 1.4), [("Post and charger", STEEL, 30.0)], None),
    "judge-bench": ("Judge's bench", "A raised bench for a judge and clerk.", (3.0, 0.9, 1.2), [("Bench", STEEL, 230.0)], "blender's court bay: bench, dock and witness box 495 kg of steel"),
    "dock": ("Dock", "A dock for the accused, standing.", (2.4, 1.2, 1.1), [("Dock", STEEL, 170.0)], "blender's court bay"),
    "witness-box": ("Witness box", "A witness box.", (1.2, 1.2, 1.1), [("Box", STEEL, 95.0)], "blender's court bay"),
    "web-press": ("Web press", "A small web press, its folder and its paper stand.", (5.0, 2.5, 2.5), [("Press, folder and stand", STEEL, 4200.0), ("Rollers and plates", AL, 300.0)], None),
    "bench": ("Park bench", "A bench for three.", (1.8, 0.6, 0.8), [("Frame and slats", STEEL, 40.0)], None),
    "planter": ("Planter", "A planting bed 2 m square in a concrete kerb.", (2.0, 2.0, 0.6), [("Kerb", CONC, 900.0)], None),
}


def grid(piece, n, cols, x0, y0, dx, dy, turn=0.0):
    out = []
    for i in range(n):
        out.append((piece, x0 + (i % cols) * dx, y0 + (i // cols) * dy, turn))
    return out


ARRANGEMENTS = {  # name: (title, [(piece, x, y, turn)], people served or seated, staff at work)
    "dining": ("Dining room", sum(([("table", x, y, 0)] + [("chair", x + a, y + b, t) for a, b, t in ((-0.8, 0, 90), (0.8, 0, 270), (0, -0.8, 0), (0, 0.8, 180))] for x in (-1.5, 1.5) for y in (-1.5, 1.5)), []), 16, 0),
    "sales-floor": ("Sales floor", grid("shelf", 6, 2, -1.5, -2.4, 3.0, 1.9) , 12, 0),
    "counter": ("Counter", [("counter", x, -0.6, 0) for x in (-1.8, 0.0, 1.8)] + [("back-shelf", x, 1.2, 180) for x in (-1.8, 0.0, 1.8)] + [("till", -1.8, -0.6, 0), ("till", 1.8, -0.6, 0)], 6, 2),
    "store": ("Store room", grid("rack", 4, 2, -1.4, -1.6, 2.8, 3.2), 0, 0),
    "kitchen": ("Kitchen", [("range", -2.2, -2.4, 0), ("range", -1.0, -2.4, 0), ("hood", -1.6, -2.4, 0), ("kitchen-bench", 1.2, -2.4, 0), ("kitchen-bench", -1.5, 0.6, 0), ("kitchen-bench", 1.0, 0.6, 0), ("sink", 2.2, 2.4, 180), ("fridge", -2.4, 2.4, 180), ("fridge", -1.6, 2.4, 180)], 0, 3),
    "office": ("Office", sum(([("desk", x, y, 0), ("office-chair", x, y + 0.7, 180)] for x in (-1.9, 0.0, 1.9) for y in (-1.6, 1.2)), []) + [("cabinet", -2.4, 2.7, 180), ("cabinet", 2.4, 2.7, 180)], 6, 0),
    "reception": ("Reception", [("reception-desk", 0.0, -1.8, 0), ("office-chair", 0.0, -2.5, 0)] + grid("chair", 12, 6, -2.0, 1.0, 0.8, 1.2, 180), 12, 1),
    "ward": ("Ward", sum(([("bed-ward", x, y, 0), ("locker", x + 0.75, y - 0.7, 0)] for x in (-1.5, 1.5) for y in (-1.7, 1.7)), []), 4, 1),
    "treatment": ("Treatment rooms", sum(([("couch", x, 0.5, 0), ("trolley", x + 1.0, -0.6, 0), ("sink", x, -2.4, 0), ("cabinet", x - 1.0, 2.6, 180)] for x in (-1.5, 1.5)), []), 4, 2),
    "classroom": ("Classroom", sum(([("pupil-desk", x, y, 0), ("chair", x, y + 0.5, 180)] for x in (-2.1, -0.7, 0.7, 2.1) for y in (-1.2, 0.0, 1.2, 2.4)), []) + [("desk", 0.0, -2.4, 0), ("board", 0.0, -2.95, 0)], 16, 1),
    "washroom": ("Washroom", grid("cubicle", 4, 4, -1.35, -2.2, 0.9, 0) + grid("wc", 4, 4, -1.35, -2.4, 0.9, 0) + grid("basin", 4, 4, -1.35, 0.8, 0.9, 0, 180) + [("shower", 2.4, 2.4, 180)], 0, 0),
    "flat": ("Flat", [("bed", -1.8, 1.8, 0), ("kitchen-bench", 1.8, -2.5, 0), ("sink", 0.4, -2.5, 0), ("table", 1.0, 0.6, 0), ("chair", 0.2, 0.6, 90), ("chair", 1.8, 0.6, 270), ("wc", 2.4, 2.2, 180), ("basin", 1.5, 2.6, 180), ("shower", 2.4, 1.0, 180), ("cabinet", -2.5, -1.0, 90)], 2, 0),
    "dorm": ("Dormitory", grid("bunk", 3, 3, -1.9, -1.8, 1.9, 0) + grid("locker", 6, 6, -2.0, 2.6, 0.5, 0, 180), 6, 0),
    "cell": ("Cells", [("cell-front", -1.5, -1.0, 0), ("cell-front", 1.5, -1.0, 0), ("cell-wall", 0.0, 1.0, 90), ("bed", -2.0, 1.5, 90), ("bed", 2.0, 1.5, 90), ("steel-wc", -0.5, 2.5, 180), ("steel-wc", 0.5, 2.5, 180)], 2, 0),
    "workshop": ("Workshop", [("workbench", -1.6, -2.4, 0), ("workbench", 1.6, -2.4, 0), ("tool-rack", -1.6, 2.7, 180), ("tool-rack", 1.6, 2.7, 180), ("drill", 2.5, 0.0, 270)], 0, 2),
    "seating": ("Seating", [("seat-row", x, y, 0) for x in (-1.0,) for y in (-1.8, -0.9, 0.0, 0.9, 1.8, 2.7)], 36, 0),
    "stage": ("Stage", grid("stage-deck", 9, 3, -2.0, -1.0, 2.0, 1.0), 0, 2),
    "garage": ("Vehicle bay", [("charge-point", 2.7, 2.7, 180)], 0, 0),
    "court": ("Court room", [("judge-bench", 0.0, -2.3, 0), ("dock", -2.0, 0.0, 90), ("witness-box", 2.2, -0.6, 270)] + grid("chair", 22, 11, -2.5, 1.6, 0.5, 0.8, 180) + [("chair", -0.6, -2.6, 0), ("chair", 2.2, -0.2, 270)], 24, 0),
    "press-room": ("Press room", [("web-press", 0.0, 0.0, 0)], 0, 3),
    "garden": ("Garden", [("planter", -1.5, -1.5, 0), ("planter", 1.5, 1.5, 0), ("bench", 1.5, -1.8, 0), ("bench", -1.5, 1.8, 180)], 8, 0),
}


def unit_record(name):
    title, desc, (l, w, h), parts, source = FURNITURE[name]
    return {"identity": {"key": f"unit.furniture-{name}", "name": title, "description": desc},
            "size_class": 1, "physical": {"length": l, "width": w, "height": h, "mass": round(sum(p[2] for p in parts), 1)},
            "built_of": [{"what": a, "item": b, "mass": float(c)} for a, b, c in parts],
            "basis": [{"of": ["built_of", "physical"], "tier": "derived" if source else "invented", "review": True,
                       "note": f"From {source}." if source else "From memory of such furniture; to be sourced."}]}


def arrangement_record(name):
    title, items, people, staff = ARRANGEMENTS[name]
    mass = sum(sum(p[2] for p in FURNITURE[i[0]][3]) for i in items)
    counts = {}
    for i in items:
        counts[i[0]] = counts.get(i[0], 0) + 1
    return {"identity": {"key": f"arrangement.{name}", "name": title,
                         "description": f"How a 6 m room is furnished as a {title.lower()}: " + ", ".join(f"{n} {FURNITURE[k][0].lower()}{'s' if n > 1 else ''}" for k, n in counts.items()) + "."},
            "room": [6.0, 6.0], "people": people, "staff": staff,
            "items": [{"unit": f"unit.furniture-{p}", "at": [round(x, 2), round(y, 2), 0.0], **({"turn": float(t)} if t else {})} for p, x, y, t in items],
            "mass": round(mass, 1),
            "basis": [{"of": ["items"], "tier": "invented", "review": True, "note": "Laid out by hand on the 6 m room (tools/standards/furniture.py); each piece is modelled once and placed here."}]}


def arrangement_mass(name):
    return arrangement_record(name)["mass"]
