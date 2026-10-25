#!/usr/bin/env python3
"""structure.platform: the orbital platform's frame, counted strut by strut, and its mass.

The envelope and datums are the game's (crates/core/world/src/station.rs). In free fall nothing carries weight, so the deck
is an open space frame: a 3 m strut lattice one cell deep, carried by deep lattice trusses on the pad-grid lines, square pad
plates with berthing clamps, walkways along the trusses. The main structure is a strut-lattice hall, one cell thick on all
six faces, clad in 2 mm aluminium, with the hangar door in the face toward the deck. Writes built_of and physical.mass into
standards/SFO/metadata/structures/platform.yaml; the rest of that record is kept.
"""
import math, os, sys
import yaml

sys.path.insert(0, os.path.dirname(__file__))
import frames  # noqa: E402
import lib  # noqa: E402

S = "standards/SFO/metadata/"
REC = S + "structures/platform.yaml"
CELL = 3.0
X0, Y0, Z0 = -300.0, -151.0, -375.0          # grid origin: x across, y up (deck top at -100 = j 17), z along
LINES_X = [0, 50, 100, 150, 199]             # truss cells along z at x -300, -150, 0, 150, 300
LINES_Z = [50, 100, 150, 200, 249]           # truss cells along x at z -225, -75, 75, 225, 375
PAD_X, PAD_Z = [-225, -75, 75, 225], [-150, 0, 150, 300]
PAD_SIDE = 90.0                              # square plates (blender): the 45 m capture radius fits inside
SECTION = {"truss": "al6061-tb-160x5", "deck": "al6061-tb-100x3", "hall": "al6061-tb-100x3"}
RANK = {"truss": 2, "deck": 1, "hall": 1}


def r(x, n=1):
    return round(x, n) if abs(x) < 1e6 else round(x)


def cells():
    out = {}
    def put(c, g):
        if RANK[g] > RANK.get(out.get(c), 0):
            out[c] = g
    for i in range(200):
        for k in range(50, 250):
            put((i, 16, k), "deck")                       # the deck's lattice, y -103 to -100
            if i in LINES_X or k in LINES_Z:
                for j in range(1, 16):                    # trusses, y -148 to -103: 45 m deep
                    put((i, j, k), "truss")
    door_i, door_j = range(83, 117), range(17, 37)       # 102 x 60 m, y -100 to -40, on the hall's deck face
    for i in range(200):
        for j in range(1, 100):                           # hall, y -148 to 149
            for k in range(0, 50):                        # z -375 to -225
                if not (i in (0, 199) or j in (1, 99) or k in (0, 49)):
                    continue
                if k == 49 and i in door_i and j in door_j:
                    continue
                put((i, j, k), "hall")
    return out


def count(cs):
    return frames.lattice_count(cs, RANK)


def stock(key):
    return yaml.safe_load(open(S + "mill-stock/" + key.split(".", 1)[1].upper() + ".yaml"))


