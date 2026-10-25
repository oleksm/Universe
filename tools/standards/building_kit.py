"""The building kit (the owner, 2026-10-17: a building is a layout laid of a frame, panels, fit-out units and service
kit, not one model). Panels on the strut frame's 3 m grid for a world with air (site air) and for an airless one
(site airless: pressure walls, roof and floor, plug doors, a viewport, pressure partitions); for a world with air: walls (insulated, glazed, with a
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
from lib import r  # noqa: E402
import lib  # noqa: E402
from packages import FORMS as PACKAGE_OF  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata")
STEEL, AL, PIR, GLASS, RUB, CONC = ("material.structural-steel-a36", "material.aluminium-alloy-6061", "material.pir-foam",
                                    "material.soda-lime-glass", "material.epdm-rubber", "good.concrete")
CELL = 3.0
MEMORY = "From memory of modular building practice; to be sourced."


def sandwich(area, pir):
    """Two 0.5 mm steel faces on PIR `pir` m thick: (parts, kg, U)."""
    faces = area * 2 * 0.0005 * 7850
    foam = area * pir * 32
    return [("Steel faces", STEEL, faces, "two 0.5 mm coated steel skins"), ("Insulation core", PIR, foam, f"{pir * 1000:g} mm of PIR")], faces + foam, 0.022 / pir


GLAZED_KG_M2 = 42.0


def glazed(area):
    return [("Glazing", GLASS, area * 30.0, "double glazing, two 6 mm panes"), ("Mullions and transoms", AL, area * 12.0, "aluminium curtain wall frame"), ("Seals", RUB, area * 0.6, "EPDM gaskets")], area * GLAZED_KG_M2


PANELS = []


def panel(slug, name, kind, cell, thick, parts, desc, template, opening=None, u=None, load=None, basis=MEMORY, site="air", tier="invented"):
    mass = sum(p[2] for p in parts)
    if kind in ("wall", "partition"):
        ph = {"length": cell[0], "width": thick, "height": cell[1], "mass": r(mass)}
    elif kind == "stair":
        ph = {"length": 2.74, "width": 5.24, "height": 4.053, "mass": r(mass)}
    else:
        ph = {"length": cell[0], "width": cell[1], "height": thick, "mass": r(mass)}
    d = {"identity": {"key": f"panel.{slug}", "name": name, "description": desc}, "kind": kind, "site": site, "cell": list(cell), "thickness": thick}
    if opening:
        d["opening"] = opening
    if u:
        d["u_value"] = r(u, 2)
    if load:
        d["load"] = load
    d["physical"] = ph
    d["built_of"] = [{"what": w, "item": i, "mass": r(m), **({"note": n} if n else {})} for w, i, m, n in parts]
    d["template"] = f"template.{template}"
    d["basis"] = [{"of": ["built_of", "physical", "u_value", "load"], "tier": tier, "review": True, "note": basis}]
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
        ("wall-roller-3x3", "Wall panel, roller shutter, 3 x 3 m", {"kind": "roller-shutter", "width": 2.8, "height": 2.5, "sill": 0.0, "vehicles": True}, None, "A cell face closed by a steel roller shutter, a 2.8 x 2.5 m opening; its drum and motor in a box across the top on the inside face, within the cell (it stands 0.35 m inside the wall's plane): stock in by forklift or truck.")):
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
    panel(slug, name, "wall", (3.0, 3.0), 0.35 if slug == "wall-roller-3x3" else 0.1, parts, desc, slug.replace("-3x3", ""), opening=op, u=uu)
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
      [("Stringers, treads, landings and frames", STEEL, 669.1, "8 mm stringers, 3 mm folded treads and landings, 50 x 50 x 3 landing frames and columns: blender's model (2026-10-17)"), ("Handrails, guards and posts", STEEL, 162.6, "35 x 35 x 2 mm: blender's model")],
      "A steel stair between floors 3 m apart, in a stair well of two cells (3 x 6 m): two flights of ten 150 mm risers on 300 mm goings, its half and end landings its own, handrails to 1 m over the upper floor.", "stair")


P_IN = 101300.0
AL_ALLOW = 110e6
AL_RHO = 2700.0
PITCH = 0.23
MLI_KG_M2, MLI_T, MLI_U = 0.6, 0.02, 0.1
PRESSURE = f"Holds {P_IN / 1e3:g} kPa against vacuum: 6061 welded at {AL_ALLOW / 1e6:g} MPa, the skin between stiffeners {PITCH:g} m apart as a clamped strip (p b^2 / 2 t^2), each stiffener a fixed-ended T over the cell's short side (w L^2 / 12) with the skin its other flange, as the cargo cabins (equipment/cargo-cabin-s1.yaml). Edge channel, seals, door hardware and the insulation from memory; to be sourced."


def skin_t(pitch=PITCH, p=P_IN):
    """mm-rounded-up skin thickness, m, for a clamped strip `pitch` wide."""
    t = pitch * (p / (2 * AL_ALLOW)) ** 0.5
    return max(0.003, -(-t // 0.0005) * 0.0005)


def tee(span, pitch=PITCH, p=P_IN, ends=12.0):
    """The lightest of a plain family of 6061 T-stiffeners (6 mm web, a 10 mm flange 0.6 x the depth wide) that carries
    p over `pitch` on `span` (M = w L^2 / ends), with the skin as its other flange: (depth m, kg/m without the skin)."""
    t = skin_t(pitch, p)
    need = p * pitch * span ** 2 / ends / AL_ALLOW
    for mm in range(40, 601, 5):
        d = mm / 1000.0
        bf, tf, tw = 0.6 * d, 0.010, 0.006
        parts = [(pitch * t, t / 2), (tw * d, t + d / 2), (bf * tf, t + d + tf / 2)]
        a = sum(x for x, _ in parts)
        y = sum(x * c for x, c in parts) / a
        i = pitch * t ** 3 / 12 + tw * d ** 3 / 12 + bf * tf ** 3 / 12 + sum(x * (c - y) ** 2 for x, c in parts)
        if i / max(y, t + d + tf - y) >= need:
            return d, (tw * d + bf * tf) * AL_RHO
    raise ValueError(f"no stiffener for a {span} m span")


def pressure_skin(w, h, cut=0.0, mli=True, what="Pressure skin"):
    """A cell face w x h of skin and stiffeners spanning its short side, less `cut` m2 of opening: (parts, depth m)."""
    t, (d, kgm) = skin_t(), tee(min(w, h))
    n = int(max(w, h) / PITCH) + 1
    area = w * h - cut
    parts = [(what, AL, area * t * AL_RHO, f"{t * 1000:g} mm welded 6061"),
             ("Stiffeners", AL, n * min(w, h) * kgm * (area / (w * h)), f"{n} T-stiffeners {d * 1000:g} mm deep at {PITCH:g} m"),
             ("Edge channel, seals and bolts", AL, 2 * (w + h) * 6.0, "a welded edge channel and two O-ring grooves on the frame's beams, 6 kg/m")]
    if mli:
        parts.append(("Multi-layer insulation", "material.polyimide-film", area * MLI_KG_M2, "about twenty layers, outside the skin"))
    return parts, t + d + 0.010 + (MLI_T if mli else 0.0)


def plug_leaf(w, h, what):
    """A door leaf that opens inward so the pressure seats it: one 5 mm skin on ribs across its width (simply supported,
    w L^2 / 8), plus hinges, latches and its seal frame (invented, 20 kg a metre of edge): (parts, depth)."""
    t, (d, kgm) = skin_t(), tee(w, ends=8.0)
    n = int(h / PITCH) + 1
    return [(what, AL, w * h * t * AL_RHO + n * w * kgm, f"a {t * 1000:g} mm skin on {n} ribs {d * 1000:g} mm deep, opening inward"),
            ("Hinges, latches and seal frame", STEEL, 2 * (w + h) * 20.0 / 2, "forged hinges, dogs all round, the door's seal land (invented)")], d


for w, h, sl in ((3.0, 3.0, "3x3"), (1.5, 3.0, "1p5x3"), (3.0, 1.5, "3x1p5")):
    parts, th = pressure_skin(w, h)
    panel(f"wall-pressure-{sl}", f"Pressure wall panel, {w:g} x {h:g} m", "wall", (w, h), r(th, 3), parts,
          "A stiffened aluminium skin welded to a channel that bolts and seals on one cell face of the frame: the outside wall of a building on an airless world, holding its air in. Insulating blankets outside; no weather to keep off.",
          "wall-pressure", u=MLI_U, load=P_IN, basis=PRESSURE, site="airless", tier="derived")
parts, th = pressure_skin(3.0, 3.0)
panel("roof-pressure-3x3", "Pressure roof panel, 3 x 3 m", "roof", (3.0, 3.0), r(th, 3), parts,
      "The wall panel's build on the frame's top cells: the pressure pushes it up into the frame's ties. Regolith shielding over it is ground works, not the panel.",
      "roof-pressure", u=MLI_U, load=P_IN, basis=PRESSURE, site="airless", tier="derived")
panel("floor-pressure-3x3", "Pressure floor panel, 3 x 3 m", "floor", (3.0, 3.0), 0.05,
      [("Pressure plate", AL, 9.0 * 0.005 * AL_RHO, "5 mm welded 6061, the vessel's bottom, bearing on the ground slab (the slab takes the pressure)"),
       ("Edge channel", AL, 19.48, "a 45 mm perimeter channel, 5 mm returns outward: blender's model (2026-10-18)"), ("Seals", RUB, 0.204, "blender's model"), ("Bolts", STEEL, 1.771, "M16 at 250 mm in the exposed border: blender's model"),
       ("Screed", CONC, 812.94, "40 mm, 2.91 m square, inset 45 mm: blender's model")],
      "The ground floor of a pressurised building: a sealed aluminium plate on the ground slab, the air's push carried by the slab, a screed over it.",
      "floor-pressure", load=5000.0, basis=PRESSURE, site="airless", tier="derived")
door = (1.0, 2.0)
lp, ld = plug_leaf(*door, "Pressure door leaf")
parts, th = pressure_skin(3.0, 3.0, cut=door[0] * door[1])
panel("wall-hatch-3x3", "Pressure wall panel with a door, 3 x 3 m", "wall", (3.0, 3.0), max(r(th, 3), 0.225),
      parts + lp + [("Door surround", AL, 40.0, "a welded doubler round the opening (invented)"), ("Leaf insulation", "material.polyimide-film", 1.19, "the blanket over the leaf's vacuum face, its seal lands bare: blender's model")],
      "A pressure wall panel with a 1.0 x 2.0 m plug door that opens inward, so the air seats it: the outer door of an airlock (its chamber one cell, the inner door a pressure partition's).",
      "wall-hatch", opening={"kind": "pressure-door", "width": door[0], "height": door[1], "sill": 0.15, "people": True}, u=MLI_U, load=P_IN, basis=PRESSURE, site="airless", tier="derived")
cargo = (2.4, 2.4)
lp, ld = plug_leaf(*cargo, "Cargo door leaf")
parts, th = pressure_skin(3.0, 3.0, cut=cargo[0] * cargo[1])
CARGO_DEPTH = 0.267
panel("wall-cargo-door-3x3", "Pressure wall panel with a cargo door, 3 x 3 m", "wall", (3.0, 3.0), max(r(max(th, ld + 0.05), 3), CARGO_DEPTH),
      parts + lp + [("Door surround", AL, 120.0, "a heavy welded ring frame round the opening (invented)"), ("Leaf insulation", "material.polyimide-film", 3.44, "the blanket over the leaf's vacuum face, its seal lands bare: blender's model")],
      "A pressure wall panel with a 2.4 x 2.4 m plug door opening inward on heavy hinges: a vehicle or stock airlock's door, two cells apart with the chamber between.",
      "wall-cargo-door", opening={"kind": "pressure-cargo-door", "width": cargo[0], "height": cargo[1], "sill": 0.1, "vehicles": True}, u=MLI_U, load=P_IN, basis=PRESSURE, site="airless", tier="derived")
vp_r, vp_t = 0.3, 0.032
parts, th = pressure_skin(3.0, 3.0, cut=3.1416 * vp_r ** 2)
panel("wall-viewport-3x3", "Pressure wall panel with a viewport, 3 x 3 m", "wall", (3.0, 3.0), r(th, 3),
      parts + [("Viewport panes", "material.fused-silica", 44.72, "two 32 mm panes 638 mm across on their seat lands, either holding the air alone: blender's model (2026-10-18)"), ("Viewport sleeve, collar and retainers", AL, 27.26, "a welded 640/680 mm sleeve through the wall's depth, a 760 mm collar, retainers, 12 bolts and dual seals: blender's model")],
      "A pressure wall panel with a 0.6 m round viewport of two fused silica panes: a look out where a world with air would have a window.",
      "wall-viewport", opening={"kind": "viewport", "width": 2 * vp_r, "height": 2 * vp_r, "sill": 1.2}, u=MLI_U, load=P_IN, basis=PRESSURE, site="airless", tier="derived")
parts, th = pressure_skin(3.0, 3.0, mli=False, what="Bulkhead skin")
panel("partition-pressure-3x3", "Pressure partition, 3 x 3 m", "partition", (3.0, 3.0), r(th, 3), parts,
      "A wall panel's build inside, without the blankets: a bulkhead that keeps one part's air if the next loses it, or closes an airlock's chamber.",
      "partition-pressure", load=P_IN, basis=PRESSURE, site="airless", tier="derived")
lp, ld = plug_leaf(*door, "Pressure door leaf")
parts, th = pressure_skin(3.0, 3.0, cut=door[0] * door[1], mli=False, what="Bulkhead skin")
HINGE_DEPTH = 0.205
panel("partition-pressure-hatch-3x3", "Pressure partition with a door, 3 x 3 m", "partition", (3.0, 3.0), max(r(th, 3), HINGE_DEPTH), parts + lp + [("Door surround", AL, 40.0, "a welded doubler round the opening (invented)")],
      "A pressure partition with a 1.0 x 2.0 m plug door: an airlock's inner door, or a door between parts that can be shut on a leak.",
      "partition-pressure-hatch", opening={"kind": "pressure-door", "width": door[0], "height": door[1], "sill": 0.15, "people": True}, load=P_IN, basis=PRESSURE, site="airless", tier="derived")

TEMPLATES = {"wall-insulated": "Insulated wall panels", "wall-glazed": "Glazed wall panels", "wall-window": "Wall panels with a window",
             "wall-door": "Wall panels with a door", "wall-double-door": "Wall panels with double doors", "wall-roller": "Roller shutter wall panels",
             "wall-shopfront": "Shop fronts", "partition": "Partition panels", "partition-door": "Partition panels with a door",
             "floor": "Floor panels", "roof": "Roof panels", "stair": "Stairs",
             "wall-pressure": "Pressure wall panels", "roof-pressure": "Pressure roof panels", "floor-pressure": "Pressure floor panels",
             "wall-hatch": "Pressure wall panels with a door", "wall-cargo-door": "Pressure wall panels with a cargo door",
             "wall-viewport": "Pressure wall panels with a viewport", "partition-pressure": "Pressure partitions",
             "partition-pressure-hatch": "Pressure partitions with a door"}


KEEP = ("visual", "model_requests")


def write(folder, slug, d, schema):
    path = os.path.join(M, folder, slug + ".yaml")
    if os.path.exists(path):
        old = yaml.safe_load(open(path, encoding="utf-8")) or {}
        d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
    os.makedirs(os.path.dirname(path), exist_ok=True)
    text = f"# yaml-language-server: $schema=../../schema/{schema}.schema.yaml\n" + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
    lib.own(path, "tools/standards/building_kit.py")
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
