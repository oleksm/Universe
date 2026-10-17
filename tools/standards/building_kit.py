"""The building kit (the owner, 2026-10-17: a building is a layout laid of a frame, panels, fit-out units and service
kit, not one model). Panels on the strut frame's 3 m grid for a world with air: walls (insulated, glazed, with a
window, a door, double doors, a shop front, a roller shutter), partitions, floors, roofs and a stair; one template a
family; each panel sold as a stock piece.

    python3 tools/standards/building_kit.py        # write them (idempotent)

Figures are from memory of modular building practice (steel-faced PIR sandwich panels, aluminium curtain walling,
steel floor cassettes with a concrete topping); every record says so.
"""
import os, sys
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from packages import FORMS as PACKAGE_OF  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata")
HEAD = "# Written by tools/standards/building_kit.py: change it there.\n"
STEEL, AL, PIR, GLASS, RUB, CONC = ("material.structural-steel-a36", "material.aluminium-alloy-6061", "material.pir-foam",
                                    "material.soda-lime-glass", "material.epdm-rubber", "good.concrete")
CELL = 3.0
MEMORY = "From memory of modular building practice; to be sourced."


def r(x, nd=1):
    return round(float(x), nd)


def sandwich(area, pir):
    """Two 0.5 mm steel faces on PIR `pir` m thick: (parts, kg, U)."""
    faces = area * 2 * 0.0005 * 7850
    foam = area * pir * 32
    return [("Steel faces", STEEL, faces, "two 0.5 mm coated steel skins"), ("Insulation core", PIR, foam, f"{pir * 1000:g} mm of PIR")], faces + foam, 0.022 / pir


GLAZED_KG_M2 = 42.0      # kg/m2 of curtain walling: an aluminium frame of about 12 and double glazing of two 6 mm panes, 30


def glazed(area):
    return [("Glazing", GLASS, area * 30.0, "double glazing, two 6 mm panes"), ("Mullions and transoms", AL, area * 12.0, "aluminium curtain wall frame"), ("Seals", RUB, area * 0.6, "EPDM gaskets")], area * GLAZED_KG_M2


PANELS = []


def panel(slug, name, kind, cell, thick, parts, desc, template, opening=None, u=None, load=None, basis=MEMORY):
    mass = sum(p[2] for p in parts)
    if kind in ("wall", "partition"):
        ph = {"length": cell[0], "width": thick, "height": cell[1], "mass": r(mass)}
    elif kind == "stair":
        ph = {"length": cell[0], "width": cell[1], "height": CELL, "mass": r(mass)}
    else:
        ph = {"length": cell[0], "width": cell[1], "height": thick, "mass": r(mass)}
    d = {"identity": {"key": f"panel.{slug}", "name": name, "description": desc}, "kind": kind, "site": "air", "cell": list(cell), "thickness": thick}
    if opening:
        d["opening"] = opening
    if u:
        d["u_value"] = r(u, 2)
    if load:
        d["load"] = load
    d["physical"] = ph
    d["built_of"] = [{"what": w, "item": i, "mass": r(m), **({"note": n} if n else {})} for w, i, m, n in parts]
    d["template"] = f"template.{template}"
    d["basis"] = [{"of": ["built_of", "physical", "u_value", "load"], "tier": "invented", "review": True, "note": basis}]
    PANELS.append((slug, d))


def frame_parts(area, kg_m2=1.5):
    return [("Edge frame and fixings", STEEL, area * kg_m2, "its edge channel and the bolts to the frame's nodes")]


for w, h, sl in ((3.0, 3.0, "3x3"), (1.5, 3.0, "1p5x3"), (3.0, 1.5, "3x1p5")):
    a = w * h
    sp, _, u = sandwich(a, 0.1)
    panel(f"wall-insulated-{sl}", f"Wall panel, insulated, {w:g} x {h:g} m", "wall", (w, h), 0.1, sp + frame_parts(a),
          "A steel-faced insulated sandwich panel bolted across one cell face of the frame: the outside wall of a building on a world with air.", "wall-insulated", u=u)
