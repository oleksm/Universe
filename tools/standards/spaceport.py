#!/usr/bin/env python3
"""structure.spaceport: the ground port's hangar frame, sized for its world's gravity and counted strut by strut, and its
ground works at the volumes measured on blender's model. Writes built_of, ground and physical.mass into
standards/SFO/metadata/structures/spaceport.yaml; the rest of that record is by hand.

The hangar: 96 m (x) by 156 m (y) by 36 m, a 3 m lattice, walls one cell thick, the roof four cells deep (24 to 36 m),
spanning the 96 m; its door 48 m wide and 24 m tall in the north face. A world with air at 1 g: the roof carries its own
weight, the cladding, snow and wind; the walls carry the roof.
"""
import math, os, sys
import yaml

sys.path.insert(0, os.path.dirname(__file__))
import frames  # noqa: E402
import lib  # noqa: E402

S = "standards/SFO/metadata/"
REC = S + "structures/spaceport.yaml"
G = 9.81
CELL, NX, NY, NZ, ROOF = 3.0, 32, 52, 12, 4      # 96 x 156 x 36 m, the top four layers the roof (12 m deep, span/8; 6 m was too shallow)
DOOR_I, DOOR_K = range(8, 24), range(0, 8)        # x -24 to 24, z 0 to 24, in the north face (j = NY - 1)
SNOW_WIND = 1000.0                                # Pa: snow and wind on the roof, invented
CLAD = 0.002                                      # m of 6061 sheet
SECTIONS = sorted([f[:-11].lower() for f in os.listdir(S + "mill-stock") if f.startswith("ST4340-TB-") and f.endswith("-S3000.yaml")])

# Measured on blender's model (structure-spaceport candidate v01, quantities.json, 2026-10-14), m3.
MEASURED = {
    "Pads": ("good.concrete", 71020.8, "16 pads 90 m square, 0.6 m of reinforced concrete, less their trenches"),
    "Pad bases": ("good.aggregate", 59184.0, "0.5 m of crushed stone under the pads"),
    "Flame trenches, concrete": ("good.concrete", 6806.6 + 7455.1 + 322.0, "floors, walls and west caps, 0.6 m, of the 16 trenches"),
    "Flame trenches, lining": ("good.refractory-brick", 2860.7 + 3456.0 + 126.7, "0.3 m of refractory brick inside them"),
    "Trench outlets, concrete": ("good.concrete", 947.2 + 533.7, "the 12 m outlets rising to grade east of each pad"),
    "Trench outlets, lining": ("good.refractory-brick", 400.7 + 230.4, "their brick"),
    "Taxiways and roads": ("good.concrete", 39873.1, "0.4 m: the taxiways, the pad connectors, the spine to the hangar and the 6 m service road"),
    "Terminal hardstand": ("good.concrete", 230.4, "24 m square at (200, -450), for the fitted ground terminal"),
    "Hangar slab": ("good.concrete", 5990.4, "0.4 m under the hangar, 96 x 156 m"),
}
GRATES = 10.75                                    # m3 of steel in the ditch grates, measured on the same model


def r(x):
    return round(x, 1) if abs(x) < 1e6 else round(x)


def stock(key):
    return yaml.safe_load(open(S + "mill-stock/" + key.split(".", 1)[1].upper() + ".yaml"))


def capacity(section):
    """N a 3 m straight strut of this section takes in compression, buckling or yield (frames' rule, as plants.py)."""
    s = stock(f"stock.{section}")
    z = s["size"]; d, t = z["diameter"], z["wall"]
    I = math.pi / 64 * (d ** 4 - (d - 2 * t) ** 4)
    return min(math.pi ** 2 * 205e9 * I / CELL ** 2, frames.area(s) * 470e6)


def lightest(force):
    """The lightest section whose strut takes force with half again to spare."""
    ok = [(stock(f"stock.{s}-s3000")["physical"]["mass"], s) for s in SECTIONS if capacity(s) >= 1.5 * force]
    return min(ok)[1]


def cells():
    out = {}
    for i in range(NX):
        for j in range(NY):
            for k in range(NZ):
                if k >= NZ - ROOF:
                    out[(i, j, k)] = "roof"
                elif i in (0, NX - 1) or j in (0, NY - 1):
                    if j == NY - 1 and i in DOOR_I and k in DOOR_K:
                        continue
                    out[(i, j, k)] = "wall"
    return out


