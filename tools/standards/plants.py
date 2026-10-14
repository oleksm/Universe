"""Plant mounts, units and layouts (SFO 24): a works built the way a ship is.

    python3 tools/standards/plants.py

The trial (docs/proposals/modular-factories.md): the electric arc furnace (module.arc-furnace) built from units on a world
with air at 1 g. Each unit's mass is worked out from its material and dimensions below; the hall is a lattice of standard
struts (SFO 13) checked against its loads; the bays' mounts are set round the units fitted with a quarter to spare, as the
ship mounts are (rule.mounts-margin). Writes standards/SFO/metadata/{plant-mounts,units,layouts}/. Idempotent. Run from the
repository root. Figures marked "from memory" are not sourced: each unit's basis says so.
"""
import math, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import frames  # noqa: E402

S = "standards/SFO/metadata/"
G = 9.80665
RHO = {"steel": 7850, "copper": 8960, "aluminium": 2700, "refractory": 3000, "concrete": 2400, "water": 1000, "oil": 880}
STEEL, A36, CU, AL, REF, CONC = "material.high-strength-steel-4340", "material.structural-steel-a36", "material.copper", "material.aluminium-1350", "good.refractory-brick", "good.concrete"
M = 1.25                                     # the mounts' margin (rule.mounts-margin)
BAY = {1: 6.0, 2: 12.0, 3: 24.0, 4: 48.0}   # a bay's side by class: twice the last


def r(v, n=4):
    return float(f"{v:.{n}g}")


def box_tube(w, h, t, L):
    """m3 of a rectangular box section w x h, wall t, L long."""
    return (w * h - (w - 2 * t) * (h - 2 * t)) * L


def cyl_shell(D, H, t, bottom=0.0):
    """m3 of a cylinder's wall D across, H tall, t thick, with `bottom` (a factor on the disc) of the same plate."""
    return (math.pi * D * H + bottom * math.pi * D * D / 4) * t


def pipe_panel(area):
    """kg of steel and of water in a water-cooled panel of 70 x 8 mm pipes laid side by side: area m2."""
    n = 1 / 0.07
    return area * n * math.pi * (0.07 - 0.008) * 0.008 * RHO["steel"], area * n * math.pi / 4 * 0.054 ** 2 * RHO["water"]


def part(what, item, mass, note):
    return {"what": what, "item": item, "mass": r(mass), "note": note}


# ---------------------------------------------------------------- the furnace's figures
mod = yaml.safe_load(open(S + "modules/arc-furnace.yaml"))
recipe = mod["recipes"][0]
P = recipe["power"]                  # W at the furnace, the module's sourced figure
RATE = recipe["rate"]                # kg/s of liquid steel
HEAT = recipe["batch"]               # kg tapped
PF, V2 = 0.8, 800.0                  # power factor, secondary volts: from memory, typical of AC furnaces
S_MVA = P / PF                       # VA the transformer is rated at
I2 = P / (math.sqrt(3) * V2 * PF)    # A in each phase on the secondary side
WATER_SHARE, OFFGAS_SHARE = 0.15, 0.10   # shares of the arc's power taken by the water-cooled panels and roof, and by the off-gas duct: from memory
TRANSFORMER_LOSS = 0.005             # share of the rating lost in the transformer: from memory
DRI = recipe["inputs"][0]["quantity"] * RATE   # kg/s of sponge iron fed
SLAG = next(o["quantity"] for o in recipe["outputs"] if o["item"] == "good.slag") * HEAT