gp, _ = glazed(9.0)
panel("wall-glazed-3x3", "Wall panel, glazed, 3 x 3 m", "wall", (3.0, 3.0), 0.15, gp + frame_parts(9.0, 2.0),
      "A cell face of aluminium curtain walling and double glazing: a shop front, an office's daylight.", "wall-glazed",
      opening={"kind": "glazing", "width": 2.8, "height": 2.8, "sill": 0.1}, u=1.4)
for slug, name, op, extra, desc in (
        ("wall-window-3x3", "Wall panel with a window, 3 x 3 m", {"kind": "window", "width": 1.8, "height": 1.5, "sill": 0.9}, None, "An insulated wall panel with a 1.8 x 1.5 m double-glazed window."),
        ("wall-door-3x3", "Wall panel with a door, 3 x 3 m", {"kind": "door", "width": 1.0, "height": 2.1, "sill": 0.0, "people": True}, ("Door leaf and frame", STEEL, 45.0, "an insulated steel door, its frame, lock and closer"), "An insulated wall panel with a 1.0 x 2.1 m steel door: a way in for people."),
        ("wall-double-door-3x3", "Wall panel with double doors, 3 x 3 m", {"kind": "double-door", "width": 2.0, "height": 2.4, "sill": 0.0, "people": True}, ("Door leaves and frame", STEEL, 95.0, "a pair of insulated steel leaves, frame and panic bar"), "An insulated wall panel with a 2.0 x 2.4 m pair of steel doors: a public way in, or stock by hand truck."),
        ("wall-roller-3x3", "Wall panel, roller shutter, 3 x 3 m", {"kind": "roller-shutter", "width": 2.8, "height": 2.8, "sill": 0.0, "vehicles": True}, None, "A full cell face closed by a steel roller shutter on its drum and motor: stock in by forklift or truck.")):
    opa = op["width"] * op["height"]
    if slug == "wall-window-3x3":
        sp, _, u = sandwich(9.0 - opa, 0.1)
        gp2, _ = glazed(opa)
        parts = sp + gp2 + frame_parts(9.0)
        uu = ((9.0 - opa) * u + opa * 1.4) / 9.0
    elif slug == "wall-roller-3x3":
        parts = [("Shutter slats", STEEL, opa * 10.0, "steel slats, 10 kg/m2"), ("Drum, guides and motor", STEEL, 60.0, "")] + frame_parts(9.0, 2.5)
        uu = 3.0
    else:
        sp, _, u = sandwich(9.0 - opa, 0.1)
        parts = sp + [extra] + frame_parts(9.0, 2.0)
        uu = ((9.0 - opa) * u + opa * 1.5) / 9.0
    panel(slug, name, "wall", (3.0, 3.0), 0.1, parts, desc, slug.replace("-3x3", ""), opening=op, u=uu)
gp, _ = glazed(9.0 - 4.8)
panel("wall-shopfront-3x3", "Shop front, 3 x 3 m", "wall", (3.0, 3.0), 0.15, gp + [("Sliding doors and operator", AL, 180.0, "a pair of glazed sliding leaves, its track and motor")] + frame_parts(9.0, 2.0),
      "Curtain walling round a 2.0 x 2.4 m pair of automatic sliding doors: a shop's or a clinic's public front.", "wall-shopfront",
      opening={"kind": "sliding-door", "width": 2.0, "height": 2.4, "sill": 0.0, "people": True}, u=1.6)
sp, _, _ = sandwich(9.0, 0.05)
panel("partition-3x3", "Partition panel, 3 x 3 m", "partition", (3.0, 3.0), 0.05, sp + frame_parts(9.0, 1.0),
      "A thin steel-faced panel dividing rooms inside: no weather, some sound.", "partition")
sp, _, _ = sandwich(9.0, 0.05)
panel("partition-door-3x3", "Partition panel with a door, 3 x 3 m", "partition", (3.0, 3.0), 0.05, sandwich(9.0 - 2.1, 0.05)[0] + [("Door leaf and frame", STEEL, 30.0, "a light steel door")] + frame_parts(9.0, 1.0),
      "A partition panel with a 1.0 x 2.1 m door between rooms.", "partition-door", opening={"kind": "door", "width": 1.0, "height": 2.1, "sill": 0.0, "people": True})