def main():
    mats = frames.materials()
    al = mats["material.aluminium-alloy-6061"]["mass"]["density"]
    span, depth = NX * CELL, ROOF * CELL
    n = frames.lattice_count(cells(), {"wall": 1, "roof": 2})
    frame_kg_m2 = 40.0                                     # first guess, then from the count
    for _ in range(3):
        q = (CLAD * al + frame_kg_m2) * G + SNOW_WIND
        chord = q * CELL * span ** 2 / 8 / depth           # a 3 m strip of the roof, simply supported over the span
        wall = q * CELL * span / 2                         # its end on a wall's vertical
        web = wall / (ROOF * math.sin(math.pi / 4))         # the end shear, shared by the diagonals of the strip's ROOF layers
        sec = {"chord": lightest(chord), "web": lightest(web), "wall": lightest(wall)}
        mass = {}
        for g, (ns, nd, nn) in n.items():
            if g == "roof":                                 # chords: the span-wise members of the top and bottom planes; the rest web
                nc = 2 * NX * (NY + 1)
                c, w = stock(f"stock.{sec['chord']}-s3000"), stock(f"stock.{sec['web']}-s3000")
                wd = stock(f"stock.{sec['web']}-d3000")
                node = stock(c["kit"]["node"])
                mass[g] = (nc * c["physical"]["mass"] + (ns - nc) * w["physical"]["mass"] + nd * wd["physical"]["mass"], nn * frames.node_mass(node, mats), c, wd, node, nc, w)
            else:
                s_, d_ = stock(f"stock.{sec[g]}-s3000"), stock(f"stock.{sec[g]}-d3000")
                node = stock(s_["kit"]["node"])
                mass[g] = (ns * s_["physical"]["mass"] + nd * d_["physical"]["mass"], nn * frames.node_mass(node, mats), s_, d_, node, None, None)
        frame_kg_m2 = (mass["roof"][0] + mass["roof"][1]) / (span * NY * CELL)
    built = []
    where = {"roof": f"the roof, four 3 m cells deep over the {span:.0f} m span", "wall": "the walls, one 3 m cell thick, less the door"}
    for g in ("roof", "wall"):
        m, mn, s_, d_, node, nc, w = mass[g]
        ns, nd, nn = n[g]
        if nc:
            txt = (f"{where[g]}: {nc:,} chords {s_['identity']['key']} ({s_['physical']['mass']} kg) in its top and bottom planes, {ns - nc:,} other straights "
                   f"{w['identity']['key']} ({w['physical']['mass']} kg) and {nd:,} diagonals {d_['identity']['key']} ({d_['physical']['mass']} kg)")
        else:
            txt = f"{where[g]}: {ns:,} straight struts {s_['identity']['key']} ({s_['physical']['mass']} kg) and {nd:,} diagonals {d_['identity']['key']} ({d_['physical']['mass']} kg)"
        built.append({"what": f"Hangar {g}: struts", "item": s_["identity"]["key"], "mass": r(m), "note": txt})
        built.append({"what": f"Hangar {g}: nodes", "item": node["identity"]["key"], "mass": r(mn), "note": f"{nn:,} nodes"})
    clad_a = 2 * (span * 156 + 156 * NZ * CELL + span * NZ * CELL) - span * 156 - 48 * 24   # walls and roof, no floor, less the door
    built.append({"what": "Hangar cladding", "item": "stock.al6061-sh-2", "mass": r(clad_a * CLAD * al), "note": f"2 mm 6061 sheet, {clad_a:,.0f} m2 over the walls and roof (blender's model has 58 m3, the same, and 15 m3 of seam strips not counted)"})
    built.append({"what": "Hangar door", "item": "stock.al6061-sh-2", "mass": r(45 * 24 * (2 * CLAD * al + 20.0)), "note": "three leaves, 45 x 24 m in all: 2 mm skins both sides on a frame of about 20 kg/m2 (from memory)"})
    built.append({"what": "Ditch grates", "item": "material.structural-steel-a36", "mass": r(GRATES * 7850), "note": f"{GRATES} m3 of steel grating where the taxiways cross the ditches, measured on blender's model"})
    ground = []
    for what, (item, vol, note) in MEASURED.items():
        rho = yaml.safe_load(open(S + "goods/" + item.split(".", 1)[1] + ".yaml"))["physical"]["bulk_density"]
        ground.append({"what": what, "item": item, "volume": r(vol), "mass": r(vol * rho), "note": note + "; measured on blender's model"})
    rec = yaml.safe_load(open(REC, encoding="utf-8"))
    rec["built_of"], rec["ground"] = built, ground
    total = sum(b["mass"] for b in built) + sum(g["mass"] for g in ground)
    rec["physical"]["mass"] = r(total)
    s_r, s_w, s_web = capacity(sec["chord"]), capacity(sec["wall"]), capacity(sec["web"])
    note = (f"tools/standards/spaceport.py. Hangar at 1 g: the roof {q:.0f} Pa (2 mm cladding, the frame {frame_kg_m2:.0f} kg/m2 as counted, {SNOW_WIND:.0f} Pa of snow and wind, invented) "
            f"over {span:.0f} m, {depth:.0f} m deep, puts {chord / 1e3:.0f} kN in a chord of each 3 m strip: {sec['chord']} takes {s_r / 1e3:.0f} kN; its web diagonals {web / 1e3:.0f} kN at the ends: {sec['web']} takes {s_web / 1e3:.0f} kN; a wall vertical takes the strip's end, "
            f"{wall / 1e3:.0f} kN: {sec['wall']} takes {s_w / 1e3:.0f} kN; each the lightest kit section with half again to spare. Not checked: wind on the walls, the door's own frame, "
            f"the footings. Ground works at blender's measured volumes, masses from the goods' bulk densities; the site's grading (106,440 m3 moved in place) not counted.")
    rec["basis"] = [b for b in rec.get("basis", []) if b.get("of") != ["built_of", "ground", "physical.mass"]] + [{"of": ["built_of", "ground", "physical.mass"], "tier": "derived", "review": True, "note": note}]
    lib.own(REC, "tools/standards/spaceport.py", ["built_of", "ground", "physical.mass"])
    with open(REC, "w", encoding="utf-8") as f:
        f.write("# yaml-language-server: $schema=../../schema/structure.schema.yaml\n")
        yaml.safe_dump(rec, f, sort_keys=False, width=1000, allow_unicode=True, default_flow_style=None)
    print(f"spaceport: {total / 1e3:,.0f} t; chord {sec['chord']}, web {sec['web']}, walls {sec['wall']}, chord {chord / 1e3:.0f} kN;", {g: v for g, v in n.items()})


if __name__ == "__main__":
    main()
