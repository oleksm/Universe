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
    # Slag spout: from the door sill (local x -3.32, 3.2 m up) out past the cradle deck's edge (x -4.5) to x -5.0, over the slag pot (blender, slag fixture)
    sp_L, sp_w, sp_d = 1.68 / math.cos(math.radians(20)), 0.8, 0.4      # sloped 20 degrees down, a trough 0.8 m wide and 0.4 m deep
    u = lambda w, d: w * d                                               # an open trough's section, its top open: w wide, d deep
    spout_s = (u(sp_w, sp_d) - u(sp_w - 0.06, sp_d - 0.03)) * sp_L * RHO["steel"]
    spout_r = (u(sp_w - 0.06, sp_d - 0.03) - u(sp_w - 0.36, sp_d - 0.18)) * sp_L * RHO["refractory"]
    hyd = 15000.0
    holds = HEAT + 0.15 * HEAT + SLAG + up_w + roof_w   # the charge, a sixth left as the heel (from memory), slag and panel water
    out["eaf-vessel-80t"] = dict(
        name="Arc furnace vessel, 80 t", size_class=2, fits="bay-s2", dims=(12.0, 11.74, 14.0),   # as built in blender's model (v3): the arms' tails and cables reach 6.25 m behind the axis, the hoses' couplings at the deck manifold 5.49 m in front
        desc=f"An AC arc furnace that taps {HEAT / 1000:.0f} t: a refractory hearth {D} m across, water-cooled upper shell and roof, three graphite electrodes on masts, a tilting cradle and an eccentric bottom tap. Fed sponge iron through a fifth hole in the roof. Stands on a platform 5 m up so a ladle car runs under its tap. Its cooling water comes by two flexible hoses of its own (it tilts), 300 mm bore, flow and return, from its ports at local (-3.8, 3.2, 2.6) and (-3.8, 3.6, 2.6), clear of the tilt pin and its actuator, down to a fixed manifold on the deck whose two ports face up at local (-3.8, 4.9, 0.5) and (-3.8, 5.3, 0.5). Its box is not centred across: the arms\' tails and the secondary cables reach 6.25 m behind the furnace\'s axis (toward the transformer), its water hoses' couplings at the deck manifold 5.49 m in front (the manifold itself is the layout's); 14 m tall with the masts at rest.",
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
            part("Slag spout", A36, spout_s, f"a steel trough {sp_w} m wide, {sp_d} m deep, 30 mm plate, {sp_L:.2f} m long, outside sizes (a clear channel {sp_w - 2 * 0.18:.2f} m wide and {sp_d - 0.18:.2f} m deep inside its wall and lining); its floor from the slag door's sill (local x -3.32, 3.2 m up) sloping 20 degrees down to x -5.0, 2.59 m up, 0.5 m past the cradle deck, so the slag falls clear into the pot"),
            part("Slag spout lining", REF, spout_r, "150 mm of refractory in the trough"),
            part("Slag spout brackets", A36, 169.8, "four seated brackets of 120 x 120 x 6 mm tube and plate that carry the trough on the cradle; measured on blender's model (eaf-vessel-80t candidate v02: tubes 83.4 kg, plates 86.4 kg)"),
            part("Water hoses", "material.silicone-rubber", 2 * 3.964 * math.pi * 0.32 * 0.02 * 1075.0, "two flexible hoses, flow and return, 300 mm bore, 20 mm wall, 3.964 m each along their centreline (measured on blender's model, v3), a supported dogleg from the vessel's ports to the deck manifold; reinforced EPDM hose, which the registry has not got: silicone rubber (1,075 kg/m3) stands for it, the wire not counted. At rest only: pressure loss, bend radius and the tilt's travel not checked"),
            part("Tilt and mast hydraulics", "good.pumps", hyd, "cylinders, pumps and valves; from memory"),
        ],
        holds=holds,
        needs={"power": P, "heat": WATER_SHARE * P, "gravity": 1.6, "atmosphere": "any"},
        recipes=mod["recipes"], life=mod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.arc-furnace), carried here for the trial: the module stays the game's until it reads layouts. Its sources are the module's."},
               {"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"From the materials and dimensions in built_of. Dimensions from memory of 80 to 100 t furnaces. The electrodes are not counted: they are a good it uses up (good.graphite-electrode). The heel, a sixth of the tap, from memory. A draft mass, not a measured total: the mast guide collars, ram seats and electrode clamps are not counted, and the strength is not checked (blender feasibility, 2026-10-13: its model matches the shell, masts and arms; its opened cradle is 0.9 t lighter)."},
               {"of": ["needs.heat"], "tier": "invented", "review": True, "note": f"The water-cooled panels and roof take about {WATER_SHARE:.0%} of the arc's power, from memory."},
               {"of": ["needs.gravity"], "tier": "invented", "review": True, "note": "Slag floats off the steel and the tap pours by gravity. Settling goes as g (Stokes), a sixth as fast at 1.6 m/s2; below that not checked."}])

    # Furnace transformer: its mass by rating.
    kg_per_va = 1.6e-3
    m = kg_per_va * S_MVA
    out["furnace-transformer-90mva"] = dict(
        name=f"Furnace transformer, {S_MVA / 1e6:.0f} MVA", size_class=2, fits="bay-s2", dims=(8.0, 5.0, 6.0),
        desc=f"Steps the grid down to about {V2:.0f} V and {I2 / 1000:.0f} kA a phase for the arcs, with on-load taps. Oil-filled in a closed tank, HV bushings and conservator on top, cooled by oil to water. Three secondary terminals on one long face (+y), 1.3 m apart, 4 m up. The oil-to-water cooler's water ports on its -x end, 1 m up. Stands in a fire-walled vault beside the furnace so the secondary bus is short.",
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
    Z_OUT, FEED_H, CONE_H, ROOF_H = 6.0, 0.8, 3.0, 0.2   # feeder outlet at the conveyor's tail, the feeder under the cone, a shallow roof
    z_cyl = Z_OUT + FEED_H + CONE_H                      # the cylinder's bottom, 9.8 m
    leg_L = z_cyl + 1.5                                  # legs carry the lower cylinder 1.5 m up its shell
    legs = 4 * box_tube(0.3, 0.3, 0.02, leg_L) * RHO["steel"]
    out["dri-bin-200t"] = dict(
        name=f"Sponge iron bin, {hold / 1000:.0f} t", size_class=1, fits="bay-s1", dims=(6.0, 6.0, r(z_cyl + Hc + ROOF_H, 3)),
        desc=f"A sealed steel bin on four legs that holds two hours of sponge iron, kept dry (it rusts and heats in wet air), with a weigh feeder under its cone onto the furnace's conveyor: feeder outlet 6 m up, 3 m south of the bin's axis (local 0, -3, 6), cone 6.8 to 9.8 m, cylinder to {z_cyl + Hc:.1f} m, shallow roof to {z_cyl + Hc + ROOF_H:.1f} m.",
        built_of=[part("Bin", A36, bin_m, f"{Db} m across, {Hc:.1f} m of cylinder over a 3 m cone, 8 mm plate"),
                  part("Legs", "stock.st4340-box-300x300x20", legs, f"four box legs 300 x 300 x 20 mm, {leg_L:.1f} m, to the lower cylinder"),
                  part("Weigh feeder", "good.electric-motors", 1500.0, "from memory")],
        holds=hold, needs={"power": 15000.0},
        basis=[{"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"Two hours of the furnace's sponge iron ({DRI:.1f} kg/s) at about 1,700 kg/m3 in bulk (from memory). The feeder's outlet 6 m up for the conveyor, the feeder 0.8 m under the cone (blender, 2026-10-14). Legs in compression about 25 MPa full; their bracing and the wind on the bin not checked."}])

    # Off-gas line and baghouse.
    Q = 800000 / 3600                         # m3/s of off-gas after dilution: from memory, for 80 to 100 t furnaces
    cloth = Q * 60 / 1.2                      # m2 at 1.2 m/min
    bags = math.ceil(cloth / (math.pi * 0.15 * 8.0))
    casing = 2 * (24 * 12 + 24 * 18 + 12 * 18) * 0.008 * RHO["steel"]
    fan = Q * 4000 / 0.75
    out["baghouse-800k"] = dict(
        name="Off-gas baghouse, 800,000 m3/h", size_class=3, fits="bay-s3", dims=(24.0, 12.0, 18.0),
        desc="Cleans the furnace's fume before it goes to the air: filter bags in a closed 18 x 8 m steel casing over six hoppers, the dust shaken down and let out through sealed valves; two fans with their stacks in the 6 m strip at one end draw the gas through. The gas comes in at the top of one long side (local 0, -6, 15) from the layout's water-cooled duct and burn-out gap.",
        built_of=[part("Casing and hoppers", A36, casing, "24 x 12 x 18 m, 8 mm plate"),
                  part("Structure", A36, casing * 0.5, "half the casing, from memory"),
                  part("Filter bags", "material.ptfe", cloth * 0.5, f"{bags} bags 150 mm across and 8 m long, {cloth:.0f} m2 of PTFE-membrane cloth at 0.5 kg/m2"),
                  part("Fans and motors", "good.electric-motors", 2 * 8000.0, "two induced-draught fans, from memory")],
        holds=0.015 * RATE * 3600 * 4, needs={"power": fan},
        basis=[{"of": ["physical", "built_of", "needs.power"], "tier": "invented", "review": True, "note": f"Off-gas 800,000 m3/h, 1.2 m/min through the cloth, fans at 4 kPa and 75%: from memory. Fan power {fan / 1e6:.2f} MW, not in the module's figure."},
               {"of": ["holds"], "tier": "derived", "note": "Four hours of the recipe's dust in the hoppers."}])
    return out


def cooling_unit(heat, whose="the furnace's"):
    """The cooling tower, sized to the heat the loop gathers."""
    cells = math.ceil(heat / 10e6)
    flow = heat / (4186 * 10)                 # kg/s at a 10 K rise
    evap = heat / 2.26e6
    pumps = flow / 1000 * 400e3 / 0.75
    fans = 0.008 * heat
    return dict(
        name=f"Cooling tower, {cells} cells", size_class=3 if cells <= 2 else 4, fits="bay-s3" if cells <= 2 else "bay-s4", dims=(12.0 * cells, 12.0, 14.0),
        desc=f"An induced-draught wet tower: {whose} closed water loop gives up its heat through a plate exchanger to tower water, which sprays down through fill against a fan's draught and evaporates. The loop's pumps and exchanger stand on a skid inside its footprint; the loop leaves by two pairs of 500 mm ports on its east end (local x +{6.0 * cells:.0f}, 0.75 m up, at y 0 and +4.5), and the layout's pipes and concrete basin are not the unit's. On a world with air only; airless ground needs radiators instead.",
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
        desc="A refractory-lined steel ladle that takes one tap, seated in a cradle on a motor car that runs on rails from under the furnace's tap to the ladle station. It travels along its x; eight wheels on four axles at 3 m gauge, its origin on the rails' running plane (the flanges reach 40 mm below); the ladle's lip 4.4 m up.",
        built_of=[part("Ladle lining", REF, lining, f"{Di} m inside, {Hi} m deep, 0.2 m walls, 0.3 m bottom"),
                  part("Ladle shell and trunnion ring", A36, shell * 1.3, "40 mm plate, the trunnion ring and lugs three tenths more"),
                  part("Car frame and deck", "stock.st4340-box-400x400x25", frame_ + deck, "four box beams 400 x 400 x 25 mm, 6 m, and a deck 6 x 4 m of 40 mm plate"),
                  part("Drives", "good.electric-motors", 2000.0, "from memory")],
        holds=HEAT * 1.1, needs={"power": 50000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Sized to hold a tap of 80 t of steel (7,000 kg/m3) with its slag cover and freeboard: 17 m3 inside. Lining thicknesses from memory."}])
    vol = SLAG / 1500 * 1.25
    R1, R2, tp = 1.45, 1.25, 0.08                                    # pot radii inside, top and bottom, and its wall (blender, 2026-10-14)
    Hp = vol / (math.pi / 3 * (R1 ** 2 + R1 * R2 + R2 ** 2))         # its depth inside for the volume
    frustum = lambda a, b, h: math.pi / 3 * h * (a ** 2 + a * b + b ** 2)
    pot_m = (frustum(R1 + tp, R2 + tp, Hp + tp) - frustum(R1, R2, Hp)) * RHO["steel"]
    out["slag-pot-car"] = dict(
        name=f"Slag pot car, {vol:.0f} m3", size_class=1, dims=(5.0, 3.5, 3.4),
        desc="A tapered cast steel pot (2.9 m across at the top, 2.5 m at the bottom, 1.74 m deep) seated on a four-wheel rail car that runs under the furnace's slag door and takes the slag out to be cooled and broken. It travels along its length; 2.5 m gauge between the heads' inner faces, its origin on the running plane; the pot's lip 3.4 m up. The slag falls from the vessel's door through the platform's slag tunnel; the car has no chute.",
        built_of=[part("Pot", A36, pot_m, f"cast steel 80 mm: a frustum {2 * R1:.1f} m across at the top, {2 * R2:.1f} m at the bottom, {Hp:.2f} m deep inside, {vol:.0f} m3"),
                  part("Trunnion ring and pins", A36, 2516.0, "measured on blender's model (slag-pot-car candidate v01: ring 2,200 kg, pins 316 kg)"),
                  part("Car", A36, 9000.0, "from memory"),
                  part("Drives", "good.electric-motors", 1500.0, "from memory")],
        holds=SLAG, needs={"power": 30000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"A tap's slag ({SLAG / 1000:.0f} t, the recipe's 0.15) foamed to about 1,500 kg/m3 (from memory), a quarter to spare."}])
    # Hall crane: a double-girder bridge, rails at y -11.4 and +11.4 so its ends (0.3 m past the wheels) clear the wall's inner nodes (y +/-12, 160 mm balls) by about 0.2 m (blender, assembly check).
    span, cap = 22.8, 20000.0
    g_len = span + 0.6                       # the girders run over the end carriages to the crane's ends; they bend over the span between the rails
    gird = 2 * box_tube(0.6, 1.4, 0.012, g_len) * RHO["steel"]
    I = (0.6 * 1.4 ** 3 - (0.6 - 0.024) * (1.4 - 0.024) ** 3) / 12   # one girder, the box exactly
    defl = cap * G * span ** 3 / (48 * 205e9 * 2 * I)
    defl_all = (cap + 6000.0) * G * span ** 3 / (48 * 205e9 * 2 * I) + 5 * gird / g_len * G * span ** 4 / (384 * 205e9 * 2 * I)   # with the trolley and the girders' own weight
    out["hall-crane-20t"] = dict(
        name="Hall crane, 20 t", size_class=2, dims=(6.0, r(g_len, 3), 3.0),
        desc=f"A double-girder bridge crane on the hall's runways, for the roof, the electrodes and relining. Hangs on the frame; names no bay. It travels along its x on four wheels, two end carriages 6 m long, their wheels at y -11.4 and +11.4 ({span} m between the rails), their ends 0.3 m beyond; its origin on the runway's rail plane, the two girders 1.8 m either side of it; the trolley runs 9.5 m either way along y, the hook raised to 0.1 m under the rail plane. Parked at the hall's centre.",
        built_of=[part("Bridge girders", A36, gird, f"two box girders 1.4 x 0.6 m, 12 mm wall, {g_len:.1f} m long over a {span} m span: at mid-span {defl * 1000:.1f} mm under the 20 t load alone (span/{span / defl:.0f}), {defl_all * 1000:.1f} mm with the trolley and their own weight (span/{span / defl_all:.0f}); checked by blender, 2026-10-14"),
                  part("Trolley and hoist", A36, 6000.0, "from memory"),
                  part("End carriages and drives", "good.electric-motors", 4000.0, "from memory")],
        needs={"power": 60000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Girders sized for 20 t at mid-span within span/800, which they meet with the trolley and their own weight too; trolley and drives from memory; the hook, reeving and drives not sized."}])
    return out


# ---------------------------------------------------------------- the ladle station's figures
lmod = yaml.safe_load(open(S + "modules/ladle-station.yaml"))
LREC = lmod["recipes"][0]
L_P, L_RATE, L_HEAT = LREC["power"], LREC["rate"], LREC["batch"]   # W (sourced, 50 kWh a tonne), kg/s and kg (invented)
L_V2 = 300.0                          # secondary volts of a ladle furnace, from memory
L_HEATING = 4.0 / 60                  # K/s: a ladle furnace heats its melt about 4 degrees a minute, from memory
L_ARC_EFF = 0.4                       # share of the arc's power that reaches the steel, from memory
L_ARC = L_HEAT * 820.0 * L_HEATING / L_ARC_EFF     # W of arc to heat the melt at that rate (liquid steel about 820 J/kg K)
L_MVA = L_ARC / PF
L_I2 = L_MVA / (math.sqrt(3) * L_V2)
L_WATER = 0.20                        # share of the arc's power the water-cooled roof takes, from memory
VD_PUMPS = 600e3                      # W of the degasser's dry screw and Roots pumps, from memory


def transformer_unit(mva, purpose):
    """A furnace transformer by its rating: 1.6 t a MVA, sized in proportion to the 90 MVA one (8 x 5 x 6 m)."""
    m = 1.6e-3 * mva
    k = (mva / S_MVA) ** (1 / 3)
    L, W, H = (round(8.0 * k, 1), round(5.0 * k, 1), round(6.0 * k, 1))
    return dict(
        name=f"Furnace transformer, {mva / 1e6:.0f} MVA", size_class=1 if max(L, W) <= 6 else 2, fits="bay-s1" if max(L, W) <= 6 else "bay-s2", dims=(L, W, H),
        desc=f"Steps the grid down for {purpose}, with on-load taps. Oil-filled in a closed tank, HV bushings and conservator on top, cooled by oil to water. Three secondary terminals on one long face (+y), {H * 2 / 3:.1f} m up; the oil-to-water cooler's water ports on its -x end, 1 m up. Stands in a fire-walled vault.",
        built_of=[part("Core", A36, 0.40 * m, "grain-oriented electrical steel, which the registry has not got: structural steel stands for it"),
                  part("Windings", CU, 0.25 * m, ""),
                  part("Tank and fittings", A36, 0.15 * m, ""),
                  part("Oil", "material.hydraulic-fluid", 0.20 * m, "transformer mineral oil, which the registry has not got: hydraulic fluid stands for it")],
        needs={"power": TRANSFORMER_LOSS * mva, "heat": TRANSFORMER_LOSS * mva},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": f"1.6 t a MVA and the shares of core, copper, tank and oil from memory of furnace transformers; its size scaled from the 90 MVA one by the cube root of the rating."},
               {"of": ["needs"], "tier": "invented", "review": True, "note": f"Losses {TRANSFORMER_LOSS:.1%} of its rating, carried off by the cooling loop; from memory."}])


def baghouse_unit(q_m3h, dims):
    """A pulse-jet baghouse for q_m3h of fume, in a casing of dims."""
    Q = q_m3h / 3600
    cloth = Q * 60 / 1.2
    bags = math.ceil(cloth / (math.pi * 0.15 * 8.0))
    L, W, H = dims
    casing = 2 * (L * W + L * H + W * H) * 0.008 * RHO["steel"]
    fan = Q * 4000 / 0.75
    return dict(
        name=f"Fume baghouse, {q_m3h:,.0f} m3/h", size_class=2 if max(L, W) <= 12 else 3, fits="bay-s2" if max(L, W) <= 12 else "bay-s3", dims=dims,
        desc=f"Cleans a furnace's fume before it goes to the air: filter bags in a closed steel casing over hoppers, the dust shaken down and let out through sealed valves; a fan and its stack at one end draw the gas through. The gas comes in at the top of one long side (local 0, -{W / 2:g}, {H - 2:g}).",
        built_of=[part("Casing and hoppers", A36, casing, f"{L:g} x {W:g} x {H:g} m, 8 mm plate"),
                  part("Structure", A36, casing * 0.5, "half the casing, from memory"),
                  part("Filter bags", "material.ptfe", cloth * 0.5, f"{bags} bags 150 mm across and 8 m long, {cloth:.0f} m2 of PTFE-membrane cloth at 0.5 kg/m2"),
                  part("Fan and motor", "good.electric-motors", 8000.0 * q_m3h / 400000, "an induced-draught fan, 8 t for 400,000 m3/h, in proportion; from memory")],
        needs={"power": fan},
        basis=[{"of": ["physical", "built_of", "needs.power"], "tier": "invented", "review": True, "note": f"{q_m3h:,.0f} m3/h, 1.2 m/min through the cloth, the fan at 4 kPa and 75%: from memory. Fan power {fan / 1e3:.0f} kW."}])


def bridge_crane_unit(cap, span, trolley, kind="Ladle crane", job="it lifts and carries a full ladle: off its car into the degasser and back, or over the moulds to pour", gear="a main hoist and a ladle beam", g=G, h_min=1.4):
    """A double-girder bridge crane: the lightest pair of box girders that keeps the load's deflection within span/800 and
    their bending stress, all loads on, within 150 MPa (0.6 of A36's yield)."""
    best = None
    for h in [x / 10 for x in range(round(h_min * 10), 31)]:
        for t in (0.012, 0.016, 0.020, 0.025):
            b, g_len = 0.8, span + 0.6
            I = (b * h ** 3 - (b - 2 * t) * (h - 2 * t) ** 3) / 12
            m = 2 * box_tube(b, h, t, g_len) * RHO["steel"]
            defl = cap * g * span ** 3 / (48 * 205e9 * 2 * I)
            M = (cap + trolley) * g * span / 4 + m / g_len * g * span ** 2 / 8
            sigma = M / (2 * I / (h / 2))
            if defl <= span / 800 and sigma <= 150e6 and (best is None or m < best[0]):
                best = (m, h, t, I, defl, sigma)
    m, h, t, I, defl, sigma = best
    ends = 0.15 * cap                                     # end carriages, wheels and drives: from memory
    return dict(
        name=f"{kind}, {cap / 1000:.0f} t", size_class=2, dims=(8.0, r(span + 0.6, 3), r(h + 1.6, 3)),
        desc=f"A double-girder {kind.lower()} on the hall's runways: {job}. Hangs on the frame; names no bay. It travels along its x, its wheels at y -{span / 2:g} and +{span / 2:g} ({span} m between the rails), its ends 0.3 m beyond; its origin on the runway's rail plane; the trolley, with {gear}, runs 9 m either way along y, the hook raised to 0.1 m under the rail plane. Parked at the hall's centre.",
        built_of=[part("Bridge girders", A36, m, f"two box girders {h:.1f} x 0.8 m, {t * 1000:.0f} mm wall, {span + 0.6:.1f} m long over a {span} m span, the lightest that meet span/800 under the load ({defl * 1000:.1f} mm, span/{span / defl:.0f}) and 150 MPa with every load on ({sigma / 1e6:.0f} MPa)"),
                  part("Trolley and hoist", A36, trolley, f"with {gear}: {trolley / cap:.0%} of the load, from memory"),
                  part("End carriages and drives", "good.electric-motors", ends, "15% of the load, from memory")],
        needs={"power": 2.0 * cap},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Girders sized as the desc says; trolley and end carriages from memory; the hook, reeving and drives not sized."},
               {"of": ["needs.power"], "tier": "invented", "review": True, "note": "2 W a kilogram of rated load, from memory."}])


def ladle_units():
    """The ladle station's own units."""
    out = {}
    # Ladle furnace: a gantry over the ladle rail carrying a water-cooled roof, three electrodes on masts and their arms.
    roof_s, roof_w = pipe_panel(1.15 * math.pi * 2.3 ** 2)
    delta = math.pi * 0.6 ** 2 * 0.3 * RHO["refractory"]
    gantry = (4 * box_tube(0.6, 0.6, 0.025, 12.0) + 2 * box_tube(0.8, 0.5, 0.025, 10.0)) * RHO["steel"]
    masts = 3 * box_tube(0.5, 0.5, 0.025, 7.0) * RHO["steel"]
    arms = 3 * box_tube(0.5, 0.35, 0.02, 4.0) * RHO["aluminium"]
    cable_area = L_I2 / 2 / 5e6
    cables = 3 * 2 * 10.0 * cable_area * RHO["copper"]
    out["ladle-furnace-80t"] = dict(
        name="Ladle furnace, 80 t", size_class=2, fits="bay-s2", dims=(12.0, 10.7, 14.0),   # y -5.34 to 5 on blender's model: the cables' hanger at the rear
        desc=f"Reheats and trims an 80 t heat in its ladle, the ladle standing on its car under the furnace: a steel gantry straddles the ladle rail (a lane 5 m wide left clear under it along x), carrying a water-cooled roof 4.6 m across that is lowered onto the ladle (an annular seat skirt, radius 1.45 to 1.70 m, sits on its rim), three graphite electrodes on masts with their arms, and an alloy chute through the roof. Its arc, about {L_ARC / 1e6:.1f} MW at full heat, comes from its own transformer by a bus on its -y side (local 0, -5, 6); its cooling water by a manifold at local (-4.5, -4.5, 1); the alloys by a chute whose top stands at local (0, 3, 9); its fume leaves by an elbow on the roof at local (2, 0, 7.5). Argon is blown through the ladle's porous plug from the car. At rest: the roof's rim on the ladle's lip (z 4.4), its crown at 5.2; three 300 mm graphite electrodes on a 0.35 m pitch radius through the roof's delta; the masts at the rear (y -3.8), their arms fanning forward, the centre arm 0.6 m higher; the gantry's columns at x -5 and 5, y -4.4 and 4.4, 12 m tall, its two cross beams on top.",
        built_of=[part("Roof panels", A36, roof_s * 1.5, "water-cooled pipe panels over a dome 4.6 m across, 1.15 times the disc, the lifting frame half as much again"),
                  part("Roof delta", REF, 0.2572 * RHO["refractory"], "the refractory centre round the electrode ports, 1.2 m across, 0.3 m thick, less its electrode, feed and fume bores: 0.257 m3 as measured on blender's model (candidate v01)"),
                  part("Gantry", A36, 53897.6, f"four box columns 0.6 m, 25 mm wall, 12 m tall, two cross beams 0.8 x 0.5 m, 10 m, longitudinal ties, the mast bench, the roof-lift heads, a grounded hanger beam over the mast bank for the cables and 200 mm baseplates (these provisional, not load-sized): measured on blender's model (candidate v01); columns and beams alone {gantry:,.0f} kg"),
                  part("Electrode masts", A36, masts, "three box masts 0.5 m, 25 mm wall, 7 m"),
                  part("Current-conducting arms", AL, 843.0, "three box arms 0.5 x 0.35 m, 20 mm wall, trimmed at their insulated carriages: measured on blender's model (candidate v01)"),
                  part("Secondary cables", CU, 1528.3, f"two water-cooled copper cables a phase, each {cable_area * 1e6:.0f} mm2 for half of {L_I2 / 1000:.0f} kA at 5 A/mm2, run behind the steel and over the top on six insulators: measured on blender's model ({cables:,.0f} kg for 10 m each)"),
                  part("Roof lift and mast hydraulics", "good.pumps", 6000.0, "from memory")],
        holds=roof_w, needs={"power": L_P, "heat": L_WATER * L_P, "gravity": 1.6, "atmosphere": "any"},
        recipes=lmod["recipes"], life=lmod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipes (module.ladle-station), carried here: the module stays the game's until it reads layouts. Their sources are the module's; its power, 50 kWh a tonne, is the average the arc draws."},
               {"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"From the materials and dimensions in built_of; dimensions from memory of 80 to 100 t ladle furnaces. The electrodes are a good it uses, not counted. Secondary {L_V2:.0f} V, from memory."},
               {"of": ["needs.heat"], "tier": "invented", "review": True, "note": f"The water-cooled roof takes about {L_WATER:.0%} of the arc's power, from memory."}])
    out["furnace-transformer-14mva"] = transformer_unit(L_MVA, f"the ladle furnace's arc: {L_ARC / 1e6:.1f} MW to heat 80 t at {L_HEATING * 60:.0f} degrees a minute, {L_ARC_EFF:.0%} of it reaching the steel, at a power factor of {PF}; about {L_V2:.0f} V and {L_I2 / 1000:.0f} kA a phase")
    # Vacuum tank degasser: the ladle is lowered into a tank in a pit, the lid closed, the tank pumped down while argon stirs.
    Ti, Th = 4.5, 5.5
    VD_LINE = 11.04                    # m of vacuum line, measured on blender's model
    tank = cyl_shell(Ti + 0.08, Th, 0.04, bottom=1.0) * RHO["steel"]
    tank_ref = math.pi * (Ti / 2) ** 2 * 0.3 * RHO["refractory"]
    lid = math.pi * 2.6 ** 2 * 1.15 * 0.05 * RHO["steel"]
    out["vacuum-degasser-80t"] = dict(
        name="Vacuum tank degasser, 80 t", size_class=2, fits="bay-s2", dims=(12.0, 10.0, 9.0),
        desc="Takes the dissolved hydrogen and nitrogen out of an 80 t heat: the crane lowers the ladle into a steel tank standing in a pit, a lid on a car closes it, and pumps take it down to about 67 Pa while argon stirs the melt. Local frame: the tank's centre at (-3, 2), 4.5 m inside and 5.5 m deep, its rim at the floor; the lid car on its own rails parks at (3, 2); the pump set fills the strip y -5 to -2 along the unit, its vacuum line to a flange on the tank's side. 3 m above the floor and 6 m below it (the pit is the layout's).",
        built_of=[part("Tank", A36, 30893.7, f"{Ti} m inside, {Th} m deep, 40 mm plate, its bottom the same, with its rim; measured on blender's model ({tank:.0f} kg as a plain shell)"),
                  part("Tank floor lining", REF, 14290.9, "0.3 m of refractory under the ladle, measured"),
                  part("Lid", A36, 9320.9, "a shallow dish 5.2 m across, 50 mm plate, measured"),
                  part("Tank seats", A36, 2717.7, "four hollow seats of 25 mm plate with their feet, from the tank's bottom to the pit floor at z -6; measured"),
                  part("Lid car and rails", A36, 12749.6, "12 mm hollow main beams and the bridge girders over the pit's open ring measured; the drive and axles still an allowance"),
                  part("Vacuum pump set", "good.pumps", 25000.0, "dry screw pumps behind Roots boosters, from memory"),
                  part("Vacuum line", A36, 3061.0, f"1 m across, 10 mm, {VD_LINE} m of main, header and branches (measured centreline) with flanges, run at z -1.2 under the rail bridge; measured")],
        needs={"power": VD_PUMPS},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "Sizes and the pumps' power from memory of tank degassers for 80 to 100 t ladles."}])
    # Alloy bins: eight bins on a steel stand, a weigh hopper and a belt to the furnace's chute.
    Db, Hc, Hk = 2.5, 3.0, 1.5
    vol1 = math.pi * (Db / 2) ** 2 * Hc + math.pi * (Db / 2) ** 2 * Hk / 3
    bins = 8 * (cyl_shell(Db, Hc, 0.006) + math.pi * Db / 2 * math.hypot(Db / 2, Hk) * 0.006) * RHO["steel"]
    stand = (6 * box_tube(0.3, 0.3, 0.02, 9.7) + 2 * box_tube(0.4, 0.4, 0.025, 12.0) + 10 * box_tube(0.3, 0.3, 0.02, 2.1)) * RHO["steel"] + 12 * 6 * 0.01 * RHO["steel"]
    out["alloy-bins-8"] = dict(
        name="Alloy bins, 8", size_class=3, fits="bay-s3", dims=(12.56, 6.3, 15.14),   # blender's model (v1): the legs' feet reach x 6.28, the walkway's edge beams y 3.15, the fill caps z 15.14; past a class 2 bay's 12 m
        desc=("Eight steel bins of ferroalloys and fluxes hung in a deck 12 m up, two rows of four at local x -4.5, -1.5, 1.5, 4.5 and y -1.5, 1.5: "
              f"each {Db} m across, its cylinder z 12 to 15, its cone down to an outlet at z 10.5. Under each outlet a vibrating feeder (z 10 to 10.5) sends its alloy inward to a weigh hopper "
              "shared by the pair across y, four hoppers on load cells at x -4.5, -1.5, 1.5, 4.5, y 0, z 8.6 to 9.8, each weighing up to 2 t. "
              "The hoppers empty onto two collecting belts at z 8, the west two running east and the east two running west, both into a transfer chute at x 0, y 0, "
              "whose foot at local (0, 0, 7.4) drops onto the layout's belt to the ladle furnace's chute. "
              "Load path: each bin hangs by a bearing collar (radius about 1.5 m) on uprights at x -6, -3, 0, 3, 6, each upright shared by two bins; the uprights stand on two row beams under the bins' centrelines (y -1.5, 1.5, top z 9.9, below the feeders); the row beams on six legs at x -6, 0, 6, y -1.5, 1.5, braced across. A 10 mm walkway deck at z 12 round the collars."),
        built_of=[part("Bins", A36, 14341.4, f"8 bins {Db} m across, {Hc} m of cylinder over a {Hk} m cone, 6 mm plate, with roofs, fill necks, caps and gates; measured ({bins:.0f} kg as plain shells)"),
                  part("Stand", "stock.st4340-box-300x300x20", 45758.3, f"six legs 9.7 m and ten uprights 2.1 m of 300 x 300 x 20 mm box, two 12 m row beams of 400 x 400 x 25 mm box (a full bin at mid-span of 6 m: 730 kN m, 165 MPa), braces, collars, feet, guards and a holed 12 x 6 m walkway deck of 10 mm plate (its access at local 6, 0, 12); measured ({stand:.0f} kg in the main members alone)"),
                  part("Feeders, weigh hoppers and collecting belts", "good.electric-motors", 6000.0, "eight vibrating feeders, four weigh hoppers on load cells, two short belts and a transfer chute, from memory")],
        holds=8 * vol1 * 3500 * 0.8, needs={"power": 40000.0},
        basis=[{"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"Eight bins of {vol1:.1f} m3 at about 3,500 kg/m3 of ferroalloy lumps (from memory), four fifths full."}])
    out["baghouse-200k"] = baghouse_unit(200000, (12.0, 8.0, 12.0))
    # Stair tower: four flights to a 12 m walkway in a 3 x 6 m lattice of kit struts.
    sg, _, sn, _ = lattice((-1.5, -3.0, 0.0, 1.5, 3.0, 12.0), 3.0, 3, None, [])
    ssec = frames.lightest_section((4 * 750.0 + 3.0 * 6.0 * 5000.0) * G / 4, with_diagonal=True)
    inner = {False: 3, True: 4}                       # the y 0 face's three ties and four diagonals left out: they would cross the flights (blender)
    sframe = sum((n - (inner[diag] if g_ == "wall" else 0)) * strut(ssec, 3.0, diag)[1]["physical"]["mass"] for g_ in ("wall", "roof") for diag, n in ((False, sg[g_][0]), (True, sg[g_][1])))
    snode = yaml.safe_load(open(S + "mill-stock/" + strut(ssec, 3.0)[1]["kit"]["node"].split(".", 1)[1].upper() + ".yaml"))
    sframe += sn * frames.node_mass(snode, frames.materials())
    out["stair-tower-12m"] = dict(
        name="Stair tower, 12 m", size_class=1, fits="bay-s1", dims=(3.0, 6.0, 13.1),
        desc="Four switchback flights of steel stairs (each 18 risers of 167 mm over a 4 m run, 0.9 m wide) with 1 m landings at the ends, inside a perimeter lattice of kit struts 3 x 6 m (none across its middle, where the flights pass); the top landing at z 12 on its -x face at local (-1.5, 0, 12) meets a walkway, its guard rails to z 13.1. For people and hand tools only.",
        built_of=[part("Stairs and landings", A36, 4 * 750.0, "four flights with treads and rails and their landings, 750 kg each, from memory"),
                  part("Frame", f"stock.{ssec}-s3000", sframe, f"the lattice in {ssec} with its nodes, the kit's lightest for its own weight and 5 kPa on its landings with half again to spare")],
        needs={},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Frame counted from its lattice; the stairs from memory."}])
    out["ladle-crane-150t"] = bridge_crane_unit(150000.0, 22.8, 37500.0)
    return out


def lattice(box, cell, top_k, wall_cells, openings, cuts=()):
    """Members of a lattice of cubic cells: chords along the edges, one diagonal in each square face. An opening drops the cells
    whose centres lie in it; a cut drops only the members whose middles lie inside it (a stair through cells, a doorway in a face). Returns
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
    if cuts:
        mid = lambda pts: [x0 + cell * sum(p[n] for p in pts) / len(pts) if n == 0 else (y0 if n == 1 else z0) + cell * sum(p[n] for p in pts) / len(pts) for n in range(3)]
        inside = lambda m: any(a < m[0] < d and b < m[1] < e and c < m[2] < f for a, b, c, d, e, f in cuts)
        edges = {e: g for e, g in edges.items() if not inside(mid(e))}
        def face_mid(fk):
            ax, lv, u, v = fk
            q = [0.0, 0.0, 0.0]
            q[ax] = lv
            rest = [n for n in range(3) if n != ax]
            q[rest[0]], q[rest[1]] = u + 0.5, v + 0.5
            return mid([tuple(q)])
        faces = {fk: g for fk, g in faces.items() if not inside(face_mid(fk))}
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
    if u.get("rejects"):
        d["rejects"] = {k: r(v) for k, v in u["rejects"].items()}
    if u.get("recipes"):
        d["recipes"] = u["recipes"]
    d["life"] = u.get("life") or mod.get("life")
    d["basis"] = u["basis"] + [{"of": ["life"], "rule": "rule.life-plant"}]
    return d


def link(kind, name, a, b, item, per_m, power=None, note=None, heat=None, via=()):
    pts = [a, *via, b]
    L = sum(math.dist(p, q) for p, q in zip(pts, pts[1:]))
    d = {"kind": kind, "name": name, "from": list(a), "to": list(b), "length": r(L), "item": item, "mass": r(per_m * L)}
    if via:
        d["via"] = [list(v) for v in via]
    if power:
        d["power"] = r(power)
    if heat:
        d["heat"] = r(heat)
    if note:
        d["note"] = note
    return d


def arc_furnace_air(us, recs):
    """The trial: the arc furnace shop on a world with air at 1 g. Returns the layout and its bays."""
    # The layout: hall x[-24, 9] y[-15, 15], 3 m lattice walls one cell thick, a roof one cell deep at 24 m.
    # West wall one cell out from the slag rail (x -16.5) so the 3.5 m slag car runs 2.75 m clear of it (blender, assembly check).
    cell, box = 3.0, (-24.0, -15.0, 0.0, 9.0, 15.0, 27.0)
    doors = [(6.0, 3.0, 0.0, 9.0, 9.0, 6.0),          # ladle rail, east
             (-21.0, 12.0, 0.0, -12.0, 15.0, 6.0),    # slag rail, north: 9 m, centred on the rail
             (-15.0, 12.0, 9.0, -9.0, 15.0, 15.0),    # sponge iron conveyor through the north wall, z 10.9 to 11.9 with its gallery
             (-9.0, 12.0, 12.0, 0.0, 15.0, 18.0)]     # off-gas duct, 2 m across, through the north wall (its slant reaches the jamb at x -6)
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
    runway = 2 * (x1_in := box[3] - box[0] - 2 * cell) * frames.area({"identity": {"form": "box"}, "size": {"width": 0.4, "height": 0.4, "wall": 0.025}}) * RHO["steel"]
    members.append({"item": "stock.st4340-box-400x400x25", "count": 4, "where": f"crane runways, two a side, {x1_in / 2:.1f} m lengths, along x on the inside of the long walls: their rail plane 20.5 m up at y -11.4 and +11.4, 3.5 m under the roof's bottom chord"})
    fmass += runway
    x0, y0, z0, x1, y1, z1 = box
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in doors)
    FLASH = 12.67 + 35.43 + 35.41 + 50.05      # m2 of roof edge and corner returns and the conveyor and duct flashings, measured on blender's hall (candidate v01)
    clad_net = clad_area
    clad_area += FLASH
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
        {"id": "C", "mount": "plant-mount.bay-s3", "unit": "unit.cooling-tower", "at": [-39.0, -6.0, 0.0], "note": "outside, west, 3 m from the hall"},
    ]

    pipe_m = math.pi * 0.5 * 0.008 * RHO["steel"] * 2
    tr_pipe_m = math.pi * 0.15 * 0.005 * RHO["steel"] * 2
    bus_area = I2 / 4e6
    links = [
        link("conveyor", "sponge iron feed", (-12.0, 30.0, 6.0), (-12.0, 7.8, 13.7), A36, 150.0, DRI * G * 7.7 / 0.5 + 20000, f"0.8 m chevron-cleated belt in a covered gallery, 150 kg/m with the gallery (from memory), {math.degrees(math.atan2(7.7, 22.2)):.1f} degrees: steeper than a plain belt carries pellets (they roll back above about 15 degrees), so cleated; its head 0.4 m over the roof's fifth hole, 1.8 m north of the electrodes, with a short drop chute into it"),
        link("duct", "off-gas", (-10.0, 7.5, 13.65), (6.0, 24.0, 15.0), A36, math.pi * 2.0 * 0.010 * RHO["steel"], None, f"2 m across, 10 mm plate, water-cooled for its first part (it takes about {OFFGAS_SHARE:.0%} of the arc's power from the hot gas, from memory), starting above the burn-out gap: 0.15 m between the top of the vessel's elbow (z 13.5, it moves with the furnace) and the fixed duct's catch hood (from z 13.65), so the furnace can tilt and air is drawn in to burn the CO (0.15 m nominal between the port planes, adjustable on real plant; on blender's models the elbow's flange reaches z 13.61, leaving 44 mm at the least; not sized for combustion); from the roof's fourth hole, 2 m east and 1.5 m north of the electrodes: up from the hole, east-north-east through the north wall's opening, then along +y into the baghouse's inlet, with elbows at the turns", heat=OFFGAS_SHARE * P, via=[(-10.0, 7.5, 15.0), (-4.5, 13.5, 15.0), (-3.0, 16.6, 15.0), (6.0, 19.0, 15.0)]),
        link("busbar", "secondary bus", (-12.0, -3.5, 4.0), (-12.0, 0.8, 7.3), CU, 3 * bus_area * RHO["copper"], None, f"three water-cooled copper tube bars, {bus_area * 1e6:.0f} mm2 each for {I2 / 1000:.0f} kA at 4 A/mm2; from the transformer's three secondary terminals (its +y face, x -1.3, 0 and +1.3 m, 4 m up) through the vault wall to three insulated terminals spread 1.3 m apart along x behind the mast bank, then the furnace's cables"),
        link("rail", "ladle rail", (-10.5, 6.0, 0.0), (33.0, 6.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails (heads 72 mm wide) at 3 m gauge between the heads' inner faces, as railways measure it; set in the floor, their running plane the floor (z 0), with flangeways 40 mm deep beside the heads, under the tap, out of the hall's east door to the ladle station; from and to are the rails' ends, with buffers: the car's centre stops between x -7.5 (under the tap) and 30"),
        link("rail", "slag rail", (-16.5, 0.5, 0.0), (-16.5, 23.25, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails at 2.5 m gauge between the heads' inner faces, set in the floor with 40 mm flangeways, running plane the floor; under the slag door, out of the north door; from and to are the rails' ends, with buffers, the south end 0.5 m clear of the vault's wall: the car's centre stops between y 2.75 and 21, under the spout at y 6"),
        link("pipe", "cooling water to the hall", (-27.0, -6.0, 0.75), (-15.8, 11.1, 5.5), A36, pipe_m, None, "a pair of 500 mm pipes, 8 mm wall, flow and return, from the tower's first port pair: up to 1.5 m before the west wall (clear of its diagonals), east through a sleeve at y -6, north in the gap between the wall and the vault, up beside the platform's south-west corner, north along the deck's west edge (shielded where it passes the slag well) and east along its north edge to a fixed manifold beside the vessel's water ports; the vessel's own flexible hoses (it tilts) run from the manifold to its panels and roof", via=[(-25.5, -6.0, 0.75), (-25.5, -6.0, 1.5), (-19.5, -6.0, 1.5), (-19.5, -0.5, 1.5), (-19.5, -0.5, 5.5), (-19.5, 11.1, 5.5)]),
        link("pipe", "cooling water to the transformer", (-19.5, -6.0, 1.5), (-16.0, -6.0, 1.0), A36, tr_pipe_m, None, f"a pair of 150 mm pipes, 5 mm wall, teed off the hall pair, through the vault's west wall to the transformer's cooler ports (its -x end, 1 m up): {TRANSFORMER_LOSS * S_MVA / 1e3:.0f} kW at a 10 K rise is {TRANSFORMER_LOSS * S_MVA / (4186 * 10):.0f} kg/s", via=[(-16.5, -6.0, 1.5)]),
        link("pipe", "cooling water to the off-gas duct", (-27.0, -1.5, 0.75), (1.3, 17.8, 14.0), A36, pipe_m, None, "a pair of 500 mm pipes, 8 mm wall, flow and return, from the tower's second port pair: outside the hall, down into a covered trench 1 m deep, north past the hall's west end, east along y 18 under the slag rail, up a riser to the underside of the duct's water jacket just outside the hall's north wall; the jacket carries the water back along the duct's water-cooled first part (the baghouse takes no water)", via=[(-26.0, -1.5, 0.75), (-26.0, -1.5, -1.0), (-26.0, 18.0, -1.0), (1.3, 18.0, -1.0), (1.3, 18.0, 13.0)]),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-7.5, 6.0, 0.0]},
              {"unit": "unit.slag-pot-car", "rides": "slag rail", "at": [-16.5, 6.0, 0.0]}]

    # Furnace platform, top 5 m (the vessel's seat), x -20 to -6, y 0 to 12, on a 1 m slab. The rockers bear on it at y about 2 and 10.
    # Ladle slot, open above, between the rockers: x -11 to -6 (the car stowed at x -7.5 reaches -10.5), y 3.5 to 8.5, the tap over it. Slag tunnel along y under the slag door:
    # x -19 to -14, 4.4 m clear (a 0.6 m deck, the pot's lip 1 m under it), and a 3 x 3 m well in its deck where the spout's stream falls.
    P0, P1, PY = (-20.0, -6.0), (0.0, 12.0), 5.0
    slot = 5.0 * 5.0 * PY
    tunnel, well = 5.0 * 12.0 * 4.4, 3.0 * 3.0 * 0.6
    plat_v = (P0[1] - P0[0]) * (P1[1] - P1[0]) * PY - slot - tunnel - well + (P0[1] - P0[0]) * (P1[1] - P1[0]) * 1.0
    plat_note = ("x -20 to -6, y 0 to 12, top 5 m, on a 1 m slab; a ladle slot open above between the rockers (x -11 to -6, y 3.5 to 8.5, the tap over it); "
                 "a slag tunnel along y (x -19 to -14, 4.4 m clear under a 0.6 m deck) with a 3 x 3 m well in the deck (x -18.5 to -15.5, y 4.5 to 7.5) under the spout")
    tw = us["cooling-tower"]["dims"][0]
    ground = [
        {"what": "Furnace platform", "item": CONC, "volume": r(plat_v), "note": plat_note},
        {"what": "Transformer vault", "item": CONC, "volume": r(12 * 12 * 0.6 + 48 * 8 * 0.3), "note": "x -18 to -6, y -12 to 0 round the transformer: a 0.6 m slab and 0.3 m fire walls 8 m tall; the bus through its north wall"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Baghouse slab", "item": CONC, "volume": r(24 * 12 * 0.6)},
        {"what": "Cooling tower basin", "item": CONC, "volume": r((tw + 0.6) * (12.6) * 0.5 + 2 * (tw + 0.3 + 12.3) * 2.0 * 0.3 + 10 * 0.6 * 0.6 * 2.0),
         "note": f"sunk, so the tower's lower louvres and ports stay clear: 0.3 m walls round its {tw:.0f} x 12 m footprint, their tops at z 0, the floor's top at z -2 (0.5 m thick); ten 0.6 x 0.6 m pedestals, six under the tower's feet and four under its pump skid's corners, at z 0 (blender)"},
        {"what": "Bin footing", "item": CONC, "volume": 36.0},
        # Measured on blender's hall (candidate v01, hall-quantities.json): the smaller works the layout's links stand on.
        {"what": "Rail beds", "item": CONC, "volume": 8.27, "note": "under the ladle and slag rails, with their 40 mm flangeways; measured on blender's model"},
        {"what": "Pipe trench", "item": CONC, "volume": 11.23 + 11.80, "note": "floor (11.2 m3) and covers (11.8 m3) of the 1 m trench the duct's water pair runs in; measured on blender's model"},
        {"what": "Link supports", "item": CONC, "volume": 2.16 + 1.44 + 1.20 + 3.85, "note": "footings for the conveyor's trestles (2.2 m3), the duct's supports (1.4 m3) and the water riser (1.2 m3), and the pipes' plinths (3.8 m3); measured on blender's model"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])

    # Totals.
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [recs["hall-crane-20t"]]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used) + sum(l.get("heat", 0) for l in links)
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
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 24 m tall; a roof one cell deep over a {x1_in:.0f} x 24 m floor; doors 6 m tall for the ladle rail (east, 6 m wide) and the slag rail (north, 9 m wide); framed service openings in the north wall for the conveyor (6 x 6 m) and the off-gas duct (9 x 6 m), the lattice's cells there left out, the cells over them (z 18 to 24) bridging the gap under the runway.",
                  "openings": [list(d) for d in doors], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.hall-crane-20t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": RATE, "power": r(power), "heat": r(heat), "water": r(water), "mass": r(total), "against_module": against},
        "staff": mod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile", "frame.box", "frame.openings"], "tier": "invented", "review": True, "note": "Laid out for the trial: the platform and vault side by side under the hall, the rest outside. Lengths from memory of melt shops."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": f"Counted from the lattice (tools/standards/plants.py): every cell edge a straight strut, every square face one diagonal, a node at every corner; masses from the strut and node records. Cladding: the walls and roof less the openings ({clad_net:.0f} m2; blender's hall measured 4,222 m2), with {FLASH:.0f} m2 of returns and flashings as measured on blender's hall (candidate v01, 2026-10-14)."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes chosen for the trial, concrete at 2,400 kg/m3; not sized for the ground's bearing."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged: whether staff go by unit is open."}],
    }
    print(f"layout.arc-furnace-air: {total / 1000:.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to the tower, {water:.1f} kg/s water, frame {frame_mass / 1000:.0f} t")
    return layout, bays


def ladle_station_air(us, recs):
    """The ladle station on a world with air at 1 g: the ladle furnace and the degasser under a crane hall on the ladle rail.
    The ladle comes in by the west door from the arc furnace shop and leaves by the east door for the casting bay."""
    cell, box = 3.0, (-21.0, -15.0, 0.0, 21.0, 15.0, 27.0)
    doors = [(-21.0, -3.0, 0.0, -18.0, 3.0, 6.0),     # ladle rail in, west
             (18.0, -3.0, 0.0, 21.0, 3.0, 6.0),       # ladle rail out, east
             (-12.0, 12.0, 6.0, -6.0, 15.0, 15.0),    # alloy belt through the north wall
             (-3.0, 12.0, 9.0, 3.0, 15.0, 15.0)]      # fume duct, 1 m across, through the north wall
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 8, None, doors)
    x0, y0, z0, x1, y1, z1 = box
    span_in = y1 - y0 - 2 * cell
    crane = recs["ladle-crane-150t"]
    cap, trolley = 150000.0, 37500.0
    RUNWAY = 18.0                                         # the crane's rail plane: its top 3.8 m higher, under the roof's bottom chord at 24
    roof_q = (0.002 * RHO["aluminium"] + 20.0) * G + 1000.0
    chord = roof_q * cell * span_in ** 2 / 8 / cell
    wheel = ((cap + trolley) * (1 - 1.5 / (span_in - 1.2)) + crane["physical"]["mass"] / 2) * G / 2
    vert = wheel + roof_q * cell * span_in / 2
    sec = {"roof": frames.lightest_section(chord), "wall": frames.lightest_section(vert)}
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec[g], cell, diag)
            members.append({"item": key, "count": n, "where": g})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec["wall"], cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    # Runways: the kit's 4340 box 400 x 400 x 25 mm, one a side, bearing on the wall's nodes every 3 m.
    rw_a = 0.4 * 0.4 - 0.35 * 0.35
    rw_I = (0.4 ** 4 - 0.35 ** 4) / 12
    rw_M = wheel * cell / 4
    rw_sigma = rw_M / (rw_I / 0.2)
    assert 1.5 * rw_sigma <= 470e6, rw_sigma
    x_in = x1 - x0 - 2 * cell
    runway = 2 * x_in * rw_a * RHO["steel"]
    members.append({"item": "stock.st4340-box-400x400x25", "count": 4, "where": f"crane runways, two a side, {x_in / 2:.0f} m lengths, along x on the inside of the long walls: their rail plane {RUNWAY:g} m up at y -11.4 and +11.4"})
    fmass += runway
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in doors)
    clad_net = clad_area
    clad_area = 5300.0                                    # m2 measured on blender's hall (candidate v01): the net walls and roof with 291 m2 of reveals and eaves
    clad = clad_area * 0.002 * RHO["aluminium"]
    rcap = frames.strut_capacity(yaml.safe_load(open(S + "mill-stock/" + strut(sec["roof"], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml")))
    wcap = frames.strut_capacity(yaml.safe_load(open(S + "mill-stock/" + strut(sec["wall"], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml")))
    sized = (f"At 1 g: roof {roof_q:.0f} Pa (2 mm aluminium, the frame about 20 kg/m2, 1 kPa of snow and wind, the last invented) over a {span_in:.0f} m span in 3 m deep strips puts {chord / 1e3:.0f} kN in a chord: "
             f"{sec['roof']} takes {rcap / 1e3:.0f} kN. A wheel of the 150 t crane with a full ladle at the runway's end puts {wheel / 1e3:.0f} kN on a wall node, with the roof's share {vert / 1e3:.0f} kN: "
             f"{sec['wall']} takes {wcap / 1e3:.0f} kN. Each the kit's lightest section with half again to spare. A wheel between two nodes bends the runway to {rw_sigma / 1e6:.0f} MPa, against 470 MPa for 4340 with half again to spare. Wind on the walls and the footings' loads not yet checked.")

    tr = us["furnace-transformer-14mva"]
    tL, tW, tH = tr["dims"]
    bays = [
        {"id": "L", "mount": "plant-mount.bay-s2", "unit": "unit.ladle-furnace-80t", "at": [-9.0, 0.0, 0.0], "note": "over the ladle rail; the car stops at x -9 under its roof"},
        {"id": "T", "mount": "plant-mount.bay-s1", "unit": "unit.furnace-transformer-14mva", "at": [-9.0, -9.0, 0.0], "note": "in a fire-walled vault (y -12 to -6) south of the ladle furnace, its terminals facing it"},
        {"id": "D", "mount": "plant-mount.bay-s2", "unit": "unit.vacuum-degasser-80t", "at": [6.0, -7.0, 0.0], "note": "south of the rail, its box from y -12 to the car's lane at -2: the tank at (3, -5), its rim 0.7 m clear of the lane; the crane lifts the ladle from the car stopped at x 3"},
        {"id": "A", "mount": "plant-mount.bay-s3", "unit": "unit.alloy-bins-8", "at": [-9.0, 27.0, 0.0], "note": "outside, north, its 24 m bay clear of the hall's wall (y 15)"},
        {"id": "S", "mount": "plant-mount.bay-s1", "unit": "unit.stair-tower-12m", "at": [-1.0, 27.0, 0.0], "note": "against the bins' east end, 0.22 m clear of their feet: its top landing's -x face at x -2.5, a 0.5 m grating bridge to the walkway's access (-3, 27, 12); its 6 m square overlaps the bins' bay at its edge, not the bins"},
        {"id": "B", "mount": "plant-mount.bay-s2", "unit": "unit.baghouse-200k", "at": [9.0, 24.0, 0.0], "note": "outside, north-east"},
        {"id": "C", "mount": "plant-mount.bay-s3", "unit": "unit.cooling-tower-1cell", "at": [-33.0, -9.0, 0.0], "note": "outside, west of the hall and south of the rail"},
    ]
    pipe_m = math.pi * 0.25 * 0.006 * RHO["steel"] * 2
    tr_pipe_m = math.pi * 0.1 * 0.004 * RHO["steel"] * 2
    bus_area = L_I2 / 4e6
    tower = us["cooling-tower-1cell"]
    tx = -33.0 + tower["dims"][0] / 2
    links = [
        link("rail", "ladle rail", (-24.0, 0.0, 0.0), (24.0, 0.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails at 3 m gauge between the heads' inner faces, set in the floor with 40 mm flangeways, running plane the floor; in by the west door from the arc furnace shop's rail, out by the east door to the casting bay's; the car's centre stops at x -9 (the ladle furnace) and x 3 (the degasser's crane lift)"),
        link("busbar", "secondary bus", (-9.0, -9.0 + tW / 2, tH * 2 / 3), (-9.0, -5.0, 6.0), CU, 3 * bus_area * RHO["copper"], None, f"three water-cooled copper tube bars, {bus_area * 1e6:.0f} mm2 each for {L_I2 / 1000:.0f} kA at 4 A/mm2, from the transformer's terminals through the vault's north wall to the ladle furnace's bus on its -y side"),
        link("conveyor", "alloy feed", (-9.9, 27.0, 6.2), (-9.0, 3.0, 9.4), A36, 120.0, 20000.0, "0.65 m belt in a covered gallery, 120 kg/m with the gallery (from memory), its tail 0.9 m west of the bins' centre (clear of their middle leg at x -9, y 25.5) under a steep 1.2 m drop chute from the transfer chute's foot; rising 3.2 m (7.6 degrees) to 0.4 m over the ladle furnace's chute, through the north wall"),
        link("duct", "fume", (-7.0, 0.0, 7.5), (9.0, 20.0, 10.0), A36, math.pi * 1.0 * 0.006 * RHO["steel"], None, "1 m across, 6 mm plate: up from the ladle furnace's roof elbow, through the north wall's opening, to the baghouse's inlet, at z 11 under the gantry's north tie (its top 0.1 m below the tie at 11.6); the roof's fume is cooler than an arc furnace's, so the duct is not water-cooled", via=[(-7.0, 0.0, 11.0), (0.0, 6.0, 11.0), (0.0, 17.0, 12.0), (9.0, 17.0, 10.0)]),
        link("pipe", "cooling water to the ladle furnace", (tx, -9.0, 0.75), (-13.5, -4.5, 1.0), A36, pipe_m, None, "a pair of 250 mm pipes, 6 mm wall, flow and return, from the tower's first port pair: east through a sleeve in the west wall at y -10.5, z 2.4, between the lattice's struts (blender), north west of the vault, then east just south of the ladle furnace's south-west column (x -14, y -4.4) and up into its manifold", via=[(tx + 1.5, -9.0, 0.75), (tx + 1.5, -10.5, 2.4), (-16.0, -10.5, 2.4), (-16.0, -9.0, 1.5), (-16.0, -5.3, 1.5), (-13.5, -5.3, 1.0)]),
        link("pipe", "cooling water to the transformer", (-16.0, -9.0, 1.5), (-9.0 - tL / 2, -9.0, 1.0), A36, tr_pipe_m, None, "a pair of 100 mm pipes, 4 mm wall, teed off the furnace pair, through the vault's west wall to the transformer's cooler ports"),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-9.0, 0.0, 0.0]}]
    tw = tower["dims"][0]
    pit_r, pit_d = 3.5, 6.0
    ground = [
        {"what": "Ladle furnace foundation", "item": CONC, "volume": r(12 * 10 * 1.0 - 5 * 12 * 0.3), "note": "x -15 to -3, y -5 to 5, 1 m, a 0.3 m recess along the car's lane for the rails"},
        {"what": "Degasser pit", "item": CONC, "volume": r(math.pi * ((pit_r + 0.6) ** 2 - pit_r ** 2) * pit_d + math.pi * (pit_r + 0.6) ** 2 * 1.0), "note": f"round the tank at (3, -5): {2 * pit_r:g} m across inside, {pit_d:g} m deep, 0.6 m walls, a 1 m floor; a service notch through the wall's top at x 2.4 to 3.6, y -9.2 to -8.3, z -1.8 to 0 for the vacuum riser (1 m across, at 3, -8.4), with a cover plate"},
        {"what": "Degasser floor", "item": CONC, "volume": r(12 * 10 * 0.5 - math.pi * (pit_r + 0.6) ** 2 * 0.5), "note": "x 0 to 12, y -12 to -2 round the pit, 0.5 m"},
        {"what": "Transformer vault", "item": CONC, "volume": r(8 * 6 * 0.5 + 28 * 6 * 0.3), "note": "x -13 to -5, y -12 to -6 round the transformer: a 0.5 m slab and 0.3 m fire walls 6 m tall; the bus through its north wall, 1 m short of the ladle furnace's box"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Bins footing", "item": CONC, "volume": r(12.8 * 6.6 * 1.0), "note": "12.8 x 6.6 x 1 m at (-9, 27), past the bins' feet (12.56 x 6.30, blender)"},
        {"what": "Stair tower footing", "item": CONC, "volume": r(3.6 * 6.6 * 0.6), "note": "under the stair tower"},
        {"what": "Baghouse slab", "item": CONC, "volume": r(12 * 8 * 0.6)},
        {"what": "Cooling tower basin", "item": CONC, "volume": r((tw + 0.6) * 12.6 * 0.5 + 2 * (tw + 0.3 + 12.3) * 2.0 * 0.3 + 8 * 0.6 * 0.6 * 2.0), "note": f"sunk as the furnace shop's: 0.3 m walls round its {tw:.0f} x 12 m footprint, tops at z 0, floor top at z -2; eight pedestals, under the tower's four feet and its skid's four corners (blender)"},
        {"what": "Rail beds", "item": CONC, "volume": r(48 * 0.125), "note": "under the ladle rail, 0.125 m3 a metre as the furnace shop's"},
    ]
    # Measured on blender's assembled station (candidate v01, hall-quantities.json, 2026-10-14).
    measured = {"Degasser pit": 137.24, "Degasser floor": 37.84, "Transformer vault": 72.14, "Stair tower footing": 16.632, "Rail beds": 7.2, "Cooling tower basin": 114.66}
    for g in ground:
        if g["what"] in measured:
            g["volume"] = measured[g["what"]]
            g["note"] = (g.get("note", "") + "; measured on blender's model").lstrip("; ")
    ground.append({"what": "Link supports", "item": CONC, "volume": 3.41, "note": "footings for the alloy gallery's trestles, the duct's supports and the pipes' plinths; measured on blender's model"})
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [crane]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used) + sum(l.get("heat", 0) for l in links)
    water = sum(u.get("needs", {}).get("water", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = lmod["physical"]
    against = (f"module.ladle-station: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {L_P / 1e6:.1f} MW. "
               f"This layout: the same rate; {power / 1e6:.1f} MW (the module's figure is the arc's average; the degasser's pumps, the fan, the crane and the tower are on top); "
               f"{water:.1f} kg/s of water the module did not name; {(mass_units + mass_links + frame_mass) / 1000:.0f} t of plant and frame and {mass_ground / 1000:.0f} t of concrete.")
    xs = [x0, x1] + [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [y0, y1] + [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.ladle-station-air", "name": "Ladle station, world with air",
                     "description": "An 80 t ladle furnace and a vacuum tank degasser on the ladle rail under a crane hall, with the furnace's transformer, alloy bins, a fume baghouse and a cooling tower. The heat comes in on its car from the arc furnace shop, is reheated and trimmed under the ladle furnace, lifted by the 150 t crane into the degasser and back, and goes on to the casting bay. For a world with air at about 1 g."},
        "stands_for": "module.ladle-station",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 24 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor; doors 6 m wide and tall for the ladle rail, west and east; framed openings in the north wall for the alloy belt and the fume duct.",
                  "openings": [list(d) for d in doors], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.ladle-crane-150t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": L_RATE, "power": r(power), "heat": r(heat), "water": r(water), "mass": r(total), "against_module": against},
        "staff": lmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile", "frame.box", "frame.openings"], "tier": "invented", "review": True, "note": "Laid out after the arc furnace shop, on its ladle rail. Lengths from memory of ladle metallurgy shops."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": "Counted from the lattice (tools/standards/plants.py): every cell edge a straight strut, every square face one diagonal, a node at every corner; sections chosen by the loads in frame.sized; masses from the strut and node records."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes from the shapes in the notes, concrete at 2,400 kg/m3; not sized for the ground's bearing."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.ladle-station-air: {total / 1000:.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to the tower, {water:.1f} kg/s water, frame {frame_mass / 1000:.0f} t ({sec})")
    return layout, bays


# ---------------------------------------------------------------- the casting bay's figures
cmod = yaml.safe_load(open(S + "modules/casting-bay.yaml"))
CREC = cmod["recipes"][0]
C_RATE, C_HEAT, C_P = CREC["rate"], CREC["batch"], CREC["power"]
SOUND = 1 / CREC["inputs"][0]["quantity"]            # the share of the steel that becomes sound ingot (the module's, sourced)
CAST_IRON = "material.cast-iron-grey"
# The ingot: one to a heavy crate (package.crate-heavy: 2 t, about 1.1 x 0.9 x 0.7 m inside), lying flat.
ING_L, ING_A, ING_B = 1.0, 0.48, 0.42                 # m: long, its square side at the big end and the small
ING_V = ING_L * (ING_A ** 2 + ING_A * ING_B + ING_B ** 2) / 3
ING_M = ING_V * 7850                                  # kg of sound ingot
HOT_TOP = ING_M * (1 / SOUND - 1)                     # kg of the head cut off as scrap
MOULDS = math.ceil(C_HEAT / (ING_M + HOT_TOP))        # moulds a heat fills
PER_CAR = 12
CARS = math.ceil(MOULDS / PER_CAR)
CAR_N = int(40000.0 // (ING_M + HOT_TOP))              # stripped ingots, heads on, an ingot car takes under 40 t
assert CAR_N == 21, "the ingot car's deck pattern (3 rows of 7) assumes 21"


def casting_units():
    """The casting bay's own units."""
    out = {}
    wall = 0.12
    mould = ((ING_A + 2 * wall) ** 2 - ING_A ** 2) * ING_L * 1.15 * 7200          # a big-end-up mould 1.15 ingots tall, its wall 120 mm
    stool = (ING_A + 2 * wall + 0.08) ** 2 * 0.15 * 7200
    frame_ = (2 * box_tube(0.4, 0.4, 0.025, 6.0) + 6.0 * 2.8 * 0.04) * RHO["steel"]
    out["mould-car-12"] = dict(
        name="Ingot mould car, 12 moulds", size_class=1, dims=(6.0, 2.8, 2.3),
        desc=f"A self-propelled rail car carrying twelve cast iron ingot moulds in two rows of six at 0.85 m, each on its stool, big end up: each takes {ING_M + HOT_TOP:,.0f} kg of steel, a {ING_L:g} m ingot tapering from {ING_A:g} to {ING_B:g} m square with its hot top; the ingot fits one to a heavy crate. The ladle is held over each mould in turn by the crane and teemed from its slide-gate nozzle; the train stands while the ingots freeze, then the stripper crane lifts each ingot up out of its mould by a lifting eye in its hot top (a loop of 25 mm steel bar, about 2 kg, pushed into the head as it freezes, standing 0.15 m over the mould's lip to z 2.45; it goes to scrap with the head) (big end up, the ingot can only leave upward), its tongs' arms holding the mould down by its lugs; the moulds stay on the car. Two trunnion lugs a mould on its y faces, 100 mm across, 80 mm out, near z 2.05, for those arms and for changing a mould. It travels along its x on 2 m gauge, its origin on the running plane; the moulds' tops 2.3 m up.",
        built_of=[part("Moulds", CAST_IRON, 27490.5, f"{PER_CAR} moulds with their lugs, {wall * 1000:.0f} mm walls, {ING_L * 1.15:.2f} m tall; measured on blender's model ({PER_CAR * mould:,.0f} kg as plain walls)"),
                  part("Stools", CAST_IRON, 8251.7, f"{PER_CAR} base plates 0.15 m thick; measured"),
                  part("Car frame and deck", "stock.st4340-box-400x400x25", 8798.8, "two box beams 400 x 400 x 25 mm, 6 m, and a deck 6 x 2.8 m of 40 mm plate; measured"),
                  part("Cross members", A36, 716.2, "four 260 x 160 x 12 mm hollow members between the beams, fabricated (not kit stock); measured, provisional"),
                  part("Bogies and drive", "good.electric-motors", 3000.0, "from memory")],
        holds=PER_CAR * (ING_M + HOT_TOP), needs={"power": 30000.0},
        basis=[{"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"The ingot sized to the heavy crate (2 t); its hot top the module's scrap share. Moulds about their ingot's mass and more, as practice runs (from memory). Car and drive from memory."}])
    out["stripper-crane-20t"] = bridge_crane_unit(20000.0, 22.8, 6000.0, kind="Stripper crane", job="it lifts each frozen ingot up out of its mould by the hot top, the arms of its tongs holding the mould down by its lugs (a big-end-up mould frees its ingot only upward), and sets the ingots on the ingot car", gear="ingot tongs whose arms hold the mould down")
    sc = out["stripper-crane-20t"]
    sc["built_of"].append(part("Stripping tool", A36, 3000.0, "outer pads bearing down on a mould's y-face lugs and a centre hook that takes the lifting eye set in the hot top (the head itself is inside the mould's walls) and draws the ingot up through a 1.3 m stroke (the ingot and its head stand 1.15 m in the mould); from memory, grip and loads unsized"))
    sc["dims"] = (sc["dims"][0], sc["dims"][1], r(sc["dims"][2] + 1.6, 3))
    sc["desc"] += " Its stripping tool hangs from the trolley, raised to 1.6 m under the rail plane when parked: the box runs from there to the trolley's top."
    deck = 8.0 * 3.0 * 0.04 * RHO["steel"]
    frame2 = 2 * box_tube(0.4, 0.4, 0.025, 8.0) * RHO["steel"]
    out["ingot-car-40t"] = dict(
        name="Ingot car, 40 t", size_class=2, dims=(8.0, 3.0, 1.2),
        desc=f"A self-propelled flat rail car that takes the stripped ingots, still hot and with their hot tops on, to the ingot yard: {CAR_N} of them (each {ING_M + HOT_TOP:,.0f} kg, {CAR_N * (ING_M + HOT_TOP) / 1000:.1f} t, the most under its 40 t), standing big end up as the stripper crane sets them, in 3 rows of 7 at 1.1 m along x and 0.9 m across, so the yard crane's tongs take them by the same lifting eye. The heads are cropped in the yard. It travels along its x on 2 m gauge, its origin on the running plane; the deck 1.2 m up.",
        built_of=[part("Car frame and deck", "stock.st4340-box-400x400x25", 12234.9, f"two box beams 400 x 400 x 25 mm, 8 m, and a deck 8 x 3 m of 40 mm plate; measured on model v1 (by formula {frame2 + deck:,.0f} kg)"),
                  part("Cross members", A36, 716.2, "four fabricated 260 x 160 x 12 mm boxes between the beams (not kit stock); measured on model v1"),
                  part("Bogies and drive", "good.electric-motors", 4000.0, "from memory")],
        holds=r(CAR_N * (ING_M + HOT_TOP)), needs={"power": 40000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Frame, deck and cross members measured on model v1; the load the most stripped ingots under 40 t; bogies and drive from memory."}])
    return out


def casting_bay_air(us, recs):
    """The casting bay on a world with air at 1 g: the ladle comes in on its car, the 150 t crane teems it into the moulds on
    a train of mould cars; a second train stands while its ingots freeze; the stripper crane strips them and loads the ingot car."""
    cell, box = 3.0, (-33.0, -15.0, 0.0, 33.0, 15.0, 27.0)
    doors = [(-33.0, 3.0, 0.0, -30.0, 9.0, 6.0),      # ladle rail in, west
             (30.0, 3.0, 0.0, 33.0, 9.0, 6.0)]        # ingot rail out, east, to the ingot yard
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 8, None, doors)
    x0, y0, z0, x1, y1, z1 = box
    span_in = y1 - y0 - 2 * cell
    crane = recs["ladle-crane-150t"]
    cap, trolley = 150000.0, 37500.0
    roof_q = (0.002 * RHO["aluminium"] + 20.0) * G + 1000.0
    chord = roof_q * cell * span_in ** 2 / 8 / cell
    wheel = ((cap + trolley) * (1 - 1.5 / (span_in - 1.2)) + crane["physical"]["mass"] / 2) * G / 2
    vert = wheel + roof_q * cell * span_in / 2
    sec = {"roof": frames.lightest_section(chord), "wall": frames.lightest_section(vert)}
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec[g], cell, diag)
            members.append({"item": key, "count": n, "where": g})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec["wall"], cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    rw_a = 0.4 * 0.4 - 0.35 * 0.35
    rw_sigma = wheel * cell / 4 / ((0.4 ** 4 - 0.35 ** 4) / 12 / 0.2)
    assert 1.5 * rw_sigma <= 470e6, rw_sigma
    x_in = x1 - x0 - 2 * cell
    members.append({"item": "stock.st4340-box-400x400x25", "count": 4, "where": f"crane runways, two a side, {x_in / 2:.0f} m lengths, along x on the inside of the long walls: their rail plane 18 m up at y -11.4 and +11.4; the ladle crane works the west half, the stripper crane the east"})
    fmass += 2 * x_in * rw_a * RHO["steel"]
    members.append({"item": "stock.st4340-box-400x400x25", "count": 42, "where": "runway brackets: a 1 m stub under the runway at each inner wall node a side, measured on blender's hall (candidate v01), 12,363.8 kg"})
    members.append({"item": "stock.st4340-tb-160x8", "count": 42, "where": "runway brackets' diagonals, from each stub's end down to the wall node 3 m below, 97.8 m in all, measured on blender's hall (candidate v01), 2,932.9 kg"})
    fmass += 12363.8 + 2932.9
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in doors)
    clad_area = 7242.4                                    # m2 measured on blender's hall (candidate v01), with reveals and returns
    clad = clad_area * 0.002 * RHO["aluminium"]
    sec_stock = lambda g: yaml.safe_load(open(S + "mill-stock/" + strut(sec[g], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    rcap, wcap = frames.strut_capacity(sec_stock("roof")), frames.strut_capacity(sec_stock("wall"))
    sized = (f"At 1 g: roof {roof_q:.0f} Pa (2 mm aluminium, the frame about 20 kg/m2, 1 kPa of snow and wind, the last invented) over a {span_in:.0f} m span in 3 m deep strips puts {chord / 1e3:.0f} kN in a chord: "
             f"{sec['roof']} takes {rcap / 1e3:.0f} kN. A wheel of the 150 t crane with a full ladle at the runway's end puts {wheel / 1e3:.0f} kN on a wall node, with the roof's share {vert / 1e3:.0f} kN: "
             f"{sec['wall']} takes {wcap / 1e3:.0f} kN. Each the kit's lightest section with half again to spare. A wheel between two nodes bends the runway to {rw_sigma / 1e6:.0f} MPa, against 470 MPa for 4340 with half again to spare. Wind on the walls and the footings' loads not yet checked.")
    car_L = us["mould-car-12"]["dims"][0]
    train = CARS * car_L
    links = [
        link("rail", "ladle rail", (-36.0, 6.0, 0.0), (-12.0, 6.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails at 3 m gauge between the heads' inner faces, set in the floor with 40 mm flangeways, running plane the floor; in by the west door from the ladle station's rail; the car's centre stops at x -15, where the crane lifts the ladle off it"),
        link("rail", "teeming rail", (-30.0, -6.0, 0.0), (30.0, -6.0, 0.0), A36, 2 * 50.0, None, f"two 50 kg/m rails at 2 m gauge, set in the floor, running plane the floor: room for two trains of {CARS} mould cars ({train:.0f} m each), one being teemed in the west half while the other stands to freeze and be stripped in the east"),
        link("rail", "ingot rail", (3.0, 6.0, 0.0), (36.0, 6.0, 0.0), A36, 2 * 50.0, None, "two 50 kg/m rails at 2 m gauge, set in the floor, running plane the floor; out by the east door to the ingot yard; the car's centre stops at x 15 beside the stripping"),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-15.0, 6.0, 0.0]}]
    mobile += [{"unit": "unit.mould-car-12", "rides": "teeming rail", "at": [round(-27.0 + car_L / 2 + i * car_L, 2), -6.0, 0.0]} for i in range(CARS)]
    mobile += [{"unit": "unit.mould-car-12", "rides": "teeming rail", "at": [round(3.0 + car_L / 2 + i * car_L, 2), -6.0, 0.0]} for i in range(CARS)]
    mobile += [{"unit": "unit.ingot-car-40t", "rides": "ingot rail", "at": [15.0, 6.0, 0.0]}]
    ground = [
        {"what": "Casting floor", "item": CONC, "volume": r(x_in * span_in * 0.4), "note": f"a 0.4 m slab over the hall's floor, {x_in:.0f} x {span_in:.0f} m, so spilt steel meets concrete, not soil"},
        {"what": "Teeming pit", "item": CONC, "volume": r(30 * 6 * 0.3), "note": "x -30 to 0, y -9 to -3: 0.3 m more concrete below the floor slab under the teeming, its top flush with the floor (z 0) so the rails sit as elsewhere: a spill freezes on the floor and the thicker slab keeps it off the ground beneath"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Door aprons", "item": CONC, "volume": 27.11, "note": "a 6 x 6 x 0.4 m slab outside each rail door, the rails set in it; measured on blender's hall (candidate v01)"},
        {"what": "Rail seat beds", "item": CONC, "volume": 8.58, "note": "the rails' channels in the floor, after the flangeway cuts; measured on blender's hall (candidate v01)"},
    ]
    measured = {"Casting floor": 562.24, "Teeming pit": 54.0, "Frame footings": 261.0}   # m3 on blender's hall (candidate v01), after flangeway cuts
    for g in ground:
        if g["what"] in measured:
            g["volume"] = measured[g["what"]]
            g["note"] += "; measured on blender's hall (candidate v01)"
        g["mass"] = r(g["volume"] * RHO["concrete"])
    used = [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [crane, recs["stripper-crane-20t"]]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = cmod["physical"]
    against = (f"module.casting-bay: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {C_P / 1e6:.1f} MW. "
               f"This layout: the same rate, {C_RATE * 3.6:.0f} t an hour; {power / 1e6:.2f} MW, the cranes and the cars; no cooling water (the moulds and ingots cool in the hall's air); "
               f"{(mass_units + mass_links + frame_mass) / 1000:.0f} t of plant and frame and {mass_ground / 1000:.0f} t of concrete. A heat of {C_HEAT / 1000:.0f} t fills {MOULDS} moulds on {CARS} cars: "
               f"{MOULDS} ingots of {ING_M:,.0f} kg, one to a heavy crate.")
    xs = [x0, x1] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [y0, y1]
    layout = {
        "identity": {"key": "layout.casting-bay-air", "name": "Casting bay, world with air",
                     "description": f"Ingot teeming under a crane hall: the heat comes in on its ladle car from the ladle station; the 150 t crane holds the ladle over a train of {CARS} mould cars and teems {MOULDS} small ingots, one to a heavy crate; the train stands while they freeze, the 20 t stripper crane draws the ingots up out of the moulds and loads them on a car to the ingot yard. For a world with air at about 1 g."},
        "stands_for": "module.casting-bay",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": [], "mobile": mobile, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 24 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor; doors 6 m wide and tall for the ladle rail (west) and the ingot rail (east).",
                  "openings": [list(d) for d in doors], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.ladle-crane-150t", "unit.stripper-crane-20t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": C_RATE, "power": r(power), "heat": 0.0, "water": 0.0, "mass": r(total), "against_module": against},
        "staff": cmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile", "frame.box", "frame.openings"], "tier": "invented", "review": True, "note": "Laid out after the ladle station, on its ladle rail. Top teeming from the ladle into small ingots, chosen so an ingot fits a heavy crate; lengths from memory."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": "Counted from the lattice (tools/standards/plants.py); sections chosen by the loads in frame.sized; masses from the strut and node records."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes from the shapes in the notes, concrete at 2,400 kg/m3; not sized for the ground's bearing."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.casting-bay-air: {total / 1000:.0f} t, {power / 1e6:.2f} MW, frame {frame_mass / 1000:.0f} t, {MOULDS} moulds on {CARS} cars, ingot {ING_M:.0f} kg ({sec})")
    return layout, []


# ---------------------------------------------------------------- the shaft furnace's figures
smod = yaml.safe_load(open(S + "modules/shaft-furnace.yaml"))
SREC = smod["recipes"][0]
S_RATE, S_P = SREC["rate"], SREC["power"]                    # kg/s of sponge iron, W (sourced: 110 kWh a tonne, not heating the gas)
PELLETS = next(i["quantity"] for i in SREC["inputs"] if i["item"] == "good.iron-ore-pellets") * S_RATE
H2_USED = next(i["quantity"] for i in SREC["inputs"] if i["item"] == "element.h") * S_RATE
STEAM = next(o["quantity"] for o in SREC["outputs"] if o["item"] == "good.water") * S_RATE
CP_H2, CP_PEL, CP_DRI, CP_STEAM = 14.3e3, 900.0, 700.0, 2.0e3   # J/kg K: hydrogen, pellets, sponge iron, steam (handbook figures, rounded)
ENDO = 0.86e6                     # J/kg of iron: Fe2O3 + 3 H2 -> 2 Fe + 3 H2O takes about 96 kJ a mole of oxide (handbook)
T_GAS, T_TOP, T_PRE, T_COLD = 900.0, 350.0, 300.0, 40.0     # deg C: gas into the bustle, top gas out, gas after the recuperator, after the scrubber (from memory of hydrogen shaft furnaces)
T_BURDEN, T_OUT = 850.0, 50.0     # deg C: the burden at the bottom of the reduction zone, the sponge iron discharged cold
SHELL_LOSS = 0.05                 # share of the gas's heat lost through the shell, from memory
P_SHAFT, P_SUCTION = 2.8e5, 1.3e5  # Pa: gas into the shaft, at the compressor's suction (from memory)
V_GAS = 30.0                      # m/s in the gas mains, chosen
# Heat balance of the reduction zone: the gas brings the heat that warms the pellets and drives the reduction.
Q_SHAFT = (ENDO * S_RATE + PELLETS * CP_PEL * (T_BURDEN - 25.0)) * (1 + SHELL_LOSS)
GAS = Q_SHAFT / (CP_H2 * (T_GAS - T_TOP))                   # kg/s of hydrogen blown in
H2_TOP = GAS - H2_USED
Q_HEATER = GAS * CP_H2 * (T_GAS - T_PRE)
Q_RECUP = GAS * CP_H2 * (T_PRE - T_COLD)
T_RECUP_OUT = T_TOP - Q_RECUP / (H2_TOP * CP_H2 + STEAM * CP_STEAM)
Q_SCRUB = (H2_TOP * CP_H2 + STEAM * CP_STEAM) * (T_RECUP_OUT - T_COLD) + 0.97 * STEAM * 2.4e6   # nearly all the steam condenses at 40 deg C and 1.3 bar
GAMMA, ETA_K = 1.41, 0.80
W_COMP = GAS * CP_H2 * (T_COLD + 273.15) * ((P_SHAFT / P_SUCTION) ** ((GAMMA - 1) / GAMMA) - 1) / ETA_K
Q_COOLZONE = S_RATE * CP_DRI * (T_BURDEN - T_OUT)
COOL_GAS = Q_COOLZONE / (CP_H2 * (600.0 - T_COLD))          # kg/s of hydrogen through the cooling zone, out at 600 deg C
W_COOL = COOL_GAS * CP_H2 * (T_COLD + 273.15) * (1.3 ** ((GAMMA - 1) / GAMMA) - 1) / ETA_K
HEATER_PF = 0.95


def mol_flow(kg_h2, kg_steam=0.0):
    return kg_h2 / 2.016e-3 + kg_steam / 18.015e-3


def main_dia(mol_s, T_c, p):
    """m across a gas main carrying mol_s at T_c and p, at V_GAS."""
    q = mol_s * 8.314 * (T_c + 273.15) / p
    return math.sqrt(4 * q / (math.pi * V_GAS))


def shaft_units():
    """The hydrogen shaft furnace's own units."""
    out = {}
    D = 6.6                                            # m inside the lining, from memory of shafts for 1.5 to 2 Mt a year
    zones = {"top": 4.0, "reduction": 12.0, "transition": 4.0, "cooling": 9.0, "cone": 5.0}
    hot = zones["top"] + zones["reduction"] + zones["transition"]
    cold = zones["cooling"] + zones["cone"]
    t_hot, t_cold, t_sh = 0.35, 0.15, 0.04
    Do = D + 2 * t_hot + 2 * t_sh
    Z_OUT = 8.0                                         # the discharge's outlet above the floor, over the product belt
    z_bustle = Z_OUT + zones["cone"] + zones["cooling"] + zones["transition"]
    z_top = Z_OUT + hot + cold
    z_ring = Z_OUT + cold
    H = z_top + 8.0                                     # the charge hopper and seal leg above
    shell = cyl_shell(Do, hot + cold, t_sh) * RHO["steel"]
    lining = (math.pi * ((D / 2 + t_hot) ** 2 - (D / 2) ** 2) * hot + math.pi * ((D / 2 + t_cold) ** 2 - (D / 2) ** 2) * cold) * RHO["refractory"]
    d_b = main_dia(mol_flow(GAS), T_GAS, P_SHAFT)
    R_b = Do / 2 + 0.3 + d_b / 2
    bustle_s = math.pi * (d_b + 0.4) * 0.02 * 2 * math.pi * R_b * RHO["steel"]
    bustle_r = math.pi * (d_b + 0.2) * 0.2 * 2 * math.pi * R_b * RHO["refractory"]
    burden = (math.pi * (D / 2) ** 2 * (hot + cold - zones["cone"]) + math.pi * (D / 2) ** 2 * zones["cone"] / 3) * 1900.0
    ring = 2 * math.pi * (Do / 2 + 0.4) * box_tube(0.8, 1.2, 0.04, 1.0) * RHO["steel"]
    side = max(12.0, round(2 * R_b + d_b + 1.0))
    out["shaft-furnace-200th"] = dict(
        name=f"Hydrogen shaft furnace, {S_RATE * 3.6:.0f} t an hour", size_class=2 if side <= 12 else 3, fits="bay-s2" if side <= 12 else "bay-s3", dims=(side, side, round(H, 1)),
        desc=(f"A refractory-lined steel shaft {D:g} m across inside, through which iron ore pellets sink by their own weight for about {burden / S_RATE / 3600:.0f} hours against hot hydrogen rising, coming out at the bottom as cold sponge iron. "
              f"Local frame: the shaft's axis at (0, 0). From the top down: a charge hopper and seal leg (z {z_top:.0f} to {H:.0f}) with the pellet belt's head over it; the top gas outlet at z {z_top - 1:.0f} on +x; "
              f"the reduction zone ({zones['reduction']:g} m); the bustle main, a ring {d_b:.1f} m across round the shaft at z {z_bustle:.0f}, through whose ports hydrogen at {T_GAS:.0f} deg C enters, its inlet on -y; "
              f"the transition; the cooling zone ({zones['cooling']:g} m), cooled by a loop of cold hydrogen in at z {z_ring - 8:.0f} and out at z {z_ring - 3:.0f} (ports on -x); a discharge cone to burden feeders and a lock at z {Z_OUT:g}, over the layout's product belt. "
              f"Hung from a ring girder whose underside is at z {z_ring:.0f} (radius {Do / 2 + 0.4:.1f} m to its centre, the cooling zone's top, under the bustle), seated on the layout's columns' tops. The bustle's inlet is a short stub out of the ring on -y, its flange on the unit's face at y -{side / 2:g}. About {burden / 1000:,.0f} t of burden inside."),
        built_of=[part("Shell", A36, 247694.8, f"{Do:.2f} m across, {hot + cold:g} m, {t_sh * 1000:.0f} mm plate, with its cones, roof and bands; measured on model v1 (by formula {shell:,.0f} kg for the cylinder)"),
                  part("Lining", REF, lining, f"{t_hot * 1000:.0f} mm in the hot {hot:g} m, {t_cold * 1000:.0f} mm against wear in the cooling zone and cone"),
                  part("Bustle main", A36, bustle_s, f"a ring {2 * R_b:.1f} m across of {d_b + 0.4:.1f} m pipe, 20 mm plate; by formula (the faceted model's skin {61069.5:,.0f} kg), with 12 ports of 0.65 m into the shaft ({12 * math.pi * 0.65 ** 2 / 4:.1f} m2, half the main's bore, so the ring evens the flow)"),
                  part("Bustle lining", REF, bustle_r, "200 mm inside the ring"),
                  part("Support ring girder", A36, 35860.7, f"a box 0.8 x 1.2 m, 40 mm plate, round the shaft, with its seating land; measured on model v1 (by formula {ring:,.0f} kg)"),
                  part("Bustle struts", A36, 1028.0, "8 inclined fabricated boxes 200 x 200 x 12 mm from the ring girder's top (r 4.1 m, z 23.2) to the bustle's saddle (r 5.6 m, a 1.02 m rise), at the columns' angles: about 0.36 MN up each at 1 g, 0.64 MN along it (72 MPa); measured on model v1"),
                  part("Strut saddles and feet", A36, 1369.1, "curved 40 mm plate pads under the bustle (centre z 24.22) and on the girder (z 23.2) that spread each strut's load: 0.64 MN on about 0.5 m of pad is a few MPa of bearing; measured on model v1"),
                  part("Charge hopper, seal leg and distributor", A36, 40000.0, "from memory"),
                  part("Burden feeders and discharge lock", "good.electric-motors", 30000.0, "from memory")],
        holds=burden,
        needs={"power": S_P - W_COMP - W_COOL, "gravity": 1.0, "atmosphere": "any"},
        recipes=smod["recipes"], life=smod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.shaft-furnace), carried here: the module stays the game's until it reads layouts. Its sources are the module's."},
               {"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"From the materials and dimensions in built_of; the shaft's size and zones from memory of shafts for 1.5 to 2 Mt a year; the burden at 1,900 kg/m3 (from memory). The source's 137 m (module.shaft-furnace) is a whole plant's tower; this shaft with its hopper is {H:.0f} m."},
               {"of": ["needs.power"], "tier": "derived", "note": f"The module's {S_P / 1e6:.0f} MW (sourced, 110 kWh a tonne, gas heating left out) less the compressors, which are units of their own: feeders, belts, the charge's seals, instruments."},
               {"of": ["needs.gravity"], "tier": "invented", "review": True, "note": "The burden sinks by its own weight against the gas's drag; at low g the gas must be slowed or the bed lifts. Not checked below 1 m/s2."}])
    # Gas heater: electric resistance banks in lined vessels; hydrogen cannot be heated by burning the gas it is.
    n_h, Dh, Hh = 4, 4.0, 16.0
    elem = Q_HEATER / 20e3 / 0.155                     # kg of FeCrAl strip at 20 kW/m2 of surface, 0.155 m2/kg for 20 x 2 mm strip
    hv = n_h * (cyl_shell(Dh, Hh, 0.025, bottom=2.0) * RHO["steel"])
    hr = n_h * math.pi * ((Dh / 2) ** 2 - (Dh / 2 - 0.3) ** 2) * Hh * RHO["refractory"]
    hd_c = main_dia(mol_flow(GAS), T_PRE, P_SHAFT)     # m bore of the cold header, as the layout's warm gas pipe
    hd_h = main_dia(mol_flow(GAS), T_GAS, P_SHAFT)     # m bore of the hot header, as the layout's hot duct
    hL = 20.5                                         # m: the hot header x -10.25 to 10.25, clear of the end branches (2.03 m outside at x +-9)
    hdr_c = math.pi * (hd_c + 0.012) * 0.012 * hL * RHO["steel"]
    hdr_hs = math.pi * (hd_h + 0.4 + 0.016) * 0.016 * hL * RHO["steel"]
    hdr_hr = math.pi * (hd_h + 0.2) * 0.2 * hL * RHO["refractory"]
    out[f"gas-heater-{Q_HEATER / 1e6:.0f}mw"] = dict(
        name=f"Hydrogen heater, {Q_HEATER / 1e6:.0f} MW", size_class=3, fits="bay-s3", dims=(24.0, 12.0, 18.0),
        desc=(f"Heats {GAS:.1f} kg/s of hydrogen from {T_PRE:.0f} to {T_GAS:.0f} deg C with electric resistance elements: {n_h} lined vessels {Dh:g} m across and {Hh:g} m tall (z 0.5 to 16.5) in a row along x at 6 m (x -9, -3, 3, 9), on the unit's centre line. "
              f"The gas in at the bottom of each from a cold header ({hd_c:.2f} m bore, 12 mm wall) along x at y -3.3, z 2, whose inlet flange is on its -x end at the unit's face (x -12); "
              f"out at the top of each into a lined hot header ({hd_h:.2f} m bore inside 200 mm of refractory, 16 mm shell, {hd_h + 0.432:.2f} m outside) along x at y 4, z 15, from x -10.25 to 10.25, whose outlet is a stub up from its middle (x 0) to a flange at z 18, the unit's top. "
              "Its power by cables from its transformer to a terminal house at its -x end, on the +y side (clear of the cold header's flange), x -12 to -11.1, y 0.3 to 2.6, z 0.15 to 3, the cables entering the middle of its +y face at (-11.55, 2.6, 1.575). The headers' supports are the unit's own."),
        built_of=[part("Vessels", A36, 167601.6, f"{n_h} vessels {Dh:g} m across, {Hh:g} m, 25 mm plate, 1 m deep ellipsoidal heads, with their bands; measured on model v1 (by formula {hv:,.0f} kg)"),
                  part("Skirts and base rings", A36, 12229.5, "8 mm skirts from the floor to the vessels' bottoms; measured on model v1"),
                  part("Lining", REF, hr, "300 mm"),
                  part("Elements", "material.stainless-steel-304", elem, f"FeCrAl strip, which the registry has not got: stainless steel stands for it; {elem / 1000:.0f} t at 20 kW/m2 of surface"),
                  part("Cold header", A36, 14768.4, f"22 m of {hd_c:.2f} m pipe, 12 mm wall, with its inlet; measured on model v1 (by formula {hdr_c * 22 / hL:,.0f} kg)"),
                  part("Hot header and outlet", A36, 29445.0 + 4309.0, f"{hL:g} m of {hd_h + 0.4:.2f} m shell, 16 mm plate, and its 3 m outlet; measured on model v1"),
                  part("Hot header lining", REF, hdr_hr * (hL + 3.0) / hL, "200 mm, in the header and its outlet"),
                  part("Branches", A36, 9598.5, "a cold (1.2 m bore) and a hot (1.6 m bore, lined) branch to each vessel; measured on model v1"),
                  part("Header supports", A36, 29061.6, "box columns 400 mm under the hot header, braced at 4, 8 and 12 m, with saddles; measured on model v1"),
                  part("Flanges and blind ends", A36, 4183.4, "measured on model v1"),
                  part("Terminal house base", A36, 2845.6, "measured on model v1"),
                  part("Terminals, thyristor sets and controls", "good.electric-motors", 10000.0, "from memory")],
        needs={"power": Q_HEATER / 0.98},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "Vessels and element loading from memory; elements sized from the duty."},
               {"of": ["needs"], "tier": "derived", "note": f"The gas's {GAS:.1f} kg/s at {CP_H2 / 1e3:.1f} kJ/kg K over {T_GAS - T_PRE:.0f} K, at 98% (from memory)."}])
    ht = transformer_unit(Q_HEATER / 0.98 / HEATER_PF, "the hydrogen heater's elements, through thyristor sets in its terminal house")
    ht["name"] = ht["name"].replace("Furnace", "Heater")
    out[f"heater-transformer-{Q_HEATER / 0.98 / HEATER_PF / 1e6:.0f}mva"] = ht
    # Recuperator: the top gas warms the cold recycle gas.
    A = Q_RECUP / (100.0 * 57.0)                       # m2 at 100 W/m2 K over a log mean of about 57 K
    tubes = A / (math.pi * 0.05 * 12.0)
    out["recuperator"] = dict(
        name=f"Recuperator, {Q_RECUP / 1e6:.0f} MW", size_class=2, fits="bay-s2", dims=(12.0, 10.4, 18.0),
        desc=(f"Two vertical shell-and-tube exchangers side by side (local x -3 and 3), shells 4.5 m across and 16 m tall: the top gas from the shaft, {T_TOP:.0f} deg C with its steam, down through the tubes to {T_RECUP_OUT:.0f} deg C, "
              f"above its dew point; the cleaned recycle gas up through the shells from {T_COLD:.0f} to {T_PRE:.0f} deg C. The manifolds run along x outside the 1.5 m gap between the shells: "
              "top gas in at the middle of the top (0, 0, 18) into a 3.5 m header across both tube sheets at z 16.2; out of the tubes' bottoms into a 2.7 m wet header at y 3.8, z 1.5, its outlet flange on the +y face (0, 5.2, 1.5); "
              "recycle gas in by a 1.7 m clean header at y -3.3, z 1, flange on the -y face (0, -5.2, 1); out of the shells' upper ends, below the tube sheets, by a 2.26 m warm header at y -3.5, z 13.5 (clear of the top gas header), its outlet a riser outside to a flange on the -y face (0, -5.2, 16.75), so the flange's top stays under the unit's top."),
        built_of=[part("Tubes", "material.stainless-steel-304", tubes * math.pi * 0.047 * 0.003 * 12.0 * 7900, f"{tubes:,.0f} tubes 50 mm, 3 mm wall, 12 m: {A:,.0f} m2"),
                  part("Shells, heads and bands", A36, 82643.8, f"two shells 4.5 m across, 16 m, 20 mm plate, with their bands; measured on model v1 (by formula {2 * cyl_shell(4.5, 16.0, 0.02, bottom=2.0) * RHO['steel']:,.0f} kg)"),
                  part("Skirts and feet", A36, 9164.2, "measured on model v1"),
                  part("Headers and branches", A36, 46875.9, "the top gas header, the 8.2 m wet, clean and warm headers, the warm outlet's riser with its tee-back plate, and the branches; measured on model v1"),
                  part("Header supports", A36, 9833.5, "measured on model v1")],
        needs={},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"Area from the duty ({Q_RECUP / 1e6:.0f} MW) at 100 W/m2 K, from memory for hydrogen on both sides; shells from memory. The model's heads are 20 mm flat plates, not yet qualified for pressure."}])
    # Scrubber: cools the top gas and takes out its water and dust.
    q_s = mol_flow(H2_TOP, STEAM) * 8.314 * (T_RECUP_OUT + 273.15) / P_SUCTION
    Ds = math.sqrt(4 * q_s / 3.0 / math.pi)
    px_area = Q_SCRUB / (4000.0 * 5.0)                # m2 of plate in the exchangers
    spray_kw = Q_SCRUB / (4186 * 10) / 1000 * 300e3 / 0.75 / 1000
    out["top-gas-scrubber"] = dict(
        name="Top gas scrubber", size_class=2, fits="bay-s2", dims=(12.0, 12.0, 24.0),
        desc=(f"A packed tower {Ds:.1f} m across and 22 m tall: the top gas up through it against sprayed water, which cools it to {T_COLD:.0f} deg C, condenses the {STEAM:.1f} kg/s of water the reduction made and washes out the dust. "
              f"Gas in at the bottom on -x by a {main_dia(mol_flow(H2_TOP, STEAM), T_RECUP_OUT, P_SUCTION):.1f} m flange at (-6, 0, 2), out at the top by a {main_dia(mol_flow(H2_TOP), T_COLD, P_SUCTION):.1f} m flange at (0, 0, 23) to the compressor. "
              f"The condensate and spray water drain from its sump to a skid along x at y 5.3 (7 x 1.4 m, x -3.5 to 3.5): plate exchangers and spray pumps, the cooled water back up a riser to the sprays. The exchangers give the heat to the tower loop, {Q_SCRUB / (4186 * 10):,.0f} kg/s at a 10 K rise, by a pair of {math.sqrt(4 * Q_SCRUB / (4186 * 10) / 1000 / 2.5 / math.pi):.1f} m flanges on the +y face at z 1 (flow x 2.75, return x 4.25); "
              "the condensate overflows by a 200 mm pipe from the skid's +x end to a flange on the +x face at (6, 5.3, 0.5), to the hydrogen plant."),
        built_of=[part("Shell, heads and seams", A36, 89008.8, f"{Ds:.1f} m across, 22 m, 16 mm plate; measured on model v1 (by formula {cyl_shell(Ds, 22.0, 0.016, bottom=2.0) * RHO['steel']:,.0f} kg)"),
                  part("Packing", "material.stainless-steel-304", math.pi * Ds ** 2 / 4 * 10.0 * 250.0, "10 m of metal packing at 250 kg/m3"),
                  part("Skirt, skid, exchanger frames and supports", A36, 16308.3, "measured on model v1"),
                  part("Pipework", A36, 12567.4, "the spray riser, sump drain, tower loop and condensate pipes on the unit; measured on model v1"),
                  part("Exchanger plates", "material.stainless-steel-304", px_area * 0.0006 * 7900 * 1.15, f"{px_area:,.0f} m2 of 0.6 mm plate (and 15% for ports and edges): the {Q_SCRUB / 1e6:.0f} MW at 4,000 W/m2 K over a 5 K mean difference, from memory"),
                  part("Spray pumps and motors", "good.pumps", spray_kw * 12.0, f"three pumps for {Q_SCRUB / (4186 * 10):,.0f} kg/s at 3 bar, {spray_kw:,.0f} kW in all, at 12 kg/kW with their motors (from memory)")],
        needs={"power": Q_SCRUB / (4186 * 10) / 1000 * 300e3 / 0.75, "heat": Q_SCRUB},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"Across from the gas's {q_s:.0f} m3/s at 3 m/s; height and packing from memory."},
               {"of": ["needs"], "tier": "derived", "note": f"The gas cooled from {T_RECUP_OUT:.0f} to {T_COLD:.0f} deg C and 97% of its steam condensed at 2.4 MJ/kg: {Q_SCRUB / 1e6:.0f} MW to the tower; its spray water pumped at 3 bar and 75%."}])
    # Recycle compressor: hydrogen is light, so each stage adds little pressure.
    head = CP_H2 * (T_COLD + 273.15) * ((P_SHAFT / P_SUCTION) ** ((GAMMA - 1) / GAMMA) - 1)
    stages = math.ceil(head / 80e3)
    kr = (P_SHAFT / P_SUCTION) ** (1 / 3)            # pressure ratio a casing
    out["recycle-compressor"] = dict(
        name=f"Recycle compressor, {W_COMP / 1e6:.0f} MW", size_class=3, fits="bay-s3", dims=(22.0, 8.0, 7.0),
        desc=(f"Raises {GAS:.1f} kg/s of hydrogen (the recycle with the hydrogen plant's make-up) from {P_SUCTION / 1e5:.1f} to {P_SHAFT / 1e5:.1f} bar: {stages} centrifugal stages, about 80 kJ/kg each at the impellers' tip speed limit, "
              f"in three casings on one line along x (centres x -4.5, 0, 4.5, shaft at z 2.3, 3.3 m across), driven by an electric motor at the -x end (x -11 to -6.5). The gas runs from +x to -x: "
              f"suction by a {main_dia(mol_flow(GAS), T_COLD, P_SUCTION):.2f} m flange on the +y face at (8, 4, 1.5), the recycle with the make-up (the make-up tees into the suction duct before it); "
              f"each casing's outlet up to a cooler overhead (three shell-and-tube coolers along x at z 5.4, 2.6 m across and 3.6 m long: two intercoolers, between casings at {P_SUCTION * kr / 1e5:.2f} and {P_SUCTION * kr ** 2 / 1e5:.2f} bar by {main_dia(mol_flow(GAS), T_COLD, P_SUCTION * kr):.2f} and {main_dia(mol_flow(GAS), T_COLD, P_SUCTION * kr ** 2):.2f} m pipes, and an aftercooler), "
              f"discharge from the aftercooler by a {main_dia(mol_flow(GAS), T_COLD, P_SHAFT):.2f} m flange on the -y face at (-10, -4, 1.5). "
              f"The coolers' water, {W_COMP / (4186 * 10):,.0f} kg/s at a 10 K rise, by a pair of {math.sqrt(4 * W_COMP / (4186 * 10) / 1000 / 2.5 / math.pi):.1f} m flanges on the -y face at z 1 (flow x 5.5, return x 7). "
              "The motor's 11 kV supply into its terminal box on the +y face, cable entry at (-8.75, 4, 1). The coolers' frame is the unit's own."),
        built_of=[part("Casings and rotors", "material.high-strength-steel-4340", 8000.0 * W_COMP / 1e6 * 0.6, "from memory: 8 t a MW of drive, three fifths of it the machines"),
                  part("Motor", "good.electric-motors", 8000.0 * W_COMP / 1e6 * 0.4, "from memory"),
                  part("Intercoolers and aftercooler", "material.stainless-steel-304", 15000.0, "from memory"),
                  part("Skid, feet and cooler frame", A36, 48466.1, "measured on model v1 (fabricated square tubes and feet, sized provisionally)")],
        needs={"power": W_COMP / 0.96, "heat": W_COMP},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "8 t a MW of drive and the layout from memory."},
               {"of": ["needs"], "tier": "derived", "note": f"Isentropic head {head / 1e3:.0f} kJ/kg at {ETA_K:.0%}, a motor at 96%; all the work leaves by the intercoolers and aftercooler to the tower."}])
    # Cooling gas loop: cold hydrogen through the cooling zone, its heat to the tower.
    out["dri-cooling-loop"] = dict(
        name="Sponge iron cooling loop", size_class=2, fits="bay-s2", dims=(12.0, 8.0, 16.0),
        desc=(f"Cools the sponge iron in the shaft's cooling zone from {T_BURDEN:.0f} to {T_OUT:.0f} deg C: {COOL_GAS:.1f} kg/s of hydrogen blown up through the zone, out at about 600 deg C, "
              f"Ports to the shaft on the +y face, 2 m up: hot back by a {main_dia(mol_flow(COOL_GAS), 600.0, 1.5e5):.1f} m flange at (2, 4, 2), insulated, cold out by a {main_dia(mol_flow(COOL_GAS), T_COLD, 1.5e5):.1f} m flange at (-2, 4, 2). "
              f"The hot gas rises to the top of a vertical shell-and-tube cooler (2.6 m across, z 3 to 12, on legs, at (0, -2.1)), gas in the tubes, water in the shell, down to 60 deg C; out at its bottom by a {main_dia(mol_flow(COOL_GAS), 60.0, 1.5e5):.1f} m pipe into the bottom of a small packed scrubber (3 m across, 14 m tall, 12 mm plate, at (-3, 0)) "
              "that washes out the dust with recirculated spray and trims the gas to 40 deg C; from its top down to a compressor at x 3 (motor on its -y side) and back to the cold flange. "
              f"The cooler's and the scrubber's water, {(Q_COOLZONE + W_COOL) / (4186 * 10):,.0f} kg/s at a 10 K rise, by a pair of {math.sqrt(4 * (Q_COOLZONE + W_COOL) / (4186 * 10) / 1000 / 2.5 / math.pi):.2f} m flanges on the -x face at z 1 (flow y -1, return y 1); "
              "the scrubber's blowdown (dust slurry) by a 100 mm drain to (-6, -3, 0.5); the motor's 6.6 kV supply into its terminal box, cable entry on the -y face at (3, -4, 1)."),
        built_of=[part("Scrubber shell, ends, skirt and seams", A36, 16785.4, f"3 m across, 14 m, 12 mm plate, 18 mm end plates; measured on model v1 (by formula {cyl_shell(3.0, 14.0, 0.012, bottom=2.0) * RHO['steel']:,.0f} kg)"),
                  part("Scrubber packing", "material.stainless-steel-304", math.pi * 3.0 ** 2 / 4 * 6.0 * 250.0, "6 m of metal packing at 250 kg/m3, from memory"),
                  part("Gas cooler", A36, 20000.0, "from memory"),
                  part("Supports, skids and seats", A36, 17103.0, "the cooler's legs, the machine skid and seats, the pipes' feet and braces; measured on model v1"),
                  part("Compressor and motor", "good.electric-motors", 8000.0 * W_COOL / 1e6 + 5000.0, "from memory")],
        needs={"power": W_COOL / 0.96, "heat": Q_COOLZONE + W_COOL},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "Sizes from memory."},
               {"of": ["needs"], "tier": "derived", "note": f"The sponge iron's {S_RATE:.1f} kg/s at {CP_DRI:.0f} J/kg K over {T_BURDEN - T_OUT:.0f} K; the gas's rise 40 to 600 deg C; compression 1.3 times at {ETA_K:.0%}."}])
    return out


def shaft_furnace_air(us, recs):
    """The hydrogen shaft furnace on a world with air at 1 g: an open braced tower round the shaft, the gas loop on the
    ground beside it, cooling towers to the north. Open, not a hall: a hydrogen leak must rise away, not gather under a roof."""
    sh_key = "shaft-furnace-200th"
    sh = us[sh_key]
    side, _, H = sh["dims"]
    cell = 3.0
    top = math.ceil((H + 4.0) / cell) * cell             # the tower's roof over the belt's head house
    box = (-12.0, -12.0, 0.0, 12.0, 12.0, top)
    z_top_gas = float(sh["desc"].split("top gas outlet at z ")[1].split(" ")[0])
    z_bustle = float(sh["desc"].split("a ring ")[1].split("at z ")[1].split(",")[0])
    z_ring = float(sh["desc"].split("underside is at z ")[1].split(" ")[0])
    decks = (9, 15, 24, 30, 36, 45)                      # m: lock and feeders, cooling ports, bustle, reduction zone, under the top gas outlet, seal leg
    openings = [(9.0, -3.0, z_top_gas - 3.0, 12.0, 3.0, top - 3.0),            # east wall: the top gas main out
                (-3.0, -3.0, top - 3 * cell, 12.0, 3.0, top),                  # roof and the east wall's top: the pellet belt's gallery, rising at 15 degrees, comes in through the roof to its head over the hopper
                (-3.0, -12.0, z_bustle - 3.0, 3.0, -9.0, z_bustle + 3.0),     # south wall: the hot gas main in to the bustle
                (-12.0, -3.0, 3.0, -9.0, 3.0, z_ring)]                        # west wall: the product belt out, the cooling loop's pipes
    cuts = [(-11.9, 9.1, 0.1, -6.1, 11.9, top - 0.1),
                *[(-8.9, 8.9, z + 0.1, -6.1, 9.1, z + 2.9) for z in decks]]    # a doorway from each landing onto its platform in the stair's inner face (posts and headers kept)             # the stair's prism in the north-west wall cells: members inside it go, those in its faces (posts, face bracing) stay
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, round(top / cell) - 1, None, openings, cuts)
    x0, y0, z0, x1, y1, z1 = box
    # Wind: 1 kPa (invented) on one face; the tower's walls take its moment as chords.
    wind_M = 1000.0 * (x1 - x0) * top * top / 2
    chord_force = wind_M / (y1 - y0) / round((x1 - x0) / cell + 1)
    sec = frames.lightest_section(chord_force)
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec, cell, diag)
            members.append({"item": key, "count": n, "where": "tower walls" if g == "wall" else "tower roof"})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec, cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    # The shaft hangs from its ring girder on eight box columns.
    srec = recs[sh_key]
    load = (srec["physical"]["mass"] + srec.get("holds", 0)) * G
    col_a = 0.4 * 0.4 - 0.35 * 0.35
    n_col = 8
    col_sigma = load / n_col / col_a
    while 1.5 * col_sigma > 470e6:
        n_col += 4
        col_sigma = load / n_col / col_a
    col_r = math.sqrt((0.4 ** 4 - 0.35 ** 4) / 12 / col_a)
    members.append({"item": "stock.st4340-box-400x400x25", "count": n_col, "where": f"columns under the shaft's ring girder, {z_ring:.0f} m, on a circle of radius {float(sh['desc'].split('(radius ')[1].split(' ')[0]):g} m beside the cooling zone and cone, the first at 22.5 degrees off x so the product belt passes between; braced to the tower's walls every 6 m"})
    fmass += n_col * z_ring * col_a * RHO["steel"]
    deck = 6 * (x1 - x0) * (y1 - y0) * 0.6 * 0.01 * RHO["steel"]       # six platforms of grating, 60% of the plan
    flights = math.ceil((top - cell) / 3.0)               # switchback flights of 3 m up to the roof
    deck += flights * 750.0 * 0.75                        # the stair's flights and landings, 750 kg a 4 m flight (as the stair tower's)
    cap = frames.strut_capacity(yaml.safe_load(open(S + "mill-stock/" + strut(sec, cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml")))
    sized = (f"At 1 g the shaft with its burden, {load / 1e6:.1f} MN, hangs from its ring girder at z {z_ring:.0f} on {n_col} columns of 400 x 400 x 25 mm 4340 box: {col_sigma / 1e6:.0f} MPa each, against 470 MPa with half again to spare; "
             f"braced every 6 m their slenderness is {6.0 / col_r:.0f}. Wind of 1 kPa (invented) on a {x1 - x0:.0f} x {top:.0f} m face makes {wind_M / 1e6:.0f} MN m at the base, {chord_force / 1e3:.0f} kN in each wall chord: "
             f"{sec} takes {cap / 1e3:.0f} kN, the kit's lightest with half again to spare. Six grating platforms, 10 mm, 60% of the plan. Footings' loads and the belt's head house not yet checked.")

    heater = next(k for k in us if k.startswith("gas-heater-"))
    htr = next(k for k in us if k.startswith("heater-transformer-"))
    towers = ["cooling-tower-shaft"] * us["cooling-tower-shaft"]["count"]
    bays = [
        {"id": "F", "mount": f"plant-mount.{sh['fits']}", "unit": f"unit.{sh_key}", "at": [0.0, 0.0, 0.0], "note": "in the tower, on its columns; the product belt under its discharge"},
        {"id": "R", "mount": "plant-mount.bay-s2", "unit": "unit.recuperator", "at": [24.0, 24.0, 0.0], "note": "north-east of the tower, under the top gas main's fall"},
        {"id": "S", "mount": "plant-mount.bay-s2", "unit": "unit.top-gas-scrubber", "at": [48.0, 24.0, 0.0], "note": "east of the recuperator"},
        {"id": "K", "mount": "plant-mount.bay-s3", "unit": "unit.recycle-compressor", "at": [60.0, 0.0, 0.0], "note": "in the open, an outdoor machine (no shed), under the pellet belt's gallery (36 m over it); its casings' tops and rotors lifted by a mobile crane from a hardstanding on its south side"},
        {"id": "H", "mount": "plant-mount.bay-s3", "unit": f"unit.{heater}", "at": [24.0, -24.0, 0.0], "note": "south-east of the tower, its hot header toward the bustle"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": f"unit.{htr}", "at": [48.0, -24.0, 0.0], "note": "in a fire-walled vault beside the heater's terminal house"},
        {"id": "L", "mount": "plant-mount.bay-s2", "unit": "unit.dri-cooling-loop", "at": [-24.0, -24.0, 0.0], "note": "south-west of the tower, clear of the product belt"},
    ]
    bays += [{"id": f"C{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{t}", "at": [-102.0 + 54.0 * i, 60.0, 0.0], "note": "north, in a row 6 m apart, each with its own basin, its ports on its east end facing the gap"} for i, t in enumerate(towers)]
    d_top = main_dia(mol_flow(H2_TOP, STEAM), T_TOP, P_SUCTION)
    d_wet = main_dia(mol_flow(H2_TOP, STEAM), T_RECUP_OUT, P_SUCTION)
    d_dry = main_dia(mol_flow(H2_TOP), T_COLD, P_SUCTION)
    d_cmp = main_dia(mol_flow(GAS), T_COLD, P_SHAFT)
    d_pre = main_dia(mol_flow(GAS), T_PRE, P_SHAFT)
    d_hot = main_dia(mol_flow(GAS), T_GAS, P_SHAFT)
    d_mk = main_dia(mol_flow(H2_USED), T_COLD, P_SUCTION)
    d_cg = main_dia(mol_flow(COOL_GAS), T_COLD, 1.5e5)
    d_cg_hot = main_dia(mol_flow(COOL_GAS), 600.0, 1.5e5)
    pm = lambda d, t=0.012: math.pi * d * t * RHO["steel"]
    lined = lambda d: math.pi * (d + 0.4) * 0.016 * RHO["steel"] + math.pi * (d + 0.2) * 0.2 * RHO["refractory"]
    rise = 15.0
    belt_len = (H + 2.0 - 2.0) / math.sin(math.radians(rise))
    belt_x = belt_len * math.cos(math.radians(rise))
    loop_heat = sum(recs[k]["needs"].get("heat", 0) for k in ("top-gas-scrubber", "recycle-compressor", "dri-cooling-loop", htr))
    flow = loop_heat / (4186 * 10)
    d_cw = math.sqrt(4 * flow / 1000 / 2.5 / math.pi)
    d_scr = math.sqrt(4 * Q_SCRUB / (4186 * 10) / 1000 / 2.5 / math.pi)
    d_kc = math.sqrt(4 * W_COMP / (4186 * 10) / 1000 / 2.5 / math.pi)
    d_lc = math.sqrt(4 * (Q_COOLZONE + W_COOL) / (4186 * 10) / 1000 / 2.5 / math.pi)
    links = [
        link("conveyor", "pellet belt", (round(belt_x, 1), 0.0, 2.0), (0.0, 0.0, round(H + 2.0, 1)), A36, 180.0, 300000.0, f"a 1.2 m belt in a covered gallery on trestles every 18 m, 180 kg/m with the gallery (from memory), rising at {rise:g} degrees from the pellet plant's transfer house to a head house on the tower's roof over the charge hopper, the belt's head 2 m above the hopper's rim (z 50) dropping into it by a closed hood and chute, flanged to the rim; {PELLETS:.0f} kg/s of pellets; over the recycle compressor its trestles are portals straddling it (legs at y -5.5 and 5.5, clear to z 9, braced above)"),
        link("conveyor", "product belt", (0.0, 0.0, 5.0), (-60.0, 0.0, 5.0), A36, 150.0, 150000.0, "a 1 m belt in a gallery 5 m up, from under the discharge lock (its bottom at z 7) west to the arc furnace shop's sponge iron bin, beyond the layout; the lock drops onto it by a closed chute flanged to its bottom, loading the belt 1 m west of the axis, through an opening in the gallery's cover"),
        link("duct", "top gas main", (sh["dims"][0] / 2 - 0.5, 0.0, z_top_gas), (24.0, 24.0, 18.0), A36, pm(d_top), None, f"{d_top:.1f} m across, 12 mm plate, insulated: {T_TOP:.0f} deg C top gas with its steam, out of the tower's east wall and down to the recuperator", via=[(15.0, 0.0, z_top_gas), (24.0, 24.0, z_top_gas)]),
        link("duct", "wet gas to the scrubber", (24.0, 29.2, 1.5), (42.0, 24.0, 2.0), A36, pm(d_wet), None, f"{d_wet:.1f} m across, 12 mm plate", via=[(24.0, 32.0, 1.5), (40.0, 32.0, 1.5), (40.0, 24.0, 2.0)]),
        link("duct", "clean gas to the compressor", (48.0, 24.0, 23.0), (68.0, 4.0, 1.5), A36, pm(d_dry), None, f"{d_dry:.1f} m across, 12 mm plate; the hydrogen plant's make-up joins it here", via=[(48.0, 24.0, 26.0), (68.0, 24.0, 26.0), (68.0, 7.0, 26.0), (68.0, 7.0, 1.5)]),
        link("pipe", "compressed gas to the recuperator", (50.0, -4.0, 1.5), (24.0, 18.8, 1.0), A36, pm(d_cmp), None, f"{d_cmp:.1f} m across, 12 mm wall, {P_SHAFT / 1e5:.1f} bar", via=[(50.0, -6.0, 1.5), (36.0, -6.0, 1.5), (36.0, 14.0, 1.5), (24.0, 14.0, 1.0)]),
        link("pipe", "warm gas to the heater", (24.0, 18.8, 16.75), (12.0, -27.3, 2.0), A36, pm(d_pre), None, f"{d_pre:.1f} m across, 12 mm wall, insulated, {T_PRE:.0f} deg C: down the tower's east side and in at the heater's west end, into its cold header", via=[(24.0, 14.0, 16.75), (14.0, 14.0, 16.75), (14.0, -15.0, 16.75), (10.0, -15.0, 16.75), (10.0, -15.0, 2.0), (10.0, -27.3, 2.0)]),
        link("duct", "hot gas to the bustle", (24.0, -20.0, 18.0), (0.0, -side / 2, z_bustle), A36, lined(d_hot), None, f"{d_hot:.1f} m inside 200 mm of refractory, 16 mm shell: hydrogen at {T_GAS:.0f} deg C, in through the tower's south wall to the bustle's inlet; its shell runs warm, so it is the hydrogen-tight steel of the loop that matters (not yet specified); its support beside the heater stands outside the heater's footprint, carrying the duct on a head beam", via=[(24.0, -20.0, z_bustle), (0.0, -20.0, z_bustle)]),
        link("pipe", "hydrogen make-up", (100.0, 4.0, 1.5), (68.0, 7.0, 1.5), A36, pm(d_mk, 0.008), None, f"{d_mk:.1f} m across, 8 mm wall: {H2_USED:.2f} kg/s from the hydrogen plant, beyond the layout, bridging over the water mains at z 2.5, teeing into the suction duct at (68, 7, 1.5)", via=[(91.0, 4.0, 1.5), (91.0, 4.0, 2.5), (83.0, 4.0, 2.5), (83.0, 4.0, 1.5), (73.0, 4.0, 1.5), (73.0, 7.0, 1.5)]),
        link("pipe", "condensate to the hydrogen plant", (54.0, 29.3, 0.5), (100.0, 29.3, 0.5), A36, math.pi * 0.2 * 0.006 * RHO["steel"], None, f"200 mm, 6 mm wall: the {STEAM:.1f} kg/s of water the reduction made, back to the electrolysers, beyond the layout", via=[(83.0, 29.3, 0.5), (83.0, 29.3, 1.75), (91.0, 29.3, 1.75), (91.0, 29.3, 0.5)]),
        link("pipe", "tower loop to the scrubber", (84.0, 31.5, 0.75), (51.5, 30.0, 1.0), A36, 2 * pm(d_scr, 0.012), None, f"a pair of {d_scr:.1f} m pipes, 12 mm wall, flow and return, off the mains on the plant's east side, west along y 31.5 and up to the scrubber's flanges on its +y face", via=[(51.5, 31.5, 0.75), (51.5, 31.5, 1.0)]),
        link("pipe", "cooler water to the compressor", (66.25, -12.0, 0.75), (66.25, -4.0, 1.0), A36, 2 * pm(d_kc, 0.010), None, f"a pair of {d_kc:.1f} m pipes, 10 mm wall, off the mains' end, north to the compressor's flanges on its -y face", via=[(66.25, -5.0, 0.75), (66.25, -5.0, 1.0)]),
        link("pipe", "cooling water to the sponge iron cooling loop", (-34.0, 42.0, 0.75), (-30.0, -24.0, 1.0), A36, 2 * pm(d_lc, 0.010), None, f"a pair of {d_lc:.2f} m pipes, 10 mm wall, off the mains at the west end, south in a trench clear of the tower to the loop's flanges on its -x face", via=[(-34.0, -24.0, 0.75), (-31.0, -24.0, 0.75), (-31.0, -24.0, 1.0)]),
        *[link("pipe", f"tower water to C{i + 1}, pair {j + 1}", (-77.15 + 1.4 * j + 54.0 * i, 42.0, 0.75), (-78.0 + 54.0 * i, 60.0 + 4.5 * j, 0.75), A36, 2 * pm(0.5, 0.008), None, f"a pair of 500 mm pipes, 8 mm wall, flow and return, off the mains north up the gap between the towers to the port pair on the tower's east end; the gap's four lanes at 0.5, 1.2, 1.9 and 2.6 m east of the ports, this pair in the {('first two', 'last two')[j]}; the flow bridges over the return main at z 1.8", via=[(-77.15 + 1.4 * j + 54.0 * i, 60.0 + 4.5 * j, 0.75)]) for i in range(len(towers)) for j in range(2)],
        link("pipe", "cooling water to the heater transformer", (60.0, -12.0, 0.75), (48.0 - recs[htr]["physical"]["length"] / 2, -24.0, 1.0), A36, 2 * math.pi * 0.15 * 0.005 * RHO["steel"], None, f"a pair of 150 mm pipes, 5 mm wall, flow and return: {recs[htr]['needs']['heat'] / (4186 * 10):.0f} kg/s at a 10 K rise for its oil-to-water coolers, off the mains' end, west in a trench to x 42, south, in under the vault's wall to the water ports on its -x end, 1 m up, at y -24.6 and -23.4 (the pair's centre here); in a trench 0.8 m wide, its floor at z -1.3, the pipes' centres at z -0.6 and -0.9 under the mains and crossings, rising at x 42 through bored openings in the vault's wall", via=[(60.0, -12.0, -0.75), (42.0, -12.0, -0.75), (42.0, -24.0, -0.75), (42.0, -24.0, 1.0)]),
        link("pipe", "cooling water mains", (-79.0, 42.0, 0.75), (60.0, -12.0, 0.75), A36, 2 * pm(d_cw, 0.012), None, f"a pair of {d_cw:.1f} m pipes, 12 mm wall, flow and return (at y 41 and 43 along the row, x 86 and 88 down the east leg, capped at x -79): {flow:,.0f} kg/s at a 10 K rise, along the towers' row and down the plant to the scrubber, compressor, cooling loop and transformer", via=[(87.0, 42.0, 0.75), (87.0, -12.0, 0.75)]),
        link("pipe", "cold gas to the cooling zone", (-26.0, -20.0, 2.0), (-(side / 2 - 0.5), 0.0, z_ring - 8.0), A36, pm(d_cg, 0.008), None, f"{d_cg:.1f} m, 8 mm wall: {COOL_GAS:.1f} kg/s of hydrogen at {T_COLD:.0f} deg C, through the tower's west wall", via=[(-26.0, -17.0, 2.0), (-26.0, -17.0, z_ring - 8.0), (-26.0, 0.0, z_ring - 8.0)]),
        link("pipe", "hot gas from the cooling zone", (-(side / 2 - 0.5), 0.0, z_ring - 3.0), (-22.0, -20.0, 2.0), A36, pm(d_cg_hot, 0.010) * 1.5, None, f"{d_cg_hot:.1f} m, 10 mm wall, insulated (half as much again): about 600 deg C, back to the loop's cooler", via=[(-22.0, 0.0, z_ring - 3.0), (-22.0, -17.0, z_ring - 3.0), (-22.0, -17.0, 2.0)]),
        link("cable", "heater supply", (48.0, -21.1875, 4.5), (12.45, -21.4, 1.575), CU, 3 * Q_HEATER / HEATER_PF / (math.sqrt(3) * 6600) / 3 / 4e6 * RHO["copper"] * 3, None, "three phases at 6.6 kV in a cable tray, copper at 4 A/mm2: from the transformer's secondary terminals on its +y face, north to y -17, west along the heater's north side, down at its west end and south into its terminal house's +y face", via=[(48.0, -17.0, 4.5), (12.45, -17.0, 4.5), (12.45, -17.0, 1.575)]),
    ]
    tower_units = [recs[t] for t in towers]
    ground = [
        {"what": "Tower foundation", "item": CONC, "volume": r((x1 - x0) * (y1 - y0) * 2.5), "note": f"a mat {x1 - x0:.0f} x {y1 - y0:.0f} m, 2.5 m thick, under the tower and the shaft's columns"},
        {"what": "Compressor crane hardstanding", "item": CONC, "volume": r(26 * 10 * 0.4), "note": "26 x 10 m, 0.4 m, south of the compressor (x 47 to 73, y -14 to -4), for a mobile crane"},
        {"what": "Unit slabs", "item": CONC, "volume": r(sum(recs[b["unit"].split(".", 1)[1]]["physical"]["length"] * recs[b["unit"].split(".", 1)[1]]["physical"]["width"] * 0.8 for b in bays[1:] if not b["id"].startswith("C"))), "note": "0.8 m under each unit on the ground"},
        {"what": "Transformer vault", "item": CONC, "volume": r(12 * 8 * 0.5 + 40 * 7 * 0.3), "note": "fire walls 7 m tall round the heater's transformer"},
        {"what": "Cooling tower basins", "item": CONC, "volume": r(sum((t["physical"]["length"] + 0.6) * 12.6 * 0.5 + 2 * (t["physical"]["length"] + 0.3 + 12.3) * 2.0 * 0.3 for t in tower_units)), "note": "one under each tower, sunk as the furnace shop's: 0.3 m walls, floor top at z -2; 6 m apart, no shared walls"},
        {"what": "Gallery trestle footings", "item": CONC, "volume": r(math.ceil(belt_x / 18) * 8.0 + 4 * 4.0), "note": "8 m3 under each of the pellet belt's trestles, 4 m3 under the product belt's"},
        {"what": "Pipe supports", "item": CONC, "volume": 56.46, "note": "plinths and feet under the ground-level pipes and the duct supports; measured on blender's layout (candidate v01)"},
        {"what": "Coolant trench", "item": CONC, "volume": 24.98, "note": "the transformer's water pair's trench, 0.8 m wide, floor at z -1.3; measured on blender's layout (candidate v01)"},
    ]
    measured = {"Cooling tower basins": 1587.28, "Tower foundation": 1440.0, "Unit slabs": 683.36, "Transformer vault": 128.70, "Compressor crane hardstanding": 104.0, "Gallery trestle footings": 92.0}   # m3 on blender's layout (candidate v01)
    for g in ground:
        if g["what"] in measured:
            g["volume"] = measured[g["what"]]
            g["note"] = g.get("note", "") + "; measured on blender's layout (candidate v01)"
        g["mass"] = r(g["volume"] * RHO["concrete"])
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    water = sum(u.get("needs", {}).get("water", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + deck
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = smod["physical"]
    against = (f"module.shaft-furnace: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {S_P / 1e6:.0f} MW, which leaves out heating the gas. "
               f"This layout: the same {S_RATE * 3.6:.0f} t of sponge iron an hour; {power / 1e6:.0f} MW, of which the hydrogen heater is {Q_HEATER / 0.98 / 1e6:.0f}: hydrogen brings no heat of its own, so the shaft's {Q_SHAFT / 1e6:.0f} MW comes from {GAS:.1f} kg/s of gas "
               f"({GAS / H2_USED:.1f} times what the reduction uses) heated by electricity; {heat / 1e6:.0f} MW to the towers, {water:.0f} kg/s of water evaporated; the {STEAM:.1f} kg/s of water made goes back to the hydrogen plant. "
               f"{(mass_units + mass_links + frame_mass) / 1000:,.0f} t of plant and frame, {mass_ground / 1000:,.0f} t of concrete.")
    xs = [x0, x1] + [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [y0, y1] + [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.shaft-furnace-air", "name": "Hydrogen shaft furnace, world with air",
                     "description": "Direct reduction of iron ore pellets by hydrogen: a refractory-lined shaft in an open braced tower, its gas loop on the ground beside it (recuperator, scrubber, recycle compressor, electric heater) and a cooling loop for the sponge iron; four cooling towers to the north. The pellets come up a belt from the pellet plant, the sponge iron leaves by belt to the arc furnace shop, the hydrogen comes from the hydrogen plant and the water the reduction makes goes back to it. Open, not a hall, so a hydrogen leak rises away. For a world with air at about 1 g."},
        "stands_for": "module.shaft-furnace",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": bays, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"An open braced tower {x1 - x0:.0f} x {y1 - y0:.0f} m and {top:.0f} m tall round the shaft: walls one cell thick, a roof one cell deep carrying the belt's head house, six grating platforms at z {', '.join(str(z) for z in decks)} (the lock and feeders, the cooling zone's ports, round the bustle, the reduction zone, 3 m under the top gas main, the seal leg), each 18 x 18 m inside the walls with a 10 m hole round the shaft, but at z 24 the tower's full 24 x 24 m plan (out over the wall cells) with a 17 m hole round the bustle (about 15 m across); grating seated on the frame's nodes; "
                           f"stairs in switchback flights 1.2 m wide inside the north-west corner of the walls (x -12 to -6, y 9 to 12; the members inside that prism left out, its faces' posts and bracing kept), landing beams on the 3 m nodes, landing on each platform through a doorway 3 m wide and 3 m high in the stair's inner face (x -9 to -6, y 9) and the roof at z {top - cell:.0f}; no cladding.",
                  "openings": [list(o) for o in openings + cuts], "members": members, "carries": [f"unit.{sh_key}"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": S_RATE, "power": r(power), "heat": r(heat), "water": r(water), "mass": r(total), "against_module": against},
        "staff": smod.get("staff"),
        "basis": [{"of": ["bays", "links", "frame.box"], "tier": "invented", "review": True, "note": "Laid out from memory of direct reduction plants; the gas loop's temperatures and pressures from memory."},
                  {"of": ["totals"], "tier": "derived", "review": True, "note": f"From the heat balance in tools/standards/plants.py: the reduction takes {ENDO / 1e6:.2f} MJ a kg of iron (handbook), the pellets are warmed to {T_BURDEN:.0f} deg C, {SHELL_LOSS:.0%} is lost through the shell; the gas falls from {T_GAS:.0f} to {T_TOP:.0f} deg C."},
                  {"of": ["frame.members", "frame.mass"], "tier": "derived", "review": True, "note": "Counted from the lattice; sections and columns chosen by the loads in frame.sized."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes from the shapes in the notes, concrete at 2,400 kg/m3; not sized for the ground's bearing."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.shaft-furnace-air: {total / 1000:,.0f} t, {power / 1e6:.0f} MW (heater {Q_HEATER / 1e6:.0f}), gas {GAS:.1f} kg/s, {heat / 1e6:.0f} MW to {len(towers)} towers, {water:.0f} kg/s water, frame {frame_mass / 1000:.0f} t ({sec}, {n_col} columns)")
    return layout, bays


# ---------------------------------------------------------------- the hydrogen plant's figures
hmod = yaml.safe_load(open(S + "modules/hydrogen-plant.yaml"))
HREC = hmod["recipes"][0]
H_RATE, H_P = HREC["rate"], HREC["power"]                  # kg/s of hydrogen, W (sourced: 53 kWh a kg)
H_WATER = HREC["inputs"][0]["quantity"] * H_RATE
H_O2 = HREC["outputs"][0]["quantity"] * H_RATE
FARADAY = 96485.0
N_BLK = 5                          # electrolyser blocks, 20 MW each: chosen
V_CELL, J_CELL = 1.9, 4000.0       # V a cell and A/m2 of active area: from memory of pressurised alkaline electrolysers
V_TN = 1.48                        # V: the thermoneutral voltage; what a cell takes above it leaves as heat
CELL_A, CELLS_STACK, STACKS = 2.7, None, 4   # m2 of active area a cell (1.85 m across): from memory
I_BLK = H_RATE / N_BLK / 2.016e-3 * 2 * FARADAY             # A summed over every cell of one block (ampere-cells), all of it making gas (a Faraday efficiency of 1)
P_STACKS = I_BLK * V_CELL                                    # W into one block's cells
N_CELLS = math.ceil(I_BLK / J_CELL / CELL_A)
PER_STACK = math.ceil(N_CELLS / STACKS)
I_DC = I_BLK / PER_STACK                                     # A of direct current into a block: its stacks in parallel, each its cells in series
NI = "material.nickel-200"


def hydrogen_units():
    out = {}
    area = I_BLK / J_CELL
    n_cells, per_stack = N_CELLS, PER_STACK
    V_stack = per_stack * V_CELL
    pitch = 0.025
    L_stack = per_stack * pitch + 0.4
    plate = 3.5 * 0.003 * 7800                               # a bipolar plate of 3 mm steel over the cell's 3.5 m2 with its rim
    elec = 2 * 1.5 * CELL_A + 2 * 3.5 * 25e-6 * 8890          # two nickel mesh electrodes at 1.5 kg/m2 and 25 um of nickel plating both sides
    dia = 0.5 * CELL_A
    ends = STACKS * (2 * math.pi * 1.2 ** 2 * 0.2 * 7850 + 2000.0)
    lye = (n_cells * CELL_A * 0.006 + 2 * math.pi * 1.0 ** 2 * 6.0 * 0.5) * 1280.0
    seps = 2 * cyl_shell(2.0, 6.0, 0.012, bottom=2.0) * RHO["steel"]
    out["electrolyser-block-20mw"] = dict(
        name=f"Electrolyser block, {P_STACKS / 1e6:.0f} MW", size_class=3, fits="bay-s3", dims=(24.0, 12.0, 8.0),
        desc=(f"Alkaline electrolysis at about 3 bar: {STACKS} stacks of {per_stack} cells each ({CELL_A:g} m2 active, {J_CELL / 1e4:g} A/cm2 at {V_CELL:g} V, {V_stack:.0f} V a stack), lying along y at local x -9, -4, 1 and 6, "
              f"each {L_stack:.1f} m long and 2.4 m across on a skid 1 m high; the lye (30% potassium hydroxide) carries the gases to two separators at x 10.5 (hydrogen at y -2.5, oxygen at y 2.5), 2 m across and 6 m tall on 2 m legs, "
              f"and back by pumps through coolers. {I_DC / 1e3:.0f} kA of direct current by a busbar to its terminals at local (0, -6, 3); hydrogen and oxygen out of the separators' tops (z 8) into the layout's headers; demineralised water and the cooling loop in at local (11, 6, 1)."),
        built_of=[part("Bipolar plates", A36, n_cells * plate, f"{n_cells} plates of 3 mm steel, 3.5 m2 each with its rim, nickel-plated"),
                  part("Electrodes and plating", NI, n_cells * elec, "nickel mesh at 1.5 kg/m2 each side, 25 um of plating"),
                  part("Diaphragms", "material.polyethylene-uhmw", n_cells * dia, "porous polysulfone with zirconia, about 0.5 kg/m2, which the registry has not got: polyethylene stands for it"),
                  part("End plates and tie rods", A36, ends, f"two plates 2.4 m across, 200 mm, and 2 t of rods a stack"),
                  part("Separators", A36, seps, "two vessels 2 m across, 6 m, 12 mm plate"),
                  part("Lye", "good.water", lye, "30% potassium hydroxide lye in the cells and half the separators, 1,280 kg/m3, which the registry has not got: water stands for it"),
                  part("Lye pumps and coolers", "good.pumps", 8000.0, "from memory")],
        needs={"power": P_STACKS, "heat": I_BLK * (V_CELL - V_TN), "water": H_WATER / N_BLK},
        recipes=[dict(HREC, rate=H_RATE / N_BLK, power=P_STACKS)],
        basis=[{"of": ["recipes"], "tier": "derived", "note": f"The module's recipe (module.hydrogen-plant) for one block of {N_BLK}: the hydrogen from Faraday's law: {I_DC / 1e3:.0f} kA through {STACKS} stacks of {PER_STACK} cells in series; the power the cells' share ({P_STACKS * N_BLK / H_P:.0%} of the module's, the rest the rectifiers and plant)."},
               {"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"Cells from the current: {area:,.0f} m2 of active area at {J_CELL / 1e4:g} A/cm2; plate, mesh and diaphragm weights and the stack's layout from memory of pressurised alkaline electrolysers."},
               {"of": ["needs.heat"], "tier": "derived", "note": f"The cells' voltage over the thermoneutral {V_TN} V leaves as heat: {(V_CELL - V_TN) * I_BLK / 1e6:.1f} MW, to the cooling loop."}])
    P_rect = (H_P / N_BLK - P_STACKS) * 0.6                   # of the block's share above its cells: the rectifier's losses; the rest the plant's
    rt = transformer_unit(P_STACKS / 0.95, f"one electrolyser block: a 12-pulse thyristor rectifier in the same skid gives {I_DC / 1e3:.0f} kA at {V_stack:.0f} V direct")
    rt["name"] = f"Rectifier transformer, {P_STACKS / 0.95 / 1e6:.0f} MVA"
    rt["built_of"].append(part("Rectifier", AL, 10000.0, "thyristor stacks on water-cooled aluminium heat sinks, from memory"))
    rt["needs"] = {"power": P_rect, "heat": P_rect}
    rt["desc"] = rt["desc"].replace("Three secondary terminals on one long face (+y)", "The rectifier's two DC terminals on one long face (+y)")
    out["rectifier-transformer-21mva"] = rt
    out["hydrogen-purifier"] = dict(
        name="Hydrogen purifier", size_class=2, fits="bay-s2", dims=(12.0, 8.0, 9.0),
        desc="Takes the last oxygen out of the hydrogen over a palladium catalyst (it burns to water) and dries it in two adsorber vessels that take turns, one regenerating by heat while the other works. Gas in at local (-6, 0, 6), out at (6, 0, 2).",
        built_of=[part("Vessels and piping", A36, 25000.0, "a deoxidiser and two adsorbers 1.5 m across and 6 m, from memory"),
                  part("Adsorbent", "material.fused-silica", 8000.0, "molecular sieve beads, which the registry has not got: fused silica stands for it")],
        needs={"power": 300e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "Sizes, masses and the regeneration heaters' power from memory."}])
    out["water-treatment"] = dict(
        name="Water treatment", size_class=2, fits="bay-s2", dims=(12.0, 8.0, 5.0),
        desc=f"Makes the {H_WATER:.1f} kg/s of demineralised water the cells split: filters, reverse osmosis and an electrodeionisation stage, on skids under a roof. Raw water in at local (6, 0, 0.5), clean water out at (-6, 0, 0.5).",
        built_of=[part("Skids and membranes", A36, 25000.0, "from memory"), part("Pumps", "good.pumps", 4000.0, "from memory")],
        needs={"power": 200e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["stack-crane-20t"] = bridge_crane_unit(20000.0, 22.8, 6000.0, kind="Stack crane", job="it lifts an electrolyser stack's end plates and cell packs out for refurbishment and sets in new ones", gear="a main hoist")
    return out


def hydrogen_plant_air(us, recs):
    """The hydrogen plant on a world with air at 1 g: five electrolyser blocks in a vented hall, their rectifiers outside."""
    cell, box = 3.0, (-60.0, -15.0, 0.0, 60.0, 15.0, 18.0)
    xs_b = [-48.0, -24.0, 0.0, 24.0, 48.0]
    openings = [(x - 1.5, -15.0, 3.0, x + 1.5, -12.0, 6.0) for x in xs_b]           # the DC busbars, south wall
    openings += [(57.0, -9.0, 6.0, 60.0, 9.0, 12.0),                                 # the hydrogen and oxygen headers, east wall
                 (-3.0, 12.0, 0.0, 3.0, 15.0, 3.0),                                  # the cooling water mains, north wall
                 (57.0, -3.0, 0.0, 60.0, 3.0, 6.0)]                                  # a door, east
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 5, None, openings)
    x0, y0, z0, x1, y1, z1 = box
    span_in = y1 - y0 - 2 * cell
    crane = recs["stack-crane-20t"]
    cap, trolley = 20000.0, 6000.0
    RUNWAY = 11.0
    roof_q = (0.002 * RHO["aluminium"] + 20.0) * G + 1000.0
    chord = roof_q * cell * span_in ** 2 / 8 / cell
    wheel = ((cap + trolley) * (1 - 1.5 / (span_in - 1.2)) + crane["physical"]["mass"] / 2) * G / 2
    vert = wheel + roof_q * cell * span_in / 2
    sec = {"roof": frames.lightest_section(chord), "wall": frames.lightest_section(vert)}
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec[g], cell, diag)
            members.append({"item": key, "count": n, "where": g})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec["wall"], cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    rw_a = 0.4 * 0.4 - 0.35 * 0.35
    rw_sigma = wheel * cell / 4 / ((0.4 ** 4 - 0.35 ** 4) / 12 / 0.2)
    assert 1.5 * rw_sigma <= 470e6, rw_sigma
    x_in = x1 - x0 - 2 * cell
    members.append({"item": "stock.st4340-box-400x400x25", "count": 6, "where": f"crane runways, three a side, {x_in / 3:.0f} m lengths, along x on the inside of the long walls: their rail plane {RUNWAY:g} m up at y -11.4 and +11.4"})
    fmass += 2 * x_in * rw_a * RHO["steel"]
    vent = (x1 - x0 - 2 * cell) * 1.5                                                  # a ridge vent 1.5 m wide the hall's length
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - vent - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in openings)
    clad = clad_area * 0.002 * RHO["aluminium"]
    sec_stock = lambda g: yaml.safe_load(open(S + "mill-stock/" + strut(sec[g], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    rcap, wcap = frames.strut_capacity(sec_stock("roof")), frames.strut_capacity(sec_stock("wall"))
    sized = (f"At 1 g: roof {roof_q:.0f} Pa (2 mm aluminium, the frame about 20 kg/m2, 1 kPa of snow and wind, the last invented) over a {span_in:.0f} m span in 3 m deep strips puts {chord / 1e3:.0f} kN in a chord: "
             f"{sec['roof']} takes {rcap / 1e3:.0f} kN. A wheel of the 20 t crane loaded at the runway's end puts {wheel / 1e3:.0f} kN on a wall node, with the roof's share {vert / 1e3:.0f} kN: "
             f"{sec['wall']} takes {wcap / 1e3:.0f} kN. Each the kit's lightest section with half again to spare. The runway bends to {rw_sigma / 1e6:.0f} MPa between nodes, against 470 MPa with half again to spare. Wind and footings not yet checked.")
    blk = us["electrolyser-block-20mw"]
    rt = us["rectifier-transformer-21mva"]
    tower = us["cooling-tower-h2"]
    bays = [{"id": f"E{i + 1}", "mount": "plant-mount.bay-s3", "unit": "unit.electrolyser-block-20mw", "at": [x, 0.0, 0.0], "note": "in the hall"} for i, x in enumerate(xs_b)]
    bays += [{"id": f"R{i + 1}", "mount": "plant-mount.bay-s2", "unit": "unit.rectifier-transformer-21mva", "at": [x, -24.0, 0.0], "note": "outside, south, its DC terminals toward the hall"} for i, x in enumerate(xs_b)]
    bays += [{"id": "P", "mount": "plant-mount.bay-s2", "unit": "unit.hydrogen-purifier", "at": [72.0, 0.0, 0.0], "note": "outside, east"},
             {"id": "W", "mount": "plant-mount.bay-s2", "unit": "unit.water-treatment", "at": [72.0, -24.0, 0.0], "note": "outside, south-east"},
             {"id": "C", "mount": f"plant-mount.{tower['fits']}", "unit": "unit.cooling-tower-h2", "at": [0.0, 48.0, 0.0], "note": "north"}]
    rW, rH = rt["dims"][1], rt["dims"][2]
    bus_a = I_DC / 1.5e6
    d_h2 = main_dia(mol_flow(H_RATE), 40.0, 3e5) * 30.0 ** 0.5 / 15.0 ** 0.5            # at 15 m/s in the headers
    d_o2 = math.sqrt(4 * (H_O2 / 32e-3 * 8.314 * 313 / 3e5) / (math.pi * 15.0))
    pm = lambda d, t: math.pi * d * t * RHO["steel"]
    loop = sum(recs[k]["needs"].get("heat", 0) for k in ("electrolyser-block-20mw",)) * N_BLK + recs["rectifier-transformer-21mva"]["needs"]["heat"] * N_BLK
    flow = loop / (4186 * 10)
    d_cw = math.sqrt(4 * flow / 1000 / 2.5 / math.pi)
    ss = "material.stainless-steel-304"
    links = [link("busbar", f"DC bus {i + 1}", (x, -24.0 + rW / 2, rH * 2 / 3), (x, -6.0, 3.0), AL, 2 * bus_a * RHO["aluminium"], None, f"two aluminium bars, {bus_a * 1e4:.0f} cm2 each for {I_DC / 1e3:.0f} kA at 1.5 A/mm2, through the hall's south wall") for i, x in enumerate(xs_b)]
    links += [
        link("pipe", "hydrogen header", (xs_b[0] + 10.5, -2.5, 8.0), (66.0, 0.0, 6.0), ss, pm(d_h2, 0.006), None, f"{d_h2:.2f} m stainless, 6 mm wall, at 3 bar: from each block's hydrogen separator along the hall at z 9, out by the east wall to the purifier", via=[(xs_b[0] + 10.5, -2.5, 9.0), (58.5, -2.5, 9.0), (58.5, 0.0, 6.0)]),
        link("pipe", "hydrogen to the shaft furnace", (78.0, 0.0, 2.0), (110.0, 0.0, 2.0), ss, pm(d_h2, 0.006), None, f"{H_RATE:.3f} kg/s of dry hydrogen at about 3 bar, on to the shaft furnace's make-up, beyond the layout"),
        link("pipe", "oxygen header", (xs_b[0] + 10.5, 2.5, 8.0), (110.0, 6.0, 9.0), ss, pm(d_o2, 0.006), None, f"{d_o2:.2f} m stainless, 6 mm wall: the {H_O2:.1f} kg/s of oxygen, out by the east wall to a vent stack or to buyers (the arc furnace lances it), beyond the layout; clean, oil-free, as oxygen service needs", via=[(xs_b[0] + 10.5, 2.5, 9.0), (58.5, 2.5, 9.0), (58.5, 6.0, 9.0)]),
        link("pipe", "demineralised water", (66.0, -24.0, 0.5), (59.0, 6.0, 1.0), ss, pm(0.1, 0.004), None, f"100 mm stainless, {H_WATER:.1f} kg/s to the blocks' feed points along the hall's north side", via=[(62.0, -24.0, 0.5), (62.0, 6.0, 0.5)]),
        link("pipe", "raw water", (110.0, -24.0, 0.5), (78.0, -24.0, 0.5), A36, pm(0.15, 0.005), None, "150 mm: town or river water, or the shaft furnace's condensate, beyond the layout"),
        link("pipe", "cooling water mains", (tower["dims"][0] / 2, 48.0, 0.75), (-48.0 + 11.0, 6.0, 1.0), A36, 2 * pm(d_cw, 0.010), None, f"a pair of {d_cw:.2f} m pipes, 10 mm wall, flow and return: {flow:,.0f} kg/s at a 10 K rise, into the hall by the north wall and along it to each block's coolers, a branch out to the rectifiers", via=[(tower["dims"][0] / 2 + 1.5, 48.0, 0.75), (tower["dims"][0] / 2 + 1.5, 20.0, 0.75), (0.0, 20.0, 0.75), (0.0, 9.0, 1.0), (-37.0, 9.0, 1.0)]),
    ]
    ground = [
        {"what": "Hall floor", "item": CONC, "volume": r(x_in * span_in * 0.4), "note": f"0.4 m, {x_in:.0f} x {span_in:.0f} m, with a lye-proof coating and bunds round each block"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Rectifier slabs and fire walls", "item": CONC, "volume": r(N_BLK * (12 * 8 * 0.5) + (N_BLK + 1) * 8 * 6 * 0.3), "note": "a slab under each, a fire wall between"},
        {"what": "Outside slabs", "item": CONC, "volume": r(2 * 12 * 8 * 0.5), "note": "under the purifier and the water treatment"},
        {"what": "Cooling tower basin", "item": CONC, "volume": r((tower["dims"][0] + 0.6) * 12.6 * 0.5 + 2 * (tower["dims"][0] + 0.3 + 12.3) * 2.0 * 0.3), "note": "sunk as the furnace shop's"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [crane]
    power = sum(u.get("needs", {}).get("power", 0) for u in used)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    water = sum(u.get("needs", {}).get("water", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = hmod["physical"]
    xs = [x0, x1] + [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [y0, y1] + [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    site_area = (max(xs) - min(xs)) * (max(ys) - min(ys))
    against = (f"module.hydrogen-plant: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {H_P / 1e6:.0f} MW. "
               f"This layout: the same {H_RATE * 3600:,.0f} kg of hydrogen an hour; {power / 1e6:.1f} MW (the cells {N_BLK * P_STACKS / 1e6:.1f}, at {V_CELL} V, {N_BLK * P_STACKS / H_RATE / 3.6e6:.1f} kWh a kg); {heat / 1e6:.1f} MW to the tower, {water:.1f} kg/s of water used and evaporated; "
               f"{(mass_units + mass_links + frame_mass) / 1000:,.0f} t of plant and frame, {mass_ground / 1000:,.0f} t of concrete; {site_area / 1e4:.1f} ha, against the source's 0.8 to 1.7 for 100 MW. "
               f"It makes {H_RATE:.3f} kg/s; one shaft furnace at its full rate takes {H2_USED:.2f}, about {H2_USED / H_RATE:.1f} of these.")
    layout = {
        "identity": {"key": "layout.hydrogen-plant-air", "name": "Hydrogen plant, world with air",
                     "description": f"Water split by electricity: {N_BLK} pressurised alkaline electrolyser blocks of {P_STACKS / 1e6:.0f} MW in a hall with a ridge vent (hydrogen rises and must find its way out), each fed by its own rectifier transformer outside; a purifier, water treatment and a cooling tower. The hydrogen goes on to the shaft furnace, the oxygen to a vent or to buyers. For a world with air at about 1 g."},
        "stands_for": "module.hydrogen-plant",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": bays, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 15 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor, open along its ridge (a vent 1.5 m wide) so hydrogen cannot gather; framed openings in the south wall for the DC busbars, in the east for the gas headers and a door, in the north for the cooling water.",
                  "openings": [list(o) for o in openings], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.stack-crane-20t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": H_RATE, "power": r(power), "heat": r(heat), "water": r(water), "mass": r(total), "against_module": against},
        "staff": hmod.get("staff"),
        "basis": [{"of": ["bays", "links", "frame.box", "frame.openings"], "tier": "invented", "review": True, "note": "Laid out from memory of alkaline electrolysis plants."},
                  {"of": ["totals"], "tier": "derived", "note": "Summed from the units; the cells' power and heat from Faraday's law and the cell voltage."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": "Counted from the lattice; sections chosen by the loads in frame.sized."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes from the shapes in the notes, concrete at 2,400 kg/m3."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.hydrogen-plant-air: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to the tower, {water:.1f} kg/s water, {site_area / 1e4:.2f} ha, frame {frame_mass / 1000:.0f} t ({sec})")
    return layout, bays


# ---------------------------------------------------------------- the pellet plant's figures
pmod = yaml.safe_load(open(S + "modules/pellet-plant.yaml"))
PREC = pmod["recipes"][0]
P_RATE, P_P = PREC["rate"], PREC["power"]                 # kg/s of fired pellets, W (sourced: 45 kWh and 0.5 GJ of heat a tonne)
P_FIRE = 0.5e9 * P_RATE / 1000                            # W of firing heat (the module's source), made here by electricity
P_ELEC = P_P - P_FIRE                                     # W of drives, fans and the rest
P_ORE = PREC["inputs"][0]["quantity"] * P_RATE
MOIST = 0.09                                              # share of water in the green balls, from memory
P_WATER = P_ORE * MOIST / (1 - MOIST)                     # kg/s of water dried off the green balls
GRATE_W, GRATE_L = 4.0, 28.0                              # m: the induration grate, chosen for about 27 t a m2 a day (straight grates run 20 to 30, from memory)


def pellet_units():
    out = {}
    D, Lm = 4.4, 7.0
    shell = cyl_shell(D, Lm, 0.045, bottom=2.0) * RHO["steel"]
    liners = math.pi * (D - 0.15) * Lm * 0.075 * RHO["steel"]
    balls = math.pi * ((D - 0.3) / 2) ** 2 * Lm * 0.35 * 4650.0
    P_mill = 18.0 * 3600 * P_ORE                          # W at 18 kWh a tonne (from memory, for pellet feed from fines)
    out["ball-mill-4m4"] = dict(
        name=f"Ball mill, {D:g} m", size_class=3, fits="bay-s3", dims=(18.0, 9.0, 7.0),
        desc=(f"Grinds the ore wet, to about 80% finer than 45 um, so it can be balled: a steel drum {D:g} m across and {Lm:g} m long on trunnion bearings, its axis along x 3 m up, a third full of steel balls, turned by a girth gear and a {P_mill / 1e6:.1f} MW motor at its +x end. "
              "Ore and water in by a chute at its -x trunnion (local -8, 0, 4); the slurry out of its +x trunnion into a sump and pumped on (local 8, 3, 0.5)."),
        built_of=[part("Shell and heads", A36, shell, f"{D:g} m across, {Lm:g} m, 45 mm plate"),
                  part("Liners", A36, liners, "75 mm wear liners of manganese steel, which the registry has not got: structural steel stands for it"),
                  part("Trunnions, bearings and girth gear", "material.high-strength-steel-4340", 40000.0, "from memory"),
                  part("Motor and gearbox", "good.electric-motors", 6.0 * P_mill / 1e3, "6 t a MW, from memory"),
                  part("Slurry sump and pumps", "good.pumps", 8000.0, "from memory")],
        holds=balls, needs={"power": P_mill, "water": P_WATER},
        basis=[{"of": ["physical", "built_of", "holds", "needs.power"], "tier": "derived", "review": True, "note": f"Drum from memory for a mill drawing about {P_mill / 1e6:.1f} MW; masses from its plate; the ball charge a third of its volume at 4,650 kg/m3 (from memory)."},
               {"of": ["needs.water"], "tier": "derived", "note": f"The water the plant loses: the green balls hold {MOIST:.0%}, dried off on the grate; the rest of the grinding water comes back from the filters."}])
    A_f = P_ORE * 3600 / 1000 / 1.0
    out["disc-filter-2"] = dict(
        name="Disc filters, 2", size_class=2, fits="bay-s2", dims=(12.0, 8.0, 6.0),
        desc=f"Two vacuum disc filters side by side ({A_f / 2:.0f} m2 each) take the water out of the slurry down to {MOIST:.0%}; the cake drops onto the layout's belt below their discharge (local 0, -4, 1); the vacuum pumps on their -x end, the filtrate back to the mill.",
        built_of=[part("Filters", A36, 30000.0 * 2, "from memory"), part("Vacuum pumps", "good.pumps", 12000.0, "from memory")],
        needs={"power": 600e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": f"{A_f:.0f} m2 at a tonne a m2 an hour, masses and power from memory."}])
    out["balling-disc-7m5"] = dict(
        name="Balling disc, 7.5 m", size_class=2, fits="bay-s2", dims=(9.0, 9.0, 7.0),
        desc="A shallow pan 7.5 m across, tilted 45 degrees and turning, rolls the moist ore into green balls 9 to 16 mm across; they spill over its rim onto a roller screen that sends the small ones back. Cake in from above (local 0, 2, 7), green balls out at (0, -4.5, 1).",
        built_of=[part("Pan, frame and screen", A36, 40000.0, "from memory"), part("Drive", "good.electric-motors", 4000.0, "from memory")],
        needs={"power": 90e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory; about 45 t of green balls an hour each, with what is sent back."}])
    pallets = math.ceil((2 * GRATE_L + 10.0) / 1.5)
    hood_a = (2 * 4.0 + 6.0) * GRATE_L
    torches = math.ceil(P_FIRE / 1.5e6)
    fans = 20.0 * 3600 * P_RATE                           # W at 20 kWh a tonne (from memory)
    out["induration-grate-125th"] = dict(
        name=f"Induration grate, {P_RATE * 3.6:.0f} t an hour", size_class=4, fits="bay-s4", dims=(44.0, 12.0, 18.0),
        desc=(f"A straight grate {GRATE_W:g} m wide with {GRATE_L:g} m under its hood: a chain of {pallets} pallet cars carries a bed of green balls on a layer of fired ones along +x through drying, preheating, firing at about 1,300 deg C, and cooling; "
              f"the cooling zone's hot air goes back over the bed to the firing zone, so it needs only {P_FIRE / P_RATE / 1e3:.0f} kJ of new heat a kilogram, here {P_FIRE / 1e6:.1f} MW from {torches} electric plasma torches in the hood (there is no fuel to burn). "
              f"Wind boxes under the grate, 5 m up on the machine's frame, draw the gas down through the bed to four process fans. Green balls in on a roller feeder at local (-20, 0, 9); fired pellets off its +x end at (21, 0, 2); waste gas out to the layout's duct at (0, 6, 3)."),
        built_of=[part("Pallet cars", "material.stainless-steel-304", pallets * 5000.0, f"{pallets} cars {GRATE_W:g} x 1.5 m of heat-resisting cast steel, 5 t each (from memory); stainless steel stands for the alloy"),
                  part("Hood lining", REF, hood_a * 0.3 * RHO["refractory"], f"{hood_a:.0f} m2 of hood, 300 mm"),
                  part("Hood casing", A36, hood_a * 0.010 * RHO["steel"], "10 mm"),
                  part("Wind boxes", A36, GRATE_L * GRATE_W * 4.0 * 0.5 * 0.010 * RHO["steel"] * 4, "under the grate, 10 mm plate"),
                  part("Machine frame, sprockets and drive", A36, 200000.0, "from memory"),
                  part("Plasma torches", "good.electric-motors", torches * 2000.0, f"{torches} torches of 1.5 MW, 2 t each with their supplies, from memory"),
                  part("Process fans", "good.electric-motors", 4 * 10000.0, "four fans, from memory")],
        needs={"power": P_FIRE / 0.9 + fans, "gravity": 1.0},
        recipes=pmod["recipes"], life=pmod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.pellet-plant), carried here: the module stays the game's until it reads layouts."},
               {"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"The grate from the rate at about {P_RATE * 86.4 / (GRATE_W * GRATE_L):.0f} t a m2 a day; the hood, pallets and frame from memory."},
               {"of": ["needs.power"], "tier": "derived", "note": f"The firing heat the module's source gives, 0.5 GJ a tonne, made by torches at 90%; the fans {fans / 1e6:.1f} MW at 20 kWh a tonne (from memory)."},
               {"of": ["needs.gravity"], "tier": "invented", "review": True, "note": "The bed lies on the grate by its weight against the gas drawn down through it; not checked below 1 m/s2."}])
    out["mill-crane-30t"] = bridge_crane_unit(30000.0, 22.8, 9000.0, kind="Maintenance crane", job="it lifts the mill's liners and the grate's pallet cars and fans for their change", gear="a main hoist")
    tp = transformer_unit(P_P / 0.9 / PF * 1.1, "the pellet plant: its torches, mill, fans and drives")
    tp["name"] = tp["name"].replace("Furnace", "Plant")
    out[f"plant-transformer-{P_P / 0.9 / PF * 1.1 / 1e6:.0f}mva"] = tp
    out["baghouse-450k"] = baghouse_unit(450000, (24.0, 12.0, 14.0))
    return out


def pellet_plant_air(us, recs):
    """The pellet plant on a world with air at 1 g: grinding, filtering, balling and the induration grate in one crane hall."""
    cell, box = 3.0, (-60.0, -15.0, 0.0, 69.0, 15.0, 27.0)
    openings = [(-60.0, -3.0, 6.0, -57.0, 3.0, 12.0),     # the ore belt in, west
                (66.0, -3.0, 0.0, 69.0, 3.0, 6.0),        # the pellet belt out, east
                (36.0, 12.0, 0.0, 42.0, 15.0, 6.0),       # the waste gas duct out, north
                (-3.0, -15.0, 0.0, 3.0, -12.0, 6.0)]      # a door and the cables, south
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 8, None, openings)
    x0, y0, z0, x1, y1, z1 = box
    span_in = y1 - y0 - 2 * cell
    crane = recs["mill-crane-30t"]
    cap, trolley = 30000.0, 9000.0
    RUNWAY = 18.0
    roof_q = (0.002 * RHO["aluminium"] + 20.0) * G + 1000.0
    chord = roof_q * cell * span_in ** 2 / 8 / cell
    wheel = ((cap + trolley) * (1 - 1.5 / (span_in - 1.2)) + crane["physical"]["mass"] / 2) * G / 2
    vert = wheel + roof_q * cell * span_in / 2
    sec = {"roof": frames.lightest_section(chord), "wall": frames.lightest_section(vert)}
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec[g], cell, diag)
            members.append({"item": key, "count": n, "where": g})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec["wall"], cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    rw_a = 0.4 * 0.4 - 0.35 * 0.35
    rw_sigma = wheel * cell / 4 / ((0.4 ** 4 - 0.35 ** 4) / 12 / 0.2)
    assert 1.5 * rw_sigma <= 470e6, rw_sigma
    x_in = x1 - x0 - 2 * cell
    members.append({"item": "stock.st4340-box-400x400x25", "count": 8, "where": f"crane runways, four a side, {x_in / 4:.1f} m lengths, along x on the inside of the long walls: their rail plane {RUNWAY:g} m up at y -11.4 and +11.4"})
    fmass += 2 * x_in * rw_a * RHO["steel"]
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in openings)
    clad = clad_area * 0.002 * RHO["aluminium"]
    sec_stock = lambda g: yaml.safe_load(open(S + "mill-stock/" + strut(sec[g], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    rcap, wcap = frames.strut_capacity(sec_stock("roof")), frames.strut_capacity(sec_stock("wall"))
    sized = (f"At 1 g: roof {roof_q:.0f} Pa (2 mm aluminium, the frame about 20 kg/m2, 1 kPa of snow and wind, the last invented) over a {span_in:.0f} m span in 3 m deep strips puts {chord / 1e3:.0f} kN in a chord: "
             f"{sec['roof']} takes {rcap / 1e3:.0f} kN. A wheel of the 30 t crane loaded at the runway's end puts {wheel / 1e3:.0f} kN on a wall node, with the roof's share {vert / 1e3:.0f} kN: "
             f"{sec['wall']} takes {wcap / 1e3:.0f} kN. Each the kit's lightest section with half again to spare. The runway bends to {rw_sigma / 1e6:.0f} MPa between nodes, against 470 MPa with half again to spare. Wind and footings not yet checked.")
    tkey = next(k for k in us if k.startswith("plant-transformer-"))
    bays = [
        {"id": "M", "mount": "plant-mount.bay-s3", "unit": "unit.ball-mill-4m4", "at": [-45.0, 0.0, 0.0], "note": "in the hall, west end"},
        {"id": "F", "mount": "plant-mount.bay-s2", "unit": "unit.disc-filter-2", "at": [-24.0, 0.0, 6.0], "note": "on a deck 6 m up, so the cake falls to the belt"},
        {"id": "B1", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [-6.0, -6.0, 0.0], "note": "two rows of two"},
        {"id": "B2", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [6.0, -6.0, 0.0], "note": ""},
        {"id": "B3", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [-6.0, 6.0, 0.0], "note": ""},
        {"id": "B4", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [6.0, 6.0, 0.0], "note": ""},
        {"id": "G", "mount": "plant-mount.bay-s4", "unit": "unit.induration-grate-125th", "at": [41.0, 0.0, 0.0], "note": "in the hall's east end: its 48 m mount square is wider than the hall (the floor it stands on runs wall to wall)"},
        {"id": "D", "mount": "plant-mount.bay-s3", "unit": "unit.baghouse-450k", "at": [39.0, 36.0, 0.0], "note": "outside, north, its stack at its east end"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": f"unit.{tkey}", "at": [0.0, -24.0, 0.0], "note": "outside, south, in a fire-walled vault"},
    ]
    q_gas = 450000 / 3600
    d_gas = math.sqrt(4 * q_gas / (math.pi * 20.0))
    links = [
        link("conveyor", "ore belt", (-100.0, 0.0, 2.0), (-53.0, 0.0, 4.0), A36, 150.0, 100000.0, f"a 1 m belt in a gallery: {P_ORE:.1f} kg/s of ore fines from the ore yard, beyond the layout, through the west wall into the mill's chute", via=[(-62.0, 0.0, 9.0)]),
        link("pipe", "slurry", (-37.0, 3.0, 0.5), (-24.0, 0.0, 9.0), A36, math.pi * 0.25 * 0.010 * RHO["steel"], None, "250 mm rubber-lined, 10 mm wall, pumped from the mill's sump up to the filters' troughs on their deck"),
        link("conveyor", "cake belt", (-24.0, -4.0, 7.0), (6.0, 8.0, 7.5), A36, 100.0, 40000.0, "a 0.8 m belt from under the filters, shuttling over the four discs' feed points", via=[(-12.0, -4.0, 8.0), (-12.0, 0.0, 8.5), (6.0, 0.0, 8.5)]),
        link("conveyor", "green ball belt", (-6.0, -10.5, 1.0), (21.0, 0.0, 9.0), A36, 100.0, 40000.0, "a 0.8 m belt collecting the four discs' green balls (the north pair's by a short cross belt from y 1.5), rising to the grate's roller feeder", via=[(12.0, -10.5, 1.0)]),
        link("conveyor", "pellet belt", (62.0, 0.0, 2.0), (110.0, 0.0, 2.0), A36, 150.0, 100000.0, f"a 1 m belt: {P_RATE:.1f} kg/s of fired pellets off the grate's end, out by the east wall to the shaft furnace's pellet belt, beyond the layout"),
        link("duct", "waste gas", (41.0, 6.0, 3.0), (39.0, 30.0, 12.0), A36, math.pi * d_gas * 0.006 * RHO["steel"], None, f"{d_gas:.1f} m across, 6 mm plate, insulated: from the process fans' discharge through the north wall to the baghouse's inlet", via=[(39.0, 6.0, 3.0), (39.0, 20.0, 3.0), (39.0, 20.0, 12.0)]),
        link("cable", "plant supply", (0.0, -24.0 + us[tkey]["dims"][1] / 2, 4.0), (0.0, -10.0, 3.0), CU, 3 * P_P / 0.9 / PF / (math.sqrt(3) * 6600) / 4e6 * RHO["copper"], None, "three phases at 6.6 kV in a cable tray, copper at 4 A/mm2, through the south door to the hall's switchroom"),
    ]
    ground = [
        {"what": "Hall floor", "item": CONC, "volume": r(x_in * span_in * 0.4), "note": f"0.4 m, {x_in:.0f} x {span_in:.0f} m"},
        {"what": "Mill foundation", "item": CONC, "volume": r(18 * 9 * 3.0), "note": "two piers under the trunnions on a 3 m block, sized to the drum's dynamic load (not yet checked)"},
        {"what": "Filter deck", "item": CONC, "volume": r(12 * 8 * 0.5 + 6 * 1.0 * 1.0 * 6.0), "note": "a 0.5 m deck 6 m up on six columns"},
        {"what": "Grate foundations", "item": CONC, "volume": r(44 * 12 * 1.0), "note": "under the machine's frame"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Outside slabs", "item": CONC, "volume": r(24 * 12 * 0.6 + 12 * 8 * 0.5 + 28 * 6 * 0.3), "note": "the baghouse's slab and the transformer's vault"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [crane]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    water = sum(u.get("needs", {}).get("water", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = pmod["physical"]
    xs = [x0, x1] + [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [y0, y1] + [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    against = (f"module.pellet-plant: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {P_P / 1e6:.0f} MW (45 kWh and 0.5 GJ of heat a tonne, the heat counted as power). "
               f"This layout: the same {P_RATE * 3.6:.0f} t an hour; {power / 1e6:.1f} MW, of which {P_FIRE / 0.9 / 1e6:.1f} the grate's plasma torches; no cooling water (the pellets cool on the grate in the air it draws, the waste gas leaves by the baghouse's stack); "
               f"{water:.1f} kg/s of water dried off the green balls; {(mass_units + mass_links + frame_mass) / 1000:,.0f} t of plant and frame, {mass_ground / 1000:,.0f} t of concrete. "
               f"One shaft furnace at its full rate takes {PELLETS:.1f} kg/s of pellets, about {PELLETS / P_RATE:.1f} of these.")
    layout = {
        "identity": {"key": "layout.pellet-plant-air", "name": "Pellet plant, world with air",
                     "description": "Fine ore made into fired pellets for the shaft furnace: a ball mill grinds it wet, disc filters dry it, four balling discs roll it into green balls, and a straight induration grate dries, fires and cools them, its heat from electric plasma torches; a baghouse cleans the waste gas. One crane hall. For a world with air at about 1 g."},
        "stands_for": "module.pellet-plant",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": bays, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 24 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor; framed openings for the ore belt (west), the pellet belt (east), the waste gas duct (north) and a door (south).",
                  "openings": [list(o) for o in openings], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.mill-crane-30t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": P_RATE, "power": r(power), "heat": 0.0, "water": r(water), "mass": r(total), "against_module": against},
        "staff": pmod.get("staff"),
        "basis": [{"of": ["bays", "links", "frame.box", "frame.openings"], "tier": "invented", "review": True, "note": "Laid out from memory of straight-grate pellet plants."},
                  {"of": ["totals"], "tier": "derived", "note": "Summed from the units."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": "Counted from the lattice; sections chosen by the loads in frame.sized."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes from the shapes in the notes, concrete at 2,400 kg/m3."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.pellet-plant-air: {total / 1000:,.0f} t, {power / 1e6:.1f} MW (torches {P_FIRE / 0.9 / 1e6:.1f}), {water:.1f} kg/s water, frame {frame_mass / 1000:.0f} t ({sec})")
    return layout, bays


# ---------------------------------------------------------------- the yards at the line's two ends
omod = yaml.safe_load(open(S + "modules/ore-yard.yaml"))
imod = yaml.safe_load(open(S + "modules/ingot-yard.yaml"))
ORE_BULK = yaml.safe_load(open(S + "goods/iron-ore.yaml"))["physical"]["bulk_density"]
REPOSE = 37.0                      # degrees: iron ore's angle of repose, from memory
CRATE = yaml.safe_load(open(S + "packages/crate-heavy.yaml"))


CUT_RATE = 300e-4 / 60                                 # m2/s a large band saw cuts in carbon steel (300 cm2 a minute, from memory)


def yard_units():
    out = {}
    out["stacker-reclaimer-1500th"] = dict(
        name="Bucket wheel stacker-reclaimer, 1,500 t an hour", size_class=3, dims=(40.0, 12.0, 20.0),
        desc="Runs on its rail between two stockpile beds: stacking, its boom (32 m, slewing either side) drops ore from the yard belt's tripper onto a bed; reclaiming, the bucket wheel at the boom's end cuts into the pile's face and sends the ore back down the boom onto the yard belt, which then runs the other way. Stacks 1,500 t an hour (a ship's load landed at the port in a day), reclaims 500. Travels along its x on 10 m gauge, its origin on the running plane at the slewing centre.",
        built_of=[part("Boom, counterweight boom and slewing frame", A36, 260000.0, "from memory"),
                  part("Portal and bogies", A36, 120000.0, "from memory"),
                  part("Bucket wheel, drives and belts", "good.electric-motors", 70000.0, "from memory")],
        needs={"power": 900e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "Masses, rates and power from memory of stacker-reclaimers of this size."}])
    out["truck-dump-hopper"] = dict(
        name="Truck dump hopper", size_class=2, fits="bay-s2", dims=(12.0, 10.0, 8.0),
        desc="A steel hopper over a pit that a tipping truck backs onto and empties into, 100 t, with an apron feeder under it onto the yard belt; its grizzly bars keep out the lumps. The trucks come from the port, its feeder's outlet at local (0, 0, -4) in the pit.",
        built_of=[part("Hopper and grizzly", A36, 45000.0, "from memory"), part("Apron feeder", "good.electric-motors", 25000.0, "from memory")],
        holds=100000.0, needs={"power": 150e3},
        basis=[{"of": ["physical", "built_of", "holds", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["ingot-crane-10t"] = bridge_crane_unit(10000.0, 22.8, 3000.0, kind="Ingot crane", job="it lifts hot ingots off the ingot car with its tongs by their lifting eyes, one at a time, onto the cooling racks; later lays each cooled ingot on a crop saw's table and the cropped ingot onto the crating line's roller table", gear="ingot tongs")
    out["sampling-lab"] = dict(
        name="Sampling laboratory", size_class=1, fits="bay-s1", dims=(6.0, 6.0, 3.5),
        desc="A cabin with a saw that cuts a sample from one ingot of each heat, a grinder, and a spark emission spectrometer that reads the steel's composition in minutes, so the heat's grade can be certified on its crates' labels.",
        built_of=[part("Cabin", A36, 8000.0, "from memory"), part("Saw, grinder and spectrometer", "good.electric-motors", 3000.0, "from memory")],
        needs={"power": 60e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    # The crop saws: each cooled ingot's hot top is sawn off at its big end before crating.
    cut_area = ING_A ** 2
    t_cut = cut_area / CUT_RATE + 120.0                    # s a cut, with the ingot's handling
    ing_h = C_RATE * SOUND / ING_M                         # ingots a second
    n_saws = math.ceil(ing_h * t_cut)
    assert n_saws <= 9, "the crop saws' unit is laid out as 3 rows of 3"
    out["crop-saws"] = dict(
        name=f"Crop saws, {n_saws}", size_class=2, fits="bay-s2", dims=(12.0, 10.0, 3.0),
        desc=f"{n_saws} horizontal band saws in 3 rows of 3 along x, each with its roller table: the ingot crane sets a cooled ingot on a table lying along x, its big end at the blade, and the blade cuts the hot top off ({HOT_TOP:,.0f} kg, with its lifting eye) through the {ING_A:g} m square section in about {t_cut / 60:.0f} minutes with the handling; the head drops into a scrap bin under the table, which goes back to the arc furnace as scrap, and the crane lifts the sound ingot onto the crating line. {ing_h * 3600:.0f} ingots an hour from the casting bay need {n_saws}.",
        built_of=[part("Band saws", "good.electric-motors", n_saws * 9000.0, "about 9 t a saw for a 0.5 m section, from memory"),
                  part("Roller tables and scrap bins", A36, n_saws * 1500.0, "from memory")],
        needs={"power": n_saws * 15e3},
        basis=[{"of": ["built_of", "needs", "physical"], "tier": "invented", "review": True, "note": f"The count from the casting bay's rate and a cut of {CUT_RATE * 1e4 * 60:.0f} cm2 a minute (a large band saw in carbon steel, from memory); saw mass and power from memory."}])
    out["crating-line"] = dict(
        name="Crating line", size_class=2, fits="bay-s2", dims=(12.0, 4.0, 3.0),
        desc="A roller table along x: a cooled ingot set on at its -x end is weighed, laid in a heavy crate on its skids, the lid nailed on and the label printed with the heat, grade and mass; the crate leaves at the +x end for a forklift.",
        built_of=[part("Roller table, scale and nailer", A36, 12000.0, "from memory")],
        needs={"power": 40e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["forklift-2t5"] = dict(
        name="Forklift, 2.5 t", size_class=1, dims=(3.6, 1.3, 2.2),
        desc="A battery-electric counterbalance forklift that moves heavy crates from the crating line to the stacks and from the stacks onto trucks; it lifts a loaded crate (1.6 t) onto another, two high.",
        built_of=[part("Chassis and counterweight", A36, 3200.0, "from memory"), part("Mast, motors and battery", "good.electric-motors", 1000.0, "from memory")],
        needs={"power": 8e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory; power the average charge."}])
    return out


def ore_yard_air(us, recs):
    """The ore yard on a world with air: two stockpile beds either side of a stacker-reclaimer's rail."""
    hold = omod["capacity"]["holds"]
    h = 15.0
    base = 2 * h / math.tan(math.radians(REPOSE))
    bed_L = hold / 2 / ORE_BULK / (base * h / 2)
    x0 = -bed_L / 2
    bays = [{"id": "H", "mount": "plant-mount.bay-s2", "unit": "unit.truck-dump-hopper", "at": [r(x0 - 24.0), 0.0, 0.0], "note": "at the rail's west end, its feeder in a pit onto the yard belt"}]
    gauge_y = 0.0
    beds_y = (base / 2 + 8.0)
    links = [
        link("rail", "machine rail", (r(x0 - 10), 0.0, 0.0), (r(-x0 + 10), 0.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails at 10 m gauge on a concrete bed"),
        link("conveyor", "yard belt", (r(x0 - 24.0), 0.0, -4.0), (r(-x0 + 20), 0.0, 1.0), A36, 200.0, 400000.0, "a 1.4 m reversible belt beside the rail, with the machine's tripper on it: ore from the hopper to the machine stacking, from the machine to the out belt reclaiming", via=[(r(x0 - 14.0), 0.0, 1.0)]),
        link("conveyor", "out belt", (r(-x0 + 20), 0.0, 1.0), (r(-x0 + 80), 0.0, 1.0), A36, 150.0, 100000.0, "a 1 m belt to the pellet plant's ore belt, beyond the layout"),
        link("pipe", "dust sprays", (r(x0), beds_y + base / 2 + 2, 1.0), (r(-x0), -(beds_y + base / 2 + 2), 1.0), A36, math.pi * 0.1 * 0.004 * RHO["steel"], None, "100 mm, along both beds' outer edges, spraying water to keep the dust down", via=[(r(-x0), beds_y + base / 2 + 2, 1.0), (r(-x0) + 2, 0.0, 1.0), (r(x0) - 2, 0.0, 1.0), (r(x0), -(beds_y + base / 2 + 2), 1.0)]),
    ]
    mobile = [{"unit": "unit.stacker-reclaimer-1500th", "rides": "machine rail", "at": [0.0, 0.0, 0.0]}]
    ground = [
        {"what": "Rail bed", "item": CONC, "volume": r((bed_L + 20) * 2 * 1.0 * 0.8), "note": "two strips 1 m wide and 0.8 m deep under the rails"},
        {"what": "Hopper pit", "item": CONC, "volume": r(12 * 10 * 0.6 + 2 * (12 + 10) * 6 * 0.4), "note": "6 m deep under the hopper, 0.4 m walls"},
        {"what": "Bed pads", "item": CONC, "volume": r(2 * bed_L * base * 0.2), "note": f"two pads {bed_L:.0f} x {base:.0f} m, 0.2 m, at y -{beds_y:.0f} and {beds_y:.0f}, sloped to drains"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    L = r(-x0 + 80 - (x0 - 30))
    W = r(2 * (beds_y + base / 2 + 3))
    water = 0.2
    mp = omod["physical"]
    against = (f"module.ore-yard: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, holding {hold / 1e6:.0f} kt. "
               f"This layout: two beds {bed_L:.0f} m long, {base:.0f} m across and {h:g} m high (iron ore at {ORE_BULK} kg/m3 lying at {REPOSE:g} degrees) hold the same; {power / 1e6:.2f} MW with every drive running, against the module's {omod['needs']['power'] / 1e6:.1f}; "
               f"{(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of concrete.")
    layout = {
        "identity": {"key": "layout.ore-yard-air", "name": "Ore yard, world with air",
                     "description": "Two stockpile beds of ore either side of a bucket wheel stacker-reclaimer's rail. Trucks from the port tip into a hopper; the yard belt takes the ore to the machine, which stacks it, and later reclaims it onto the belt for the pellet plant. Water sprays keep the dust down. Open ground, no hall. For a world with air at about 1 g."},
        "stands_for": "module.ore-yard",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": L, "width": W, "height": 20.0, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": 0.0, "power": r(power), "heat": 0.0, "water": water, "mass": r(total), "against_module": against},
        "staff": omod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out from memory of ore yards."},
                  {"of": ["ground", "physical"], "tier": "derived", "review": True, "note": f"The beds from the module's {hold / 1e6:.0f} kt, the ore's bulk density and its angle of repose (from memory); concrete at 2,400 kg/m3."},
                  {"of": ["totals.water"], "tier": "invented", "review": True, "note": "Dust sprays, from memory."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.ore-yard-air: {total / 1000:,.0f} t, beds {bed_L:.0f} x {base:.0f} x {h:g} m, {L:.0f} x {W:.0f} m")
    return layout, bays


def ingot_yard_air(us, recs):
    """The ingot yard on a world with air: a cooling hall with an ingot crane, the sampling lab, a crating line, and the
    crates block-stacked two high on a paved yard, worked by forklifts."""
    cell, box = 3.0, (-30.0, -15.0, 0.0, 30.0, 15.0, 15.0)
    doors = [(-30.0, 3.0, 0.0, -27.0, 9.0, 6.0),           # the ingot rail in, west
             (27.0, -6.0, 0.0, 30.0, 0.0, 6.0)]            # crates out to the yard, east
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 4, None, doors)
    x0, y0, z0, x1, y1, z1 = box
    span_in = y1 - y0 - 2 * cell
    crane = recs["ingot-crane-10t"]
    cap, trolley = 10000.0, 3000.0
    roof_q = (0.002 * RHO["aluminium"] + 20.0) * G + 1000.0
    chord = roof_q * cell * span_in ** 2 / 8 / cell
    wheel = ((cap + trolley) * (1 - 1.5 / (span_in - 1.2)) + crane["physical"]["mass"] / 2) * G / 2
    vert = wheel + roof_q * cell * span_in / 2
    sec = {"roof": frames.lightest_section(chord), "wall": frames.lightest_section(vert)}
    members, fmass = [], 0.0
    for g in ("wall", "roof"):
        for diag, n in ((False, groups[g][0]), (True, groups[g][1])):
            key, st = strut(sec[g], cell, diag)
            members.append({"item": key, "count": n, "where": g})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec["wall"], cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    rw_a = 0.4 * 0.4 - 0.35 * 0.35
    x_in = x1 - x0 - 2 * cell
    members.append({"item": "stock.st4340-box-400x400x25", "count": 2, "where": f"crane runways, one a side, {x_in:.0f} m, their rail plane 9 m up at y -11.4 and +11.4"})
    fmass += 2 * x_in * rw_a * RHO["steel"]
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in doors)
    clad = clad_area * 0.002 * RHO["aluminium"]
    sec_stock = lambda g: yaml.safe_load(open(S + "mill-stock/" + strut(sec[g], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    sized = (f"At 1 g: roof {roof_q:.0f} Pa over a {span_in:.0f} m span puts {chord / 1e3:.0f} kN in a chord: {sec['roof']} takes {frames.strut_capacity(sec_stock('roof')) / 1e3:.0f} kN. "
             f"A wheel of the 10 t crane loaded at the runway's end, with the roof's share, puts {vert / 1e3:.0f} kN on a wall node: {sec['wall']} takes {frames.strut_capacity(sec_stock('wall')) / 1e3:.0f} kN. Wind and footings not yet checked.")
    # The ingots: as the casting bay makes them, cooled a day on racks.
    ing_rate = C_RATE * SOUND / ING_M                      # ingots a second
    cool_h = 24.0
    on_racks = math.ceil(ing_rate * cool_h * 3600)
    rack_heat = C_RATE * SOUND * 600.0 * (700.0 - 40.0)    # W the ingots give the hall's air, cooling from about 700 deg C
    # The stacks: block-stacked two high in lanes ten crates deep off 4.5 m forklift aisles.
    crates = math.ceil(imod["capacity"]["holds"] / (ING_M + CRATE["physical"]["mass"]))
    lanes = math.ceil(crates / CRATE["stack"] / 10)
    lane_w = CRATE["physical"]["width"] + 0.1
    lane_d = 10 * CRATE["physical"]["length"]
    rows = 6                                               # aisles, so the yard comes out near square
    per_row = math.ceil(lanes / (2 * rows))                # lanes face both sides of each aisle
    yard_L = per_row * lane_w
    yard_W = rows * (2 * lane_d + 4.5)
    yx0 = x1 + 10.0
    aisle_y = [r(-yard_W / 2 + lane_d + 2.25 + i * (2 * lane_d + 4.5)) for i in range(rows)]
    aisle_path = []
    for i, y in enumerate(aisle_y):
        ends = (yx0, yx0 + yard_L) if i % 2 == 0 else (yx0 + yard_L, yx0)
        aisle_path += [(r(ends[0]), y, 0.0), (r(ends[1]), y, 0.0)]
    bays = [
        {"id": "S", "mount": "plant-mount.bay-s1", "unit": "unit.sampling-lab", "at": [-21.0, -9.0, 0.0], "note": "in the hall, by the ingot rail"},
        {"id": "K", "mount": "plant-mount.bay-s2", "unit": "unit.crating-line", "at": [18.0, -3.0, 0.0], "note": "in the hall, running out of its east door"},
        {"id": "C", "mount": "plant-mount.bay-s2", "unit": "unit.crop-saws", "at": [3.0, -6.0, 0.0], "note": "in the hall, west of the crating line, under the crane"},
    ]
    links = [
        link("rail", "ingot rail", (-36.0, 6.0, 0.0), (-6.0, 6.0, 0.0), A36, 2 * 50.0, None, "two 50 kg/m rails at 2 m gauge, set in the floor: in by the west door from the casting bay; the car stops at x -15 under the crane"),
        link("road", "yard aisles", aisle_path[0], aisle_path[-1], CONC, 4.5 * 0.25 * RHO["concrete"], None, f"{rows} forklift aisles 4.5 m wide, {yard_L:.0f} m long, joined end to end, paved 0.25 m (the rest of the yard is the ground's)", via=aisle_path[1:-1]),
        link("road", "truck lane", (x1, -3.0, 0.0), (yx0 + yard_L + 20.0, -3.0, 0.0), CONC, 7.0 * 0.3 * RHO["concrete"], None, "7 m wide, paved 0.3 m: trucks load crates at the yard's east end for the port, beyond the layout"),
    ]
    mobile = [{"unit": "unit.forklift-2t5", "rides": "yard aisles", "at": [r(yx0 + 10.0 + 20.0 * i), aisle_y[i % rows], 0.0]} for i in range(6)]
    ground = [
        {"what": "Cooling floor and racks' footings", "item": CONC, "volume": r(x_in * span_in * 0.4), "note": f"0.4 m under the hall: steel racks (in the hall's frame) for the {on_racks} ingots of a day's cooling"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Stack yard", "item": CONC, "volume": r((yard_L * yard_W - rows * yard_L * 4.5) * 0.25), "note": f"{yard_L:.0f} x {yard_W:.0f} m paved 0.25 m east of the hall: {crates:,} crates two high in {lanes} lanes ten deep, either side of its aisles; outdoors, the crates under tarpaulins"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * RHO["concrete"])
    racks = on_racks * 60.0                                # kg of rack steel an ingot place, from memory
    members.append({"item": "stock.st4340-box-300x300x20", "count": math.ceil(racks / (box_tube(0.3, 0.3, 0.02, 6.0) * RHO["steel"])), "where": f"cooling racks in 6 m lengths for {on_racks} ingots, from memory"})
    fmass += racks
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [crane]
    power = sum(u.get("needs", {}).get("power", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = imod["physical"]
    L = r(yx0 + yard_L + 20.0 - (x0 - 6.0))
    W = r(max(y1 - y0, yard_W))
    against = (f"module.ingot-yard: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, holding {imod['capacity']['holds'] / 1e6:.0f} kt. "
               f"This layout: {crates:,} heavy crates of one ingot each, two high as the crate allows, need a yard {yard_L:.0f} x {yard_W:.0f} m ({yard_L * yard_W / 1e4:.1f} ha), more than the module's box; "
               f"the cooling hall gives {rack_heat / 1e6:.1f} MW of the ingots' heat to the air; {power / 1e6:.2f} MW against the module's {imod['needs']['power'] / 1e6:.1f}; "
               f"{(mass_units + mass_links + frame_mass) / 1000:,.0f} t of plant, frame and paving, {mass_ground / 1000:,.0f} t of concrete.")
    layout = {
        "identity": {"key": "layout.ingot-yard-air", "name": "Ingot yard, world with air",
                     "description": "The ingots come in hot on their car from the casting bay; a crane lays them on racks in a cooling hall for a day; one of each heat is sampled and analysed; band saws crop each ingot's hot top (back to the arc furnace as scrap); a crating line puts each ingot in a heavy crate with its label; forklifts stack the crates two high on a paved yard and load them onto trucks for the port. For a world with air at about 1 g."},
        "stands_for": "module.ingot-yard",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": L, "width": W, "height": z1, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 12 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor; doors 6 m wide for the ingot rail (west) and the crates (east). Its walls' lower cells open at the sides for air (not yet drawn).",
                  "openings": [list(d) for d in doors], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.ingot-crane-10t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": 0.0, "power": r(power), "heat": 0.0, "water": 0.0, "mass": r(total), "against_module": against},
        "staff": imod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile", "frame.box"], "tier": "invented", "review": True, "note": "Laid out from memory; a day's cooling on the racks from memory."},
                  {"of": ["ground", "physical"], "tier": "derived", "review": True, "note": "The yard from the module's holding, the crate's size and its stacking limit (package.crate-heavy), lanes ten deep and 4.5 m aisles (from memory)."},
                  {"of": ["frame.members", "frame.mass", "frame.cladding"], "tier": "derived", "review": True, "note": "Counted from the lattice; sections chosen by the loads in frame.sized; racks from memory."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.ingot-yard-air: {total / 1000:,.0f} t, {crates:,} crates, yard {yard_L:.0f} x {yard_W:.0f} m, {on_racks} ingots cooling, {rack_heat / 1e6:.1f} MW to air")
    return layout, bays


# ---------------------------------------------------------------- airless: a moon's ground
SIGMA = 5.670e-8
MOON_G = (1.6, 2.0)                # m/s2: the site range the airless layouts are sized for (Earth's moon is 1.62)
RAD_EPS, RAD_ALPHA, SUN = 0.9, 0.2, 1361.0   # a white-coated panel's emissivity and absorptance (from memory), the sun at 1 au
T_UNDER = 300.0                    # K: the ground in a raised panel's shade at lunar noon, which its underside sees (invented)
SINTER = "material.regolith-sintered"
RHO_SINTER = 2500.0
DRI_C = 0.02                       # share of carbon in the sponge iron, from memory


def radiator_unit(T, whose):
    """A field of raised horizontal radiator panels filling a class 4 bay, rejecting a loop's heat at T to the sky.
    Sized for the worst case on an airless moon: the sun overhead on the top face, warm shaded ground under."""
    top = RAD_EPS * SIGMA * T ** 4 - RAD_ALPHA * SUN
    under = RAD_EPS * SIGMA * (T ** 4 - T_UNDER ** 4)
    q = top + under
    A = 44.0 * 44.0                                   # m2 of panel, 2 m gaps at the bay's edges for the loop's headers
    panels = A / (6.0 * 12.0)
    face = A * 0.002 * 2 * RHO["aluminium"]
    tubes = A / 0.15 * math.pi * 0.02 * 0.0015 * RHO["aluminium"]
    legs = panels * 4 * box_tube(0.1, 0.1, 0.004, 3.0) * RHO["aluminium"]
    water = A / 0.15 * math.pi / 4 * 0.017 ** 2 * RHO["water"]
    pumps = q * A / (4186 * 10) / 1000 * 300e3 / 0.75
    return dict(
        name=f"Radiator field, {q * A / 1e6:.1f} MW at {T:.0f} K", size_class=4, fits="bay-s4", dims=(48.0, 48.0, 3.5),
        desc=(f"{panels:.0f} aluminium panels 6 x 12 m, raised 3 m on legs and laid flat edge to edge over 44 x 44 m, carrying {whose} loop's water at about {T:.0f} K through tubes bonded between their two face sheets; "
              f"the top face, white, radiates to the sky, the underside to the shaded ground. {q:.0f} W a m2 of panel at lunar noon, the worst case: the sun overhead on the top ({RAD_ALPHA:g} of {SUN:.0f} W/m2 absorbed), the ground under it at {T_UNDER:.0f} K; more at night. "
              "The loop's headers run round the edge, its ports at local (-24, 0, 1). For airless ground only: on a world with air a tower is lighter. Kept clean of dust, which darkens the white."),
        built_of=[part("Panels", "material.aluminium-alloy-6061", face + tubes, f"two 2 mm face sheets and tubes 20 mm across, 1.5 mm wall, at 150 mm"),
                  part("Legs", "material.aluminium-alloy-6061", legs, "four 100 mm box legs, 4 mm, 3 m, a panel"),
                  part("Headers and pumps", "good.pumps", 4000.0 + 20.0 * A / 44.0, "from memory")],
        holds=water, rejects={"heat": q * A, "temperature": T}, needs={"power": pumps, "atmosphere": "vacuum"},
        basis=[{"of": ["rejects", "physical"], "tier": "derived", "review": True, "note": f"Stefan-Boltzmann at emissivity {RAD_EPS}; absorptance, the shaded ground's temperature and the panel's layout invented or from memory."},
               {"of": ["built_of", "holds", "needs"], "tier": "derived", "review": True, "note": "From the panel's sheets and tubes; pumps at 3 bar and 75%."}])


def airless_units(us):
    """The airless furnace shop's own units."""
    out = {}
    v = dict(us["eaf-vessel-80t"])
    v["name"] = "Arc furnace vessel, 80 t, sealed"
    v["desc"] = ("The 80 t arc furnace closed for airless ground: its roof sealed to the shell, glands round the three electrodes, the sponge iron fed through a lock in the fifth hole, and the off-gas taken by a sealed bellows joint from the fourth, so the furnace holds its own gas (the CO its charge gives off, started with argon) at about 1 bar: "
                 "the arc burns as in air, the melt neither boils nor loses its manganese to the vacuum, and the slag foams. It tilts only a few degrees to tap and 15 to deslag, through the bellows. Otherwise as unit.eaf-vessel-80t: " + us["eaf-vessel-80t"]["desc"])
    v["built_of"] = list(us["eaf-vessel-80t"]["built_of"]) + [
        part("Roof seal and electrode glands", A36, 4000.0, "a sand seal round the roof's rim and three water-cooled glands, from memory"),
        part("Feed lock", A36, 3000.0, "two valves and a chamber over the fifth hole, from memory"),
        part("Off-gas bellows", "material.stainless-steel-304", 1500.0, "2 m across, from memory")]
    v["needs"] = dict(us["eaf-vessel-80t"]["needs"], atmosphere="any")
    v["basis"] = us["eaf-vessel-80t"]["basis"] + [{"of": ["identity.description"], "tier": "invented", "review": True, "note": "Sealed as closed ferroalloy furnaces are; not a design that has been built for vacuum."}]
    out["eaf-vessel-80t-sealed"] = v
    co = DRI * DRI_C * 28.0 / 12.0
    out["offgas-filter-sealed"] = dict(
        name="Off-gas filter, sealed", size_class=2, fits="bay-s2", dims=(12.0, 6.0, 10.0),
        desc=(f"Takes the furnace's {co:.1f} kg/s of CO with its dust from the steam-cooled duct at about 1 bar and 250 deg C, through ceramic candle filters in a closed vessel, and lets it out past the valve that holds the furnace's pressure, up a short stack: on an airless world the gas simply leaves (or a later unit can compress it as fuel). "
              "The dust is let out through a lock into sealed bins. Gas in at local (-6, 0, 8)."),
        built_of=[part("Vessel", A36, cyl_shell(4.0, 8.0, 0.012, bottom=2.0) * RHO["steel"], "4 m across, 8 m, 12 mm"),
                  part("Candles", "material.silicon-carbide", 3000.0, "from memory"),
                  part("Valve, stack and dust lock", A36, 6000.0, "from memory")],
        needs={"power": 50e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": f"From memory; the CO from {DRI_C:.0%} carbon in the sponge iron (from memory) burnt to CO by the melt's oxygen."}])
    out["gantry-crane-20t-moon"] = bridge_crane_unit(20000.0, 24.0, 6000.0, kind="Gantry crane", job="on its own legs over the furnace and its transformer: it sets electrodes, lifts the roof and swaps the shell", gear="a main hoist", g=MOON_G[1], h_min=0.6)
    gc = out["gantry-crane-20t-moon"]
    legs = 4 * box_tube(0.8, 0.8, 0.02, 22.0) * RHO["steel"] + 2 * box_tube(0.8, 0.6, 0.02, 10.0) * RHO["steel"]
    gc["built_of"].append(part("Legs and sill beams", A36, legs, "four box legs 0.8 x 0.8 m, 20 mm, 22 m, and two sill beams"))
    gc["dims"] = (10.0, 25.0, 25.0)
    gc["desc"] = gc["desc"].replace("on the hall's runways", "on its own legs, running on two ground rails 24 m apart").replace("its origin on the runway's rail plane", "its origin on the ground rails' plane, its girders 22 m up") + f" Sized for {MOON_G[1]:g} m/s2."
    out["crew-post"] = dict(
        name="Crew post, pressurised", size_class=1, fits="bay-s1", dims=(6.0, 6.0, 5.0),
        desc="The shift's control room on airless ground: an aluminium cylinder 4 m across and 6 m long at 1 bar, an airlock at its -x end, windows onto the furnace and the controls, air and water kept for its watch, under a berm of loose regolith 1 m deep against radiation and the cold.",
        built_of=[part("Shell", "material.aluminium-alloy-6061", cyl_shell(4.0, 6.0, 0.006, bottom=2.0) * RHO["aluminium"], "4 m across, 6 m, 6 mm (1 bar gives 33 MPa)"),
                  part("Airlock, life support and fit-out", "good.electric-motors", 3000.0, "from memory")],
        needs={"power": 20e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["radiator-field-330k"] = radiator_unit(330.0, "the panels' and transformer's")
    out["radiator-field-485k"] = radiator_unit(485.0, "the off-gas duct's evaporative")
    return out


def arc_furnace_airless(us, recs):
    """The arc furnace shop on airless ground at about a moon's gravity: no hall (no weather, and no air to carry heat
    off), a sealed furnace, radiators for the tower, sintered regolith for concrete (no water to make it with)."""
    vessel_at = (-12.0, 6.0, 5.0)
    hot = 485.0
    low_heat = WATER_SHARE * P + TRANSFORMER_LOSS * S_MVA
    duct_heat = OFFGAS_SHARE * P
    f_low, f_hot = recs["radiator-field-330k"]["rejects"]["heat"], recs["radiator-field-485k"]["rejects"]["heat"]
    n_low, n_hot = math.ceil(low_heat / f_low), math.ceil(duct_heat / f_hot)
    bays = [
        {"id": "V", "mount": "plant-mount.bay-s2", "unit": "unit.eaf-vessel-80t-sealed", "at": list(vessel_at), "note": "on the furnace platform, 5 m up, as in the shop for air"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": "unit.furnace-transformer-90mva", "at": [-12.0, -6.0, 0.0], "note": "on its slab beside the platform: no fire walls, since nothing burns without air"},
        {"id": "F", "mount": "plant-mount.bay-s1", "unit": "unit.dri-bin-200t", "at": [-12.0, 33.0, 0.0], "note": "north, as for air"},
        {"id": "G", "mount": "plant-mount.bay-s2", "unit": "unit.offgas-filter-sealed", "at": [6.0, 24.0, 0.0], "note": "north-east, where the baghouse stands for air"},
        {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [6.0, -12.0, 0.0], "note": "south-east, under its berm, its windows toward the furnace"},
    ]
    cols = 4
    low_at = [(-48.0 - 48.0 * (i % cols), -6.0 - 48.0 * (i // cols)) for i in range(n_low)]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": "unit.radiator-field-330k", "at": [x, y, 0.0], "note": f"west and south-west, in rows of {cols}"} for i, (x, y) in enumerate(low_at)]
    bays += [{"id": f"RH{i + 1}", "mount": "plant-mount.bay-s4", "unit": "unit.radiator-field-485k", "at": [-48.0 - 48.0 * i, 42.0, 0.0], "note": "north-west"} for i in range(n_hot)]
    bus_area = I2 / 4e6
    pipe_m = math.pi * 0.5 * 0.008 * RHO["steel"] * 2
    rows_y = sorted({y for _, y in low_at}, reverse=True)
    snake = []
    for k, y in enumerate(rows_y):
        xs_row = [x for x, yy in low_at if yy == y]
        ends = (max(xs_row) + 24.0, min(xs_row) - 22.0) if k % 2 == 0 else (min(xs_row) - 22.0, max(xs_row) + 24.0)
        snake += [(ends[0], y - 24.0, 1.0), (ends[1], y - 24.0, 1.0)]
    links = [
        link("conveyor", "sponge iron feed", (-12.0, 30.0, 6.0), (-12.0, 7.8, 14.2), A36, 150.0, DRI * MOON_G[1] * 8.2 / 0.5 + 20000, "0.8 m chevron-cleated belt, as for air but open (no gallery), its head over the feed lock; its bearings and grease vacuum-rated"),
        link("duct", "off-gas", (-10.0, 7.5, 13.65), (0.0, 24.0, 8.0), A36, math.pi * 1.2 * 0.010 * RHO["steel"], None, f"1.2 m across (the gas is not diluted with air, as it is for air: a fifth of the flow), 10 mm plate, evaporatively cooled: water boils in its jacket at about {hot:.0f} K and the steam gives its heat to the hot radiators; from the bellows on the roof's fourth hole to the filter", heat=duct_heat, via=[(-10.0, 7.5, 15.0), (-4.0, 13.5, 15.0), (0.0, 20.0, 12.0)]),
        link("busbar", "secondary bus", (-12.0, -3.5, 4.0), (-12.0, 0.8, 7.3), CU, 3 * bus_area * RHO["copper"], None, f"three water-cooled copper tube bars, as for air; in vacuum their gaps hold no air to break down, but the gas a seal lets out can (Paschen), so the bars are wrapped and spaced as if in air"),
        link("rail", "ladle rail", (-10.5, 6.0, 0.0), (33.0, 6.0, 0.0), A36, 2 * 60.0, None, "as for air: two 60 kg/m rails at 3 m gauge, on sintered regolith sleepers; on to the ladle station. A ladle standing in vacuum loses its hydrogen: the degasser's work begins here"),
        link("rail", "slag rail", (-16.5, 0.5, 0.0), (-16.5, 23.25, 0.0), A36, 2 * 60.0, None, "as for air: 2.5 m gauge, on sintered regolith sleepers"),
        link("rail", "crane rails", (-30.0, -12.0, 0.0), (6.0, 12.0, 0.0), A36, 2 * 50.0, None, "the gantry crane's two rails, 24 m apart along x (y -12 and 12), 36 m long", via=[(6.0, -12.0, 0.0), (6.0, 12.0, 0.0), (-30.0, 12.0, 0.0)]),
        link("pipe", "cooling water to the radiators", snake[-1], (-15.8, 11.1, 5.5), A36, pipe_m, None, f"a pair of 500 mm pipes, 8 mm wall, flow and return, at about 330 K: along the edges of the {n_low} low radiator fields' rows, their headers teed off it, then east to the furnace's manifold, a branch to the transformer", via=list(reversed(snake[:-1])) + [(-24.0, -30.0, 1.0), (-24.0, -0.5, 1.0), (-19.5, -0.5, 1.5), (-19.5, -0.5, 5.5), (-19.5, 11.1, 5.5)]),
        link("pipe", "steam to the hot radiators", (-4.0, 13.5, 14.0), (-72.0 + 24.0, 42.0, 1.0), A36, math.pi * 0.4 * 0.012 * RHO["steel"] * 2, None, f"a pair of 400 mm pipes, 12 mm wall: steam from the duct's jacket at about {hot:.0f} K (17 bar) to the hot radiator field, its water back", via=[(-4.0, 13.5, 1.0), (-24.0, 42.0, 1.0)]),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-7.5, 6.0, 0.0]},
              {"unit": "unit.slag-pot-car", "rides": "slag rail", "at": [-16.5, 6.0, 0.0]},
              {"unit": "unit.gantry-crane-20t-moon", "rides": "crane rails", "at": [-12.0, 0.0, 0.0]}]
    P0, P1, PY = (-20.0, -6.0), (0.0, 12.0), 5.0
    plat_v = (P0[1] - P0[0]) * (P1[1] - P1[0]) * PY - 5.0 * 5.0 * PY - 5.0 * 12.0 * 4.4 - 3.0 * 3.0 * 0.6 + (P0[1] - P0[0]) * (P1[1] - P1[0]) * 1.0
    ground = [
        {"what": "Furnace platform", "item": SINTER, "volume": r(plat_v), "note": "as for air (the ladle slot, slag tunnel and well), of sintered regolith blocks: there is no water to make concrete"},
        {"what": "Transformer slab", "item": SINTER, "volume": r(12 * 12 * 0.6), "note": "no fire walls"},
        {"what": "Crew post berm", "item": "material.regolith", "volume": r(10 * 8 * 3.0 - math.pi * 4 * 6), "note": "loose regolith heaped 1 m over the post"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r((n_low + n_hot) * 4 * (44.0 * 44.0 / 72.0) * 0.3 * 0.3 * 0.3), "note": "a 0.3 m block under each leg"},
        {"what": "Rail sleepers and pads", "item": SINTER, "volume": r(43.5 * 1.0 + 22.75 * 1.0 + 72.0 * 0.6 + 12 * 6 * 0.4 + 6.0), "note": "under the ladle, slag and crane rails, the filter and the bin"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO_SINTER if g["item"] == SINTER else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used) + sum(l.get("heat", 0) for l in links)
    rejects = sum(u.get("rejects", {}).get("heat", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    hp_w = low_heat / (0.5 * hot / (hot - 330.0))
    hp_fields = math.ceil((low_heat + hp_w + duct_heat) / f_hot)
    against = (f"module.arc-furnace: one box, the same on every world. This layout: the same rate; {power / 1e6:.1f} MW; {heat / 1e6:.1f} MW to {n_low + n_hot} radiator fields ({rejects / 1e6:.1f} MW at lunar noon), "
               f"{n_low} at 330 K for the panels and transformer and {n_hot} at {hot:.0f} K for the duct, in place of the tower; no water used up (closed loops); no hall, no baghouse (the furnace is sealed); "
               f"{(mass_units + mass_links) / 1000:,.0f} t of plant and {mass_ground / 1000:,.0f} t of sintered regolith. The radiators cover {(n_low + n_hot) * 48 * 48 / 1e4:.1f} ha: the heat that went up a tower's plume as steam must here leave as light, and at 330 K a panel in the noon sun sheds little. A heat pump lifting the panels' heat to {hot:.0f} K (at half Carnot's) would need about {hp_w / 1e6:.0f} MW more power and {hp_fields} hot fields in place of all {n_low + n_hot} (not fitted).")
    xs = [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.arc-furnace-airless", "name": "Arc furnace shop, airless",
                     "description": "The 80 t arc furnace on airless ground at about a moon's gravity: sealed, holding its own gas at 1 bar; no hall (no weather to keep off) but a gantry crane on its own legs; its heat to fields of radiator panels instead of a cooling tower, one set cool for the panels' water and one hot for the duct's steam; its off-gas filtered and let go; the crew in a pressurised post under a berm; its platform of sintered regolith. Built separately from the shop for air, not reused."},
        "stands_for": "module.arc-furnace",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": 25.0, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links,
        "ground": ground,
        "totals": {"rate": RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": mod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out from the shop for air: the platform, vault and rails where they were; the rest for airless ground."},
                  {"of": ["totals.heat"], "tier": "derived", "note": "The units' and the duct's heat; the radiator fields from the rejection a field gives at lunar noon (unit.radiator-field-330k, -485k)."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes as for air, of sintered regolith at 2,500 kg/m3 and loose regolith at 1,500."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, links and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged; it works from the crew post."}],
    }
    print(f"layout.arc-furnace-airless: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to {n_low}+{n_hot} radiator fields ({f_low / 1e6:.1f} and {f_hot / 1e6:.1f} MW each)")
    return layout, bays


def airless_ladle_units(us):
    """The airless ladle station's and casting bay's own units."""
    out = {}
    lf = dict(us["ladle-furnace-80t"])
    lf["name"] = "Ladle furnace, 80 t, sealed"
    lf["desc"] = ("The 80 t ladle furnace closed for airless ground: its roof's skirt seals on the ladle's rim with a soft ring, glands round the electrodes, the alloys in through a lock in the chute, and argon fed under the roof to about 50 kPa, so the arc burns as in air and the manganese it adds is not boiled off into the vacuum (open to vacuum, an 80 t heat at 1,600 deg C would lose about a fifth of its manganese in a quarter hour). "
                 "Its fume, mostly argon, leaves by the roof's elbow to the layout's filter. Otherwise as unit.ladle-furnace-80t: " + us["ladle-furnace-80t"]["desc"])
    lf["built_of"] = list(us["ladle-furnace-80t"]["built_of"]) + [
        part("Rim seal and electrode glands", A36, 2500.0, "from memory"),
        part("Alloy lock", A36, 1200.0, "two valves in the chute, from memory")]
    lf["needs"] = dict(us["ladle-furnace-80t"]["needs"])
    lf["basis"] = us["ladle-furnace-80t"]["basis"] + [{"of": ["identity.description"], "tier": "invented", "review": True, "note": "Manganese's loss: its vapour pressure over a 1% melt (about 40 Pa at 1,873 K, from memory) through Hertz-Knudsen over the ladle's 7 m2. Sealed roofs as on closed furnaces; not built for vacuum."}]
    out["ladle-furnace-80t-sealed"] = lf
    out["degassing-stand"] = dict(
        name="Degassing stand", size_class=2, fits="bay-s2", dims=(10.0, 8.0, 8.0),
        desc=("Where the ladle stands on its car in the open vacuum for a quarter hour while argon through its porous plug stirs the melt: the hydrogen and nitrogen leave of themselves, as they do in a degasser's tank on a world with air, and the carbon and oxygen meet as CO. "
              "A refractory heat shield on a swing arm hangs 1 m over the ladle's open top, with gaps for the gas, so the melt loses less heat by radiation (about 4 MW open, 4 degrees a minute); the argon and a sampling lance come from the stand. Before the alloys, which the sealed ladle furnace adds. The car stops at local (0, 0)."),
        built_of=[part("Swing arm and post", A36, 12000.0, "from memory"),
                  part("Heat shield", REF, math.pi * 1.8 ** 2 * 0.15 * RHO["refractory"], "3.6 m across, 150 mm"),
                  part("Argon valve stand and lance", A36, 2000.0, "from memory")],
        needs={"power": 30e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "From memory."},
               {"of": ["identity.description"], "tier": "derived", "review": True, "note": "Radiation from 7 m2 of melt at 1,873 K at emissivity 0.9; 80 t at 820 J/kg K."}])
    for cap, trolley, kind, job, gear, key in ((150000.0, 37500.0, "Ladle gantry crane", "on its own legs over the teeming rail and the ladle rail: it lifts the full ladle off its car and holds it over each mould in turn", "a main hoist and a ladle beam", "gantry-crane-150t-moon"),
                                                (20000.0, 6000.0, "Stripper gantry crane", "on its own legs over the teeming rail's east half: it lifts each frozen ingot up out of its mould by the hot top, the mould held down by its lugs, and sets the ingots on the ingot car", "ingot tongs whose arms hold the mould down", "stripper-gantry-20t-moon")):
        c = bridge_crane_unit(cap, 24.0, trolley, kind=kind, job=job, gear=gear, g=MOON_G[1], h_min=0.6)
        legs = 4 * box_tube(1.0, 1.0, 0.025, 24.0) * RHO["steel"] if cap > 50000 else 4 * box_tube(0.8, 0.8, 0.02, 22.0) * RHO["steel"]
        c["built_of"].append(part("Legs and sill beams", A36, legs * 1.15, "four box legs and two sill beams, from the legs' size (the sills 15%)"))
        c["dims"] = (10.0, 25.0, 27.0)
        c["desc"] = c["desc"].replace("on the hall's runways", "on its own legs, running on two ground rails 24 m apart").replace("its origin on the runway's rail plane", "its origin on the ground rails' plane, its girders 24 m up") + f" Sized for {MOON_G[1]:g} m/s2."
        out[key] = c
    return out


def ladle_station_airless(us, recs):
    """The ladle station on airless ground: the ladle degasses on an open stand, then the sealed ladle furnace alloys and
    reheats it under argon; no degasser, no hall, no ladle crane."""
    lf_heat = recs["ladle-furnace-80t-sealed"]["needs"].get("heat", 0) + recs["furnace-transformer-14mva"]["needs"].get("heat", 0)
    f_low = recs["radiator-field-330k"]["rejects"]["heat"]
    n_low = math.ceil(lf_heat / f_low)
    bays = [
        {"id": "D", "mount": "plant-mount.bay-s2", "unit": "unit.degassing-stand", "at": [-24.0 + 9.0, 0.0, 0.0], "note": "first on the rail from the arc furnace shop: degas before alloying"},
        {"id": "L", "mount": "plant-mount.bay-s2", "unit": "unit.ladle-furnace-80t-sealed", "at": [3.0, 0.0, 0.0], "note": "over the ladle rail"},
        {"id": "T", "mount": "plant-mount.bay-s1", "unit": "unit.furnace-transformer-14mva", "at": [3.0, -9.0, 0.0], "note": "on its slab south of the ladle furnace, no fire walls"},
        {"id": "A", "mount": "plant-mount.bay-s3", "unit": "unit.alloy-bins-8", "at": [3.0, 27.0, 0.0], "note": "north; open feeders and belt, vacuum-rated"},
        {"id": "G", "mount": "plant-mount.bay-s2", "unit": "unit.offgas-filter-sealed", "at": [21.0, 18.0, 0.0], "note": "north-east: the ladle furnace's argon and fume"},
        {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [18.0, -12.0, 0.0], "note": "south-east, under its berm"},
    ]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": "unit.radiator-field-330k", "at": [-24.0 - 48.0 * i, -42.0, 0.0], "note": "south-west"} for i in range(n_low)]
    tr = us["furnace-transformer-14mva"]
    tL, tW, tH = tr["dims"]
    links = [
        link("rail", "ladle rail", (-30.0, 0.0, 0.0), (30.0, 0.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails at 3 m gauge on sintered regolith sleepers, from the arc furnace shop to the casting bay; the car stops at x -15 (the stand) and 3 (the ladle furnace)"),
        link("busbar", "secondary bus", (3.0, -9.0 + tW / 2, tH * 2 / 3), (3.0, -5.0, 6.0), CU, 3 * L_I2 / 4e6 * RHO["copper"], None, "as for air"),
        link("conveyor", "alloy feed", (3.0, 27.0, 7.0), (3.0, 3.0, 9.4), A36, 100.0, 20000.0, "0.65 m belt, open (no gallery on airless ground), into the furnace's alloy lock"),
        link("duct", "fume", (5.0, 0.0, 7.5), (15.0, 18.0, 8.0), A36, math.pi * 0.5 * 0.006 * RHO["steel"], None, "0.5 m: the argon and fume from under the sealed roof, a tenth of the air shop's flow", via=[(5.0, 0.0, 10.0), (15.0, 10.0, 10.0)]),
        link("pipe", "cooling water to the radiators", (-24.0 - 48.0 * (n_low - 1) - 22.0, -18.0, 1.0), (-1.5, -4.5, 1.0), A36, math.pi * 0.25 * 0.006 * RHO["steel"] * 2, None, f"a pair of 250 mm pipes, flow and return, at about 330 K, along the {n_low} radiator fields' north edge to the furnace's manifold, a branch to the transformer", via=[(-1.5, -18.0, 1.0)]),
        link("rail", "crane rails", (-24.0, -12.0, 0.0), (12.0, 12.0, 0.0), A36, 2 * 50.0, None, "the maintenance gantry crane's two rails, 24 m apart, over the stand and the ladle furnace", via=[(12.0, -12.0, 0.0), (12.0, 12.0, 0.0), (-24.0, 12.0, 0.0)]),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-15.0, 0.0, 0.0]},
              {"unit": "unit.gantry-crane-20t-moon", "rides": "crane rails", "at": [-6.0, 0.0, 0.0]}]
    ground = [
        {"what": "Ladle furnace foundation", "item": SINTER, "volume": r(12 * 10 * 1.0 - 5 * 12 * 0.3), "note": "as for air, of sintered regolith"},
        {"what": "Stand and transformer pads", "item": SINTER, "volume": r(10 * 8 * 0.5 + 6 * 6 * 0.5)},
        {"what": "Bins and filter pads", "item": SINTER, "volume": r(12.6 * 6.3 * 1.0 + 12 * 6 * 0.4)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": r(10 * 8 * 3.0 - math.pi * 4 * 6)},
        {"what": "Rail sleepers", "item": SINTER, "volume": r(60 * 1.0 + 72 * 0.6)},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(n_low * 4 * (44.0 * 44.0 / 72.0) * 0.027)},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO_SINTER if g["item"] == SINTER else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    against = (f"module.ladle-station: one box, the same on every world. This layout: the same rate; {power / 1e6:.1f} MW; {heat / 1e6:.1f} MW to {n_low} radiator fields at 330 K; no water used up. "
               f"No degasser: the vacuum degasses the ladle on an open stand, but must not see the alloys, which the sealed ladle furnace adds under argon; no ladle crane (nothing is lifted into a tank) and no hall. "
               f"{(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of sintered regolith.")
    xs = [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.ladle-station-airless", "name": "Ladle station, airless",
                     "description": "The ladle station on airless ground at about a moon's gravity: the heat comes on its car from the arc furnace shop, stands a quarter hour on an open degassing stand where the vacuum takes its gases, then goes under the sealed ladle furnace, which alloys and reheats it under argon, and on to the casting bay. Radiators for the tower, a maintenance gantry crane, a pressurised crew post. Built separately from the station for air."},
        "stands_for": "module.ladle-station",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": 27.0, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": L_RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": lmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out for airless ground."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes as for air, of sintered regolith at 2,500 kg/m3 and loose regolith at 1,500."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged; it works from the crew post."}],
    }
    print(f"layout.ladle-station-airless: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to {n_low} radiator fields")
    return layout, bays


def casting_bay_airless(us, recs):
    """The casting bay on airless ground: as for air, the cranes on their own legs and no hall; the moulds and ingots
    cool by radiation alone."""
    car_L = us["mould-car-12"]["dims"][0]
    train = CARS * car_L
    links = [
        link("rail", "ladle rail", (-36.0, 6.0, 0.0), (-12.0, 6.0, 0.0), A36, 2 * 60.0, None, "as for air, on sintered regolith sleepers; the car stops at x -15, under the ladle gantry"),
        link("rail", "teeming rail", (-30.0, -6.0, 0.0), (30.0, -6.0, 0.0), A36, 2 * 50.0, None, f"as for air: two trains of {CARS} mould cars ({train:.0f} m each); teeming in vacuum takes the hydrogen out of the stream as it falls"),
        link("rail", "ingot rail", (3.0, 6.0, 0.0), (36.0, 6.0, 0.0), A36, 2 * 50.0, None, "as for air: out to the ingot yard"),
        link("rail", "ladle gantry rails", (-33.0, -12.0, 0.0), (0.0, 12.0, 0.0), A36, 2 * 60.0, None, "the ladle gantry's two rails, 24 m apart, over the west half", via=[(0.0, -12.0, 0.0), (0.0, 12.0, 0.0), (-33.0, 12.0, 0.0)]),
        link("rail", "stripper gantry rails", (0.0, -12.0, 0.0), (33.0, 12.0, 0.0), A36, 2 * 50.0, None, "the stripper gantry's two rails over the east half, beside the ladle gantry's ends", via=[(33.0, -12.0, 0.0), (33.0, 12.0, 0.0), (0.0, 12.0, 0.0)]),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-15.0, 6.0, 0.0]}]
    mobile += [{"unit": "unit.mould-car-12", "rides": "teeming rail", "at": [round(-27.0 + car_L / 2 + i * car_L, 2), -6.0, 0.0]} for i in range(CARS)]
    mobile += [{"unit": "unit.mould-car-12", "rides": "teeming rail", "at": [round(3.0 + car_L / 2 + i * car_L, 2), -6.0, 0.0]} for i in range(CARS)]
    mobile += [{"unit": "unit.ingot-car-40t", "rides": "ingot rail", "at": [15.0, 6.0, 0.0]},
               {"unit": "unit.gantry-crane-150t-moon", "rides": "ladle gantry rails", "at": [-15.0, 0.0, 0.0]},
               {"unit": "unit.stripper-gantry-20t-moon", "rides": "stripper gantry rails", "at": [15.0, 0.0, 0.0]}]
    bays = [{"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [0.0, -21.0, 0.0], "note": "south, under its berm"}]
    ground = [
        {"what": "Casting floor", "item": SINTER, "volume": r(60 * 24 * 0.4), "note": "0.4 m of sintered regolith under the teeming and stripping, so spilt steel meets stone"},
        {"what": "Gantry rail sleepers", "item": SINTER, "volume": r(4 * 33 * 1.0)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": r(10 * 8 * 3.0 - math.pi * 4 * 6)},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO_SINTER if g["item"] == SINTER else 1500.0))
    used = [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [recs[b["unit"].split(".", 1)[1]] for b in bays]
    power = sum(u.get("needs", {}).get("power", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    against = (f"module.casting-bay: one box, the same on every world. This layout: the same rate; {power / 1e6:.2f} MW; no water; no hall: the 150 t and 20 t cranes stand on their own legs, sized for {MOON_G[1]:g} m/s2. "
               f"The moulds and ingots cool by radiation alone, with no air to help: the second train stands longer (not yet worked out). {(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of sintered regolith.")
    xs = [-36.0, 36.0]
    layout = {
        "identity": {"key": "layout.casting-bay-airless", "name": "Casting bay, airless",
                     "description": "Ingot teeming on airless ground at about a moon's gravity: as for air, a train of mould cars teemed from the ladle and a second freezing, but under gantry cranes on their own legs and no hall; the stream degasses as it falls through the vacuum. Built separately from the bay for air."},
        "stands_for": "module.casting-bay",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": 72.0, "width": 48.0, "height": 27.0, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": C_RATE, "power": r(power), "heat": 0.0, "water": 0.0, "mass": r(total), "against_module": against},
        "staff": cmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "As the bay for air, on airless ground."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Sintered regolith at 2,500 kg/m3, loose at 1,500."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.casting-bay-airless: {total / 1000:,.0f} t, {power / 1e6:.2f} MW")
    return layout, bays


# The shaft furnace on airless ground: a high-pressure loop so the top gas's water condenses warm, a hot discharge.
HP_SHAFT, HP_SUCTION = 8.5e5, 7.3e5      # Pa: the loop at about 8 bar, as high-pressure shafts run (from memory)
T_COND = 80.0                            # deg C: the recycle gas out of the condenser, its water 6% (the rest condensed)
T_REC_OUT = 125.0                        # deg C: the top gas out of the recuperator, above its dew point at 8 bar (about 116)
T_HOT_DRI = 650.0                        # deg C: the sponge iron discharged hot
_top_cap = H2_TOP * CP_H2 + STEAM * CP_STEAM
HP_Q_RECUP = _top_cap * (T_TOP - T_REC_OUT)
HP_T_PRE = T_COND + HP_Q_RECUP / (GAS * CP_H2)
HP_Q_HEATER = GAS * CP_H2 * (T_GAS - HP_T_PRE)
_left = 0.06 / 0.94 * mol_flow(H2_TOP) * 18.015e-3          # kg/s of water the recycle carries on
HP_Q_COND = _top_cap * (T_REC_OUT - T_COND) + (STEAM - _left) * 2.33e6
HP_W_COMP = GAS * CP_H2 * (T_COND + 273.15) * ((HP_SHAFT / HP_SUCTION) ** ((GAMMA - 1) / GAMMA) - 1) / ETA_K
HP_T_LOOP = 345.0                        # K: the condenser's water loop to the radiators


def shaft_airless_units(us):
    out = {}
    D = 6.6
    zones = {"top": 4.0, "reduction": 12.0, "transition": 4.0, "cone": 4.0}
    hot = zones["top"] + zones["reduction"] + zones["transition"] + zones["cone"]
    t_ref, t_sh = 0.35, 0.04
    Do = D + 2 * t_ref + 2 * t_sh
    Z_OUT = 8.0
    z_ring = Z_OUT + zones["cone"]
    z_bustle = z_ring + zones["transition"]
    z_top = Z_OUT + hot
    H = z_top + 8.0
    hoop = HP_SHAFT * Do / (2 * t_sh)
    shell = cyl_shell(Do, hot, t_sh) * RHO["steel"]
    lining = math.pi * ((D / 2 + t_ref) ** 2 - (D / 2) ** 2) * hot * RHO["refractory"]
    burden = (math.pi * (D / 2) ** 2 * (hot - zones["cone"]) + math.pi * (D / 2) ** 2 * zones["cone"] / 3) * 1900.0
    d_b = main_dia(mol_flow(GAS), T_GAS, HP_SHAFT)
    R_b = Do / 2 + 0.3 + d_b / 2
    side = max(12.0, round(2 * R_b + d_b + 1.0))
    out["shaft-furnace-200th-hp"] = dict(
        name=f"Hydrogen shaft furnace, {S_RATE * 3.6:.0f} t an hour, 8 bar, hot discharge", size_class=2 if side <= 12 else 3, fits="bay-s2" if side <= 12 else "bay-s3", dims=(side, side, round(H, 1)),
        desc=(f"The shaft of unit.shaft-furnace-200th built for airless ground: the gas loop at about {HP_SHAFT / 1e5:.0f} bar, so the shell is a pressure vessel ({hoop / 1e6:.0f} MPa in its {t_sh * 1000:.0f} mm plate) and the charge and discharge go through pressure locks; "
              f"no cooling zone: the sponge iron leaves through a lock at about {T_HOT_DRI:.0f} deg C into a closed hot conveyor to the arc furnace shop, which in vacuum it cannot rust on the way. "
              f"Local frame: the axis at (0, 0); the discharge lock at z {Z_OUT:g}; the ring girder at z {z_ring:.0f} (radius {Do / 2 + 0.4:.1f} m); the bustle main ({d_b:.1f} m pipe) at z {z_bustle:.0f}, its inlet on -y; the top gas outlet at z {z_top - 1:.0f} on +x; the charge lock and hopper z {z_top:.0f} to {H:.0f}. About {burden / 1000:,.0f} t of burden inside."),
        built_of=[part("Shell", A36, shell, f"{Do:.2f} m across, {hot:g} m, {t_sh * 1000:.0f} mm plate"),
                  part("Lining", REF, lining, f"{t_ref * 1000:.0f} mm"),
                  part("Bustle main", A36, math.pi * (d_b + 0.4) * 0.025 * 2 * math.pi * R_b * RHO["steel"], f"a ring {2 * R_b:.1f} m across, 25 mm plate for the pressure"),
                  part("Bustle lining", REF, math.pi * (d_b + 0.2) * 0.2 * 2 * math.pi * R_b * RHO["refractory"], "200 mm"),
                  part("Support ring girder", A36, 2 * math.pi * (Do / 2 + 0.4) * box_tube(0.8, 1.2, 0.04, 1.0) * RHO["steel"], "as for air"),
                  part("Charge and discharge pressure locks", A36, 90000.0, "two lock hoppers with their valves, from memory"),
                  part("Burden feeders", "good.electric-motors", 25000.0, "from memory")],
        holds=burden,
        needs={"power": 400e3, "gravity": 1.0, "atmosphere": "any"},
        recipes=smod["recipes"], life=smod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.shaft-furnace), carried here."},
               {"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": "As unit.shaft-furnace-200th less its cooling zone; the shell checked for the loop's pressure; the locks from memory."},
               {"of": ["needs.gravity"], "tier": "invented", "review": True, "note": "At 1.6 m/s2 the burden weighs about 3 kPa a metre of bed against the gas's drag of about 1 (from memory): it still sinks; at 8 bar the gas is slower."}])
    out["top-gas-condenser-8bar"] = dict(
        name=f"Top gas condenser, {HP_Q_COND / 1e6:.0f} MW", size_class=2, fits="bay-s2", dims=(12.0, 8.0, 14.0),
        desc=(f"Two vertical shell-and-tube condensers at 8 bar: the top gas down through the tubes from {T_REC_OUT:.0f} to {T_COND:.0f} deg C, its water condensing (dew point about 116 deg C at this pressure, so the heat leaves warm), the loop's water round the tubes from about {HP_T_LOOP - 10 - 273.15:.0f} to {HP_T_LOOP - 273.15:.0f} deg C; "
              f"{STEAM - _left:.1f} kg/s of water drains to a pot and on to the hydrogen plant. A candle filter ahead of it takes the dust. Gas in at local (-6, 0, 12), out at (6, 0, 2)."),
        built_of=[part("Shells and tubes", "material.stainless-steel-304", 2 * cyl_shell(3.0, 12.0, 0.016, bottom=2.0) * 7900 + 30000.0, "two shells 3 m across, 12 m, 16 mm, and their tubes (30 t, from memory)"),
                  part("Filter and drain pots", A36, 15000.0, "from memory")],
        needs={"heat": HP_Q_COND + HP_W_COMP, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "From memory."},
               {"of": ["needs.heat"], "tier": "derived", "note": f"The top gas cooled {T_REC_OUT:.0f} to {T_COND:.0f} deg C and its water condensed down to 6% of the recycle (2.33 MJ/kg); with the compressor's aftercooler."}])
    out["recycle-compressor-8bar"] = dict(
        name=f"Recycle compressor, {HP_W_COMP / 1e6:.1f} MW, 8 bar", size_class=2, fits="bay-s2", dims=(12.0, 6.0, 5.0),
        desc=f"Raises {GAS:.1f} kg/s of hydrogen from {HP_SUCTION / 1e5:.1f} to {HP_SHAFT / 1e5:.1f} bar, the loop's pressure drop only: two centrifugal stages in one casing and a motor. At 8 bar the gas is dense enough that this is a small machine. Suction at local (6, 0, 1.5), discharge at (-6, 0, 1.5).",
        built_of=[part("Casing, rotor and motor", "material.high-strength-steel-4340", 8000.0 * HP_W_COMP / 1e6, "8 t a MW of drive, from memory")],
        needs={"power": HP_W_COMP / 0.96, "atmosphere": "any"},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "From memory."},
               {"of": ["needs"], "tier": "derived", "note": f"Isentropic at {ETA_K:.0%}, a motor at 96%; its heat leaves by the condenser's aftercooler."}])
    hh = dict(us[next(k for k in us if k.startswith("gas-heater-"))])
    hh["name"] = f"Hydrogen heater, {HP_Q_HEATER / 1e6:.0f} MW, 8 bar"
    hh["desc"] = hh["desc"].replace(f"from {T_PRE:.0f} to", f"from {HP_T_PRE:.0f} to") + " Its vessels hold the loop's 8 bar (25 mm plate at 4 m: 64 MPa, the shell kept cool by its lining)."
    hh["needs"] = {"power": HP_Q_HEATER / 0.98}
    out[f"gas-heater-{HP_Q_HEATER / 1e6:.0f}mw-8bar"] = hh
    out[f"radiator-field-{HP_T_LOOP:.0f}k"] = radiator_unit(HP_T_LOOP, "the condenser's")
    return out


def shaft_furnace_airless(us, recs):
    """The hydrogen shaft furnace on airless ground at about a moon's gravity."""
    sh_key = "shaft-furnace-200th-hp"
    sh = us[sh_key]
    side, _, H = sh["dims"]
    cell = 3.0
    top = math.ceil((H + 4.0) / cell) * cell
    box = (-12.0, -12.0, 0.0, 12.0, 12.0, top)
    z_top_gas = float(sh["desc"].split("top gas outlet at z ")[1].split(" ")[0])
    z_bustle = float(sh["desc"].split("bustle main (")[1].split("at z ")[1].split(",")[0])
    z_ring = float(sh["desc"].split("ring girder at z ")[1].split(" ")[0])
    openings = [(9.0, -3.0, z_top_gas - 3.0, 12.0, 3.0, top - 3.0), (-3.0, -12.0, z_bustle - 3.0, 3.0, -9.0, z_bustle + 3.0), (-12.0, -3.0, 3.0, -9.0, 3.0, 9.0)]
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, round(top / cell) - 1, None, openings)
    srec = recs[sh_key]
    g = MOON_G[1]
    load = (srec["physical"]["mass"] + srec.get("holds", 0)) * g
    col_a = 0.3 * 0.3 - 0.26 * 0.26
    n_col = 8
    col_sigma = load / n_col / col_a
    assert 1.5 * col_sigma <= 470e6
    # No wind: the lattice only braces the columns and carries the platforms; the kit's lightest aluminium strut does.
    plat_q = 6 * (box[3] - box[0]) * (box[4] - box[1]) * 0.6 * 0.006 * RHO["aluminium"] * g
    per_node = plat_q / max(1, ground_nodes) * 4
    sec = frames.lightest_section(per_node + 1.0, metal="al6061", with_diagonal=True)
    members, fmass = [], 0.0
    for gname in ("wall", "roof"):
        for diag, n in ((False, groups[gname][0]), (True, groups[gname][1])):
            key, st = strut(sec, cell, diag)
            members.append({"item": key, "count": n, "where": "tower walls" if gname == "wall" else "tower roof"})
            fmass += n * st["physical"]["mass"]
    node_key = strut(sec, cell)[1]["kit"]["node"]
    node = yaml.safe_load(open(S + "mill-stock/" + node_key.split(".", 1)[1].upper() + ".yaml"))
    members.append({"item": node_key, "count": n_nodes, "where": "nodes"})
    fmass += n_nodes * frames.node_mass(node, frames.materials())
    members.append({"item": "stock.st4340-box-300x300x20", "count": n_col, "where": f"columns under the shaft's ring girder, {z_ring:.0f} m, braced to the tower every 6 m"})
    fmass += n_col * z_ring * col_a * RHO["steel"]
    deck = 6 * (box[3] - box[0]) * (box[4] - box[1]) * 0.6 * 0.006 * RHO["aluminium"]
    sized = (f"At {g:g} m/s2 the shaft with its burden, {load / 1e6:.1f} MN, on {n_col} columns of 300 x 300 x 20 mm 4340 box: {col_sigma / 1e6:.0f} MPa each. No wind: the lattice braces the columns and carries six aluminium grating platforms (6 mm, 60% of the plan), "
             f"the kit's lightest aluminium strut ({sec}). Footings not yet checked.")
    heater = next(k for k in us if k.startswith("gas-heater-") and k.endswith("-8bar"))
    rad = f"radiator-field-{HP_T_LOOP:.0f}k"
    loop_heat = recs["top-gas-condenser-8bar"]["needs"]["heat"] + recs["heater-transformer-131mva"]["needs"].get("heat", 0)
    n_rad = math.ceil(loop_heat / recs[rad]["rejects"]["heat"])
    cols = 6
    rad_at = [(-60.0 - 48.0 * (i % cols), 60.0 + 48.0 * (i // cols)) for i in range(n_rad)]
    bays = [
        {"id": "F", "mount": f"plant-mount.{sh['fits']}", "unit": f"unit.{sh_key}", "at": [0.0, 0.0, 0.0], "note": "in the tower, on its columns; the hot conveyor under its discharge lock"},
        {"id": "R", "mount": "plant-mount.bay-s2", "unit": "unit.recuperator", "at": [24.0, 24.0, 0.0], "note": "as for air; its shells hold 8 bar"},
        {"id": "S", "mount": "plant-mount.bay-s2", "unit": "unit.top-gas-condenser-8bar", "at": [48.0, 24.0, 0.0], "note": "where the scrubber stands for air"},
        {"id": "K", "mount": "plant-mount.bay-s2", "unit": "unit.recycle-compressor-8bar", "at": [48.0, 0.0, 0.0], "note": "east"},
        {"id": "H", "mount": "plant-mount.bay-s3", "unit": f"unit.{heater}", "at": [24.0, -24.0, 0.0], "note": "south-east"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": "unit.heater-transformer-131mva", "at": [48.0, -24.0, 0.0], "note": "on its slab, no fire walls"},
        {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [-24.0, -24.0, 0.0], "note": "south-west, under its berm"},
    ]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{rad}", "at": [x, y, 0.0], "note": f"north-west, in rows of {cols}"} for i, (x, y) in enumerate(rad_at)]
    d_top = main_dia(mol_flow(H2_TOP, STEAM), T_TOP, HP_SUCTION)
    d_hot = main_dia(mol_flow(GAS), T_GAS, HP_SHAFT)
    d_gas = main_dia(mol_flow(GAS), T_COND, HP_SHAFT)
    pm = lambda d, t=0.016: math.pi * d * t * RHO["steel"]
    lined = lambda d: math.pi * (d + 0.4) * 0.025 * RHO["steel"] + math.pi * (d + 0.2) * 0.2 * RHO["refractory"]
    flow = loop_heat / (4186 * 10)
    d_cw = math.sqrt(4 * flow / 1000 / 2.5 / math.pi)
    rise = 15.0
    belt_x = H / math.sin(math.radians(rise)) * math.cos(math.radians(rise))
    links = [
        link("conveyor", "pellet belt", (round(belt_x, 1), 0.0, 2.0), (0.0, 0.0, round(H + 2.0, 1)), A36, 120.0, 300000.0 * MOON_G[1] / G + 50000.0, f"a 1.2 m belt on trestles, open, rising at {rise:g} degrees to the charge lock's hopper; its bearings and grease vacuum-rated"),
        link("conveyor", "hot sponge iron", (0.0, 0.0, 6.0), (-60.0, 0.0, 6.0), "material.stainless-steel-304", 400.0, 150000.0, f"a closed drag chain conveyor in an insulated casing: {S_RATE:.1f} kg/s of sponge iron at about {T_HOT_DRI:.0f} deg C to the arc furnace shop's bin, beyond the layout; the hot charge saves the furnace about {S_RATE * CP_DRI * (T_HOT_DRI - 25) / 1e6:.0f} MW"),
        link("pipe", "top gas main", (side / 2 - 0.5, 0.0, z_top_gas), (24.0, 24.0, 18.0), A36, pm(d_top), None, f"{d_top:.1f} m, 16 mm wall for 8 bar, insulated", via=[(15.0, 0.0, z_top_gas), (24.0, 24.0, z_top_gas)]),
        link("pipe", "gas to the condenser and compressor", (24.0, 29.2, 1.5), (42.0, 0.0, 1.5), A36, pm(d_gas), None, f"{d_gas:.1f} m, 16 mm", via=[(24.0, 32.0, 1.5), (42.0, 32.0, 1.5), (42.0, 24.0, 12.0), (54.0, 24.0, 2.0), (54.0, 0.0, 1.5)]),
        link("pipe", "compressed gas back to the recuperator and heater", (42.0, 0.0, 1.5), (12.0, -27.3, 2.0), A36, pm(d_gas), None, f"{d_gas:.1f} m, 16 mm, insulated after the recuperator", via=[(36.0, 0.0, 1.5), (36.0, 14.0, 1.5), (24.0, 14.0, 16.75), (14.0, 14.0, 16.75), (14.0, -15.0, 16.75), (10.0, -15.0, 16.75), (10.0, -15.0, 2.0), (10.0, -27.3, 2.0)]),
        link("pipe", "hot gas to the bustle", (24.0, -20.0, 18.0), (0.0, -side / 2, z_bustle), A36, lined(d_hot), None, f"{d_hot:.1f} m inside 200 mm of refractory, 25 mm shell for 8 bar", via=[(24.0, -20.0, z_bustle), (0.0, -20.0, z_bustle)]),
        link("pipe", "hydrogen make-up", (100.0, 4.0, 1.5), (54.0, 4.0, 1.5), "material.stainless-steel-304", pm(0.25, 0.008), None, f"{H2_USED:.2f} kg/s from the hydrogen plant's electrolysers, which run at 10 bar so that no make-up compressor is needed"),
        link("pipe", "condensate to the hydrogen plant", (54.0, 24.0, 0.5), (100.0, 24.0, 0.5), "material.stainless-steel-304", math.pi * 0.2 * 0.006 * 7900, None, f"200 mm: {STEAM - _left:.1f} kg/s of water back to the electrolysers: on a moon every kilogram comes back"),
        link("pipe", "loop water to the radiators", (rad_at[-1][0] - 22.0, rad_at[-1][1], 1.0), (48.0, 20.0, 1.0), A36, 2 * pm(d_cw, 0.010), None, f"a pair of {d_cw:.2f} m pipes at about {HP_T_LOOP:.0f} K: {flow:,.0f} kg/s along the {n_rad} radiator fields' rows to the condenser, a branch to the transformer", via=[(-36.0, rad_at[-1][1], 1.0), (-36.0, 36.0, 1.0), (48.0, 36.0, 1.0)]),
        link("cable", "heater supply", (48.0, -21.1875, 4.5), (12.45, -21.4, 1.575), CU, 3 * HP_Q_HEATER / HEATER_PF / (math.sqrt(3) * 6600) / 4e6 * RHO["copper"], None, "as for air", via=[(48.0, -17.0, 4.5), (12.45, -17.0, 4.5), (12.45, -17.0, 1.575)]),
    ]
    ground = [
        {"what": "Tower foundation", "item": SINTER, "volume": r(24 * 24 * 1.5), "note": "a mat of sintered regolith blocks 1.5 m thick (a fifth of the load of 1 g)"},
        {"what": "Unit pads", "item": SINTER, "volume": r(sum(recs[b["unit"].split(".", 1)[1]]["physical"]["length"] * recs[b["unit"].split(".", 1)[1]]["physical"]["width"] * 0.5 for b in bays[1:] if not b["id"].startswith("RL"))), "note": "0.5 m under each unit"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(n_rad * 4 * (44.0 * 44.0 / 72.0) * 0.027)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": r(10 * 8 * 3.0 - math.pi * 4 * 6)},
        {"what": "Trestle footings", "item": SINTER, "volume": r(math.ceil(belt_x / 18) * 3.0 + 6 * 2.0)},
    ]
    for gr in ground:
        gr["mass"] = r(gr["volume"] * (RHO_SINTER if gr["item"] == SINTER else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(gr["mass"] for gr in ground)
    frame_mass = fmass + deck
    total = mass_units + mass_links + frame_mass + mass_ground
    air_power = yaml.safe_load(open(S + "layouts/shaft-furnace-air.yaml"))["totals"]["power"] if os.path.exists(S + "layouts/shaft-furnace-air.yaml") else 0
    against = (f"module.shaft-furnace: one box, {S_P / 1e6:.0f} MW. The layout for air draws {air_power / 1e6:.0f} MW and sends 130 MW up four towers. This layout: the same rate; {power / 1e6:.0f} MW (heater {HP_Q_HEATER / 0.98 / 1e6:.0f}, compressor {HP_W_COMP / 1e6:.1f}: at 8 bar the compressor only makes up the loop's pressure drop); "
               f"{heat / 1e6:.0f} MW to {n_rad} radiator fields at {HP_T_LOOP:.0f} K ({n_rad * 48 * 48 / 1e4:.0f} ha), for the water must condense somewhere and only light can take the heat away; no cooling zone or loop (the sponge iron goes on hot); no water used up. "
               f"{(mass_units + mass_links + frame_mass) / 1000:,.0f} t of plant and frame, {mass_ground / 1000:,.0f} t of sintered regolith.")
    xs = [b["at"][0] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [b["at"][1] + s * BAY[int(b["mount"][-1])] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.shaft-furnace-airless", "name": "Hydrogen shaft furnace, airless",
                     "description": "Direct reduction by hydrogen on airless ground at about a moon's gravity: the gas loop at 8 bar so the top gas's water condenses warm enough for radiators to take its heat, the sponge iron discharged hot through a lock straight to the arc furnace shop, the heat to fields of radiators. Hydrogen leaking into vacuum just leaves (no air, no fire), so the tower is an open lattice for bracing, not for venting. Built separately from the plant for air."},
        "stands_for": "module.shaft-furnace",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": top, "mass": r(total)},
        "bays": bays, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"An open braced tower {box[3] - box[0]:.0f} x {box[4] - box[1]:.0f} m and {top:.0f} m tall of aluminium kit struts round the shaft; six grating platforms; no cladding.",
                  "openings": [list(o) for o in openings], "members": members, "carries": [f"unit.{sh_key}"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": S_RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": smod.get("staff"),
        "basis": [{"of": ["bays", "links", "frame.box"], "tier": "invented", "review": True, "note": "Laid out as the plant for air, for airless ground; the loop's pressure and temperatures from memory of high-pressure shafts."},
                  {"of": ["totals"], "tier": "derived", "review": True, "note": "The heat balance as for air with the loop at 8 bar: the recuperator stops above the top gas's dew point, the condenser takes the rest down to 80 deg C."},
                  {"of": ["frame.members", "frame.mass"], "tier": "derived", "review": True, "note": "Counted from the lattice; sections chosen by the loads in frame.sized."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Sintered regolith at 2,500 kg/m3, loose at 1,500."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, links, frame and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.shaft-furnace-airless: {total / 1000:,.0f} t, {power / 1e6:.0f} MW (heater {HP_Q_HEATER / 1e6:.0f}, comp {HP_W_COMP / 1e6:.1f}), {heat / 1e6:.0f} MW to {n_rad} fields of {recs[rad]['rejects']['heat'] / 1e6:.2f} MW, T_pre {HP_T_PRE:.0f} C")
    return layout, bays


def hydrogen_airless_units(us):
    out = {}
    b = dict(us["electrolyser-block-20mw"])
    b["name"] = b["name"] + ", 10 bar"
    seps = 2 * cyl_shell(2.0, 6.0, 0.025, bottom=2.0) * RHO["steel"]
    b["built_of"] = [p_ for p_ in us["electrolyser-block-20mw"]["built_of"] if p_["what"] != "Separators"] + [part("Separators", A36, seps, "two vessels 2 m across, 6 m, 25 mm plate for 10 bar (40 MPa)")]
    b["desc"] = ("The electrolyser block built for airless ground: its stacks and separators held at about 10 bar, so the hydrogen goes to the shaft furnace's 8 bar loop with no compressor, standing in the open on a pad with no hall (in vacuum there is no air for a leak to burn in, but the oxygen side is kept apart and its lines are clean). "
                 "Its lye at about 80 deg C gives its heat to the layout's loop through its coolers. Otherwise as unit.electrolyser-block-20mw: " + us["electrolyser-block-20mw"]["desc"])
    b["needs"] = dict(us["electrolyser-block-20mw"]["needs"], atmosphere="any")
    out["electrolyser-block-20mw-10bar"] = b
    return out


def hydrogen_plant_airless(us, recs):
    """The hydrogen plant on airless ground: the blocks at 10 bar in the open, radiators for the tower, every kilogram
    of water kept."""
    xs_b = [-48.0, -24.0, 0.0, 24.0, 48.0]
    rad = f"radiator-field-{HP_T_LOOP:.0f}k"
    loop = N_BLK * (recs["electrolyser-block-20mw-10bar"]["needs"]["heat"] + recs["rectifier-transformer-21mva"]["needs"]["heat"])
    n_rad = math.ceil(loop / recs[rad]["rejects"]["heat"])
    cols = 6
    rad_at = [(-120.0 + 48.0 * (i % cols), 48.0 + 48.0 * (i // cols)) for i in range(n_rad)]
    bays = [{"id": f"E{i + 1}", "mount": "plant-mount.bay-s3", "unit": "unit.electrolyser-block-20mw-10bar", "at": [x, 0.0, 0.0], "note": "on its pad in the open"} for i, x in enumerate(xs_b)]
    bays += [{"id": f"R{i + 1}", "mount": "plant-mount.bay-s2", "unit": "unit.rectifier-transformer-21mva", "at": [x, -24.0, 0.0], "note": "south, no fire walls"} for i, x in enumerate(xs_b)]
    bays += [{"id": "P", "mount": "plant-mount.bay-s2", "unit": "unit.hydrogen-purifier", "at": [72.0, 0.0, 0.0], "note": "east"},
             {"id": "W", "mount": "plant-mount.bay-s2", "unit": "unit.water-treatment", "at": [72.0, -24.0, 0.0], "note": "south-east: polishes the shaft furnace's condensate and the ice miners' water"},
             {"id": "Q", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [-72.0, -24.0, 0.0], "note": "south-west, under its berm"}]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{rad}", "at": [x, y, 0.0], "note": f"north, in rows of {cols}"} for i, (x, y) in enumerate(rad_at)]
    rt = us["rectifier-transformer-21mva"]
    rW, rH = rt["dims"][1], rt["dims"][2]
    bus_a = I_DC / 1.5e6
    ss = "material.stainless-steel-304"
    pm = lambda d, t: math.pi * d * t * 7900
    d_h2 = main_dia(mol_flow(H_RATE), 40.0, 10e5) * math.sqrt(30.0 / 15.0)
    flow = loop / (4186 * 10)
    d_cw = math.sqrt(4 * flow / 1000 / 2.5 / math.pi)
    links = [link("busbar", f"DC bus {i + 1}", (x, -24.0 + rW / 2, rH * 2 / 3), (x, -6.0, 3.0), AL, 2 * bus_a * RHO["aluminium"], None, "as for air, its insulation spaced for vacuum") for i, x in enumerate(xs_b)]
    links += [
        link("pipe", "hydrogen header", (xs_b[0] + 10.5, -2.5, 8.0), (66.0, 0.0, 6.0), ss, pm(d_h2, 0.006), None, f"{d_h2:.2f} m stainless at 10 bar, to the purifier", via=[(xs_b[0] + 10.5, -2.5, 9.0), (58.5, -2.5, 9.0), (58.5, 0.0, 6.0)]),
        link("pipe", "hydrogen to the shaft furnace", (78.0, 0.0, 2.0), (110.0, 0.0, 2.0), ss, pm(d_h2, 0.006), None, f"{H_RATE:.3f} kg/s at 10 bar, beyond the layout"),
        link("pipe", "oxygen header", (xs_b[0] + 10.5, 2.5, 8.0), (110.0, 6.0, 9.0), ss, pm(math.sqrt(4 * (H_O2 / 32e-3 * 8.314 * 353 / 10e5) / (math.pi * 15.0)), 0.006), None, f"the {H_O2:.1f} kg/s of oxygen, beyond the layout: on a moon it is worth keeping (crews' air, the pellet plant's firing)", via=[(xs_b[0] + 10.5, 2.5, 9.0), (58.5, 2.5, 9.0), (58.5, 6.0, 9.0)]),
        link("pipe", "water in", (110.0, -24.0, 0.5), (78.0, -24.0, 0.5), ss, pm(0.15, 0.005), None, "150 mm: the shaft furnace's condensate and make-up from ice, beyond the layout"),
        link("pipe", "demineralised water", (66.0, -24.0, 0.5), (59.0, 6.0, 1.0), ss, pm(0.1, 0.004), None, f"{H_WATER:.1f} kg/s to the blocks", via=[(62.0, -24.0, 0.5), (62.0, 6.0, 0.5)]),
        link("pipe", "loop water to the radiators", (rad_at[-1][0] + 24.0, rad_at[-1][1], 1.0), (-48.0 + 11.0, 6.0, 1.0), A36, 2 * math.pi * d_cw * 0.010 * RHO["steel"], None, f"a pair of {d_cw:.2f} m pipes at about {HP_T_LOOP:.0f} K: {flow:,.0f} kg/s along the {n_rad} radiator fields' rows to the blocks' coolers, a branch to the rectifiers", via=[(84.0, rad_at[-1][1], 1.0), (84.0, 20.0, 1.0), (-37.0, 20.0, 1.0)]),
        link("rail", "crane rails", (-60.0, -12.0, 0.0), (60.0, 12.0, 0.0), A36, 2 * 50.0, None, "the maintenance gantry's two rails, 24 m apart, along the blocks", via=[(60.0, -12.0, 0.0), (60.0, 12.0, 0.0), (-60.0, 12.0, 0.0)]),
    ]
    mobile = [{"unit": "unit.gantry-crane-20t-moon", "rides": "crane rails", "at": [0.0, 0.0, 0.0]}]
    ground = [
        {"what": "Block pads", "item": SINTER, "volume": r(N_BLK * 24 * 12 * 0.4), "note": "0.4 m of sintered regolith with lye-proof bunds"},
        {"what": "Rectifier and outside pads", "item": SINTER, "volume": r(N_BLK * 12 * 8 * 0.5 + 2 * 12 * 8 * 0.5)},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(n_rad * 4 * (44.0 * 44.0 / 72.0) * 0.027)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": r(10 * 8 * 3.0 - math.pi * 4 * 6)},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO_SINTER if g["item"] == SINTER else 1500.0))
    used = [recs[b_["unit"].split(".", 1)[1]] for b_ in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    against = (f"module.hydrogen-plant: one box, {H_P / 1e6:.0f} MW. This layout: the same rate; {power / 1e6:.1f} MW; {heat / 1e6:.1f} MW to {n_rad} radiator fields at {HP_T_LOOP:.0f} K ({n_rad * 48 * 48 / 1e4:.1f} ha); "
               f"its {H_WATER:.1f} kg/s of water all comes back from the shaft furnace, none evaporated; no hall; the blocks at 10 bar feed the furnace's loop with no compressor. {(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of sintered regolith.")
    xs = [b_["at"][0] + s * BAY[int(b_["mount"][-1])] / 2 for b_ in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"])]
    ys = [b_["at"][1] + s * BAY[int(b_["mount"][-1])] / 2 for b_ in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"])]
    layout = {
        "identity": {"key": "layout.hydrogen-plant-airless", "name": "Hydrogen plant, airless",
                     "description": f"Water split by electricity on airless ground at about a moon's gravity: {N_BLK} electrolyser blocks at 10 bar in the open, their rectifiers beside them, the heat to radiator fields; the water comes back from the shaft furnace, the hydrogen goes to it at its loop's pressure, the oxygen is kept. Built separately from the plant for air."},
        "stands_for": "module.hydrogen-plant",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": 25.0, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": H_RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": hmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "As the plant for air, for airless ground."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links and ground."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Sintered regolith at 2,500 kg/m3, loose at 1,500."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.hydrogen-plant-airless: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to {n_rad} fields")
    return layout, bays


def mounts_for(bays, recs):
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
                                         {"of": ["bears", "feeds"], "rule": "rule.mounts-margin", "note": f"Round the units fitted to it in the trial ({', '.join(dict.fromkeys(v['units']))}) at 1 g, full."}]}
    for b in bays:
        u, m = recs[b["unit"].split(".", 1)[1]], mounts[b["mount"].split(".", 1)[1]]
        assert u["physical"]["length"] <= m["footprint"]["length"] and u["physical"]["width"] <= m["footprint"]["width"], b

    return mounts


def main():
    us = units()
    loop_heat = sum(u["needs"].get("heat", 0) for u in us.values()) + OFFGAS_SHARE * P   # the units and the layout's water-cooled duct
    us["cooling-tower"] = cooling_unit(loop_heat)
    us.update(car_units())
    lu = ladle_units()
    lu["cooling-tower-1cell"] = cooling_unit(sum(u["needs"].get("heat", 0) for u in lu.values()), "the ladle station's")
    us.update(lu)
    us.update(casting_units())
    su = shaft_units()
    sh_heat = sum(u["needs"].get("heat", 0) for u in su.values()) + TRANSFORMER_LOSS * Q_HEATER / 0.98 / HEATER_PF
    n_t = math.ceil(sh_heat / 40e6)
    su["cooling-tower-shaft"] = cooling_unit(sh_heat / n_t, f"the shaft furnace's (one of {n_t})")
    su["cooling-tower-shaft"]["count"] = n_t          # placed n_t times in the layout; not part of the record
    us.update(su)
    hu = hydrogen_units()
    hu["cooling-tower-h2"] = cooling_unit(N_BLK * (hu["electrolyser-block-20mw"]["needs"]["heat"] + hu["rectifier-transformer-21mva"]["needs"]["heat"]), "the electrolysers' and rectifiers'")
    us.update(hu)
    us.update(pellet_units())
    us.update(yard_units())
    us.update(airless_units(us))
    us.update(airless_ladle_units(us))
    us.update(shaft_airless_units(us))
    us.update(hydrogen_airless_units(us))
    recs = {slug: unit_record(slug, u) for slug, u in us.items()}
    layouts = {"arc-furnace-air": arc_furnace_air(us, recs), "ladle-station-air": ladle_station_air(us, recs), "casting-bay-air": casting_bay_air(us, recs), "shaft-furnace-air": shaft_furnace_air(us, recs), "hydrogen-plant-air": hydrogen_plant_air(us, recs), "pellet-plant-air": pellet_plant_air(us, recs), "ore-yard-air": ore_yard_air(us, recs), "ingot-yard-air": ingot_yard_air(us, recs), "arc-furnace-airless": arc_furnace_airless(us, recs),
               "ladle-station-airless": ladle_station_airless(us, recs), "casting-bay-airless": casting_bay_airless(us, recs),
               "shaft-furnace-airless": shaft_furnace_airless(us, recs), "hydrogen-plant-airless": hydrogen_plant_airless(us, recs)}
    mounts = mounts_for([b for _, bays in layouts.values() for b in bays], recs)
    for slug, d in mounts.items():
        write(S + f"plant-mounts/{slug}.yaml", d)
    for slug, d in recs.items():
        write(S + f"units/{slug}.yaml", d)
    for slug, (d, _) in layouts.items():
        write(S + f"layouts/{slug}.yaml", d)
    print(f"{len(mounts)} mounts, {len(recs)} units, {len(layouts)} layouts")


if __name__ == "__main__":
    main()
