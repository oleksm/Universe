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
        name=f"Cooling tower, {cells} cells", size_class=3, fits="bay-s3", dims=(12.0 * cells, 12.0, 14.0),
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


def bridge_crane_unit(cap, span, trolley, kind="Ladle crane", job="it lifts and carries a full ladle: off its car into the degasser and back, or over the moulds to pour", gear="a main hoist and a ladle beam"):
    """A double-girder bridge crane: the lightest pair of box girders that keeps the load's deflection within span/800 and
    their bending stress, all loads on, within 150 MPa (0.6 of A36's yield)."""
    best = None
    for h in [x / 10 for x in range(14, 31)]:
        for t in (0.012, 0.016, 0.020, 0.025):
            b, g_len = 0.8, span + 0.6
            I = (b * h ** 3 - (b - 2 * t) * (h - 2 * t) ** 3) / 12
            m = 2 * box_tube(b, h, t, g_len) * RHO["steel"]
            defl = cap * G * span ** 3 / (48 * 205e9 * 2 * I)
            M = (cap + trolley) * G * span / 4 + m / g_len * G * span ** 2 / 8
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
    stand = 6 * box_tube(0.3, 0.3, 0.02, 9.0) * RHO["steel"] + 12 * 6 * 0.02 * RHO["steel"]
    out["alloy-bins-8"] = dict(
        name="Alloy bins, 8", size_class=2, fits="bay-s2", dims=(12.0, 6.0, 15.0),
        desc="Eight steel bins of ferroalloys and fluxes on a stand 9 m up, each over a vibrating feeder into one weigh hopper; the hopper's outlet at local (0, -2.5, 10) drops onto the layout's belt to the ladle furnace's chute.",
        built_of=[part("Bins", A36, bins, f"8 bins {Db} m across, {Hc} m of cylinder over a {Hk} m cone, 6 mm plate"),
                  part("Stand", "stock.st4340-box-300x300x20", stand, "six box legs 300 x 300 x 20 mm, 9 m, and a 12 x 6 m deck of 20 mm plate"),
                  part("Feeders and weigh hopper", "good.electric-motors", 4000.0, "from memory")],
        holds=8 * vol1 * 3500 * 0.8, needs={"power": 40000.0},
        basis=[{"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"Eight bins of {vol1:.1f} m3 at about 3,500 kg/m3 of ferroalloy lumps (from memory), four fifths full."}])
    out["baghouse-200k"] = baghouse_unit(200000, (12.0, 8.0, 12.0))
    out["ladle-crane-150t"] = bridge_crane_unit(150000.0, 22.8, 37500.0)
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
        {"id": "A", "mount": "plant-mount.bay-s2", "unit": "unit.alloy-bins-8", "at": [-9.0, 21.0, 0.0], "note": "outside, north"},
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
        link("conveyor", "alloy feed", (-9.0, 18.5, 10.0), (-9.0, 3.0, 9.4), A36, 120.0, 20000.0, "0.65 m belt in a covered gallery, 120 kg/m with the gallery (from memory), falling gently from the weigh hopper to 0.4 m over the ladle furnace's chute, through the north wall"),
        link("duct", "fume", (-7.0, 0.0, 7.5), (9.0, 20.0, 10.0), A36, math.pi * 1.0 * 0.006 * RHO["steel"], None, "1 m across, 6 mm plate: up from the ladle furnace's roof elbow, through the north wall's opening, to the baghouse's inlet; the roof's fume is cooler than an arc furnace's, so the duct is not water-cooled", via=[(-7.0, 0.0, 12.0), (0.0, 6.0, 12.0), (0.0, 17.0, 12.0), (9.0, 17.0, 10.0)]),
        link("pipe", "cooling water to the ladle furnace", (tx, -9.0, 0.75), (-13.5, -4.5, 1.0), A36, pipe_m, None, "a pair of 250 mm pipes, 6 mm wall, flow and return, from the tower's first port pair: up to 1.5 m, east through a sleeve in the west wall at y -9, north west of the vault to the ladle furnace's manifold", via=[(tx + 1.5, -9.0, 0.75), (tx + 1.5, -9.0, 1.5), (-16.0, -9.0, 1.5), (-16.0, -4.5, 1.5)]),
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
        {"what": "Bins footing", "item": CONC, "volume": 72.0, "note": "12 x 6 x 1 m"},
        {"what": "Baghouse slab", "item": CONC, "volume": r(12 * 8 * 0.6)},
        {"what": "Cooling tower basin", "item": CONC, "volume": r((tw + 0.6) * 12.6 * 0.5 + 2 * (tw + 0.3 + 12.3) * 2.0 * 0.3 + 5 * 0.6 * 0.6 * 2.0), "note": f"sunk as the furnace shop's: 0.3 m walls round its {tw:.0f} x 12 m footprint, tops at z 0, floor top at z -2; five pedestals"},
        {"what": "Rail beds", "item": CONC, "volume": r(48 * 0.125), "note": "under the ladle rail, 0.125 m3 a metre as the furnace shop's"},
    ]
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


def casting_units():
    """The casting bay's own units."""
    out = {}
    wall = 0.12
    mould = ((ING_A + 2 * wall) ** 2 - ING_A ** 2) * ING_L * 1.15 * 7200          # a big-end-up mould 1.15 ingots tall, its wall 120 mm
    stool = (ING_A + 2 * wall + 0.08) ** 2 * 0.15 * 7200
    frame_ = (2 * box_tube(0.4, 0.4, 0.025, 6.0) + 6.0 * 2.8 * 0.04) * RHO["steel"]
    out["mould-car-12"] = dict(
        name="Ingot mould car, 12 moulds", size_class=1, dims=(6.0, 2.8, 2.3),
        desc=f"A self-propelled rail car carrying twelve cast iron ingot moulds in two rows of six at 0.85 m, each on its stool, big end up: each takes {ING_M + HOT_TOP:,.0f} kg of steel, a {ING_L:g} m ingot tapering from {ING_A:g} to {ING_B:g} m square with its hot top; the ingot fits one to a heavy crate. The ladle is held over each mould in turn by the crane and teemed from its slide-gate nozzle; the train stands while the ingots freeze, then the stripper crane lifts the moulds off. It travels along its x on 2 m gauge, its origin on the running plane; the moulds' tops 2.3 m up.",
        built_of=[part("Moulds", CAST_IRON, PER_CAR * mould, f"{PER_CAR} moulds, {wall * 1000:.0f} mm walls, {ING_L * 1.15:.2f} m tall: {mould:,.0f} kg each"),
                  part("Stools", CAST_IRON, PER_CAR * stool, f"{PER_CAR} base plates 0.15 m thick: {stool:,.0f} kg each"),
                  part("Car frame and deck", "stock.st4340-box-400x400x25", frame_, "two box beams 400 x 400 x 25 mm, 6 m, and a deck 6 x 2.8 m of 40 mm plate"),
                  part("Bogies and drive", "good.electric-motors", 3000.0, "from memory")],
        holds=PER_CAR * (ING_M + HOT_TOP), needs={"power": 30000.0},
        basis=[{"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"The ingot sized to the heavy crate (2 t); its hot top the module's scrap share. Moulds about their ingot's mass and more, as practice runs (from memory). Car and drive from memory."}])
    out["stripper-crane-20t"] = bridge_crane_unit(20000.0, 22.8, 6000.0, kind="Stripper crane", job="it lifts the moulds off the frozen ingots, a ram in its tongs pushing a sticking ingot down, and sets the ingots on the ingot car and the moulds back on their cars", gear="stripping tongs and a ram")
    deck = 8.0 * 3.0 * 0.04 * RHO["steel"]
    frame2 = 2 * box_tube(0.4, 0.4, 0.025, 8.0) * RHO["steel"]
    out["ingot-car-40t"] = dict(
        name="Ingot car, 40 t", size_class=2, dims=(8.0, 3.0, 1.2),
        desc="A self-propelled flat rail car that takes the stripped ingots, still hot, to the ingot yard: 25 ingots in rows on its steel deck. It travels along its x on 2 m gauge, its origin on the running plane; the deck 1.2 m up.",
        built_of=[part("Car frame and deck", "stock.st4340-box-400x400x25", frame2 + deck, "two box beams 400 x 400 x 25 mm, 8 m, and a deck 8 x 3 m of 40 mm plate"),
                  part("Bogies and drive", "good.electric-motors", 4000.0, "from memory")],
        holds=40000.0, needs={"power": 40000.0},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "From the dimensions; bogies and drive from memory."}])
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
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in doors)
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
        {"what": "Teeming pit", "item": CONC, "volume": r(30 * 6 * 0.3), "note": "x -30 to 0, y -9 to -3: a 0.3 m deeper lining under the teeming, to catch a spill"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
    ]
    for g in ground:
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
                     "description": f"Ingot teeming under a crane hall: the heat comes in on its ladle car from the ladle station; the 150 t crane holds the ladle over a train of {CARS} mould cars and teems {MOULDS} small ingots, one to a heavy crate; the train stands while they freeze, the 20 t stripper crane lifts the moulds off and loads the ingots on a car to the ingot yard. For a world with air at about 1 g."},
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
                                         {"of": ["bears", "feeds"], "rule": "rule.mounts-margin", "note": f"Round the units fitted to it in the trial ({', '.join(v['units'])}) at 1 g, full."}]}
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
    recs = {slug: unit_record(slug, u) for slug, u in us.items()}
    layouts = {"arc-furnace-air": arc_furnace_air(us, recs), "ladle-station-air": ladle_station_air(us, recs), "casting-bay-air": casting_bay_air(us, recs)}
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