def main():
    mats = frames.materials()
    n = count(cells())
    built, frame_m = [], 0.0
    names = {"truss": "Deck trusses", "deck": "Deck lattice", "hall": "Hall frame"}
    where = {"truss": "ten lattice trusses 3 m thick and 45 m deep under the deck, on the pad-grid lines (x -300, -150, 0, 150, 300 along z; z -225, -75, 75, 225, 375 along x)",
             "deck": "the deck's lattice, 3 m cells one deep, 600 x 600 m, its top at y -100",
             "hall": "the main structure's frame, 600 x 150 x 297 m, one 3 m cell thick on all six faces, less the hangar door"}
    for g, (ns, nd, nn) in n.items():
        ks, kd = f"stock.{SECTION[g]}-s3000", f"stock.{SECTION[g]}-d3000"
        s_, d_ = stock(ks), stock(kd)
        node = stock(s_["kit"]["node"])
        m = ns * s_["physical"]["mass"] + nd * d_["physical"]["mass"]
        mn = nn * frames.node_mass(node, mats)
        built.append({"what": f"{names[g]}: struts", "item": ks, "mass": r(m),
                      "note": f"{where[g]}: {ns:,} straight struts {ks} ({s_['physical']['mass']} kg) and {nd:,} diagonals {kd} ({d_['physical']['mass']} kg), one in every square face"})
        built.append({"what": f"{names[g]}: nodes", "item": s_["kit"]["node"], "mass": r(mn), "note": f"{nn:,} nodes"})
        frame_m += m + mn
    # Pad plates: aluminium honeycomb sandwich, 2 mm faces on a 50 mm core.
    hc = mats["material.aluminium-honeycomb"]["mass"]["density"]
    al = mats["material.aluminium-alloy-6061"]["mass"]["density"]
    sand = 2 * 0.002 * al + 0.05 * hc
    plates = 16 * PAD_SIDE ** 2 * sand
    built.append({"what": "Pad plates", "item": "material.aluminium-honeycomb", "mass": r(plates),
                  "note": f"16 square plates {PAD_SIDE:.0f} m a side on the deck lattice at x {PAD_X} and z {PAD_Z}: 2 mm 6061 faces on a 50 mm honeycomb core, {sand:.1f} kg/m2 (faces counted with the core here)"})
    built.append({"what": "Berthing clamps", "item": "good.electric-motors", "mass": 16 * 4 * 500.0, "note": "four recessed clamps at each pad's edges, 500 kg each with their drives, from memory"})
    spur = 150 / 2 - PAD_SIDE / 2 - 1.5           # from a pad's edge to the walkway's side over the next truss (blender)
    walk = (len(LINES_X) + len(LINES_Z)) * 600.0 * 3.0 + 16 * 3.0 * spur
    grate = walk * 10.0
    built.append({"what": "Walkway grating", "item": "material.aluminium-alloy-6061", "mass": r(grate), "note": f"3 m walkways over the ten trusses and a {spur:.1f} m spur from each pad's edge to the nearest, {walk:,.0f} m2 at about 10 kg/m2 (from memory); the rest of the deck is open lattice"})
    clad_a = 2 * (600 * 297 + 150 * 297 + 600 * 150) - 102 * 60
    clad = clad_a * 0.002 * al
    built.append({"what": "Hall cladding", "item": "stock.al6061-sh-2", "mass": r(clad), "note": f"2 mm 6061 sheet over the hall's six faces, {clad_a:,.0f} m2, against sun and debris; the closed hangar door's leaves the same"})
    total = sum(b["mass"] for b in built)
    # Berthing: in free fall the deck takes only the ships' berthing.
    ship, v, stop = 150e3, 0.3, 0.5
    F = ship * v ** 2 / (2 * stop)
    rec = yaml.safe_load(open(REC, encoding="utf-8"))
    rec["built_of"] = built
    rec["physical"]["mass"] = r(total)
    note = (f"Counted by tools/standards/station_platform.py from the 3 m lattice (every cell edge a straight strut, every square face one diagonal, a node at every corner; "
            f"where groups meet, the truss's heavier kit). Masses from the strut and node records. Free fall: berthing a 150 t ship at {v} m/s, stopped over {stop} m, "
            f"puts about {F / 1e3:.0f} kN on its pad, which a {SECTION['deck']} strut carries many times over; the game's deck speed (10 m/s) would put about "
            f"{ship * 10 ** 2 / (2 * stop) / 1e6:.0f} MN, which no deck takes. Not checked: the station's turn, thermal cycling, the pressurised modules (not designed).")
    rec["basis"] = [b for b in rec.get("basis", []) if b.get("of") != ["built_of", "physical.mass"]] + [{"of": ["built_of", "physical.mass"], "tier": "derived", "review": True, "note": note}]
    lib.own(REC, "tools/standards/station_platform.py", ["built_of", "physical.mass"])
    with open(REC, "w", encoding="utf-8") as f:
        f.write("# yaml-language-server: $schema=../../schema/structure.schema.yaml\n")
        yaml.safe_dump(rec, f, sort_keys=False, width=1000, allow_unicode=True, default_flow_style=None)
    print(f"platform: frame {frame_m / 1e3:,.0f} t, total {total / 1e3:,.0f} t;", {g: v for g, v in n.items()})


if __name__ == "__main__":
    main()