panel("floor-3x3", "Floor panel, 3 x 3 m", "floor", (3.0, 3.0), 0.2,
      [("Joists and deck", STEEL, 9.0 * 24.0, "a cold-formed steel cassette: joists 15 kg/m2 and a 0.9 mm deck 9"), ("Topping", CONC, 9.0 * 0.06 * 2400, "60 mm of concrete")],
      "A steel floor cassette with a concrete topping spanning one cell between the frame's nodes: an upper floor, or a floor laid over the ground slab's service void.", "floor", load=5000.0)
sp, _, u = sandwich(9.0, 0.15)
panel("roof-3x3", "Roof panel, 3 x 3 m", "roof", (3.0, 3.0), 0.15, sp + [("Membrane", RUB, 9.0 * 1.7, "1.5 mm EPDM")] + frame_parts(9.0),
      "An insulated sandwich roof panel on the frame's top cells, a rubber membrane over it, laid to fall to its gutters.", "roof", u=u, load=1000.0)
panel("stair-3m", "Stair, 3 m rise", "stair", (3.0, 6.0), 0.3,
      [("Stringers, treads and landing", STEEL, 560.0, "two flights of 1.2 m with a half landing"), ("Handrails", STEEL, 90.0, "")],
      "A steel stair between floors 3 m apart, in a stair well of two cells (3 x 6 m).", "stair")

TEMPLATES = {"wall-insulated": "Insulated wall panels", "wall-glazed": "Glazed wall panels", "wall-window": "Wall panels with a window",
             "wall-door": "Wall panels with a door", "wall-double-door": "Wall panels with double doors", "wall-roller": "Roller shutter wall panels",
             "wall-shopfront": "Shop fronts", "partition": "Partition panels", "partition-door": "Partition panels with a door",
             "floor": "Floor panels", "roof": "Roof panels", "stair": "Stairs"}


def write(folder, slug, d, schema):
    path = os.path.join(M, folder, slug + ".yaml")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    text = f"# yaml-language-server: $schema=../../schema/{schema}.schema.yaml\n" + HEAD + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
    if not os.path.exists(path) or open(path).read() != text:
        open(path, "w").write(text)
        return 1
    return 0


def main():
    n = 0
    members = {}
    for slug, d in PANELS:
        n += write("panels", slug, d, "panel")
        members.setdefault(d["template"], []).append(d["identity"]["key"])
    for t, name in TEMPLATES.items():
        ms = members.get(f"template.{t}", [])
        d = {"identity": {"key": f"template.{t}", "name": name, "description": f"{name}: one look for every panel of the family, scaled to each record's cell (docs/asset-contract.md, Templates)."},
             "family": "panel", "draws": "model", "scales_by": ["physical"], "reference": ms[0], "members": len(ms),
             "basis": [{"of": ["draws", "scales_by"], "tier": "derived", "note": "The owner, 2026-10-17: buildings as layouts; one template a panel family (building_kit.py)."}]}
        n += write("templates", t, d, "template")
    for slug, d in PANELS:
        code = "KIT-PANEL-" + slug.upper()
        mats = list(dict.fromkeys(p["item"] for p in d["built_of"]))
        ph = d["physical"]
        st = {"identity": {"key": "stock." + code.lower(), "code": code, "name": d["identity"]["name"], "traded_as": "market.machinery", "form": "panel"},
              "package": "package." + PACKAGE_OF["panel"], "made_from": [{"item": m} for m in mats],
              "size": {"length": ph["length"], "width": ph["width"], "height": ph["height"]}, "supplies": {"item": d["identity"]["key"]},
              "physical": {"mass": ph["mass"]}, "basis": [{"of": ["size", "physical.mass"], "tier": "derived", "note": f"One {d['identity']['key']}, as made."}]}
        n += write("mill-stock", code, st, "mill-stock")
    print(f"{len(PANELS)} panels, {len(TEMPLATES)} templates, their stock; {n} written")


if __name__ == "__main__":
    main()