def units():
    out = {}

    # Arc furnace vessel: hearth, lining, water-cooled upper shell and roof, tilting cradle, masts and arms, cables.
    D, Hh, Hu = 6.6, 2.0, 2.5
    shell = cyl_shell(D, Hh, 0.05, bottom=1.25) * RHO["steel"]
    lining = (math.pi * (D / 2) ** 2 * 0.9 + math.pi * D * Hh * 0.45) * RHO["refractory"]
    up_s, up_w = pipe_panel(math.pi * D * Hu)
    roof_s, roof_w = pipe_panel(1.15 * math.pi * 3.4 ** 2)
    delta = math.pi * 1.0 ** 2 * 0.4 * RHO["refractory"]
    cradle = (2 * box_tube(1.5, 1.0, 0.04, 9.0) + 9 * 9 * 0.04) * RHO["steel"]
    masts = box_tube(0.8, 0.8, 0.04, 7.27 + 8.17 + 9.07) * RHO["steel"]   # staggered so the arms clear each other (blender, construction QC)
    arm = box_tube(0.8, 0.6, 0.03, 4.0) + box_tube(0.55, 0.6, 0.03, 0.5) + box_tube(0.3, 0.6, 0.03, 1.5)   # front narrowed to clear the other rods (blender, phase-clearance QC)
    arms = 3 * arm * RHO["aluminium"]
    cable_area = I2 / 2 / 5e6           # m2 each of two water-cooled cables a phase at 5 A/mm2
    cables = 3 * 2 * 12.0 * cable_area * RHO["copper"]
    hyd = 15000.0
    holds = HEAT + 0.15 * HEAT + SLAG + up_w + roof_w   # the charge, a sixth left as the heel (from memory), slag and panel water
    out["eaf-vessel-80t"] = dict(
        name="Arc furnace vessel, 80 t", size_class=2, fits="bay-s2", dims=(12.0, 10.0, 16.0),
        desc=f"An AC arc furnace that taps {HEAT / 1000:.0f} t: a refractory hearth {D} m across, water-cooled upper shell and roof, three graphite electrodes on masts, a tilting cradle and an eccentric bottom tap. Fed sponge iron through a fifth hole in the roof. Stands on a platform 5 m up so a ladle car runs under its tap.",
        built_of=[
            part("Hearth shell", A36, shell, f"{D} m across, {Hh} m tall, 50 mm plate, its dished bottom 1.25 times the disc"),
            part("Hearth lining", REF, lining, "0.9 m under the bath, 0.45 m up the hearth wall"),
            part("Upper shell panels and cage", A36, up_s * 1.5, f"water-cooled pipe panels {Hu} m tall round the shell (70 x 8 mm pipes side by side), the cage that holds them half as much again"),
            part("Roof panels", A36, roof_s, "water-cooled pipe panels over a dome 6.8 m across, 1.15 times the disc"),
            part("Roof delta", REF, delta, "the refractory centre round the three electrode ports, 2 m across, 0.4 m thick"),
            part("Tilting cradle", A36, cradle, "two rocker box girders 1.5 x 1 m, 40 mm wall, 9 m long, and a deck 9 x 9 m of 40 mm plate"),
            part("Electrode masts", A36, masts, "three box masts 0.8 x 0.8 m, 40 mm wall, 7.27, 8.17 and 9.07 m long, staggered so the arms clear; shown at rest, moving in fixed guide collars on seated rams"),
            part("Current-conducting arms", AL, arms, "three box arms 6 m long, 0.6 m deep, 30 mm wall: rear 4 m 0.8 m wide, 0.5 m taper, front 1.5 m 0.3 m wide to clear the neighbouring rods (geometric gap 30 mm; electrical clearance not yet checked)"),
            part("Secondary cables", CU, cables, f"two water-cooled copper cables a phase, 12 m, each {cable_area * 1e6:.0f} mm2 for half of {I2 / 1000:.0f} kA at 5 A/mm2"),
            part("Tilt and mast hydraulics", "good.pumps", hyd, "cylinders, pumps and valves; from memory"),
        ],
        holds=holds,
        needs={"power": P, "heat": WATER_SHARE * P, "gravity": 1.6, "atmosphere": "any"},
        recipes=mod["recipes"], life=mod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.arc-furnace), carried here for the trial: the module stays the game's until it reads layouts. Its sources are the module's."},
               {"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"From the materials and dimensions in built_of. Dimensions from memory of 80 to 100 t furnaces. The electrodes are not counted: they are a good it uses up (good.graphite-electrode). The heel, a sixth of the tap, from memory."},
               {"of": ["needs.heat"], "tier": "invented", "review": True, "note": f"The water-cooled panels and roof take about {WATER_SHARE:.0%} of the arc's power, from memory."},
               {"of": ["needs.gravity"], "tier": "invented", "review": True, "note": "Slag floats off the steel and the tap pours by gravity. Settling goes as g (Stokes), a sixth as fast at 1.6 m/s2; below that not checked."}])

    # Furnace transformer: its mass by rating.
    kg_per_va = 1.6e-3
    m = kg_per_va * S_MVA
    out["furnace-transformer-90mva"] = dict(
        name=f"Furnace transformer, {S_MVA / 1e6:.0f} MVA", size_class=2, fits="bay-s2", dims=(8.0, 5.0, 6.0),
        desc=f"Steps the grid down to about {V2:.0f} V and {I2 / 1000:.0f} kA a phase for the arcs, with on-load taps. Oil-filled, cooled by oil to water. Stands in a fire-walled vault beside the furnace so the secondary bus is short.",
        built_of=[
            part("Core", A36, 0.40 * m, "grain-oriented electrical steel, which the registry has not got: structural steel stands for it"),
            part("Windings", CU, 0.25 * m, ""),
            part("Tank and fittings", A36, 0.15 * m, ""),
            part("Oil", "material.hydraulic-fluid", 0.20 * m, "transformer mineral oil, which the registry has not got: hydraulic fluid stands for it"),
        ],
        needs={"power": TRANSFORMER_LOSS * S_MVA, "heat": TRANSFORMER_LOSS * S_MVA},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": f"{kg_per_va * 1e3:.1f} t a MVA and the shares of core, copper, tank and oil from memory of furnace transformers; size from memory. Rated at the arc's power over a power factor of {PF}."},
               {"of": ["needs"], "tier": "invented", "review": True, "note": f"Losses {TRANSFORMER_LOSS:.1%} of its rating, drawn on top of the arc's power and carried off by the cooling loop; from memory."}])

    # Sponge iron bin and feeder: holds two hours of feed.
    hold = DRI * 7200
    vol = hold / 1700.0                      # DRI about 1,700 kg/m3 in bulk, from memory
    Db = 5.0
    Hc = (vol - math.pi * Db ** 2 / 4 * 3.0 / 3) / (math.pi * Db ** 2 / 4)   # cylinder above a 3 m cone
    bin_m = cyl_shell(Db, Hc, 0.008) * RHO["steel"] + math.pi * Db / 2 * math.hypot(Db / 2, 3.0) * 0.008 * RHO["steel"]
    legs = 4 * box_tube(0.3, 0.3, 0.02, 6.0 + Hc) * RHO["steel"]
    out["dri-bin-200t"] = dict(
        name=f"Sponge iron bin, {hold / 1000:.0f} t", size_class=1, fits="bay-s1", dims=(6.0, 6.0, r(6.0 + Hc + 1.0, 3)),
        desc="A sealed steel bin on four legs that holds two hours of sponge iron, kept dry (it rusts and heats in wet air), with a weigh feeder under its cone onto the furnace's conveyor.",
        built_of=[part("Bin", A36, bin_m, f"{Db} m across, {Hc:.1f} m of cylinder over a 3 m cone, 8 mm plate"),
                  part("Legs", "stock.st4340-box-300x300x20", legs, f"four box legs 300 x 300 x 20 mm, {6.0 + Hc:.1f} m"),
                  part("Weigh feeder", "good.electric-motors", 1500.0, "from memory")],
        holds=hold, needs={"power": 15000.0},
        basis=[{"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"Two hours of the furnace's sponge iron ({DRI:.1f} kg/s) at about 1,700 kg/m3 in bulk (from memory). The cone's outlet 6 m up for the conveyor."}])

    # Off-gas line and baghouse.
    Q = 800000 / 3600                         # m3/s of off-gas after dilution: from memory, for 80 to 100 t furnaces
    cloth = Q * 60 / 1.2                      # m2 at 1.2 m/min
    bags = math.ceil(cloth / (math.pi * 0.15 * 8.0))
    casing = 2 * (24 * 12 + 24 * 18 + 12 * 18) * 0.008 * RHO["steel"]
    fan = Q * 4000 / 0.75
    out["baghouse-800k"] = dict(
        name="Off-gas baghouse, 800,000 m3/h", size_class=3, fits="bay-s3", dims=(24.0, 12.0, 18.0),
        desc="Cleans the furnace's fume before it goes to the air: a water-cooled duct off the roof's fourth hole, a burn-out gap, then filter bags in a steel casing, the dust shaken into hoppers. Two fans draw the gas through.",
        built_of=[part("Casing and hoppers", A36, casing, "24 x 12 x 18 m, 8 mm plate"),
                  part("Structure", A36, casing * 0.5, "half the casing, from memory"),
                  part("Filter bags", "material.ptfe", cloth * 0.5, f"{bags} bags 150 mm across and 8 m long, {cloth:.0f} m2 of PTFE-membrane cloth at 0.5 kg/m2"),
                  part("Fans and motors", "good.electric-motors", 2 * 8000.0, "two induced-draught fans, from memory")],
        holds=0.015 * RATE * 3600 * 4, needs={"power": fan, "heat": OFFGAS_SHARE * P},
        basis=[{"of": ["physical", "built_of", "needs.power"], "tier": "invented", "review": True, "note": f"Off-gas 800,000 m3/h, 1.2 m/min through the cloth, fans at 4 kPa and 75%: from memory. Fan power {fan / 1e6:.2f} MW, not in the module's figure."},
               {"of": ["needs.heat"], "tier": "invented", "review": True, "note": f"The water-cooled duct takes about {OFFGAS_SHARE:.0%} of the arc's power from the hot gas, from memory."},
               {"of": ["holds"], "tier": "derived", "note": "Four hours of the recipe's dust in the hoppers."}])
    return out


def cooling_unit(heat):
    """The cooling tower, sized to the heat the loop gathers."""
    cells = math.ceil(heat / 10e6)
    flow = heat / (4186 * 10)                 # kg/s at a 10 K rise
    evap = heat / 2.26e6
    pumps = flow / 1000 * 400e3 / 0.75
    fans = 0.008 * heat
    return dict(
        name=f"Cooling tower, {cells} cells", size_class=3, fits="bay-s3", dims=(12.0 * cells, 12.0, 14.0),
        desc=f"An induced-draught wet tower: the furnace's closed water loop gives up its heat through a plate exchanger to tower water, which sprays down through fill against a fan's draught and evaporates. On a world with air only; airless ground needs radiators instead.",
        built_of=[part("Cells", A36, 60000.0 * cells, "steel frame, fill, drift eliminators and fan deck: 60 t a cell, from memory"),
                  part("Fans and motors", "good.electric-motors", 6000.0 * cells, "one 10 m fan a cell, from memory"),
                  part("Loop pumps and exchanger", "good.pumps", 25000.0, "from memory")],
        needs={"power": pumps + fans, "water": evap * 1.25},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "10 MW a 12 m cell and the masses from memory."},
               {"of": ["needs"], "tier": "derived", "review": True, "note": f"The loop's {heat / 1e6:.1f} MW at a 10 K rise is {flow:.0f} kg/s, pumped at 4 bar and 75%: {pumps / 1e3:.0f} kW; fans 0.8% of the heat (from memory). Evaporating it all takes {evap:.1f} kg/s of water (2.26 MJ/kg); blowdown and drift a quarter more (from memory)."}])


def car_units():
    out = {}
    Di, Hi, wall, bot = 2.9, 2.6, 0.2, 0.3
    lining = (math.pi * ((Di / 2 + wall) ** 2 - (Di / 2) ** 2) * Hi + math.pi * (Di / 2 + wall) ** 2 * bot) * RHO["refractory"]
    Do = Di + 2 * wall + 0.08
    shell = cyl_shell(Do, Hi + bot, 0.04, bottom=1.0) * RHO["steel"]
    frame_ = 4 * 6.0 * frames.area({"identity": {"form": "box"}, "size": {"width": 0.4, "height": 0.4, "wall": 0.025}}) * RHO["steel"]
    deck = 6 * 4 * 0.04 * RHO["steel"]
    out["ladle-car-80t"] = dict(
        name="Ladle and transfer car, 80 t", size_class=2, dims=(6.0, 4.0, 4.4),
        desc="A refractory-lined steel ladle that takes one tap, on a motor car that runs on rails from under the furnace's tap to the ladle station.",
        built_of=[part("Ladle lining", REF, lining, f"{Di} m inside, {Hi} m deep, 0.2 m walls, 0.3 m bottom"),
                  part("Ladle shell and trunnion ring", A36, shell * 1.3, "40 mm plate, the trunnion ring and lugs three tenths more"),
                  part("Car frame and deck", "stock.st4340-box-400x400x25", frame_ + deck, "four box beams 400 x 400 x 25 mm, 6 m, and a deck 6 x 4 m of 40 mm plate"),
                  part("Drives", "good.electric-motors", 2000.0, "from memory")],
        holds=HEAT * 1.1, needs={"power": 50000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Sized to hold a tap of 80 t of steel (7,000 kg/m3) with its slag cover and freeboard: 17 m3 inside. Lining thicknesses from memory."}])
    vol = SLAG / 1500 * 1.25
    out["slag-pot-car"] = dict(
        name=f"Slag pot car, {vol:.0f} m3", size_class=1, dims=(5.0, 3.5, 3.4),
        desc="A cast steel pot on a rail car that runs under the furnace's slag door and takes the slag out to be cooled and broken.",
        built_of=[part("Pot", A36, 25 * 0.08 * RHO["steel"] * vol / 10, f"cast steel 80 mm, {vol:.0f} m3"),
                  part("Car", A36, 9000.0, "from memory"),
                  part("Drives", "good.electric-motors", 1500.0, "from memory")],
        holds=SLAG, needs={"power": 30000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"A tap's slag ({SLAG / 1000:.0f} t, the recipe's 0.15) foamed to about 1,500 kg/m3 (from memory), a quarter to spare."}])
    # Hall crane: a double-girder bridge over the 24 m span.
    span, cap = 24.0, 20000.0
    gird = 2 * box_tube(0.6, 1.4, 0.012, span) * RHO["steel"]
    I = 2 * 0.6 * 0.012 * 0.7 ** 2 + 2 * 0.012 * 1.4 ** 3 / 12
    defl = cap * G * span ** 3 / (48 * 205e9 * 2 * I)
    out["hall-crane-20t"] = dict(
        name="Hall crane, 20 t", size_class=2, dims=(6.0, span, 3.0),
        desc="A double-girder bridge crane on the hall's runways, for the roof, the electrodes and relining. Hangs on the frame; names no bay.",
        built_of=[part("Bridge girders", A36, gird, f"two box girders 1.4 x 0.6 m, 12 mm wall, {span:.0f} m: {defl * 1000:.0f} mm under 20 t at mid-span, span/{span / defl:.0f}"),
                  part("Trolley and hoist", A36, 6000.0, "from memory"),
                  part("End carriages and drives", "good.electric-motors", 4000.0, "from memory")],
        needs={"power": 60000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Girders sized for 20 t at mid-span within span/800; trolley and drives from memory."}])
    return out


def lattice(box, cell, top_k, wall_cells, openings):
    """Members of a lattice of cubic cells: chords along the edges, one diagonal in each square face. Returns
    {"wall"|"roof": (straights, diagonals)}, ground nodes, nodes, the cells."""
    x0, y0, z0, x1, y1, z1 = box
    nx, ny, nz = round((x1 - x0) / cell), round((y1 - y0) / cell), round((z1 - z0) / cell)
    cells = set()
    for i in range(nx):
        for j in range(ny):
            for k in range(nz):
                wall = (i in (0, nx - 1) or j in (0, ny - 1)) and k < top_k
                if not (wall or k == top_k):
                    continue
                cx, cy, cz = x0 + (i + .5) * cell, y0 + (j + .5) * cell, z0 + (k + .5) * cell
                if any(a <= cx <= d and b <= cy <= e and c <= cz <= f for a, b, c, d, e, f in openings):
                    continue
                cells.add((i, j, k))
    edges, faces = {}, {}
    for (i, j, k) in cells:
        grp = "roof" if k == top_k else "wall"
        corners = [(i + a, j + b, k + c) for a in (0, 1) for b in (0, 1) for c in (0, 1)]
        for p in corners:
            for q in corners:
                if p < q and sum(abs(p[n] - q[n]) for n in range(3)) == 1:
                    edges[(p, q)] = "wall" if "wall" in (edges.get((p, q)), grp) else grp
        for ax in range(3):
            for s in (0, 1):
                f = (ax, (i, j, k)[ax] + s) + tuple(v for n, v in enumerate((i, j, k)) if n != ax)
                faces[f] = "wall" if "wall" in (faces.get(f), grp) else grp
    nodes = {p for e in edges for p in e}
    out = {g: (sum(1 for v in edges.values() if v == g), sum(1 for v in faces.values() if v == g)) for g in ("wall", "roof")}
    return out, sum(1 for p in nodes if p[2] == 0), len(nodes), cells


def strut(section, cell, diag=False):
    key = f"stock.{section}-{'d' if diag else 's'}{int(cell * 1000)}"
    f = S + "mill-stock/" + key.split(".", 1)[1].upper() + ".yaml"
    return key, yaml.safe_load(open(f))


def euler(section_stock, L):
    """N a member takes in compression over L pinned, the lesser of buckling and yield (yield 0.6 of 4340's tensile)."""
    z = section_stock["size"]
    d, t = z["diameter"], z["wall"]
    I = math.pi / 64 * (d ** 4 - (d - 2 * t) ** 4)
    return min(math.pi ** 2 * 205e9 * I / L ** 2, frames.area(section_stock) * 470e6)


def write(path, d):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path):                       # (an installed model's visual, written by install_model.py, stays)
        old = yaml.safe_load(open(path, encoding="utf-8")) or {}
        if old.get("visual"):
            d = {k: v for k, v in d.items() if k not in ("basis",)} | {"visual": old["visual"], "basis": d.get("basis")}
    head = {"plant-mounts": "plant-mount", "units": "unit", "layouts": "layout"}[path.split("/")[-2]]
    with open(path, "w", encoding="utf-8") as f:
        f.write(f"# yaml-language-server: $schema=../../schema/{head}.schema.yaml\n# Written by tools/standards/plants.py: change it there.\n")
        yaml.safe_dump(d, f, sort_keys=False, width=1000, allow_unicode=True, default_flow_style=None)


def unit_record(slug, u):
    mass = sum(p["mass"] for p in u["built_of"])
    d = {"identity": {"key": f"unit.{slug}", "name": u["name"], "description": u["desc"]}, "size_class": u["size_class"]}
    if u.get("fits"):
        d["fits"] = "plant-mount." + u["fits"]
    L, W, H = u["dims"]
    d["physical"] = {"length": L, "width": W, "height": H, "mass": r(mass)}
    d["built_of"] = [{k: v for k, v in p.items() if v != ""} for p in u["built_of"]]
    if u.get("holds"):
        d["holds"] = r(u["holds"])
    d["needs"] = {k: (r(v) if isinstance(v, float) else v) for k, v in u.get("needs", {}).items()}
    if u.get("recipes"):
        d["recipes"] = u["recipes"]
    d["life"] = u.get("life") or mod.get("life")
    d["basis"] = u["basis"] + [{"of": ["life"], "rule": "rule.life-plant"}]
    return d


def main():
    us = units()
    loop_heat = sum(u["needs"].get("heat", 0) for u in us.values())
    us["cooling-tower"] = cooling_unit(loop_heat)
    us.update(car_units())
    recs = {slug: unit_record(slug, u) for slug, u in us.items()}

    # The layout: hall x[-21, 9] y[-15, 15], 3 m lattice walls one cell thick, a roof one cell deep at 24 m.
    cell, box = 3.0, (-21.0, -15.0, 0.0, 9.0, 15.0, 27.0)
    doors = [(6.0, 3.0, 0.0, 9.0, 9.0, 6.0), (-18.0, 12.0, 0.0, -15.0, 15.0, 6.0)]
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 8, None, doors)
    sec = {"wall": "st4340-tb-120x5", "roof": "st4340-tb-100x5"}
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec[g], cell, diag)
            members.append({"item": key, "count": n, "where": g})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec["wall"], cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    nm = frames.node_mass(node, frames.materials())
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * nm
    runway = 2 * 24.0 * frames.area({"identity": {"form": "box"}, "size": {"width": 0.4, "height": 0.4, "wall": 0.025}}) * RHO["steel"]
    members.append({"item": "stock.st4340-box-400x400x25", "count": 4, "where": "crane runways, two a side, 12 m lengths"})
    fmass += runway
    x0, y0, z0, x1, y1, z1 = box
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum((d - a) * (f - c) for a, b, c, d, e, f in doors)
    clad = clad_area * 0.002 * RHO["aluminium"]

    # The frame's loads, checked: roof strips spanning 24 m, wall verticals under the crane.
    roof_q = (0.002 * RHO["aluminium"] + 20.0) * G + 1000.0   # Pa: cladding, frame about 20 kg/m2, 1 kPa snow and wind
    chord = roof_q * cell * 24.0 ** 2 / 8 / cell
    crane = recs["hall-crane-20t"]["physical"]["mass"]
    wheel = ((20000 + 6000) * (1 - 1.5 / 24) + crane / 2) * G / 2
    vert = wheel + roof_q * cell * 12.0
    _, rs = strut(sec["roof"], cell); rsec = yaml.safe_load(open(S + "mill-stock/" + rs["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    _, ws = strut(sec["wall"], cell); wsec = yaml.safe_load(open(S + "mill-stock/" + ws["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    rcap, wcap = euler(rsec, cell), euler(wsec, cell)
    assert 1.5 * chord <= rcap and 1.5 * vert <= wcap, (chord, rcap, vert, wcap)
    sized = (f"At 1 g: roof {roof_q:.0f} Pa (2 mm aluminium, the frame about 20 kg/m2, 1 kPa of snow and wind, the last invented) over a 24 m span in 3 m deep strips puts {chord / 1e3:.0f} kN in a chord; "
             f"a {sec['roof']} strut takes {rcap / 1e3:.0f} kN over 3 m. A crane wheel at the runway's end puts {wheel / 1e3:.0f} kN on a wall node, with the roof's share {vert / 1e3:.0f} kN; a {sec['wall']} strut takes {wcap / 1e3:.0f} kN. "
             "Both with half again to spare. Wind on the walls and the footings' loads not yet checked.")

    vessel_at = (-12.0, 6.0, 5.0)
    bays = [
        {"id": "V", "mount": "plant-mount.bay-s2", "unit": "unit.eaf-vessel-80t", "at": list(vessel_at), "note": "on the furnace platform, 5 m up; the tap faces +x over the ladle rail, the slag door -x over the slag rail"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": "unit.furnace-transformer-90mva", "at": [-12.0, -6.0, 0.0], "note": "in the fire-walled vault beside the platform"},
        {"id": "F", "mount": "plant-mount.bay-s1", "unit": "unit.dri-bin-200t", "at": [-12.0, 33.0, 0.0], "note": "outside, north, far enough for the conveyor to climb at a belt's slope"},
        {"id": "B", "mount": "plant-mount.bay-s3", "unit": "unit.baghouse-800k", "at": [6.0, 30.0, 0.0], "note": "outside, north-east"},
        {"id": "C", "mount": "plant-mount.bay-s3", "unit": "unit.cooling-tower", "at": [-36.0, -6.0, 0.0], "note": "outside, west"},
    ]

    def link(kind, name, a, b, item, per_m, power=None, note=None):
        L = math.dist(a, b)
        d = {"kind": kind, "name": name, "from": list(a), "to": list(b), "length": r(L), "item": item, "mass": r(per_m * L)}
        if power:
            d["power"] = r(power)
        if note:
            d["note"] = note
        return d
    pipe_m = math.pi * 0.5 * 0.008 * RHO["steel"] * 2
    bus_area = I2 / 4e6
    links = [
        link("conveyor", "sponge iron feed", (-12.0, 30.0, 6.0), (-12.0, 7.8, 13.3), A36, 150.0, DRI * G * 7.0 / 0.5 + 20000, "0.8 m troughed belt, 150 kg/m with its gallery (from memory), 16 degrees; into the roof's fifth hole, 1.8 m north of the electrodes"),
        link("duct", "off-gas", (-10.0, 7.5, 13.5), (6.0, 24.0, 15.0), A36, math.pi * 2.0 * 0.010 * RHO["steel"], None, "2 m across, 10 mm plate, water-cooled for its first part; from the roof's fourth hole, 2 m east and 1.5 m north of the electrodes, to the baghouse"),
        link("busbar", "secondary bus", (-12.0, -1.0, 4.0), (-12.0, 0.8, 7.3), CU, 3 * bus_area * RHO["copper"], None, f"three water-cooled copper tube bars, {bus_area * 1e6:.0f} mm2 each for {I2 / 1000:.0f} kA at 4 A/mm2; through the vault wall to three insulated terminals spread 1.3 m apart along x behind the mast bank, then the furnace's cables"),
        link("rail", "ladle rail", (-7.5, 6.0, 0.0), (30.0, 6.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails under the tap, out of the hall's east door to the ladle station"),
        link("rail", "slag rail", (-16.5, 0.0, 0.0), (-16.5, 21.0, 0.0), A36, 2 * 60.0, None, "under the slag door, out of the north door"),
        link("pipe", "cooling water to the hall", (-24.0, -6.0, 0.0), (-18.0, 0.0, 5.0), A36, pipe_m, None, "a pair of 500 mm pipes, 8 mm wall, flow and return"),
        link("pipe", "cooling water to the baghouse", (-24.0, 0.0, 0.0), (-6.0, 24.0, 2.0), A36, pipe_m, None, "a pair of 500 mm pipes, 8 mm wall"),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-7.5, 6.0, 0.0]},
              {"unit": "unit.slag-pot-car", "rides": "slag rail", "at": [-16.5, 6.0, 0.0]}]

    ground = [
        {"what": "Furnace platform", "item": CONC, "volume": 384.0, "note": "12 x 12 x 5 m with tunnels for the ladle and slag cars, on a 1 m slab"},
        {"what": "Transformer vault", "item": CONC, "volume": r(12 * 12 * 0.6 + 48 * 8 * 0.3), "note": "a 0.6 m slab and 0.3 m fire walls 8 m tall"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Baghouse slab", "item": CONC, "volume": r(24 * 12 * 0.6)},
        {"what": "Cooling tower basin", "item": CONC, "volume": r(us["cooling-tower"]["dims"][0] * 12 * 0.5 + 2 * (us["cooling-tower"]["dims"][0] + 12) * 2 * 0.3), "note": "a 0.5 m floor and 2 m walls"},
        {"what": "Bin footing", "item": CONC, "volume": 36.0},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])
        g.pop("note", None)

    # Mounts: each class round what its bays take, a quarter to spare, at the site's most gravity.
    g_max = 9.81 * 1.0
    by_class = {}
    for b in bays:
        u = recs[b["unit"].split(".", 1)[1]]
        c = int(b["mount"][-1])
        need = u.get("needs", {})
        cur = by_class.setdefault(c, {"load": 0, "power": 0, "heat": 0, "water": 0, "units": []})
        cur["load"] = max(cur["load"], (u["physical"]["mass"] + u.get("holds", 0)) * g_max)
        for k in ("power", "heat", "water"):
            cur[k] = max(cur[k], need.get(k, 0))
        cur["units"].append(u["identity"]["key"])
    mounts = {}
    for c, v in sorted(by_class.items()):
        feeds = {k: r(v[k] * M, 3) for k in ("power", "heat", "water") if v[k]}
        mounts[f"bay-s{c}"] = {"identity": {"key": f"plant-mount.bay-s{c}", "name": f"Plant bay, class {c}", "size_class": c,
                                            "description": f"A {BAY[c]:.0f} m square of prepared ground or floor and the feeds to it."},
                               "footprint": {"length": BAY[c], "width": BAY[c]}, "bears": {"load": r(v["load"] * M, 3)}, **({"feeds": feeds} if feeds else {}),
                               "basis": [{"of": ["footprint"], "tier": "invented", "note": "Sides 6, 12, 24, 48 m by class: chosen, each twice the last, so the bays of a layout share one grid."},
                                         {"of": ["bears", "feeds"], "rule": "rule.mounts-margin", "note": f"Round the units fitted to it in the trial ({', '.join(v['units'])}) at 1 g, full."}]}
    for b in bays:
        u, m = recs[b["unit"].split(".", 1)[1]], mounts[b["mount"].split(".", 1)[1]]
        assert u["physical"]["length"] <= m["footprint"]["length"] and u["physical"]["width"] <= m["footprint"]["width"], b

    # Totals.
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [recs["hall-crane-20t"]]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    water = sum(u.get("needs", {}).get("water", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    parts_mass = 0.0
    pdir = S + "parts/arc-furnace/"
    for f in sorted(os.listdir(pdir)):
        parts_mass += (yaml.safe_load(open(pdir + f)).get("physical") or {}).get("mass", 0)
    mp = mod["physical"]
    against = (f"module.arc-furnace: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {P / 1e6:.0f} MW, its parts {parts_mass / 1000:.0f} t. "
               f"This layout: the same rate; {power / 1e6:.1f} MW (the module left out the transformer's losses, the fans and pumps); "
               f"{water:.1f} kg/s of water the module did not name (the tower's evaporation); {(mass_units + mass_links + frame_mass) / 1000:.0f} t of plant and frame and {mass_ground / 1000:.0f} t of concrete.")

    xs = [x0, x1] + [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [y0, y1] + [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.arc-furnace-air", "name": "Arc furnace shop, world with air",
                     "description": "One 80 t arc furnace with its transformer, sponge iron feed, fume plant and cooling tower, under a strut-lattice hall that keeps rain off the steel (water on molten steel bursts into steam). For a world with air at about 1 g. The trial layout of SFO 24."},
        "stands_for": "module.arc-furnace",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": "Walls one cell thick, 24 m tall; a roof one cell deep over a 24 x 24 m floor; doors 6 m tall for the ladle rail (east) and the slag rail (north).",
                  "openings": [list(d) for d in doors], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.hall-crane-20t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": RATE, "power": r(power), "heat": r(heat), "water": r(water), "mass": r(total), "against_module": against},
        "staff": mod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile", "frame.box", "frame.openings"], "tier": "invented", "review": True, "note": "Laid out for the trial: the platform and vault side by side under the hall, the rest outside. Lengths from memory of melt shops."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": "Counted from the lattice (tools/standards/plants.py): every cell edge a straight strut, every square face one diagonal, a node at every corner; masses from the strut and node records."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes chosen for the trial, concrete at 2,400 kg/m3; not sized for the ground's bearing."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged: whether staff go by unit is open."}],
    }
    for slug, d in mounts.items():
        write(S + f"plant-mounts/{slug}.yaml", d)
    for slug, d in recs.items():
        write(S + f"units/{slug}.yaml", d)
    write(S + "layouts/arc-furnace-air.yaml", layout)
    print(f"{len(mounts)} mounts, {len(recs)} units, 1 layout: {total / 1000:.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to the tower, {water:.1f} kg/s water, frame {frame_mass / 1000:.0f} t")


if __name__ == "__main__":
    main()
