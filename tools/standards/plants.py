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
import plant_kit as kit  # noqa: E402
ENVELOPES = yaml.safe_load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "unit_envelopes.yaml"))) or {}

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


def baghouse_unit(q_m3h, dims, stack_over=None, stack_kg=None):
    """A pulse-jet baghouse for q_m3h of fume, in a casing of dims; with stack_over (m, the tallest building near it), its own
    stack to good-engineering-practice height, that building's height and half again, so the plume clears its wake."""
    Q = q_m3h / 3600
    cloth = Q * 60 / 1.2
    bags = math.ceil(cloth / (math.pi * 0.15 * 8.0))
    L, W, H = dims
    casing = 2 * (L * W + L * H + W * H) * 0.008 * RHO["steel"]
    fan = Q * 4000 / 0.75
    stack_h = float(math.ceil(2.5 * stack_over)) if stack_over else None
    d_st = math.ceil(math.sqrt(4 * Q / 20.0 / math.pi) * 10) / 10
    return dict(
        name=f"Fume baghouse, {q_m3h:,.0f} m3/h", size_class=2 if max(L, W) <= 12 else 3, fits="bay-s2" if max(L, W) <= 12 else "bay-s3", dims=(L, W, stack_h) if stack_h else dims,
        desc=f"Cleans a furnace's fume before it goes to the air: filter bags in a closed steel casing over hoppers, the dust shaken down and let out through sealed valves; a fan and its stack at one end draw the gas through. The gas comes in at the top of one long side (local 0, -{W / 2:g}, {H - 2:g})." + (f" Its stack, {d_st:g} m across and {stack_h:g} m tall, stands at the fan's end (centre local ({L / 2 - 3:g}, 0)): {stack_over:g} m of building near it and half again, so the plume clears the building's wake; steel, 12 mm at its foot tapering to 8 mm, on its own foundation." if stack_h else ""),
        built_of=[part("Casing and hoppers", A36, casing, f"{L:g} x {W:g} x {H:g} m, 8 mm plate"),
                  part("Structure", A36, casing * 0.5, "half the casing, from memory"),
                  part("Filter bags", "material.ptfe", cloth * 0.5, f"{bags} bags 150 mm across and 8 m long, {cloth:.0f} m2 of PTFE-membrane cloth at 0.5 kg/m2"),
                  part("Fan and motor", "good.electric-motors", 8000.0 * q_m3h / 400000, "an induced-draught fan, 8 t for 400,000 m3/h, in proportion; from memory")]
                 + ([part("Stack", A36, stack_kg or math.pi * d_st * 0.010 * stack_h * RHO["steel"] * 1.1, (f"the tapered shell (12 to 8 mm) with its side entry, rim, base ring, closure and eight anchor chairs; measured on model v1" if stack_kg else f"{d_st:g} m across, {stack_h:g} m, 10 mm on average, a tenth more for its base ring, ladder and platforms") + "; its wind loads not yet checked")]
                    + ([part("Stack ladder and platforms", A36, 2000.0, "a caged ladder and two rest platforms, from memory")] if stack_kg else []) if stack_h else []),
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
    head = {"plant-mounts": "plant-mount", "units": "unit", "layouts": "layout", "connectors": "connector", "runs": "run"}[path.split("/")[-2]]
    with open(path, "w", encoding="utf-8") as f:
        f.write(f"# yaml-language-server: $schema=../../schema/{head}.schema.yaml\n# Written by tools/standards/plants.py: change it there.\n")
        yaml.safe_dump(d, f, sort_keys=False, width=1000, allow_unicode=True, default_flow_style=None)


def cover_model(path, d):
    """A layout's box grows to cover its installed model's measured size (above grade): the model holds what the record
    has no shape for (basins, aprons, walkways, eaves)."""
    if not os.path.exists(path):
        return
    v = (yaml.safe_load(open(path)) or {}).get("visual")
    man = os.path.join(os.environ.get("UNIVERSE_ASSETS") or os.path.expanduser("~/git/freefall-assets"), v["path"], "manifest.json") if v else None
    if not man or not os.path.exists(man):
        return
    import json
    (x0, _, z0), (x1, y1, z1) = json.load(open(man))["bounds_m"]
    ph = d["physical"]
    ph["length"], ph["width"], ph["height"] = max(ph["length"], r(x1 - x0)), max(ph["width"], r(z1 - z0)), max(ph["height"], r(y1))


def unit_dims(recs, bay):
    """A bay's unit's length and width: the layout's box is what is fitted, not the mounts' parcels."""
    ph = recs[bay["unit"].split(".", 1)[1]]["physical"]
    return ph["length"], ph["width"]


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
    if u.get("ports"):
        d["ports"] = u["ports"]
    if f"unit.{slug}" in ENVELOPES:
        env = ENVELOPES[f"unit.{slug}"]
        if [e for e in env if isinstance(e, list)]:
            d["envelope"] = [e for e in env if isinstance(e, list)]
        if [e for e in env if isinstance(e, dict)]:
            d["shells"] = [{"kind": k, "at": v} for e in env if isinstance(e, dict) for k, v in e.items()]
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
        link("pipe", "cooling water to the transformer", (-19.5, -6.0, 1.5), (-16.0, -6.0, 1.0), A36, tr_pipe_m, None, f"a pair of 150 mm pipes, 5 mm wall, teed off the hall pair, through the vault's west wall to the transformer's cooler ports (its -x end, 1 m up): {TRANSFORMER_LOSS * S_MVA / 1e3:.0f} kW at a 10 K rise is {TRANSFORMER_LOSS * S_MVA / (4186 * 10):.0f} kg/s; the returns run 0.45 m up under the flows at 0.75, 500 mm reducers at the tower's ports", via=[(-16.5, -6.0, 1.5)]),
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

    xs = [x0, x1] + [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [y0, y1] + [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
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
    xs = [x0, x1] + [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [y0, y1] + [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
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
    xs = [x0, x1] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
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
    # Its cable boxes (SFO 25: a power port is the gland plate of the unit's cable box; the phases land inside it).
    ht["built_of"] = ht["built_of"] + [part("HV cable box", A36, 980.7, "measured on blender's model v2; over the three HV bushings on top (their ends at x -2.36, 0 and 2.36, y -0.96, z 6.75): 5.8 x 0.9 x 0.8 m, 6 mm, its gland plate on top at (0, -0.96, 7.55)"),
                                       part("LV cable box", A36, 824.3, "measured on blender's model v2; over the three secondary pads on the +y face, 4.5 m up: 5 x 0.5 x 1.2 m, 6 mm, its gland plate facing +y at (0, 3.35, 4.5)"),
                                       part("Cable boxes' brackets", A36, 381.6, "on the tank and its lid, so the boxes do not bear on the conductors; measured on blender's model v2")]
    ht["dims"] = (ht["dims"][0], max(ht["dims"][1], 2 * 3.35), max(ht["dims"][2], 7.6))       # (the LV cable box's face, kept about the centre; the HV box's top)
    ht["desc"] += " Cable boxes over its HV bushings (gland plate on top) and over its secondary pads (gland plate facing +y), so each three-phase circuit enters at one plate."
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
                  part("Headers and branches", A36, 42556.7, "the top gas header, the 8.2 m wet, clean and warm headers, the warm outlet's riser with its tee-back plate, and the branches, their outlet stubs reduced to the 8 bar loop's ports (SFO 25) and ending plain; measured on blender's model v2"),
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
        link("pipe", "cooling water to the heater transformer", (60.0, -12.0, 0.75), (48.0 - recs[htr]["physical"]["length"] / 2, -24.0, 1.0), A36, 2 * math.pi * 0.15 * 0.005 * RHO["steel"], None, f"a pair of 150 mm pipes, 5 mm wall, flow and return: {recs[htr]['needs']['heat'] / (4186 * 10):.0f} kg/s at a 10 K rise for its oil-to-water coolers, off the mains' end, west in a trench to x 42, south, in under the vault's wall to the water ports on its -x end, 1 m up, at y -24.6 and -23.4 (the pair's centre here); in a trench 0.8 m wide, its floor at z -1.3, the pipes' centres at z -0.6 and -0.9 under the mains and crossings, rising at x 42 through bored openings in the vault's wall (the return's west leg and riser at x 41.45, the flow's at 41.7, so they do not cross)", via=[(60.0, -12.0, -0.75), (42.0, -12.0, -0.75), (42.0, -24.0, -0.75), (42.0, -24.0, 1.0)]),
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
    xs = [x0, x1] + [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [y0, y1] + [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
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
RT_V = 33e3                        # V: the rectifier transformers' primary, chosen
V_CELL, J_CELL = 1.9, 4000.0       # V a cell and A/m2 of active area: from memory of pressurised alkaline electrolysers
V_TN = 1.48                        # V: the thermoneutral voltage; what a cell takes above it leaves as heat
CELL_A, CELLS_STACK, STACKS = 2.7, None, 4   # m2 of active area a cell (1.85 m across): from memory
I_BLK = H_RATE / N_BLK / 2.016e-3 * 2 * FARADAY             # A summed over every cell of one block (ampere-cells), all of it making gas (a Faraday efficiency of 1)
P_STACKS = I_BLK * V_CELL                                    # W into one block's cells
N_CELLS = math.ceil(I_BLK / J_CELL / CELL_A)
PER_STACK = math.ceil(N_CELLS / STACKS)
V_STACK = PER_STACK * V_CELL                                 # V across a stack, and so on the block's bus
I_DC = I_BLK / PER_STACK                                     # A of direct current into a block: its stacks in parallel, each its cells in series
NI = "material.nickel-200"


def hydrogen_purifier(name, P, T_in, T_cool, cop, cool_txt, measured=None):
    """A deoxidiser and two dryers for the plant's hydrogen at P (Pa): saturated at T_in from the separators, cooled by water to T_cool and
    chilled to 5 deg C (by a chiller of `cop`) before the beds."""
    P_d = 1.2 * P + 2e5                                          # design pressure: a fifth over working and 2 bar for the relief valve's margin
    n_h2 = mol_flow(H_RATE)
    q3 = n_h2 * 8.314 * (T_cool + 273.15) / P                   # m3/s at the plant's pressure, after the coolers
    v_bed = 0.5 * math.sqrt(3e5 / P)                             # m/s through the bed: 0.5 at 3 bar, less as the gas gets denser, so the beds stay seated
    D_dry = math.ceil(math.sqrt(4 * q3 / v_bed / math.pi) * 10) / 10
    pv = lambda T: 610.94 * math.exp(17.625 * T / (T + 243.04))     # Pa of water over water (Magnus)
    O2_IN = 0.005                                                  # oxygen in the separators' hydrogen, mol fraction: from memory of alkaline plants
    n_o2 = n_h2 * O2_IN
    dT_deoxo = n_o2 * 484e3 / (n_h2 * 29.0)                      # 2 H2 + O2 to steam, 484 kJ a mol of oxygen
    n_w40 = n_h2 * pv(T_in) / P + 2 * n_o2                       # saturated as it leaves the separators, and what the deoxidiser makes
    n_w35, n_w5 = n_h2 * pv(T_cool) / P, n_h2 * pv(5) / P
    q_after = n_h2 * 29.0 * (T_in + dT_deoxo - T_cool) + (n_w40 - n_w35) * 18.015e-3 * 2.43e6
    q_chill = n_h2 * 29.0 * (T_cool - 5) + (n_w35 - n_w5) * 18.015e-3 * 2.47e6
    w_cycle = n_w5 * 18.015e-3 * 8 * 3600                        # kg of water a bed takes in an 8 hour turn
    sieve = w_cycle / 0.10                                       # at 10% by weight, a working capacity from memory
    bed_h = sieve / 700 / (math.pi * D_dry ** 2 / 4) + 1.0      # 700 kg/m3, and a metre for the transfer zone
    H_dry = math.ceil(bed_h + 1.5)
    t_dry = math.ceil((P_d * D_dry / 2 / (138e6 * 0.85 - 0.6 * P_d) + 0.0015) * 1000) / 1000
    regen = sieve * 900 * 230 + w_cycle * 2.6e6 + cyl_shell(D_dry, H_dry, t_dry, bottom=2.2) * 7900 * 500 * 230
    p_heat = regen / (4 * 3600) / 0.85                           # W over 4 hours of the 8 hour turn, 85% reaching the bed
    p_chill = q_chill / cop
    q_cw = q_after + q_chill + p_chill + p_heat * 0.5            # to the cooling water: the after-cooler, the chiller's condenser, the regeneration cooler (averaged)
    cat_v = n_h2 * 22.414e-3 * 3600 / 7000                       # m3 of catalyst at 7,000 an hour of gas at normal conditions
    D_deo = 1.6
    t_deo = math.ceil((P_d * D_deo / 2 / (138e6 * 0.85 - 0.6 * P_d) + 0.0015) * 1000) / 1000
    vessels = (2 * cyl_shell(D_dry, H_dry, t_dry, bottom=2.2) + cyl_shell(D_deo, 3.5, t_deo, bottom=2.2)
               + cyl_shell(1.0, 2.5, 0.008, bottom=2.2) + cyl_shell(0.6, 2.0, 0.006, bottom=2.2) + cyl_shell(0.5, 2.0, 0.006, bottom=2.2)) * 7900
    d_cwp = math.sqrt(4 * q_cw / (4186 * 10) / 1000 / 2.0 / math.pi)
    u = dict(
        name=name, size_class=2, fits="bay-s2", dims=(12.0, 8.0, 9.0),
        desc=(f"Takes the last oxygen out of the electrolysers' hydrogen and dries it to a dew point near -60 deg C, at {P / 1e5:.0f} bar (rated {P_d / 1e5:.0f}). {cool_txt} In order: "
              f"a feed/effluent exchanger (0.9 m across, 3.5 m long, horizontal along x at y 0, z 6, x -5.6 to -2.1, on a stand) warms the wet gas a little against the hot outlet; "
              f"the deoxidiser ({D_deo:g} m across, 3.5 m tall, on 1 m legs, centre (-4, -2)) burns the {O2_IN:.1%} of oxygen to water over {cat_v:.1f} m3 of palladium catalyst, {dT_deoxo:.0f} K warmer; "
              f"a water-cooled after-cooler (0.8 m across, 3.2 m long, along x at y 0.8, z 1.2, x -3.6 to -0.4) to {T_cool:.0f} deg C and a knockout drum (1 m across, 2.5 m, centre (0.4, 0.8)); "
              f"a chiller package (x -2 to 1.6, y -3.8 to -1.4, 2.2 m tall) to 5 deg C with its cold separator; "
              f"two dryers ({D_dry:g} m across, {H_dry:g} m straight with dished heads, {t_dry * 1000:.0f} mm wall, on skirts, centres (4, -2.2) and (4, 2.2)), one adsorbing for 8 hours while the other is heated 4 hours by a slip of dry gas and cooled; "
              f"a regeneration heater cabinet (x -1.5 to 1.5, y 2.4 to 3.6, 2.2 m tall), its cooler (0.5 m across, 2 m, along x on the cabinet's roof) and a knockout pot (0.6 m across, 2 m, centre (2, 3.4)); a dust filter (0.5 m, 2 m, centre (5.5, 0)) before the outlet. "
              f"Gas in at local (-6, 0, 6), out at (6, 0, 2); the cooling water's pair of {d_cwp * 1000:.0f} mm flanges on the -y face at (0, -4, 1) flow and (0.6, -4, 1) return ({q_cw / (4186 * 10):.0f} kg/s at a 10 K rise); "
              f"condensate 50 mm out at (-6, 3, 0.3); 400 V supply at (6, -3.5, 0.5)."),
        built_of=[part("Vessels", "material.stainless-steel-304", vessels, f"two dryers {D_dry:g} m by {H_dry:g} m in {t_dry * 1000:.0f} mm plate, the deoxidiser {D_deo:g} m by 3.5 m in {t_deo * 1000:.0f} mm, the drums and the filter, heads as 1.1 discs each"),
                  part("Exchangers", "material.stainless-steel-304", 6000.0, "feed/effluent exchanger, after-cooler and regeneration cooler, from memory"),
                  part("Chiller package", "good.pumps", 4500.0, f"a {q_chill / 1e3:.0f} kW screw chiller on its frame, from memory"),
                  part("Regeneration heater", "material.stainless-steel-304", 1500.0, f"{p_heat / 1e3:.0f} kW of electric elements in a cabinet, from memory"),
                  part("Adsorbent", "material.fused-silica", 2 * sieve, f"{sieve / 1e3:.1f} t of 4A molecular sieve a bed (the water of an 8 hour turn at 10%), which the registry has not got: fused silica stands for it"),
                  part("Catalyst", "material.fused-silica", cat_v * 800, "palladium on alumina pellets at 800 kg/m3, 0.5% palladium, which the registry has not got: fused silica stands for it"),
                  part("Skid, supports and piping", A36, 12000.0, "from memory")],
        needs={"power": p_heat + p_chill + 20e3, "heat": q_cw},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "derived", "review": True,
                "note": (f"From the plant's {n_h2:.0f} mol/s of hydrogen: the dryers' diameter at {v_bed:.2f} m/s at {P / 1e5:.0f} bar, their walls for {P_d / 1e5:.0f} bar in 304 at 138 MPa (85% joints, 1.5 mm allowance), the sieve from the water left at 5 deg C over 8 hours, "
                         f"regeneration warming sieve and shell 230 K and driving the water off in 4 hours ({p_heat / 1e3:.0f} kW), the chiller {q_chill / 1e3:.0f} kW at a COP of {cop:g}; oxygen content, capacities and the catalyst's space velocity from memory.")}])
    for what, m in (measured or {}).items():                   # groups measured on the model: replace or add
        u["built_of"] = [x for x in u["built_of"] if x["what"] != what]
        if isinstance(m, float):
            u["built_of"].insert(0, part(what, "material.stainless-steel-304", m, "the dryers, deoxidiser, drums and filter shells with their heads and nozzles; measured on model v1"))
        elif m:
            u["built_of"].append(part(what, m[0], m[1], m[2] + "; measured on model v1"))
    return u


def water_treatment(name, airless=False, measured=None):
    """Demineralised water for the cells: two sand filters, a cartridge filter, a two-stage reverse osmosis bank and an
    electrodeionisation stage. For air under an open roof; airless in a closed, insulated house with every tank sealed."""
    q_p = H_WATER * 3.6                                          # m3/h of product
    rec = 0.85 if airless else 0.75                              # the bank's recovery: condensate and ice melt are cleaner than river water
    perm = q_p / 0.9                                             # the EDI keeps 90%; its reject goes back to the bank's feed
    feed = perm / rec
    raw = feed - (perm - q_p)
    conc = feed - perm
    A_f = raw / 10.0                                             # m2 of sand at 10 m/h
    D_f = math.ceil(math.sqrt(4 * A_f / math.pi) * 10) / 10
    t_f = max(0.006, math.ceil((7e5 * D_f / 2 / (138e6 * 0.85 - 0.6 * 7e5) + 0.0015) * 1000) / 1000)
    n_el = math.ceil(perm / (0.020 * 37.2))                     # 8 inch elements of 37.2 m2 at 20 L/m2/h
    n_v = math.ceil(n_el / 6)
    n_v1 = math.ceil(n_v * 0.6)
    n_el = n_v * 6
    p_ro = 10e5 if airless else 12e5
    t_pv = math.ceil((1.5 * p_ro * 0.21 / 2 / (138e6 * 0.85) + 0.0015) * 1000) / 1000
    n_edi = math.ceil(perm / 3.6)                                # modules of 3.6 m3/h
    V_bt = perm / 4                                              # 15 minutes of permeate
    H_bt = math.ceil(V_bt / (math.pi * 2.0 ** 2 / 4) * 10) / 10
    pumps = {"feed booster": raw / 3600 * 3e5 / 0.7, "high-pressure": feed / 3600 * p_ro / 0.75, "product": q_p / 3600 * 4e5 / 0.7}
    p_bw = 40 * A_f / 3600 * 2.5e5 / 0.7                         # backwash at 40 m/h, ten minutes a filter a day
    p_edi = 0.4e3 * q_p                                          # W: 0.4 kWh a m3
    p_heat = 10e3 if airless else 0.0                            # trace heating for the lunar night, from memory
    power = sum(pumps.values()) + p_edi + 5e3 + p_heat
    pump_m = sum(100 + 30 * w / 1e3 for w in list(pumps.values()) + [p_bw]) + 2 * 150 + 300   # 30 kg/kW and 100 kg a set; the CIP pump, the dosing skid
    where = ("in a closed house of insulated sandwich panels under multi-layer insulation, unpressurised (the crews work in suits), every tank sealed under a nitrogen blanket with no vent to the vacuum, trace heaters for the night"
             if airless else "on a skid under an open-sided roof on six columns, its eaves 4.5 m up")
    feed_txt = "the shaft furnace's condensate and melted ice" if airless else "town or river water"
    reject = ("Its reject is kept: back with the raw water's return, beyond the layout, to the ice plant's evaporator."
              if airless else "Its reject goes to drain.")
    u = dict(
        name=name, size_class=2, fits="bay-s2", dims=(12.0, 8.0, 5.0),
        desc=(f"Makes the {H_WATER:.1f} kg/s ({q_p:.1f} m3/h) of demineralised water the cells split, below 1 uS/cm, from {raw:.1f} m3/h of {feed_txt}, {where}. A service aisle along x at y -1.2 to 1.2. In order: "
              f"two sand filters east ({D_f:g} m across, 2.5 m straight with dished heads, {t_f * 1000:.0f} mm plate rubber-lined, on 0.5 m legs, centres (4.6, -2.2) and (4.6, 2.2)), one backwashing while the other runs; "
              f"a cartridge filter (0.4 m across, 1.5 m, at (2.6, 1.6)) and the antiscalant dosing; the reverse osmosis bank north, {n_v} pressure tubes 0.21 m across and 6.7 m long in stainless, horizontal along x from x -3.6 to 3.1, "
              f"in a rack y 1.8 to 3.6, z 0.4 to 2.6 ({n_v1} first stage, {n_v - n_v1} second, {n_el} elements, {p_ro / 1e5:.0f} bar, {rec:.0%} recovery); "
              f"a closed cabinet west for the electrodeionisation stage (x -5.8 to -4.2, y -2.2 to 2.2, 2.2 m tall: {n_edi} modules and their DC supplies); "
              f"the pump skid south (x -3.6 to 3.4, y -3.8 to -1.6): the feed booster, high-pressure, product, backwash and cleaning pumps, the cleaning tank (1 m3) and the permeate break tank (2 m across, {H_bt:g} m tall, at (-2.4, -2.7)). "
              f"Raw water in at local {'(6, -1.6, 0.5), beside the crew door at the aisle\'s east end (y -0.6 to 0.6, at grade)' if airless else '(6, 0, 0.5)'}, clean water out at (-6, 0, 0.5); reject and backwash out by a 150 mm flange at (6, 3.4, 0.3) ({conc:.1f} m3/h of concentrate, {40 * A_f:.0f} m3/h while a filter backwashes). {reject} 400 V supply at (-6, -3.4, 0.5)."),
        built_of=[part("Sand filters", A36, 2 * cyl_shell(D_f, 2.5, t_f, bottom=2.2) * RHO["steel"] + 2 * 150, f"two shells {D_f:g} m by 2.5 m in {t_f * 1000:.0f} mm for 7 bar, heads as 1.1 discs, their legs and nozzle plates"),
                  part("Filter sand", "good.silica-sand", 2 * 1.2 * math.pi * D_f ** 2 / 4 * 1600, "1.2 m of graded sand and gravel in each, 1,600 kg/m3"),
                  part("Pressure tubes", "material.stainless-steel-304", n_v * (cyl_shell(0.21, 6.7, t_pv) * 7900 + 30), f"{n_v} tubes 0.21 m by 6.7 m in {t_pv * 1000:.0f} mm for {1.5 * p_ro / 1e5:.0f} bar, 30 kg of end caps each"),
                  part("Membrane elements", "material.polyethylene-uhmw", n_el * 16, f"{n_el} spiral-wound polyamide elements, 16 kg each, which the registry has not got: polyethylene stands for them"),
                  part("Electrodeionisation", A36, n_edi * 180 + 1200, f"{n_edi} modules of ion-exchange membranes and resin between steel end plates, 180 kg each, and the cabinet with its DC supplies, from memory"),
                  part("Tanks and cartridge filter", "material.stainless-steel-304", cyl_shell(2.0, H_bt, 0.004, bottom=2.2) * 7900 + 350, f"the break tank 2 m by {H_bt:g} m in 4 mm, the cleaning tank and the cartridge housing"),
                  part("Pumps and dosing", "good.pumps", pump_m, "five pump sets at 30 kg/kW and 100 kg each, the dosing skid, from memory"),
                  part("Piping and valves", "material.stainless-steel-304", 1500.0, "from memory"),
                  part("Skid deck and rack", A36, 96 * 40 + 1500, "a beam deck under the bay at 40 kg/m2 and the tube rack, from memory")]
                 + ([part("House frame", A36, 5008.8, "200 x 6 mm hollow mains and 120 x 5 mm posts; measured on blender's model v1 (2026-10-16)"), part("House panels", "material.honeycomb-sandwich-panel", 287.93 * 0.06 * 150, "287.9 m2 of walls and roof (measured), 60 mm panels"),
                     part("Insulation", "material.multi-layer-insulation", 287.93 * 0.02 * 50, "20 mm of blanket over the house, 287.9 m2"),
                     part("Stubs, seats and roof edges", "material.stainless-steel-304", 40.1 + 22.3, "the ports' plain stubs and their seats, the folded 1 mm roof edges; measured")]
                    if airless else [part("Roof and columns", A36, 6 * 5 * 60 + 96 * 20, "six columns at 60 kg/m and 96 m2 of roof at 20 kg/m2 with its purlins, from memory")]),
        needs=dict({"power": power}, **({"atmosphere": "any"} if airless else {})),
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "derived", "review": True,
                "note": (f"From the cells' {q_p:.1f} m3/h: the EDI keeps 90%, the bank {rec:.0%} at 20 L/m2/h on 37.2 m2 elements, six to a tube; the sand filters at 10 m/h, their shells for 7 bar in A36 at 138 MPa (85% joints, 1.5 mm allowance); "
                         f"pumps at 70-75% ({', '.join(f'{k} {v / 1e3:.1f} kW' for k, v in pumps.items())}), the EDI at 0.4 kWh/m3, 5 kW of controls; backwash {p_bw / 1e3:.0f} kW ten minutes a day, not counted. Fluxes, recoveries and equipment weights from memory.")}])
    for what, m in (measured or {}).items():                   # groups measured on the model: replace or add
        u["built_of"] = [x for x in u["built_of"] if x["what"] != what]
        if m:
            u["built_of"].append(part(what, m[0], m[1], m[2] + "; measured on model v1"))
    return u


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
              f"and back by pumps through coolers. The stacks are in parallel: {I_DC / 1e3:.1f} kA of direct current at {V_stack:.0f} V by a busbar to its terminals at local (0, -6, 3), a branch to each stack's ends ({I_DC / STACKS / 1e3:.1f} kA a stack); hydrogen and oxygen out of the separators' tops (z 8) into the layout's headers; "
              f"on the +y face, 1 m up, round local (11, 6, 1): the cooling loop's pair of 250 mm flanges ({I_BLK * (V_CELL - V_TN) / (4186 * 10):.0f} kg/s at a 10 K rise; flow x 10.6, return x 11.4) and the demineralised water's 50 mm flange at x 9.8."),
        built_of=[part("Bipolar plates", A36, n_cells * plate, f"{n_cells} plates of 3 mm steel, 3.5 m2 each with its rim, nickel-plated"),
                  part("Electrodes and plating", NI, n_cells * elec, "nickel mesh at 1.5 kg/m2 each side, 25 um of plating"),
                  part("Diaphragms", "material.polyethylene-uhmw", n_cells * dia, "porous polysulfone with zirconia, about 0.5 kg/m2, which the registry has not got: polyethylene stands for it"),
                  part("End plates and tie rods", A36, ends, f"two plates 2.4 m across, 200 mm, and 2 t of rods a stack"),
                  part("Separators", A36, seps, "two vessels 2 m across, 6 m, 12 mm plate"),
                  part("Lye", "good.water", lye, "30% potassium hydroxide lye in the cells and half the separators, 1,280 kg/m3, which the registry has not got: water stands for it"),
                  part("Lye pumps and coolers", "good.pumps", 8000.0, "from memory"),
                  part("Skids, legs and stands", A36, 17869.6, "the stacks' skids and legs, the separators' feet, pipe and busbar stands; measured on model v1")],
        needs={"power": P_STACKS, "heat": I_BLK * (V_CELL - V_TN), "water": H_WATER / N_BLK},
        recipes=[dict(HREC, rate=H_RATE / N_BLK, power=P_STACKS)],
        basis=[{"of": ["recipes"], "tier": "derived", "note": f"The module's recipe (module.hydrogen-plant) for one block of {N_BLK}: the hydrogen from Faraday's law: {I_DC / 1e3:.0f} kA through {STACKS} stacks of {PER_STACK} cells in series; the power the cells' share ({P_STACKS * N_BLK / H_P:.0%} of the module's, the rest the rectifiers and plant)."},
               {"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"Cells from the current: {area:,.0f} m2 of active area at {J_CELL / 1e4:g} A/cm2; plate, mesh and diaphragm weights and the stack's layout from memory of pressurised alkaline electrolysers."},
               {"of": ["needs.heat"], "tier": "derived", "note": f"The cells' voltage over the thermoneutral {V_TN} V leaves as heat: {(V_CELL - V_TN) * I_BLK / 1e6:.1f} MW, to the cooling loop."}])
    P_rect = (H_P / N_BLK - P_STACKS) * 0.6                   # of the block's share above its cells: the rectifier's losses; the rest the plant's
    rt = transformer_unit(P_STACKS / 0.95, f"one electrolyser block: a 12-pulse thyristor rectifier in the same skid gives {I_DC / 1e3:.0f} kA at {V_stack:.0f} V direct")
    rt["name"] = f"Rectifier transformer, {P_STACKS / 0.95 / 1e6:.0f} MVA"
    L_t, W_t, H_t = rt["dims"]
    rt["dims"] = (L_t, W_t + 3.0, H_t)                    # the transformer on the -y side, its rectifier house on the +y side, 0.6 m between
    dc_cu = 2 * I_DC / 2e6 * 6.0 * RHO["copper"]          # two poles at 2 A/mm2, 6 m each inside the house
    rt["built_of"] += [part("Rectifier stacks and heat sinks", AL, 3000.0, "two 6-pulse thyristor bridges (12-pulse together) on water-cooled aluminium heat sinks, from memory"),
                       part("DC bus", CU, dc_cu, f"two copper poles inside the house, {I_DC / 2e6 * 1e4:.0f} cm2 each at 2 A/mm2"),
                       part("Rectifier house", A36, 2500.0, "a closed steel house, its cabinets, doors and cooling manifold, from memory"),
                       part("HV cable box", A36, 505.0, f"over the three HV bushings on the tank's top at y -1.5: 3 x 0.8 x 0.8 m, 6 mm, its gland plate on top at (0, -1.5, {H_t + 0.8:g}); measured on blender's model v2 (2026-10-16)"),
                       part("HV box posts", A36, 58.6, "four 80 x 5 mm posts and their plates under the box; measured")]
    rt["dims"] = (L_t, W_t + 3.0, H_t + 0.8)
    rt["needs"] = {"power": P_rect, "heat": P_rect}
    rt["desc"] = (rt["desc"].replace("Three secondary terminals on one long face (+y), 4.5 m up", "Its secondary bushings on its +y face into the rectifier house")
                  .replace("Three secondary terminals on one long face (+y)", "Its secondary bushings on its +y face into the rectifier house")
                  + f" Local frame: the transformer's tank on the -y side (y -{(W_t + 3.0) / 2:g} to {W_t - (W_t + 3.0) / 2:g}); the rectifier in a closed house on the +y side (x -2.1 to 2.1, y 0.6 to {(W_t + 3.0) / 2:g}, 2.8 m tall, three service doors), fed across the gap by a shielded AC bridge, its heat sinks' water joining the transformer cooler's pair, whose ports are on the -x end at (-{L_t / 2:g}, -1.86, 1) and (-{L_t / 2:g}, -1.14, 1); "
                  f"the rectifier's DC terminals on the house's +y face, 2.4 m up, toward the cells: one bus joint centred at (0, {(W_t + 3.0) / 2:g}, 2.4), its two poles' pads at x -0.3 (+) and 0.3 (-). "
                  f"Fed at {RT_V / 1e3:g} kV through a cable box over its HV bushings, its gland plate on top at (0, -1.5, {H_t + 0.8:g}).")
    out["rectifier-transformer-21mva"] = rt
    out["hydrogen-purifier"] = hydrogen_purifier("Hydrogen purifier", 3e5, 40.0, 35.0, 4.0, "Cooled by the tower's water.",
                                                 measured={"Vessels": 7548.5, "Skid, supports and piping": None,
                                                           "Skid, skirts and supports": (A36, 24152.5, "the skid's beams and feet, the dryers' skirts and base rings, the exchangers' stands and saddles, the headers' posts"),
                                                           "Piping and valve bodies": ("material.stainless-steel-304", 3916.0, "the process, regeneration and water pipes and the valves' bodies")})
    out["hydrogen-purifier-10bar"] = hydrogen_purifier("Hydrogen purifier, 10 bar", 10e5, 80.0, 80.0, 2.0,
                                                       f"For airless ground: its loop water comes back from the radiators at about {HP_T_LOOP - 273.15:.0f} deg C, so the after-cooler only takes the gas to 80 deg C and the chiller lifts all the rest to its condenser at about 85 deg C, at half the air plant's COP.",
                                                       measured={"Vessels": 9608.2, "Skid, supports and piping": None,
                                                                 "Skid, skirts and supports": (A36, 23142.7, "the skid's beams and feet, the dryers' skirts and base rings, the exchangers' stands and saddles, the headers' posts"),
                                                                 "Piping and valve bodies": ("material.stainless-steel-304", 3875.7, "the process, regeneration and water pipes and the valves' bodies")})
    out["water-treatment"] = water_treatment("Water treatment", measured={
        "Roof and columns": None, "Skid deck and rack": None,
        "Sand filters": (A36, 2854.0, "the two shells with their heads, seams, legs, feet and isolation valves"),
        "Tanks and cartridge filter": ("material.stainless-steel-304", 757.4, "the break tank, the cleaning tank and the cartridge housing"),
        "Roof, columns and pipe hangers": (A36, 7257.8, "six columns and feet, crossbeams, eaves, purlins, fascias, the roof sheet and the pipe crossbars and hangers under it"),
        "Tube rack": (A36, 842.9, "the reverse osmosis bank's rack"),
        "Pump skid": (A36, 3791.4, "the pump skid's frame, base and the bearers under the pumps' feet; the other groups stand on the pad on their own feet")})
    out["water-treatment-sealed"] = water_treatment("Water treatment, sealed", airless=True)
    out["stack-crane-20t"] = bridge_crane_unit(20000.0, 22.8, 6000.0, kind="Stack crane", job="it lifts an electrolyser stack's end plates and cell packs out for refurbishment and sets in new ones", gear="a main hoist")
    return out


def hydrogen_plant_air(us, recs):
    """The hydrogen plant on a world with air at 1 g: five electrolyser blocks in a vented hall, their rectifiers outside."""
    cell, box = 3.0, (-63.0, -15.0, 0.0, 63.0, 15.0, 18.0)          # ends a cell past the end blocks, whose service parts reach x 59.6
    xs_b = [-48.0, -24.0, 0.0, 24.0, 48.0]
    openings = [(x - 1.5, -15.0, 3.0, x + 1.5, -12.0, 6.0) for x in xs_b]           # the DC busbars, south wall
    openings += [(60.0, -9.0, 6.0, 63.0, 9.0, 12.0),                                 # the hydrogen and oxygen headers, east wall
                 (-3.0, 12.0, 0.0, 3.0, 15.0, 3.0),                                  # the cooling water mains, north wall
                 (60.0, -3.0, 0.0, 63.0, 3.0, 6.0),                                  # a door, east
                 (60.0, 6.0, 0.0, 63.0, 9.0, 3.0)]                                   # the demineralised water, east wall
    vent_cuts = [(box[0] + (i + 0.5) * cell - 0.5, 1.0, box[5] - 0.1, box[0] + (i + 0.5) * cell + 0.5, 2.0, box[5] + 0.1) for i in range(1, round((box[3] - box[0]) / cell) - 1)]   # the roof's top diagonals over the vent, y 0 to 3
    groups, ground_nodes, n_nodes, cells = lattice(box, cell, 5, None, openings, vent_cuts)
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
    clad_area = 9425.56                                                               # m2 measured on blender's layout (candidate v01), the returns round openings and the vent included
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
    d_cw = math.sqrt(4 * flow / 1000 / 3.0 / math.pi)                                 # at 3 m/s, so the pair fits the tower's 500 mm ports
    ss = "material.stainless-steel-304"
    q_r = recs["rectifier-transformer-21mva"]["needs"]["heat"]
    q_p = recs["hydrogen-purifier"]["needs"]["heat"]
    d_rh = math.sqrt(4 * (q_r * N_BLK + q_p) / (4186 * 10) / 1000 / 2.5 / math.pi)
    d_pb = math.sqrt(4 * q_p / (4186 * 10) / 1000 / 2.0 / math.pi)
    d_rb = max(0.08, math.sqrt(4 * q_r / (4186 * 10) / 1000 / 2.5 / math.pi))
    links = [link("busbar", f"DC bus {i + 1}", (x, -24.0 + rW / 2, 2.4), (x, -6.0, 3.0), AL, 2 * bus_a * RHO["aluminium"], None, f"two aluminium bars, {bus_a * 1e4:.0f} cm2 each for {I_DC / 1e3:.0f} kA at 1.5 A/mm2: up to 3.6 m on a post south of the wall, level through the south wall's opening (z 3 to 6), down to the block's terminal", via=[(x, -17.0, 3.6), (x, -7.0, 3.6)]) for i, x in enumerate(xs_b)]
    links += [
        link("pipe", "hydrogen header", (xs_b[0] + 10.5, -2.5, 8.0), (66.0, 0.0, 6.0), ss, pm(d_h2, 0.006), None, f"{d_h2:.2f} m stainless, 6 mm wall, at 3 bar: from each block's hydrogen separator along the hall at z 9, out by the east wall to the purifier", via=[(xs_b[0] + 10.5, -2.5, 9.0), (64.0, -2.5, 9.0), (64.0, 0.0, 6.0)]),
        link("pipe", "hydrogen to the shaft furnace", (78.0, 0.0, 2.0), (110.0, 0.0, 2.0), ss, pm(d_h2, 0.006), None, f"{H_RATE:.3f} kg/s of dry hydrogen at about 3 bar, on to the shaft furnace's make-up, beyond the layout"),
        link("pipe", "oxygen header", (xs_b[0] + 10.5, 2.5, 8.0), (110.0, 6.0, 9.0), ss, pm(d_o2, 0.006), None, f"{d_o2:.2f} m stainless, 6 mm wall: the {H_O2:.1f} kg/s of oxygen, out by the east wall to a vent stack or to buyers (the arc furnace lances it), beyond the layout; clean, oil-free, as oxygen service needs", via=[(xs_b[0] + 10.5, 2.5, 9.0), (58.5, 2.5, 9.0), (58.5, 6.0, 9.0)]),
        link("pipe", "demineralised water", (66.0, -24.0, 0.5), (xs_b[0] + 9.8, 7.5, 0.5), ss, pm(0.1, 0.004), None, f"100 mm stainless, {H_WATER:.1f} kg/s: north along x 64.5, into the hall by the east wall's low opening at y 7.5, west along y 7.5 past every block", via=[(64.5, -24.0, 0.5), (64.5, 7.5, 0.5)]),
        *[link("pipe", f"demineralised water to block {i + 1}", (x + 9.8, 7.5, 0.5), (x + 9.8, 6.0, 1.0), ss, pm(0.05, 0.003), None, "50 mm off the header into the block's flange") for i, x in enumerate(xs_b)],
        link("pipe", "raw water", (110.0, -24.0, 0.5), (78.0, -24.0, 0.5), A36, pm(0.15, 0.005), None, "150 mm: town or river water, or the shaft furnace's condensate, beyond the layout"),
        link("pipe", "reject to drain", (78.0, -20.6, 0.3), (110.0, -20.6, 0.3), A36, pm(0.15, 0.005), None, "150 mm: the reverse osmosis concentrate and the filters' backwash, beyond the layout"),
        link("pipe", "cooling water to the rectifiers", (tower["dims"][0] / 2, 52.5, 0.75), (xs_b[0] - 3.45, -30.0, 0.75), A36, 2 * pm(d_rh, 0.005), None, f"a pair of {d_rh:.2f} m pipes, 5 mm wall, flow and return: {(q_r * N_BLK + q_p) / (4186 * 10):.0f} kg/s for the rectifier transformers' coolers and the purifier, from the tower's ports at y 52.5, east along y 52.5 (north of the mains, which turn south at x 19.5, so the two never cross), south along x 84 east of the purifier and the water treatment (a tee for the purifier at y -6) and west along y -30 behind the water treatment and the rectifiers", via=[(84.0, 52.5, 0.75), (84.0, -30.0, 0.75)]),
        link("pipe", "cooling water to the purifier", (84.0, -6.0, 0.75), (72.3, -4.0, 1.0), A36, 2 * pm(d_pb, 0.004), None, f"a pair of {d_pb * 1000:.0f} mm pipes off the rectifiers' header, west along y -6 and north, level into the purifier's flanges on its -y face (x 72 flow, 72.6 return)", via=[(72.3, -6.0, 0.75), (72.3, -6.0, 1.0)]),
        *[link("pipe", f"cooling water to rectifier {i + 1}", (x - 3.45, -30.0, 0.75), (x - rt["dims"][0] / 2, -25.5, 1.0), A36, 2 * pm(d_rb, 0.004), None, f"a pair of {d_rb * 1000:.0f} mm pipes off the header, north and level into the cooler's ports on the -x end (y -25.86 and -25.14)", via=[(x - 3.45, -25.5, 0.75), (x - 3.45, -25.5, 1.0)]) for i, x in enumerate(xs_b)],
        link("pipe", "cooling water mains", (tower["dims"][0] / 2, 48.0, 0.75), (0.0, 9.0, 1.0), A36, 2 * pm(d_cw, 0.010), None, f"a pair of {d_cw:.2f} m pipes, 10 mm wall, flow and return: {flow:,.0f} kg/s at a 10 K rise, from the tower's ports at y 48, south along x 19.5, west along y 20, in by the north wall's opening at x 0 to the header", via=[(tower["dims"][0] / 2 + 1.5, 48.0, 0.75), (tower["dims"][0] / 2 + 1.5, 20.0, 0.75), (0.0, 20.0, 0.75), (0.0, 12.0, 1.0)]),
        link("pipe", "cooling water header", (xs_b[0] + 11.0, 9.0, 1.0), (xs_b[-1] + 11.0, 9.0, 1.0), A36, 2 * pm(d_cw, 0.010), None, "the mains' pair on along y 9 inside the north wall, west and east of the tee at x 0, past every block's cooler ports", via=[(0.0, 9.0, 1.0)]),
        *[link("pipe", f"cooling water to block {i + 1}", (x + 11.0, 9.0, 1.0), (x + 11.0, 6.0, 1.0), A36, 2 * pm(0.25, 0.006), None, "a pair of 250 mm pipes off the header, south, level into the block's flanges (x 10.6 flow, 11.4 return)") for i, x in enumerate(xs_b)],
    ]
    ground = [
        {"what": "Hall floor", "item": CONC, "volume": r(x_in * span_in * 0.4), "note": f"0.4 m, {x_in:.0f} x {span_in:.0f} m, top at z 0, with a lye-proof coating"},
        {"what": "Bunds", "item": CONC, "volume": r(N_BLK * 2 * (22.4 + 12.8 + 0.4) * 0.2 * 0.3), "note": "a curb 0.2 m thick and 0.3 m high round each whole block (inside it, block local x -10.7 to 11.7, y -6.4 to 6.4), to hold a stack's lye; E5's east curb ends 0.1 m short of the hall's inner wall plane"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes"},
        {"what": "Rectifier slabs and fire walls", "item": CONC, "volume": r(N_BLK * (12 * 8 * 0.5) + (N_BLK + 1) * 8 * 6 * 0.3), "note": "a 12 x 8 x 0.5 m slab under each, its top at z 0 with an oil sump; fire walls 8 m long, 6 m tall, 0.3 m thick along y at x -60, -36, -12, 12, 36 and 60"},
        {"what": "Outside slabs", "item": CONC, "volume": r(2 * 12 * 8 * 0.5), "note": "12 x 8 x 0.5 m under the purifier and the water treatment, tops at z 0, the water treatment's with a drain channel to its reject flange"},
        {"what": "Cooling tower basin", "item": CONC, "volume": r((tower["dims"][0] + 0.6) * 12.6 * 0.5 + 2 * (tower["dims"][0] + 0.3 + 12.3) * 2.0 * 0.3), "note": "sunk as the furnace shop's: a 0.5 m floor 0.6 m wider than the tower each way, walls 0.3 m thick and 2 m deep, the water just under z 0"},
        {"what": "Tower pedestals", "item": CONC, "volume": r(12 * 0.6 * 0.6 * 2.0), "note": "twelve 0.6 m square piers from the basin floor (z -2) to z 0 under the tower's eight feet (x ±17.6 and ±6, y ±5.6) and its skid's four corners"},
    ]
    ground += [{"what": "Support footings", "item": CONC, "volume": 8.62, "note": "feet under the DC bus posts (1.01 m3), the gas header portals (2.87), the oxygen line's supports (0.68), the ground pipes' supports (3.97) and the hydrogen elbow (0.08); measured on blender's layout (candidate v01)"},
               {"what": "Support steel", "item": A36, "volume": r(67276.1 / RHO["steel"]), "note": "the crane runways' seats (25 mm boxes), the gas header portals (200 x 200 x 8 beams on 160 x 8 round posts at y -8 and 8 every 6 m along x, beams about z 8.65), the bus posts and the pipes' small 6 mm boxes, with solid shoes; joints and anchors left out, not checked for strength; measured on blender's layout (candidate v01), 67,276 kg"}]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO["steel"] if g["item"] == A36 else RHO["concrete"]))
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
    xs = [x0, x1] + [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [y0, y1] + [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    site_area = (max(xs) - min(xs)) * (max(ys) - min(ys))
    against = (f"module.hydrogen-plant: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {H_P / 1e6:.0f} MW. "
               f"This layout: the same {H_RATE * 3600:,.0f} kg of hydrogen an hour; {power / 1e6:.1f} MW (the cells {N_BLK * P_STACKS / 1e6:.1f}, at {V_CELL} V, {N_BLK * P_STACKS / H_RATE / 3.6e6:.1f} kWh a kg); {heat / 1e6:.1f} MW to the tower, {water:.1f} kg/s of water used and evaporated; "
               f"{(mass_units + mass_links + frame_mass + sum(g["mass"] for g in ground if g["item"] == A36)) / 1000:,.0f} t of plant, frame and supports, {sum(g["mass"] for g in ground if g["item"] == CONC) / 1000:,.0f} t of concrete; {site_area / 1e4:.1f} ha, against the source's 0.8 to 1.7 for 100 MW. "
               f"It makes {H_RATE:.3f} kg/s; one shaft furnace at its full rate takes {H2_USED:.2f}, about {H2_USED / H_RATE:.1f} of these.")
    layout = {
        "identity": {"key": "layout.hydrogen-plant-air", "name": "Hydrogen plant, world with air",
                     "description": f"Water split by electricity: {N_BLK} pressurised alkaline electrolyser blocks of {P_STACKS / 1e6:.0f} MW in a hall with a ridge vent (hydrogen rises and must find its way out), each fed by its own rectifier transformer outside; a purifier, water treatment and a cooling tower. The hydrogen goes on to the shaft furnace, the oxygen to a vent or to buyers. For a world with air at about 1 g."},
        "stands_for": "module.hydrogen-plant",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": z1, "mass": r(total)},
        "bays": bays, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 15 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor, open along its ridge so hydrogen cannot gather (a vent 1.5 m wide over y 0.75 to 2.25, between the top chords at y 0 and 3, the top diagonals there left out; only the cross chords every 3 m span it); framed openings in the south wall for the DC busbars, in the east for the gas headers and a door, in the north for the cooling water.",
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
        name=f"Ball mill, {D:g} m", size_class=3, fits="bay-s3", dims=(18.0, 9.0, 6.5),
        desc=(f"Grinds the ore wet, to about 80% finer than 45 um, so it can be balled: a steel drum {D:g} m across and {Lm:g} m long on trunnion bearings, its axis along x 3 m up, a third full of steel balls, turned by a girth gear and a {P_mill / 1e6:.1f} MW motor at its +x end: the gear 6.0 m across (about 1.35 times the drum, so its pinion clears the shell; from memory) on a flange by the +x head, in a guard 6.4 m across that reaches z 6.2 and sits in a 0.4 m pit in the floor below; its pinion on the -y side at axis height, a gearbox and the motor on the floor beyond, out to y -4.4. "
              "Ore and water in by a chute at its -x trunnion (local -8, 0, 4); the slurry out of its +x trunnion into a sump and pumped on (local 8, 3, 0.5)."),
        built_of=[part("Shell and heads", A36, 43468.8, f"{D:g} m across, {Lm:g} m, 45 mm plate: the shell 33,776.9 kg and the annular heads 9,691.8; measured on model v1"),
                  part("Liners", A36, liners, "75 mm wear liners of manganese steel, which the registry has not got: structural steel stands for it"),
                  part("Trunnions and flanges", "material.high-strength-steel-4340", 8703.2, "the hollow trunnions' walls (7,478.8 kg) and their flanges to the heads (1,224.4); measured on model v1"),
                  part("Bearings and girth gear", "material.high-strength-steel-4340", 30000.0, "the two trunnion bearings and the 6 m girth gear with its pinion, from memory"),
                  part("Gear guard", A36, 1778.7, "5 mm plate round the girth gear; measured on model v1"),
                  part("Motor and gearbox", "good.electric-motors", 6.0 * P_mill / 1e3, "6 t a MW, from memory"),
                  part("Slurry sump and pumps", "good.pumps", 8000.0, "from memory")],
        holds=balls, needs={"power": P_mill, "water": P_WATER},
        basis=[{"of": ["physical", "built_of", "holds", "needs.power"], "tier": "derived", "review": True, "note": f"Drum from memory for a mill drawing about {P_mill / 1e6:.1f} MW; masses from its plate; the ball charge a third of its volume at 4,650 kg/m3 (from memory)."},
               {"of": ["needs.water"], "tier": "derived", "note": f"The water the plant loses: the green balls hold {MOIST:.0%}, dried off on the grate; the rest of the grinding water comes back from the filters."}])
    A_f = P_ORE * 3600 / 1000 / 1.0
    SOLIDS = 0.70                                             # the mill's slurry, by weight, as it comes to the filters: from memory
    w_in = P_ORE * (1 - SOLIDS) / SOLIDS                      # kg/s of water in the slurry
    w_filt = w_in - P_WATER                                   # back to the mill as filtrate
    D_d, d_hub, n_d = 3.15, 0.6, 4                            # discs: 4 a filter, 3.15 m across on a 0.6 m hub
    a_f = n_d * 2 * math.pi / 4 * (D_d ** 2 - d_hub ** 2)     # m2 a filter, both faces
    n_f = math.ceil(A_f / a_f)
    air = 1.5 * n_f * a_f / 60                                # m3/s of air drawn through the cake at 1.5 m3 a minute a m2 (from memory)
    p_vac = 1.6e3 * air * 60                                  # W: liquid ring pumps at 1.6 kW a m3 a minute, at about 0.2 bar absolute (from memory)
    p_aux = n_f * (7.5e3 + 15e3) + w_filt / 1000 * 3e5 / 0.7 + 30e3   # disc drives and agitators, the filtrate pump, the blow-back compressor
    vat = cyl_shell(D_d + 0.25, 4.0, 0.008) / 2 + 2 * (D_d + 0.25) * 2.2 * 0.008      # a U trough 4 m long, half a cylinder, two end plates
    shaft = cyl_shell(d_hub, 5.0, 0.025)
    recv = cyl_shell(1.6, 3.0, 0.010, bottom=2.2)
    per = (vat + shaft + recv) * RHO["steel"]
    out["disc-filter-2"] = dict(
        name="Disc filters, 2", size_class=2, fits="bay-s2", dims=(12.0, 6.6, 6.0),
        desc=(f"Two vacuum disc filters end to end take the water out of the mill's slurry ({SOLIDS:.0%} solids, {P_ORE:.1f} kg/s of ore) down to {MOIST:.0%}: each has {n_d} discs {D_d:g} m across on a {d_hub:g} m hollow shaft ({a_f:.0f} m2, both faces), "
              f"both shafts on one line along x at y 0, 3.7 m up, turning about once a minute in open U troughs ({D_d + 0.25:.2f} m wide, 4 m long: x -5 to -1 and 1 to 5) with agitators, each driven from its outer end (x -6 to -5, 5 to 6). "
              f"The discs rise on the +y side and fall on the -y side, where the cake is blown off into a short steep chute beside each disc, straight down onto the layout's cake belt, which runs along x under them all at y -2.4, 1 m up (x -6 to 6); no transfer inside the unit. "
              f"Slurry in by a distribution box between the troughs (local 0, 0, 3) that splits it between them; on the +y side two filtrate receivers (1.6 m across, 3 m tall, at (-3, 3) and (3, 3)), the vacuum pumps between them ({p_vac / 1e3:.0f} kW, x -1.5 to 1.5, y 2.5 to 4) and the filtrate pump; "
              f"the filtrate ({w_filt:.1f} kg/s) out at (-6, 3, 0.5) back to the mill; the pumps' air out of a vent at (0, 3.5, 5.5); 400 V supply at (6, 3.5, 0.5). Its parts span y -2.55 to 4 about the local origin (the chutes to the pump skid): its width is that extent, not centred."),
        built_of=[part("Troughs, shafts and receivers", A36, 3762.6 + 3539.4 + 3318.7, f"the two U troughs in 8 mm plate (shells 2,672.6 kg, heads 1,090.0), the hollow shafts {d_hub:g} m by 5 m in 25 mm (3,539.4), the receivers 1.6 m by 3 m in 10 mm (shells 2,348.9, heads 608.6, rings 361.2); measured on model v1"),
                  part("Disc sectors and cloth", "material.polyethylene-uhmw", n_f * a_f * 30.0, "polypropylene sectors under their cloth at 30 kg/m2 of filter face, from memory; polyethylene stands for polypropylene"),
                  part("Stand webs and chutes", A36, 1491.4 + 881.6, "the stands' saddle webs (1,491.4 kg) and the cake chutes' plates (881.6); measured on model v1"),
                  part("Drives and agitators", A36, n_f * 2500.0, "a drive and an agitator for each filter, from memory"),
                  part("Vacuum pumps and compressor", "good.pumps", 25.0 * p_vac / 1e3 + 1000.0, "liquid ring pumps at 25 kg/kW, the blow-back compressor and the filtrate pump, from memory")],
        needs={"power": p_vac + p_aux},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "derived", "review": True,
                "note": (f"From the plant's {P_ORE:.1f} kg/s of ore at a tonne a m2 an hour: {A_f:.0f} m2, {n_f} filters of {n_d} discs; {w_in:.1f} kg/s of water in at {SOLIDS:.0%} solids, {P_WATER:.2f} kg/s stays in the cake, {w_filt:.1f} back; "
                         f"{air:.1f} m3/s of air through the cake, the vacuum pumps {p_vac / 1e3:.0f} kW, drives, agitators, filtrate pump and blow-back {p_aux / 1e3:.0f} kW. Slurry density, air rate and equipment weights from memory.")}])
    D_b, tilt = 7.5, 45.0
    n_crit = 42.3 * math.sqrt(math.sin(math.radians(tilt)) / D_b)       # rpm at which the charge would ride round with the pan
    n_run = round(0.62 * n_crit, 1)
    net = (P_ORE + P_WATER) * 3600 / 1000 / 4                            # t/h of green balls each of four discs sends on
    pan = math.pi / 4 * D_b ** 2 * 0.020 * RHO["steel"]
    rim = math.pi * D_b * 0.5 * 0.012 * RHO["steel"]
    ribs = 12 * (D_b / 2 - 0.4) * 0.4 * 0.020 * RHO["steel"]
    torque = 90e3 / (n_run * 2 * math.pi / 60)
    out["balling-disc-7m5"] = dict(
        name="Balling disc, 7.5 m", size_class=2, fits="bay-s2", dims=(9.0, 9.0, 7.0),
        desc=(f"A shallow pan {D_b:g} m across, tilted {tilt:g} degrees and turning at about {n_run:g} rpm (62% of the {n_crit:.1f} rpm at which the charge would ride round with it), rolls the moist cake into green balls 9 to 16 mm across, "
              f"about {net:.0f} t an hour; they spill over its rim onto a roller screen that sends the small ones back. The pan's centre at local (0, 0, 3.5), its face looking up and toward -y (normal (0, -0.707, 0.707)), its rim 0.5 m deep; "
              f"behind it (+y) a slewing bearing and hub on a braced frame that sets its tilt, a girth drive at its back with a reducer and motor (90 kW, {torque / 1e3:.0f} kN m at the pan); fixed scrapers keep its face clear. "
              f"Cake in from above by a chute (local 0, 2, 7); the balls fall from its low rim (y -3.0, 1.2 m up) onto the roller screen: 12 rolls 100 mm across and 4 m long along x at 110 mm pitch (10 mm gaps), from y -3.15 to -4.45, their tops falling from 1.1 to 1.0 m, on rails and a drive from y -2.5 to -4.5; the fines drop through into a tray and out at (0, -3.5, 0.65) to the layout's return belt, the balls roll off the screen's end at (0, -4.5, 1)."),
        built_of=[part("Pan, rim and ribs", A36, 10374.5, "the pan's floor in 20 mm plate, its rim 0.5 m deep in 12 mm, twelve radial ribs 0.4 m deep in 20 mm under it; measured on model v1"),
                  part("Hub, bearing and girth gear", "material.high-strength-steel-4340", 5000.0, "from memory"),
                  part("Reducer and motor", "good.electric-motors", 5000.0, f"a 90 kW motor and a reducer for {torque / 1e3:.0f} kN m, from memory"),
                  part("Frame and tilt gear", A36, 14680.8, "the braced hollow-section frame and its tilt mounts; measured on model v1"),
                  part("Roller screen", A36, 1715.8 + 562.3, "its rails, frame and fines tray (1,715.8 kg) and 12 rolls of 100 x 5 mm tube, 4 m long (562.3); measured on model v1"),
                  part("Screen drive", "good.electric-motors", 400.0, "from memory"),
                  part("Scrapers and chutes", A36, 1247.45, "measured on model v1")],
        needs={"power": 90e3 + 15e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "derived", "review": True,
                "note": f"The pan's plate from its size; its speed from the critical speed 42.3 sqrt(sin a / D); a quarter of the plant's {P_ORE + P_WATER:.1f} kg/s of moist ore each; the drive 90 kW and the screen 15 kW, the frame, gear and screen's weights from memory."}])
    L_run, R_sp = GRATE_L + 5.0, 1.6                      # the straight run: the hood's length and 2.5 m each end to load and tip; the sprockets' pitch radius
    pallets = math.ceil((2 * L_run + 2 * math.pi * R_sp) / 1.5)
    hood_a = (2 * 4.0 + 6.0) * GRATE_L
    torches = math.ceil(P_FIRE / 1.5e6)
    fans = 20.0 * 3600 * P_RATE                           # W at 20 kWh a tonne (from memory)
    m_cool = P_RATE * 0.9e3 * 1200 / (1.1e3 * 900)            # kg/s of cooling air: the pellets from 1,300 to 100 deg C, the air warmed about 900 K on average
    m_recu = 1.3 * m_cool                                     # the firing boxes' gas, with what leaks in (from memory)
    duct = lambda m, T, v=20.0: math.sqrt(4 * m / (101325 * 0.029 / (8.314 * (T + 273.15))) / v / math.pi)   # m inside, at v m/s near 1 bar
    out["induration-grate-125th"] = dict(
        name=f"Induration grate, {P_RATE * 3.6:.0f} t an hour", size_class=4, fits="bay-s4", dims=(44.0, 14.4, 18.0),
        desc=(f"A straight grate {GRATE_W:g} m wide with {GRATE_L:g} m under its hood: a chain of {pallets} pallet cars carries a bed of green balls on a layer of fired ones along +x through drying, preheating, firing at about 1,300 deg C, and cooling; "
              f"the cooling zone's hot air goes back over the bed to the firing zone, so it needs only {P_FIRE / P_RATE / 1e3:.0f} kJ of new heat a kilogram, here {P_FIRE / 1e6:.1f} MW from {torches} electric plasma torches in the hood (there is no fuel to burn). "
              f"The chain's loop: a straight top run {L_run:g} m long (x -{L_run / 2:g} to {L_run / 2:g}, the grate's top 8.5 m up), round sprockets of {R_sp:g} m pitch radius at each end, and back underneath at 5.3 m, {pallets} cars of 1.5 m filling its {2 * L_run + 2 * math.pi * R_sp:.1f} m; the hood (inside {GRATE_W:g} m wide, {GRATE_L:g} m long over x -{GRATE_L / 2:g} to {GRATE_L / 2:g}, its walls' lower edge 8.75 m up, clear of the cars' wheels, with sliding seals down to the cars' side walls, 4.75 m clear inside, its outer roof 13.8 m up) leaves 2.5 m open at each end to load and tip; the hot-air recycle ducts over it reach 17.8 m. Wind boxes between the runs (6 to 8.3 m up), in four groups along the hood: updraft drying x -14 to -10, downdraft drying and preheat -10 to -2, firing and after-firing -2 to 6 (the torches in the hood's walls there), cooling 6 to 14. Four fan skids on the +y side at x -9, -3, 3 and 9 (y 4.5, centres 3 m up): at x 9 the cooling fan blows outside air up through the cooling boxes; the hot air off the bed there goes back by a duct over the hood's roof from x 11 to the firing zone at x -3 (direct recuperation); at x 3 the recuperation fan draws the firing boxes' gas and blows it up through the updraft drying boxes; at x -3 the hood exhaust fan draws the updraft drying hood, and at x -9 the waste gas fan the downdraft drying and preheat boxes, both into one waste header to the layout's duct. Gas: the cooling fan {m_cool:.0f} kg/s of air, the recuperation duct over the roof the same at about 1,000 deg C ({duct(m_cool, 1000):.1f} m inside 0.3 m of lining), the recuperation fan {m_recu:.0f} kg/s at about 350 deg C ({duct(m_recu, 350):.1f} m mains), the two waste fans as much again at about 120 deg C ({duct(m_recu, 120):.1f} m mains); each box group's down-legs 1.0 to 1.2 m, one every 4 m. Green balls in on a roller feeder at local (-20, 0, 9) over the loading end; fired pellets tip off the +x sprocket into a chute to (21, 0, 2); waste gas out to the layout's duct at (0, 6, 3); the torches' cooling water (a tenth of their power) by a pair of {math.sqrt(4 * P_FIRE / 0.9 * 0.1 / (4186 * 10) / 1000 / 2.0 / math.pi) * 1000:.0f} mm flanges on the -y face at (0, -6, 1) flow and (0.6, -6, 1) return, a ring main along the hood's walls to each torch."),
        built_of=[part("Pallet cars", "material.stainless-steel-304", pallets * 5000.0, f"{pallets} cars {GRATE_W:g} x 1.5 m of heat-resisting cast steel, 5 t each (from memory); stainless steel stands for the alloy"),
                  part("Hood lining", REF, 380256.4, "300 mm over the hood's walls and roof; measured on model v1"),
                  part("Hood casing", A36, 36133.7, "10 mm; measured on model v1"),
                  part("Wind boxes", A36, 22790.4, "between the runs, 10 mm plate; measured on model v1"),
                  part("Machine frame, supports and chutes", A36, 216857.6 - 530.5, "the machine's frame, its supports, cable trays, the feed and discharge chutes, the waste outlet's portal legs and feet; measured on model v3"),
                  part("Waste portal head", "material.high-strength-steel-4340", 530.5, "a 250 x 100 mm box with 20 mm walls over the 5.2 m between the portal's legs, under the drying main: about 190 MPa under 5 t at mid-span (an assumed 6 m of main with its lagging and dust) against 280 allowed; measured on model v3"),
                  part("Torch cooling water piping", A36, 43.7 * math.pi * 0.24 * 0.006 * RHO["steel"] + 5.4 * math.pi * 0.2 * 0.005 * RHO["steel"], "the riser and ring headers (43.7 m of 240 mm, 6 mm wall) and the torches' branches (5.4 m of 200 mm, 5 mm); lengths measured on model v3"),
                  part("Sprockets, bearings and drive", "material.high-strength-steel-4340", 20000.0, "two sprockets with their shafts and bearings and the chain's drive, from memory"),
                  part("Recuperation duct", A36, 18.4 * math.pi * 3.51 * 0.010 * RHO["steel"], "18.4 m over the roof, a 10 mm shell 3.52 m across; length measured on model v1"),
                  part("Recuperation duct lining", REF, 18.4 * (math.pi / 4 * (3.5 ** 2 - 3.3 ** 2) * 3000 + math.pi / 4 * (3.3 ** 2 - 2.9 ** 2) * 1000), "a dense 100 mm face on 200 mm of insulating castable at about 1,000 kg/m3 (from memory), which the registry has not got: refractory brick stands for it, by mass"),
                  part("Gas mains and legs", A36, 36.7 * math.pi * 2.408 * 0.008 * RHO["steel"] + 74.5 * math.pi * 1.906 * 0.006 * RHO["steel"] + 5.2 * math.pi * 1.008 * 0.008 * RHO["steel"] + 13.0 * math.pi * 1.006 * 0.006 * RHO["steel"], "36.7 m of 2.4 m main in 8 mm, 74.5 m of 1.9 m in 6 mm, 5.2 m of 1 m firing legs in 8 mm and 13 m of 1 m legs in 6 mm; lengths measured on model v1, lagging left out"),
                  part("Plasma torches", "good.electric-motors", torches * 2000.0, f"{torches} torches of 1.5 MW, 2 t each with their supplies, from memory"),
                  part("Process fans", "good.electric-motors", 4 * 10000.0, "four fans, from memory")],
        needs={"power": P_FIRE / 0.9 + fans, "heat": P_FIRE / 0.9 * 0.1, "gravity": 1.0},
        recipes=pmod["recipes"], life=pmod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.pellet-plant), carried here: the module stays the game's until it reads layouts."},
               {"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"The grate from the rate at about {P_RATE * 86.4 / (GRATE_W * GRATE_L):.0f} t a m2 a day; the hood, pallets and frame from memory."},
               {"of": ["needs.power"], "tier": "derived", "note": f"The firing heat the module's source gives, 0.5 GJ a tonne, made by torches at 90%; the fans {fans / 1e6:.1f} MW at 20 kWh a tonne (from memory)."},
               {"of": ["needs.gravity"], "tier": "invented", "review": True, "note": "The bed lies on the grate by its weight against the gas drawn down through it; not checked below 1 m/s2."}])
    out["mill-crane-30t"] = bridge_crane_unit(30000.0, 22.8, 9000.0, kind="Maintenance crane", job="it lifts the mill's liners and the grate's pallet cars and fans for their change", gear="a main hoist")
    tp = transformer_unit(P_P / 0.9 / PF * 1.1, "the pellet plant: its torches, mill, fans and drives")
    tp["name"] = tp["name"].replace("Furnace", "Plant")
    out[f"plant-transformer-{P_P / 0.9 / PF * 1.1 / 1e6:.0f}mva"] = tp
    out["baghouse-450k"] = baghouse_unit(450000, (24.0, 12.0, 14.0), stack_over=27.0, stack_kg=51875.5)
    out["cooling-tower-pellet"] = cooling_unit(out["induration-grate-125th"]["needs"]["heat"] + tp["needs"]["heat"], "the plasma torches' and the plant transformer's")
    return out


def pellet_plant_air(us, recs):
    """The pellet plant on a world with air at 1 g: grinding, filtering, balling and the induration grate in one crane hall."""
    cell, box = 3.0, (-60.0, -15.0, 0.0, 69.0, 15.0, 27.0)
    openings = [(-60.0, -3.0, 6.0, -57.0, 3.0, 12.0),     # the ore belt in, west
                (66.0, -3.0, 0.0, 69.0, 3.0, 6.0),        # the pellet belt out, east
                (36.0, 12.0, 0.0, 42.0, 15.0, 6.0),       # the waste gas duct out, north
                (-3.0, -15.0, 0.0, 3.0, -12.0, 6.0),      # a door and the cables, south
                (39.0, -15.0, 0.0, 42.0, -12.0, 3.0)]     # the torches' cooling water, south
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
    clad_area = 12673.74                # measured on blender's layout (candidate v01), net of openings and with reveals, lintels and eaves
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
        {"id": "B1", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [-6.0, -6.0, 0.0], "turn": 180.0, "note": "two rows of two, the south pair turned so all four screens face the aisle at y 0"},
        {"id": "B2", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [6.0, -6.0, 0.0], "turn": 180.0, "note": ""},
        {"id": "B3", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [-6.0, 6.0, 0.0], "note": ""},
        {"id": "B4", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [6.0, 6.0, 0.0], "note": ""},
        {"id": "G", "mount": "plant-mount.bay-s4", "unit": "unit.induration-grate-125th", "at": [41.0, 0.0, 0.0], "note": "in the hall's east end: its 48 m mount square is wider than the hall (the floor it stands on runs wall to wall)"},
        {"id": "D", "mount": "plant-mount.bay-s3", "unit": "unit.baghouse-450k", "at": [39.0, 36.0, 0.0], "note": "outside, north, its stack at its east end"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": f"unit.{tkey}", "at": [0.0, -24.0, 0.0], "note": "outside, south, in a fire-walled vault"},
        {"id": "C", "mount": "plant-mount.bay-s3", "unit": "unit.cooling-tower-pellet", "at": [30.0, -27.0, 0.0], "note": "outside, south-east"},
    ]
    q_t, q_tr = recs["induration-grate-125th"]["needs"]["heat"], us[tkey]["needs"]["heat"]
    d_tw, d_tt = math.sqrt(4 * q_t / (4186 * 10) / 1000 / 2.0 / math.pi), max(0.08, math.sqrt(4 * q_tr / (4186 * 10) / 1000 / 2.0 / math.pi))
    q_gas = 450000 / 3600
    d_gas = math.sqrt(4 * q_gas / (math.pi * 20.0))
    links = [
        link("conveyor", "ore belt", (-100.0, 0.0, 2.0), (-53.0, 0.0, 4.0), A36, 150.0, 100000.0, f"a 1 m belt in a gallery: {P_ORE:.1f} kg/s of ore fines from the ore yard, beyond the layout, through the west wall into the mill's chute", via=[(-62.0, 0.0, 9.0)]),
        link("pipe", "slurry", (-37.0, 3.0, 0.5), (-24.0, 0.0, 9.0), A36, math.pi * 0.25 * 0.010 * RHO["steel"], None, "250 mm rubber-lined, 10 mm wall, pumped from the mill's sump up to the filters' distribution box between their troughs, over the filtrate receivers at 12.2 m and down into the box's top", via=[(-37.0, 2.3, 0.5), (-37.0, 2.3, 12.2), (-24.0, 2.3, 12.2), (-24.0, 0.0, 12.2)]),
        link("pipe", "filtrate", (-30.0, 3.0, 6.5), (-52.5, 0.35, 3.8), A36, math.pi * 0.15 * 0.006 * RHO["steel"], None, "150 mm: the filtrate back from the filters' pump to the mill, into the +y wall of its feed chute (the wall at y 0.4, z 3.8, past the ore belt's head) by a nozzle seated 50 mm into it, its flange at y 0.55", via=[(-33.0, 3.0, 6.5), (-33.0, 3.0, 5.0), (-52.5, 3.0, 5.0), (-52.5, 3.0, 3.8)]),
        link("conveyor", "cake belt", (-30.0, -2.4, 6.98), (-12.0, -2.4, 8.98), A36, 100.0, 30000.0, "a 0.8 m belt on the filters' deck along x under all eight discs' chutes (y -2.4, 1 m above the deck), then rising at about 18 degrees to its head over the cake shuttle", via=[(-18.0, -2.4, 6.98)]),
        link("conveyor", "cake shuttle", (-12.0, -8.0, 8.6), (-12.0, 8.0, 8.6), A36, 100.0, 7500.0, "a 0.8 m reversible belt along x -12, 0.4 m under the cake belt's head: south to one header or north to the other"),
        link("conveyor", "south cake header", (-12.0, -8.0, 8.2), (8.0, -8.0, 8.2), A36, 100.0, 10000.0, "a 0.8 m belt along y -8, 0.4 m under the shuttle's south end, with ploughs at x -6 and 6 dropping 1.2 m by chutes into discs B1 and B2's feed (z 7)"),
        link("conveyor", "north cake header", (-12.0, 8.0, 8.2), (8.0, 8.0, 8.2), A36, 100.0, 10000.0, "a 0.8 m belt along y 8, 0.4 m under the shuttle's north end, with ploughs at x -6 and 6 dropping 1.2 m by chutes into discs B3 and B4's feed (z 7)"),
        link("conveyor", "green ball belt", (7.0, 0.0, 0.6), (-16.0, 0.0, 0.6), A36, 100.0, 15000.0, "a 0.8 m belt level along the aisle (y 0), west: each disc's screen hands its balls off (y -1.5 or 1.5, 1 m up) down a short chute at under 20 degrees onto it"),
        link("conveyor", "green ball cross belt", (-16.5, 0.0, 0.3), (-16.5, -11.35, 0.9), A36, 100.0, 6000.0, "a 0.8 m belt south along x -16.5, 0.3 m under the collecting belt's head, rising 0.6 m, east of the filter deck's columns"),
        link("conveyor", "green ball lift", (-16.5, -11.35, 0.6), (21.0, -11.35, 9.6), A36, 100.0, 30000.0, "a 0.8 m belt along the south wall rising east at under 14 degrees (green balls are soft and roll back on a steeper belt), 0.3 m under the cross belt's head; it passes the south door about 4.6 m up"),
        link("conveyor", "green ball feed", (21.0, -11.35, 9.3), (21.0, 0.0, 9.3), A36, 100.0, 10000.0, "a 0.8 m shuttle belt north over the grate's open loading end, 0.3 m under the lift's head, dropping 0.3 m onto the roller feeder (9 m) across the grate's width"),
        link("conveyor", "pellet belt", (62.25, 0.0, 0.5), (110.0, 0.0, 2.0), A36, 150.0, 100000.0, f"a 1 m belt: {P_RATE:.1f} kg/s of fired pellets, its tail 0.5 m up under a steel hopper that narrows the grate's 4 m chute (ending at z 2) to the belt's width in a 1.5 m drop at 45 degrees (fired pellets take the drop); level out through the east wall's opening, rising at 12 degrees outside to 2 m and on to the shaft furnace's pellet belt, beyond the layout", via=[(72.0, 0.0, 0.5), (79.1, 0.0, 2.0)]),
        link("duct", "waste gas", (41.0, 6.0, 3.0), (39.0, 30.0, 12.0), A36, math.pi * d_gas * 0.006 * RHO["steel"], None, f"{d_gas:.1f} m across, 6 mm plate, insulated: from the grate's 1.9 m waste nozzle by a reducer north to y 8.5, through the north wall to the baghouse's inlet", via=[(41.0, 8.5, 3.0), (39.0, 8.5, 3.0), (39.0, 20.0, 3.0), (39.0, 20.0, 12.0)]),
        link("pipe", "cooling water to the torches", (36.0, -27.0, 0.75), (41.0, -6.0, 1.0), A36, 2 * math.pi * d_tw * 0.005 * RHO["steel"], None, f"a pair of {d_tw * 1000:.0f} mm pipes, flow and return: {q_t / (4186 * 10):.0f} kg/s from the tower's ports at y 0 (local), east and north through the hall's south wall to the grate's flanges on its -y face; the returns run 0.45 m up under the flows at 0.75, 500 mm reducers at the tower's ports", via=[(40.5, -27.0, 0.75), (40.5, -8.0, 0.75), (40.5, -6.0, 1.0)]),
        link("pipe", "cooling water to the transformer", (36.0, -22.5, 0.75), (-2.9, -24.0, 1.0), A36, 2 * math.pi * d_tt * 0.004 * RHO["steel"], None, f"a pair of {d_tt * 1000:.0f} mm pipes: {q_tr / (4186 * 10):.1f} kg/s from the tower's ports at local y 4.5, west along y -19 and into the transformer's cooler ports on its -x end", via=[(37.5, -22.5, 0.75), (37.5, -19.0, 0.75), (-4.5, -19.0, 0.75), (-4.5, -24.0, 0.75), (-4.5, -24.0, 1.0)]),
        link("cable", "plant supply", (0.0, -24.0 + us[tkey]["dims"][1] / 2, 2.9), (0.0, -10.0, 3.0), CU, 3 * P_P / 0.9 / PF / (math.sqrt(3) * 6600) / 4e6 * RHO["copper"], None, "three phases at 6.6 kV in a cable tray, copper at 4 A/mm2, through the south door to the hall's switchroom; the middle phase steps 0.3 m east round the green ball lift's post at (0, -10.7)"),
    ]
    ground = [
        {"what": "Hall floor", "item": CONC, "volume": 904.8, "note": f"0.4 m, {x_in:.0f} x {span_in:.0f} m, less the machines' foundations it stops at; measured on blender's layout (candidate v01)"},
        {"what": "Mill foundation", "item": CONC, "volume": 480.37, "note": "two piers under the trunnions on a 3 m block, sized to the drum's dynamic load (not yet checked), with a pit 0.4 m deep under the girth gear's guard (x -41.8 to -40.6, y -3.2 to 3.2, its floor at z -0.4); measured on blender's layout (candidate v01), net of the pit"},
        {"what": "Filter deck", "item": CONC, "volume": 89.0, "note": "a 14 x 8 x 0.5 m deck 6 m up on six 1 m square columns (two rows of three, 6 m apart along x); the green ball lift passes south of it; measured on blender's layout (candidate v01)"},
        {"what": "Grate foundations", "item": CONC, "volume": 528.0, "note": "under the machine's frame; measured on blender's layout (candidate v01)"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes; 445.5 m3 measured on blender's layout (candidate v01)"},
        {"what": "Pellet hopper", "item": A36, "volume": r(909.5 / RHO["steel"]), "note": "the 4 m to 1 m hopper under the grate's discharge chute at x 62, 10 mm wear plate, with its seats; measured on blender's layout (candidate v01), 910 kg"},
        {"what": "Filter deck stair", "item": A36, "volume": r(20385.0 / RHO["steel"]), "note": "a steel stair in three 2 m flights with landings from the floor to the filter deck's west edge, and the deck's guard rails; solid treads and stringers as drawn, not a checked design; measured on blender's layout (candidate v01), 20,385 kg"},
        {"what": "Outside slabs", "item": CONC, "volume": 196.7, "note": "the baghouse's slab and the transformer's vault floor, trimmed where the stack's foundation takes over; measured on blender's layout (candidate v01)"},
        {"what": "Stack foundation", "item": CONC, "volume": r(7 * 7 * 2.5), "note": f"a block 7 x 7 x 2.5 m under the baghouse's {recs['baghouse-450k']['physical']['height']:g} m stack (centre (48, 36)), its top at grade, the stack's base ring bolted to it: against an invented 1 kPa of wind on the stack, its overturning moment about 3.2 MN m leaves the load within the middle third of the block (e 0.9 m against 1.2); soil bearing about 130 kPa at the toe; measured on blender's layout (candidate v01)"},
        {"what": "Cooling tower basin", "item": CONC, "volume": 114.66, "note": "a 0.5 m floor 0.6 m wider than the tower each way, walls 0.3 m thick and 2 m deep, eight piers under the tower's four feet and its skid's four corners; measured on blender's layout (candidate v01)"},
        {"what": "Transformer fire walls", "item": CONC, "volume": 42.74, "note": "the vault's walls round the plant transformer, with the cooling pipes' and cables' penetrations; measured on blender's layout (candidate v01)"},
        {"what": "Service and conveyor piers", "item": CONC, "volume": 4.25, "note": "feet under the conveyors' trestles and the pipe and cable supports outside; measured on blender's layout (candidate v01)"},
        {"what": "Runway brackets", "item": A36, "volume": r(19681.8 / RHO["steel"]), "note": "the stubs and diagonals that seat the crane runways on the wall nodes; measured on blender's layout (candidate v01), 19,682 kg"},
        {"what": "Conveyor steel", "item": A36, "volume": r(25835.6 / RHO["steel"]), "note": "the belts' stringers, posts, trestles and bases; the links' own mass a metre stays for belt, idlers and drives, so the stringers may be counted twice (at most about 12 t); measured on blender's layout (candidate v01), 25,836 kg"},
        {"what": "Pipe and cable supports", "item": A36, "volume": r(9762.7 / RHO["steel"]), "note": "portals, posts, shoes and bearings under the slurry, filtrate, cooling water, waste gas duct and supply cables; joints and anchors left out, not checked for strength; measured on blender's layout (candidate v01), 9,763 kg"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO["steel"] if g["item"] == A36 else RHO["concrete"]))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [crane]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    water = sum(u.get("needs", {}).get("water", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    frame_mass = fmass + clad
    total = mass_units + mass_links + frame_mass + mass_ground
    mp = pmod["physical"]
    xs = [x0, x1] + [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [y0, y1] + [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    against = (f"module.pellet-plant: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {P_P / 1e6:.0f} MW (45 kWh and 0.5 GJ of heat a tonne, the heat counted as power). "
               f"This layout: the same {P_RATE * 3.6:.0f} t an hour; {power / 1e6:.1f} MW, of which {P_FIRE / 0.9 / 1e6:.1f} the grate's plasma torches; the pellets cool on the grate in the air it draws and the waste gas leaves by the baghouse's stack, so only the torches' and the transformer's {(q_t + q_tr) / 1e6:.1f} MW goes to a one-cell tower; "
               f"{water:.1f} kg/s of water dried off the green balls; {(mass_units + mass_links + frame_mass + sum(g["mass"] for g in ground if g["item"] == A36)) / 1000:,.0f} t of plant, frame and supports, {sum(g["mass"] for g in ground if g["item"] == CONC) / 1000:,.0f} t of concrete. "
               f"One shaft furnace at its full rate takes {PELLETS:.1f} kg/s of pellets, about {PELLETS / P_RATE:.1f} of these.")
    layout = {
        "identity": {"key": "layout.pellet-plant-air", "name": "Pellet plant, world with air",
                     "description": "Fine ore made into fired pellets for the shaft furnace: a ball mill grinds it wet, disc filters dry it, four balling discs roll it into green balls, and a straight induration grate dries, fires and cools them, its heat from electric plasma torches; a baghouse cleans the waste gas. One crane hall. For a world with air at about 1 g."},
        "stands_for": "module.pellet-plant",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": max(z1, max(b["at"][2] + recs[b["unit"].split(".", 1)[1]]["physical"]["height"] for b in bays)), "mass": r(total)},
        "bays": bays, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 24 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor; framed openings for the ore belt (west), the pellet belt (east), the waste gas duct (north) and a door (south).",
                  "openings": [list(o) for o in openings], "members": members,
                  "cladding": {"item": "stock.al6061-sh-2", "area": r(clad_area), "mass": r(clad)},
                  "carries": ["unit.mill-crane-30t"], "mass": r(frame_mass), "sized": sized},
        "ground": ground,
        "totals": {"rate": P_RATE, "power": r(power), "heat": r(q_t + q_tr), "water": r(water), "mass": r(total), "against_module": against},
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
    # Measured on blender's candidate v02 (kg); the ballast from the moment about the slewing centre.
    boom, wheel_end, wheel = 58113.8 - 12450.9, 12450.9, 25166.6     # the boom less what sits at its tip; the tip's axle, bearings and pedestal; the wheel
    drives, belts = 50000.0, 6600.0                                   # drives, belts and machinery house; the boom's belt and drums (from memory)
    ore_on = 1500e3 / 3600 / 4.0 * 29.0                               # kg of ore on the boom's belt stacking, at 4 m/s
    arm_x, cw_x, house_x = 8.0, 14.0, 4.0                            # m behind the centre: the arm's middle, the ballast's, the house's
    arm = 26655.5                                                     # the counterweight arm to x -16, measured
    pivot, mast_x, mast = 2.5, 3.0, 8127.6 + 977.1 + 4897.6               # the luff pivot and mast forward of the axis, clear of the tripper; the mast measured
    boom_x = (pivot + 31.5) / 2
    moment = (boom + belts + ore_on) * boom_x + (wheel_end + wheel + 8000.0) * 31.5 + mast * mast_x - arm * arm_x - 20000.0 * house_x
    ballast = moment / cw_x
    out["stacker-reclaimer-1500th"] = dict(
        name="Bucket wheel stacker-reclaimer, 1,500 t an hour", size_class=3, dims=(51.0, 12.0, 20.0),
        desc="Runs on its rail between two stockpile beds: stacking, its boom (29 m from its luff pivot at x 2.5 to the wheel's axle at 31.5, slewing either side) drops ore from the yard belt's tripper onto a bed; reclaiming, the bucket wheel at the boom's end cuts into the pile's face and sends the ore back down the boom onto the yard belt, which then runs the other way. Stacks 1,500 t an hour (a ship's load landed at the port in a day), reclaims 500. Travels along its x on 10 m gauge, its origin on the running plane at the slewing centre. Stacking, its tripper (unit.yard-tripper-1500th, a separate car on the same rails, coupled behind it on -x) drops the ore into a feed hopper over the slewing centre at local (0, 0, 8.5), onto the boom's belt. "
             f"Its counterweight arm reaches back to x -16, its underside 11.8 m up, clear of the tripper's head; the ballast, {ballast / 1000:.0f} t of cast iron blocks centred at x -{cw_x:g}, balances the boom horizontal and stacking about the slewing centre. The boom reaches x 34.75 with the wheel. It slews 95 degrees either way from along +x, enough to reach both beds as it travels, but not further, so its tripper can stand behind it: between z 7 and 11.8, no part of the slewing frame may enter the tripper's corridor (y -1.8 to 1.8, from x -0.8 back) anywhere in that range, as checked by sweeping it (a point r from the axis enters it after slewing to about 180 - asin(1.8 / r) degrees, so near the axis only parts close to the boom's line pass); its counterweight arm and supports hang from the mast above z 11.8; the mast stands at x 3, the luff pivot's towers at x 2.5, y -1.6 and 1.6, outside the boom's belt. The feed hopper is the tripper's, not the machine's, and the boom's tail turns under it at the axis. It runs on four bogies, two to a rail with their centres at x -4.5 and 4.5, each 5.4 m long with four wheels of 1.2 m: sixteen wheels, about 290 kN each at 1 g.",
        built_of=[part("Portal and slewing base", A36, 69267.1, "the portal's legs and girders, the slewing base's rings, the bogies' ties and rail flanges; measured on model v2"),
                  part("Wheels, axles and bearings", STEEL, 75954.9, "sixteen running wheels of 1.2 m and their axles and bearings, drawn solid as they are; measured on model v2"),
                  part("Bogie frames", A36, 66068.5 / 3, "four bogies 5.4 m long drawn solid (66.1 t); counted at a third, about how full a fabricated box is (from memory)"),
                  part("Slewing frame and mast", A36, 44170.5, "the slewing rings, luff tower, the mast forward at x 3 with its rear stays, the counterweight's supports from the mast, the house's seats and the central chute; measured on model v2"),
                  part("Counterweight arm", A36, arm, "to x -16 from the mast, its underside 11.8 m up; measured on model v2"),
                  part("Boom", A36, 58113.8, "chords, diagonals, stays, idlers and the wheel's axle, bearings and pedestal at its tip; measured on model v2"),
                  part("Bucket wheel", A36, wheel, "rim, spokes and buckets; measured on model v2"),
                  part("Ballast", CAST_IRON, ballast, f"from the moment about the slewing centre with the boom horizontal and stacking: the boom {boom / 1000:.0f} t and its belts and ore at {boom_x:g} m, the wheel and its tip {(wheel + wheel_end + 8000) / 1000:.0f} t at 31.5, the mast {mast / 1000:.0f} t at {mast_x:g}, less the arm at {arm_x:g} and the house at {house_x:g} behind; at {cw_x:g} m; model v2's grey iron block is 157,181 kg"),
                  part("Drives, belts and machinery house", "good.electric-motors", drives, "the wheel's 8 t drive, travel, slew and luff drives, the boom's belts and drums and the house's electrics, from memory")],
        needs={"power": 900e3},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Steel measured on blender's candidate v01 where drawn hollow; the ballast from the balance; drives from memory."},
               {"of": ["needs"], "tier": "invented", "review": True, "note": "Rates and power from memory of stacker-reclaimers of this size."}])
    rise, ang = 9.5 - 1.0, 15.0                           # the yard belt lifted from 1 m to the head pulley's top at 9.5, at its steepest for ore
    incline = rise / math.tan(math.radians(ang))
    L_t = incline + 2.0 + 2.0                                 # the incline, the head's overhang and the tail's
    out["yard-tripper-1500th"] = dict(
        name="Yard belt tripper, 1,500 t an hour", size_class=3, dims=(round(L_t, 1), 12.0, 11.0),
        desc=(f"A car on the stacker-reclaimer's rails, coupled behind it by a drawbar, that lifts the yard belt out of its run: the belt climbs over the tripper at {ang:g} degrees ({incline:.1f} m to rise {rise:g} m, from 1 m up to its head pulley's top at 9.5 m), turns down round the head and back to the yard under the car. "
              "Stacking, the ore leaves over the head down a covered chute at 45 degrees into a feed hopper on the slewing axis, carried by the head's frame, its outlet at (0, 0, 8.5) over the boom's tail, which turns under it; reclaiming, a flap closes the chute and the ore the machine sends down its central chute lands on the belt beyond the tripper. "
              f"Its origin on the running plane under the head's chute, at the machine's slewing centre when coupled; it reaches back to local x -{L_t - 2.0:.1f}, its tail at 1 m. Four bogies on the 10 m gauge, two travel drives that follow the machine's; two fixed drawbars, the tripper's, from its front bogies' ends (x -8.75) by cross pins to brackets, also the tripper's, bolted flush to the machine's rear bogies' end faces (x -7.2, y -5.06 and 5.06, 1 m up), for the belt's pull on the head (about 10 kN lifting the ore, a few times that in start-up tension, from memory) and the car's own rolling."),
        built_of=[part("Trusses and supports", A36, 26462.8, "two side trusses, the idler deck and the return's supports; measured on model v1"),
                  part("Pulleys, chute and hopper", A36, 4184.2, "the head and bend pulleys (hollow drums), the covered chute, flap and the axis feed hopper with its flange; measured on model v1"),
                  part("Bogies, wheels and coupling", A36, 23784.3, "four bogies, their running wheels drawn solid, the drawbars, pins and brackets to the machine's bogies; measured on model v1"),
                  part("Travel drives", "good.electric-motors", 2 * 1000.0, "two, from memory; the belt is the yard belt's")],
        needs={"power": 30e3},
        basis=[{"of": ["physical"], "tier": "derived", "note": f"The incline from the lift, {rise:g} m at {ang:g} degrees (iron ore's steepest on a plain belt, from memory)."},
               {"of": ["built_of"], "tier": "derived", "review": True, "note": "Steel measured on blender's model v1; drives from memory."},
               {"of": ["needs"], "tier": "invented", "review": True, "note": "From memory of yard trippers."}])
    out["truck-dump-hopper"] = dict(
        name="Truck dump hopper", size_class=2, fits="bay-s2", dims=(12.0, 10.0, 8.0),
        desc="A steel hopper over a pit that a tipping truck backs onto and empties into, 100 t, with an apron feeder under it onto the yard belt; its grizzly bars keep out the lumps. The trucks come from the port, its feeder's outlet at local (0, 0, -4) in the pit; the yard belt leaves east under it through a portal in the stand's east bracing (y -1.2 to 1.2, z -4.8 to -2.2), a header over it in place of the diagonal. Its steel deck round the mouth carries the truck as it tips: a loaded rear axle of about 130 t (a 170 t truck, the load thrown back as it tips; from memory), times 1.25 for impact, 1.6 MN on two wheel groups 5 m apart; the dump-edge beam across x, 7 m between its columns at x -5.5 and 1.5, is a 700 x 400 x 30 mm box: one group at mid-span bends it 1.4 MN m, 122 MPa against A36's 165 allowed. Its grizzly: bars 80 x 200 mm solid at 380 mm (300 mm gaps, the biggest lump the 1.4 m yard belt takes), across the mouth along y on its rim beams and a 600 x 400 x 25 mm box beam across x at y 0, so they span 3 m: 2 m of ore on them (45 kPa) times 3 for the tipping's impact (from memory) bends each 61 kN m, 114 MPa.",
        built_of=[part("Hopper and grizzly", A36, 30764.4, "20 mm hopper, its rim beams and the grizzly; measured on model v1"),
                  part("Deck and supports", A36, 70195.9, "the truck deck: 700 x 400 x 30 boxes under the dump edge and wheel tracks, 400 x 300 x 16 elsewhere, 20 mm plate in the wheel strips and 10 mm chequer plate, twelve columns to z -6, the guards, and the east portal for the yard belt (300 x 300 x 20 header, sill and posts, 150 x 150 x 10 side braces); measured on model v2"),
                  part("Apron feeder", A36, 13446.2, "pans, chain, drums and frame, the drums and axles drawn solid; measured on model v1"),
                  part("Feeder drive", "good.electric-motors", 4000.0, "motor and gearbox for 150 kW, from memory")],
        holds=100000.0, needs={"power": 150e3},
        basis=[{"of": ["built_of"], "tier": "derived", "review": True, "note": "Steel measured on blender's model v1; the drive from memory."},
               {"of": ["physical", "holds", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["ingot-crane-10t"] = bridge_crane_unit(10000.0, 22.8, 3000.0, kind="Ingot crane", job="it lifts hot ingots off the ingot car, one at a time, onto the cooling racks; later lays each cooled ingot on a crop saw's table and the cropped ingot onto the crating line's roller table. Its scissor tongs hang from the single hook by their bail: closed 0.8 m along x, 0.4 m across and 1.8 m tall under the hook; their jaws, 0.4 m long, open from 0.35 to 0.7 m across y and grip an ingot by two opposite faces, standing (below its head) or lying, so a cropped ingot with no eye left is held the same way; parked, the tongs hang closed", gear="scissor ingot tongs, about 1.5 t of the trolley's")
    ic = out["ingot-crane-10t"]
    ic["dims"] = (ic["dims"][0], ic["dims"][1], r(ic["dims"][2] + 1.8, 3))        # the closed tongs hang 1.8 m under the raised hook
    ic["desc"] += " Its box runs from the closed tongs' jaws, 1.9 m under the rail plane when parked, to the trolley's top."
    out["sampling-lab"] = dict(
        name="Sampling laboratory", size_class=1, fits="bay-s1", dims=(6.0, 6.0, 3.5),
        desc=f"A closed steel cabin where the steel of each heat is read: no ingot comes in. At a crop saw, one ingot of each heat has a second slice taken off its cut face, a coupon {ING_A:g} m square and 25 mm thick (about {ING_A ** 2 * 0.025 * 7850:.0f} kg), which a worker brings on a hand cart to the lab's pass-through hatch. Inside, an abrasive cutter takes a 40 mm piece off it, a grinder dresses its face, and a spark emission spectrometer reads the steel's composition in minutes, so the heat's grade can be certified on its crates' labels; the coupon is kept a while, then goes to scrap. A staff door, the pass-through hatch (0.6 x 0.6 m, 1 m up) beside it, the extraction and power at the rear.",
        built_of=[part("Cabin", A36, 8000.0, "from memory"), part("Cutter, grinder and spectrometer", "good.electric-motors", 3000.0, "from memory")],
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
        desc=f"{n_saws} horizontal band saws in 3 rows of 3 along x, each with its roller table: the ingot crane sets a cooled ingot on a table lying along x, its big end at the blade, and the blade cuts the hot top off (for one ingot of each heat, then a 25 mm coupon off the cut face, for the sampling lab) ({HOT_TOP:,.0f} kg, with its lifting eye) through the {ING_A:g} m square section in about {t_cut / 60:.0f} minutes with the handling; the head drops into a scrap bin under the table, which goes back to the arc furnace as scrap, and the crane lifts the sound ingot onto the crating line. {ing_h * 3600:.0f} ingots an hour from the casting bay need {n_saws}.",
        built_of=[part("Band saws", "good.electric-motors", n_saws * 9000.0, "about 9 t a saw for a 0.5 m section, from memory"),
                  part("Roller tables and scrap bins", A36, n_saws * 1500.0, "from memory")],
        needs={"power": n_saws * 15e3},
        basis=[{"of": ["built_of", "needs", "physical"], "tier": "invented", "review": True, "note": f"The count from the casting bay's rate and a cut of {CUT_RATE * 1e4 * 60:.0f} cm2 a minute (a large band saw in carbon steel, from memory); saw mass and power from memory."}])
    out["crating-line"] = dict(
        name="Crating line", size_class=2, fits="bay-s2", dims=(12.0, 4.0, 3.0),
        desc="A roller table along x, its rollers' tops 0.8 m up: a forklift sets an empty heavy crate, its lid on top, at the -x end (x -6 to -2); it rolls to the crating station at x 0, where the lid is lifted aside and the ingot crane lowers a cropped ingot straight into it from the saws, lying, its tongs' jaws in the gaps between the ingot and the crate's sides; a platform scale under the station weighs crate and ingot (the crate's tare known); the crate rolls on to the nailer's portal at x 3.5, where the lid is nailed on, past the label printer at x 4.8 (heat, grade and mass), and leaves at the +x end for a forklift.",
        built_of=[part("Roller table, scale and nailer", A36, 12000.0, "from memory")],
        needs={"power": 40e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["ingot-tipper"] = dict(
        name="Ingot tipper", size_class=1, fits="bay-s1", dims=(5.4, 3.0, 2.1),
        desc=f"Lays a standing ingot down for the saws: an L-shaped cradle whose pivot runs along y at local (0, 0, 1.0), on two pedestals at y -0.8 and 0.8. At rest its base plate is level at z 1.0 from x -0.5 to 0 and its back plate stands at x 0 from z 1.0 to 2.1; the ingot crane sets a cooled ingot, big end up, on the base against the back (its centre at (-0.24, 0)). A hydraulic cylinder turns the cradle 90 degrees toward +x about the pivot: the back plate comes down level at z 1.0, the ingot lies on it along x (x 0 to 1, its big end toward +x, its top at about 1.48), the base plate now an end stop at x 0, the laid back plate (x 0 to 1.1) carrying it; the crane takes it from there to a saw, or rolls it on to rollers from x 1.2 to 2 (tops at 1.0) to wait. The cylinder works from the side, at y -1.25, clear of the ingot's sweep. About a minute a turn; {ING_M + HOT_TOP:,.0f} kg with its head. Drawn at rest.",
        built_of=[part("Cradle, pivot and table", A36, 3500.0, "from memory"), part("Cylinder and power pack", "good.pumps", 800.0, "from memory")],
        needs={"power": 11e3},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["forklift-2t5"] = dict(
        name="Forklift, 2.5 t", size_class=1, dims=(3.6, 1.3, 2.2),
        desc="A battery-electric counterbalance forklift that moves heavy crates from the crating line to the stacks and from the stacks onto trucks; it lifts a loaded crate (" + f"{(ING_M + CRATE['physical']['mass']) / 1000:.1f} t: an ingot of {ING_M:,.0f} kg in a {CRATE['physical']['mass']} kg crate, its centre 0.6 m out on the forks) onto another, two high (forks to 1 m), and onto a truck's bed (up to about 1.8 m); rated 2.5 t at 0.5 m, about 2.1 t at 0.6.",
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
    # The machine's west limit stacks the bed's end; its tripper's tail then lies 33.7 m behind, where the belt must be level.
    xm = x0 + 2.0
    tail = xm - (recs["yard-tripper-1500th"]["physical"]["length"] - 2.0)
    x_lv = tail - 2.0
    z_tail, ramp = -4.4, 14.0
    x_h = r(x_lv - (1.0 - z_tail) / math.tan(math.radians(ramp)), 3)
    x_rail = r(xm - 32.0)
    bays = [{"id": "H", "mount": "plant-mount.bay-s2", "unit": "unit.truck-dump-hopper", "at": [x_h, 0.0, 0.0], "note": "west of the rail, its feeder in a pit onto the yard belt; trucks back onto its deck from -y"}]
    gauge_y = 0.0
    beds_y = (base / 2 + 8.0)
    links = [
        link("rail", "machine rail", (x_rail, 0.0, 0.0), (r(-x0 + 10), 0.0, 0.0), A36, 33840.3 / 144.7, None, "two A120 crane rails (120 mm head, about 117 kg/m as measured on blender's layout) at 10 m gauge on a concrete bed: each of the machine's 16 wheels of 1.2 m puts about 290 kN on its rail, against about 640 kN a 120 mm head takes under a 1.2 m wheel (5.6 N/mm2 times its 96 mm of flat head times the diameter, from memory)"),
        link("conveyor", "yard belt", (x_h, 0.0, z_tail), (r(-x0 + 20), 0.0, 1.0), A36, 110.0, 400000.0, f"a 1.4 m reversible belt (its mass a metre the belt's two strands and idlers, from memory; its stringers and stands are in the ground) between the rails, with the machine's tripper on it: ore from the hopper to the machine stacking, from the machine to the out belt reclaiming. Its tail 0.4 m under the feeder's outlet, out of the pit through an opening in its east wall and up at {ramp:g} degrees to 1 m, then level from x {x_lv:.1f}, where the tripper's tail lies with the machine at its west limit (x {xm:.1f})", via=[(r(x_lv), 0.0, 1.0)]),
        link("conveyor", "out belt", (r(-x0 + 20.7), 0.0, 0.6), (r(-x0 + 80), 0.0, 1.0), A36, 80.0, 100000.0, "a 1 m belt (belt and idlers a metre, from memory) to the pellet plant's ore belt, beyond the layout: its tail 0.4 m under the yard belt's head, fed by a short chute narrowing 1.4 m to 1 m, then up to 1 m", via=[(r(-x0 + 24.7), 0.0, 1.0)]),
        link("pipe", "dust sprays, north", (r(x0 - 2), beds_y + base / 2 + 2, 1.0), (r(-x0), beds_y + base / 2 + 2, 1.0), A36, math.pi * 0.1 * 0.004 * RHO["steel"], None, "100 mm along the north bed's outer edge, spraying water to keep the dust down, fed at its west end from the yard's water, beyond the layout"),
        link("pipe", "dust sprays, south", (r(x0 - 2), -(beds_y + base / 2 + 2), 1.0), (r(-x0), -(beds_y + base / 2 + 2), 1.0), A36, math.pi * 0.1 * 0.004 * RHO["steel"], None, "100 mm along the south bed's outer edge, the same; neither crosses the rails or the belt"),
        link("road", "truck approach", (x_h - 2.0, -(beds_y + base / 2 + 2), 0.0), (x_h - 2.0, -5.6, 0.0), CONC, 7.0 * 0.3 * RHO["concrete"], None, "7 m wide, paved 0.3 m: trucks from the port turn and back north onto the hopper's deck, centred on its mouth (local x -2, between the wheel stops at -4.5 and 0.5); it ends at the pit's south edge and covers the gap there"),
    ]
    mobile = [{"unit": "unit.stacker-reclaimer-1500th", "rides": "machine rail", "at": [0.0, 0.0, 0.0]},
              {"unit": "unit.yard-tripper-1500th", "rides": "machine rail", "at": [0.0, 0.0, 0.0]}]
    ground = [
        {"what": "Rail bed", "item": CONC, "volume": 231.46, "note": "two strips 1 m wide and 0.8 m deep under the rails; measured on blender's layout (candidate v01)"},
        {"what": "Hopper pit", "item": CONC, "volume": 200.36, "note": "6 m deep under the hopper, 13 x 11.2 m inside (the hopper's feet reach y -5.15 and 5.15), 0.4 m walls, the east wall open 2 x 2 m for the yard belt; the hopper's deck bridges its top; measured on blender's layout (candidate v01)"},
        {"what": "Pit apron", "item": CONC, "volume": 7.68, "note": "a 0.3 m slab, its top at grade, bridging the 0.5 to 0.6 m between the pit's walls and the hopper's 12 x 10 m deck on the west, north and east, reinforcement not checked; a strip under the truck approach's end closes the south; measured on blender's layout (candidate v01)"},
        {"what": "Belt ramp trench", "item": CONC, "volume": 30.31, "note": "the yard belt's trench from the pit's east wall up to grade: floor 2.4 m wide, walls about 2.5 m deep on average, 0.3 m; measured on blender's layout (candidate v01)"},
        {"what": "Support footings", "item": CONC, "volume": 6.08, "note": "feet under the belts' stands, the return's seats, the spray posts and the transfer; measured on blender's layout (candidate v01)"},
        {"what": "Belt and spray steel", "item": A36, "volume": r(32157.5 / RHO["steel"]), "note": "the belts' stringers, legs and crossbars, the return's stands under the tripper, roller and drum seats, the transfer hood, the trench guards and the spray posts; the belts, idlers' rolls, drums and drives are the links'; measured on blender's layout (candidate v01), 32,158 kg"},
        {"what": "Bed pads", "item": CONC, "volume": 1680.0, "note": f"two pads {bed_L:.0f} x {base:.0f} m, 0.2 m, at y -{beds_y:.0f} and {beds_y:.0f}, sloped to drains; measured on blender's layout (candidate v01)"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO["steel"] if g["item"] == A36 else RHO["concrete"]))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    L = r(-x0 + 80 - (x_h - 6.0))
    W = r(2 * (beds_y + base / 2 + 3))
    water = 0.2
    mp = omod["physical"]
    against = (f"module.ore-yard: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, holding {hold / 1e6:.0f} kt. "
               f"This layout: two beds {bed_L:.0f} m long, {base:.0f} m across and {h:g} m high (iron ore at {ORE_BULK} kg/m3 lying at {REPOSE:g} degrees) hold the same; {power / 1e6:.2f} MW with every drive running, against the module's {omod['needs']['power'] / 1e6:.1f}; "
               f"{(mass_units + mass_links + sum(g['mass'] for g in ground if g['item'] == A36)) / 1000:,.0f} t of plant and supports, {sum(g['mass'] for g in ground if g['item'] == CONC) / 1000:,.0f} t of concrete.")
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
    clad_area = 2 * (x1 - x0) * (z1 - z0) + 2 * (y1 - y0) * (z1 - z0) + (x1 - x0) * (y1 - y0) - sum(max(d - a, e - b) * (f - c) for a, b, c, d, e, f in doors) - 2 * (x1 - x0 + y1 - y0) * 3.0 + sum(max(d - a, e - b) * min(f - c, 3.0) for a, b, c, d, e, f in doors)   # the walls' lowest 3 m left open for the cooling air
    clad_area = 3880.83                                   # measured on blender's layout (candidate v01): net of the doors, the walls' open 3 m and the roof's vent strip (y 0.9 to 2.1, x -24 to 24)
    clad = clad_area * 0.002 * RHO["aluminium"]
    sec_stock = lambda g: yaml.safe_load(open(S + "mill-stock/" + strut(sec[g], cell)[1]["kit"]["section"].split(".", 1)[1].upper() + ".yaml"))
    sized = (f"At 1 g: roof {roof_q:.0f} Pa over a {span_in:.0f} m span puts {chord / 1e3:.0f} kN in a chord: {sec['roof']} takes {frames.strut_capacity(sec_stock('roof')) / 1e3:.0f} kN. "
             f"A wheel of the 10 t crane loaded at the runway's end, with the roof's share, puts {vert / 1e3:.0f} kN on a wall node: {sec['wall']} takes {frames.strut_capacity(sec_stock('wall')) / 1e3:.0f} kN. Wind and footings not yet checked.")
    # The ingots: as the casting bay makes them, cooled a day on racks.
    ing_rate = C_RATE * SOUND / ING_M                      # ingots a second
    cool_h = 10.0                                          # h standing in the fans' draught, from about 700 to 150 deg C for the saws (from memory; still air takes about half as long again)
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
        {"id": "S", "mount": "plant-mount.bay-s1", "unit": "unit.sampling-lab", "at": [-21.0, -7.5, 0.0], "note": "in the hall, by the ingot rail, its door and hatch facing south onto a 1.2 m walkway inside the wall"},
        {"id": "K", "mount": "plant-mount.bay-s2", "unit": "unit.crating-line", "at": [18.0, -3.0, 0.0], "note": "in the hall, running out of its east door"},
        {"id": "C", "mount": "plant-mount.bay-s2", "unit": "unit.crop-saws", "at": [3.0, -6.0, 0.0], "note": "in the hall, west of the crating line, under the crane"},
        {"id": "T", "mount": "plant-mount.bay-s1", "unit": "unit.ingot-tipper", "at": [-6.0, -6.0, 0.0], "note": "west of the saws: a cooled ingot, standing, is laid down here before the saws"},
    ]
    # The cooling beds: the ingots stand big end up as they left their moulds, in rows across y at 1.0 m (room for the tongs' jaws) and 0.7 m along x.
    pitch_x, pitch_y = 0.7, 1.0
    beds = [("north", -5.0, 23.0, 0.5, 9.5), ("west", -23.0, -17.5, -4.0, 3.5), ("west centre", -17.5, -9.5, -5.5, 3.5), ("south-east", 12.0, 23.0, -9.0, -5.5)]
    places = sum(int((xb - xa) / pitch_x) * int((yb - ya) / pitch_y) for _, xa, xb, ya, yb in beds)
    assert places >= on_racks, (places, on_racks)
    links = [
        link("rail", "ingot rail", (-36.0, 6.0, 0.0), (-6.0, 6.0, 0.0), A36, 2 * 50.0, None, "two 50 kg/m rails at 2 m gauge, set in the floor: in by the west door from the casting bay; the car stops at x -15 under the crane"),
        link("road", "yard aisles", aisle_path[0], aisle_path[-1], CONC, 4.5 * 0.25 * RHO["concrete"], None, f"{rows} forklift aisles 4.5 m wide, {yard_L:.0f} m long, joined end to end, paved 0.25 m (the rest of the yard is the ground's)", via=aisle_path[1:-1]),
        link("road", "truck lane", (x1, -3.0, 0.0), (yx0 + yard_L + 20.0, -3.0, 0.0), CONC, 7.0 * 0.3 * RHO["concrete"], None, "7 m wide, paved 0.3 m: trucks load crates at the yard's east end for the port, beyond the layout"),
    ]
    mobile = [{"unit": "unit.forklift-2t5", "rides": "yard aisles", "at": [r(yx0 + 10.0 + 20.0 * i), aisle_y[i % rows], 0.0]} for i in range(6)]
    ground = [
        {"what": "Hall floor", "item": CONC, "volume": 493.4, "note": "0.4 m under the hall, less the rail channel and the beds' seats; measured on blender's layout (candidate v01)"},
        {"what": "Frame footings", "item": CONC, "volume": r(ground_nodes * 2.25), "note": f"a 1.5 x 1.5 x 1 m pad under each of the lattice's {ground_nodes} ground nodes; 243 m3 measured on blender's layout (candidate v01)"},
        {"what": "Aprons and rail channel", "item": CONC, "volume": 18.11, "note": "aprons outside the west and east doors (6.71 and 7.2 m3) and the ingot rail's channel bed (4.2); measured on blender's layout (candidate v01)"},
        {"what": "Runway brackets", "item": A36, "volume": r((6711.75 + 2874.42) / RHO["steel"]), "note": "400 x 400 x 25 mm stubs (6,712 kg) and 160 x 8 mm diagonals (2,874 kg) seating the crane runways on the inner wall nodes; measured on blender's layout (candidate v01)"},
        {"what": "Stack yard", "item": CONC, "volume": r(5985.0 - rows * yard_L * 4.5 * 0.25), "note": f"{yard_L:.0f} x {yard_W:.0f} m paved 0.25 m (5,985 m3 measured on blender's layout, candidate v01, less the aisles, which are their link's) east of the hall: {crates:,} crates two high in {lanes} lanes ten deep, either side of its aisles; outdoors, the crates under tarpaulins"},
    ]
    ground += [{"what": f"Cooling bed, {nm}", "item": CAST_IRON, "volume": r((xb - xa) * (yb - ya) * 0.05), "note": f"cast iron floor plates 50 mm thick over x {xa:g} to {xb:g}, y {ya:g} to {yb:g}, so a hot ingot does not spall the concrete; {int((xb - xa) / pitch_x) * int((yb - ya) / pitch_y)} places, the ingots standing at {pitch_x:g} m along x and {pitch_y:g} m across y in the crane's reach (y -9 to 9)"} for nm, xa, xb, ya, yb in beds]
    for g in ground:
        g["mass"] = r(g["volume"] * (7200.0 if g["item"] == CAST_IRON else RHO["steel"] if g["item"] == A36 else RHO["concrete"]))
    fans = 4 * 22e3                                        # W: four wall fans blowing through the beds, from memory
    links.append(link("cable", "cooling fans' supply", (-18.0, 11.0, 1.5), (18.0, 11.0, 1.5), CU, 0.6, None, "400 V along the north wall to four axial fans 2 m across on floor stands inside the north wall at x -18, -6, 6 and 18, their axes 1.5 m up, blowing south across the beds; the air leaves through the walls' open lowest 3 m and the roof's vents; 22 kW each, from memory (their mass with the stands about 1.2 t each, counted in the frame)"))
    fmass += 4 * 1200.0 + 96.8 * 0.002 * RHO['aluminium']   # the fans and stands (from memory); the vent's louvre caps, 96.8 m2 measured
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile] + [crane]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + fans
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
               f"{(mass_units + mass_links + frame_mass) / 1000:,.0f} t of plant, frame and paving, {sum(g["mass"] for g in ground if g["item"] in (CAST_IRON, A36)) / 1000:,.0f} t of cooling beds and runway brackets, {sum(g["mass"] for g in ground if g["item"] == CONC) / 1000:,.0f} t of concrete. {on_racks} ingots cool {cool_h:g} hours in the fans' draught ({places} places). The crane makes about four moves an ingot ({4 * ing_rate * 3600:.0f} an hour), more than one crane can at a minute or so a move: the count of cranes is not yet worked out.")
    layout = {
        "identity": {"key": "layout.ingot-yard-air", "name": "Ingot yard, world with air",
                     "description": "The ingots come in hot on their car from the casting bay; a crane lays them on racks in a cooling hall for a day; one of each heat is sampled and analysed; band saws crop each ingot's hot top (back to the arc furnace as scrap); a crating line puts each ingot in a heavy crate with its label; forklifts stack the crates two high on a paved yard and load them onto trucks for the port. For a world with air at about 1 g."},
        "stands_for": "module.ingot-yard",
        "site": {"atmosphere": "air", "gravity": [8.0, 11.0]},
        "physical": {"length": L, "width": W, "height": z1 + 0.45, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"Walls one cell thick, 12 m tall; a roof one cell deep over a {x_in:.0f} x {span_in:.0f} m floor; doors 6 m wide for the ingot rail (west) and the crates (east). Its walls' lowest 3 m left unclad for the cooling air; a vent strip 1.2 m wide along the roof (y 0.9 to 2.1, x -24 to 24) under aluminium louvre caps standing 0.45 m above it, the chords kept.",
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


def _berm():
    """m3 of loose regolith over a crew post: a frustum 1 m over its crest, its top 1 m wider than the post each way, its sides at the angle of repose, less the post's box."""
    L, W, H = 6.0, 4.4, 4.6
    h, run = H + 0.7, (H + 0.7) / math.tan(math.radians(35.0))
    a1, a2 = (L + 2) * (W + 2), (L + 2 + 2 * run) * (W + 2 + 2 * run)
    return r(h / 3 * (a1 + a2 + math.sqrt(a1 * a2)) - L * W * H)


BERM = 586.23                       # m3 measured on blender's arc furnace shop (candidate v01): a top 4 x 2 m at z 5.3, 35 degree sides, a foot 19.1 x 17.1 m, less the cavity, the ramp cut and the light wells (_berm(), a cruder frustum, gave about 1,130)


def radiator_unit(T, whose):
    """A field of raised horizontal radiator panels filling a class 4 bay, rejecting a loop's heat at T to the sky.
    Sized for the worst case on an airless moon: the sun overhead on the top face, warm shaded ground under."""
    top = RAD_EPS * SIGMA * T ** 4 - RAD_ALPHA * SUN
    under = RAD_EPS * SIGMA * (T ** 4 - T_UNDER ** 4)
    q = top + under
    A = 44.0 * 44.0                                   # m2 of panel, 2 m gaps at the bay's edges for the loop's headers
    pw, pl = 5.5, 11.0                                # m: a panel, 4 along x by 8 along y fill the 44 m square
    panels = round(A / (pw * pl))
    face = A * 0.002 * 2 * RHO["aluminium"]
    tubes = A / 0.15 * math.pi * 0.02 * 0.0015 * RHO["aluminium"]
    legs = panels * 4 * box_tube(0.1, 0.1, 0.004, 3.0) * RHO["aluminium"]
    water = A / 0.15 * math.pi / 4 * 0.017 ** 2 * RHO["water"]
    pumps = q * A / (4186 * 10) / 1000 * 300e3 / 0.75
    d_h = max(0.1, math.ceil(math.sqrt(4 * q * A / (4186 * 10) / 1000 / 2.0 / math.pi) / 0.05) * 0.05)   # m: the headers, the loop's water at 2 m/s
    rails = panels * (2 * pl + 2 * pw) * (0.1 + 2 * 0.05) * 0.003 * RHO["aluminium"]
    return dict(
        name=f"Radiator field, {q * A / 1e6:.1f} MW at {T:.0f} K", size_class=4, fits="bay-s4", dims=(48.0, 48.0, 3.5),
        desc=(f"{panels:.0f} aluminium panels {pw:g} x {pl:g} m (4 along x by 8 along y), each on a frame of 100 x 50 mm channel rails round its edge, raised 3 m on four legs and laid flat edge to edge over 44 x 44 m, carrying {whose} loop's water at about {T:.0f} K through tubes bonded between their two face sheets; "
              f"the top face, white, radiates to the sky, the underside to the shaded ground. {q:.0f} W a m2 of panel at lunar noon, the worst case: the sun overhead on the top ({RAD_ALPHA:g} of {SUN:.0f} W/m2 absorbed), the ground under it at {T_UNDER:.0f} K; more at night. "
              f"The loop's flow and return headers ({d_h * 1000:.0f} mm) run round the edge in the 2 m margin, a branch to each row of panels; its ports at local (-24, -0.6, 1) flow and (-24, 0.6, 1) return. "
              " For airless ground only: on a world with air a tower is lighter. Kept clean of dust, which darkens the white."),
        built_of=[part("Panel skins and edges", "material.aluminium-alloy-6061", 20907.1, "two 2 mm face sheets a panel and their edges; measured on blender's model of the 330 K field (v1), the same for every field"),
                  part("Panel tubes", "material.aluminium-alloy-6061", tubes, "tubes 20 mm across, 1.5 mm wall, at 150 mm, bonded between the sheets (not drawn)"),
                  part("Legs", "material.aluminium-alloy-6061", 1592.5, "four 100 mm box legs, 4 mm, 3 m, a panel; measured on model v1"),
                  part("Panel rails", "material.aluminium-alloy-6061", 1639.3, "100 x 50 x 3 mm channel round each panel's edge, carrying its skins between the legs; measured on model v1"),
                  (part("Headers, branches and supports", "material.aluminium-alloy-6061", 3174.5, "the mains round the margin, an 80 mm pair to each row and 50 mm stubs to each panel, 3 mm walls (2 mm the stubs), with their supports: measured on model v1") if T < 400 else
                   part("Headers and branches", "material.stainless-steel-304", 10763.8,
                        f"steam at about {T - 273.15:.0f} deg C and 20 bar is too hot and too high for aluminium pipe: stainless steel 304, the {d_h * 1000:.0f} mm mains 6 mm along the -x edge (about 67 MPa), a raised inlet crossover at z 1.6, 100 mm row pairs 3 mm along x, 50 mm stubs 2 mm; measured on blender's model of the 485 K field (v1); the panels' own 20 mm aluminium tubes see only about 13 MPa")),
                  *([part("Pipe supports", "material.aluminium-alloy-6061", 25458.1 - 20907.1 - 1592.5 - 1639.3, "measured on model v1")] if T >= 400 else []),
                  part("Pumps", "good.pumps", 2000.0, "from memory")],
        holds=water, rejects={"heat": q * A, "temperature": T}, needs={"power": pumps, "atmosphere": "vacuum"},
        basis=[{"of": ["rejects", "physical"], "tier": "derived", "review": True, "note": f"Stefan-Boltzmann at emissivity {RAD_EPS}; absorptance, the shaded ground's temperature and the panel's layout invented or from memory."},
               {"of": ["built_of", "holds", "needs"], "tier": "derived", "review": True, "note": "From the panel's sheets and tubes; pumps at 3 bar and 75%."}])


SEALED_GAS_T, FILTER_GAS_T, CP_CO = 1600.0, 250.0, 1200.0   # deg C: the sealed furnace's gas leaving it (invented) and at the filter; J/kg K, CO's mean heat (from memory)


def sealed_offgas():
    """The sealed furnace's gas: (kg/s of CO, W its cooler takes). Only CO, from the sponge iron's carbon: no air is
    drawn in to dilute it or burn it, so the duct takes its sensible heat only, not the share of the arc's power the
    shop for air loses up its duct."""
    co = DRI * DRI_C * 28.0 / 12.0
    return co, co * CP_CO * (SEALED_GAS_T - FILTER_GAS_T)


def offgas_cooler_unit():
    """An evaporative cooler for the sealed arc furnace's gas: a water-walled shaft and its steam drum."""
    co, q = sealed_offgas()
    D, Hs, pitch = 1.6, 8.0, 0.08                    # m: the shaft across inside its walls, their height (z 9 to 17), the tubes' pitch
    tube_od, tube_t = 0.0603, 0.005
    n = math.ceil(math.pi * (D + tube_od) / pitch)
    tubes = n * Hs * math.pi * (tube_od - tube_t) * tube_t * RHO["steel"]
    fins = n * Hs * (pitch - tube_od) * 0.005 * RHO["steel"]
    headers = 2 * math.pi * (D + 0.12) * math.pi * (0.219 - 0.012) * 0.012 * RHO["steel"]
    drum = math.pi * 1.012 * 0.012 * 3.0 * RHO["steel"] + 2 * math.pi / 4 * 1.0 ** 2 * 1.15 * 0.012 * RHO["steel"]
    pipes = (2 * 12.0 + 4 * 4.0) * math.pi * (0.168 - 0.007) * 0.007 * RHO["steel"]
    hopper = math.pi * (D / 2 + 0.2) * math.hypot(D / 2 - 0.2, 1.5) * 0.010 * RHO["steel"] + math.pi * 1.2 * 0.008 * 1.5 * RHO["steel"] + math.pi * (D + 0.02) * 0.010 * 2.0 * RHO["steel"]
    legs = 4 * box_tube(0.3, 0.3, 0.01, 17.0) * RHO["steel"] * 1.5
    water = n * Hs * math.pi / 4 * (tube_od - 2 * tube_t) ** 2 * 850.0 + math.pi / 4 * 1.0 ** 2 * 3.0 / 2 * 850.0
    area = math.pi * D * Hs
    loss = math.pi * (D + 2 * tube_od) * Hs * 0.8 * SIGMA * 485.0 ** 4
    return dict(
        name=f"Off-gas cooler, evaporative, {q / 1e6:.1f} MW", size_class=2, fits="bay-s2", dims=(6.0, 4.0, 18.0),
        desc=(f"Cools the sealed arc furnace's {co:.2f} kg/s of CO from about {SEALED_GAS_T:.0f} to {FILTER_GAS_T:.0f} deg C, {q / 1e6:.1f} MW, so its filter's candles last: the gas comes down a shaft {D:g} m across whose walls are {n} boiler tubes {tube_od * 1000:.0f} mm across welded edge to edge ({Hs:g} m of them, z 9 to 17, about {q / area / 1e3:.0f} kW a m2), then through a plain collecting section of 10 mm plate, the shaft's size, z 7 to 9, out of whose side it leaves; "
              f"the water in them boiling at about 485 K (20 bar) and rising to a steam drum 1 m across and 3 m long along y on the -x side (axis at local x -2, z 16.5). The steam goes to a radiator field and comes back as water by the field's pump. "
              f"Gas in on top at (0, 0, 18) from the furnace's lined duct; out of the collecting section's side at (3, 0, 8), straight, to the filter; steam out of the drum at (-3, 0.4, 16.5) and water back at (-3, -0.4, 16.5), both facing -x; the dust falls through a lock under the hopper into a closed bin, emptied by forklift; the lock's drives fed at (2, -2, 1). "
              f"Its bare walls radiate about {loss / 1e3:.0f} kW to the sky as well, not counted. On four box legs. For airless ground: on a world with air the shop's duct burns the gas instead."),
        built_of=[part("Water walls", A36, 3956.2, f"{n} tubes {tube_od * 1000:.1f} x {tube_t * 1000:.0f} mm, {Hs:g} m, with 5 mm bars between them; structural steel stands for boiler steel; measured on blender's model v1 ({tubes + fins:,.0f} kg from their sizes)"),
                  part("Ring headers", A36, 621.8, "a 219 x 12 mm ring at the walls' top and bottom; measured on blender's model v1"),
                  part("Steam drum", A36, 810.6, "1 m across, 3 m, 12 mm (about 100 MPa at 20 bar), and its heads; measured on blender's model v1"),
                  part("Downcomers and risers", A36, 651.9, "two 168 mm downcomers from the drum to the bottom ring and four risers; measured on blender's model v1"),
                  part("Collector, hopper and outlet", A36, 2921.6, "the plain collecting section z 7 to 9, 10 mm, the cone under it and the 1.2 m outlet stub (9.5 mm, the run's wall); measured on blender's model v1"),
                  part("Inlet lining", REF, 657.9, "refractory where the furnace's gas comes in on top; measured on blender's model v1"),
                  part("Legs, bracing and supports", A36, 13327.0, "four box legs to z 17 with their bracing, the drum's and pipes' supports; measured on blender's model v1"),
                  part("Dust lock and bin", A36, 371.1, "the lock's chamber, the closed bin, its lid clamps and fork runners; measured on blender's model v1"),
                  part("Terminal box", A36, 68.9, "measured on blender's model v1"),
                  part("Dust lock drives and valves", "good.pumps", 500.0, "two valves and their drives, from memory (drawn as proxies, not weighed)")],
        holds=water, needs={"power": 10e3, "heat": q, "atmosphere": "vacuum"},
        basis=[{"of": ["needs.heat"], "tier": "derived", "review": True, "note": f"The CO from {DRI_C:.0%} carbon in the sponge iron (from memory), its mean heat {CP_CO:.0f} J/kg K (from memory), from {SEALED_GAS_T:.0f} deg C (invented) to {FILTER_GAS_T:.0f}."},
               {"of": ["built_of"], "tier": "derived", "review": True, "note": "Measured on blender's model v1 (2026-10-16): wall volumes at steel's density, welded seats not subtracted; the pressure parts' strength, the heat and the structure not qualified."},
               {"of": ["physical", "holds"], "tier": "invented", "review": True, "note": "Sizes from memory of waste-heat boilers; the walls' heat flux not checked against the gas's radiation."}])


def arc_sizes(hot):
    """The airless shop's steam, condensate and transformer lines: (kg/s of steam at the hot field's rating, DN steam,
    DN condensate, DN transformer water)."""
    steam = hot["rejects"]["heat"] / 1.89e6           # kg/s: 1.89 MJ/kg to condense at 20 bar (from memory)
    q_t = TRANSFORMER_LOSS * S_MVA / (4186 * 10)
    return (steam, kit.dn_for(math.sqrt(4 * steam / 10.0 / 30.0 / math.pi)),      # steam at about 10 kg/m3, 30 m/s
            kit.dn_for(math.sqrt(4 * steam / 850.0 / 1.0 / math.pi)), kit.dn_for(math.sqrt(4 * q_t / 1000 / 2.5 / math.pi)))


def arc_ports(us):
    """Ports on the units of the airless arc furnace shop (SFO 25)."""
    K = 273.15
    WATER = "good.water"
    lv = lambda w: kit.termination(400, kit.area_for(amps(w, 400)))
    co, q_gas = sealed_offgas()
    low = WATER_SHARE * P / (4186 * 10)
    flow = (WATER_SHARE * P + TRANSFORMER_LOSS * S_MVA) / (4186 * 10)
    dn_main = kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi))
    hot = us["radiator-field-485k"]
    steam, dn_steam, dn_cond, dn_t = arc_sizes(hot)
    us["radiator-field-485k"]["ports"] = [
        port("steam-in", "in", "gas", kit.flange(dn_steam, 25, temp=485.0), (-24.0, -0.6, 1.0), (-1, 0, 0), item=WATER, rate=steam, pressure=20e5, temperature=485.0, note="its flow header's inlet, the middle of its -x edge (v1)"),
        port("water-out", "out", "liquid", kit.flange(dn_cond, 25, temp=485.0), (-24.0, 0.6, 1.0), (-1, 0, 0), item=WATER, rate=steam, temperature=480.0, note="the condensate, by its pump"),
        port("power-in", "in", "power", lv(hot["needs"]["power"]), (-24.0, -1.4, 0.3), (-1, 0, 0), power=hot["needs"]["power"], voltage=400, note="its pump's terminal box, beside the headers"),
    ]
    hot["desc"] = hot["desc"].replace("its ports at local (-24, -0.6, 1) flow and (-24, 0.6, 1) return.", f"its ports at local (-24, -0.6, 1) steam in (DN {dn_steam * 1000:.0f}) and (-24, 0.6, 1) water out (DN {dn_cond * 1000:.0f}), facing -x, its pump's power entry at (-24, -1.4, 0.3).")
    v = us["eaf-vessel-80t-sealed"]
    v["desc"] = v["desc"].replace("(the manifold itself is the layout's)", "(the manifold is the unit's: two DN 400 barrels standing on the deck, each taking its hose from the side)")
    v["desc"] += (f" Its ports (SFO 25): sponge iron in at the feed lock's top, local (0, 1.8, 10.1), a belt's head over it; off-gas out at the reducer's flange, (2, 1.5, 9.7), facing up, 1.2 m lined; the secondary bus's joint at the flexible cables' fixed ends, (0, -6.2, 2.3), facing -y; "
                  f"the deck manifold's water flanges, DN {dn_main * 1000:.0f} facing up, in at (-4.2, 4.95, 0.5) and out at (-3.4, 4.95, 0.5), side by side 0.8 m apart so their flanges clear, and in front of the cradle's deck, which tilts over them (v4: the manifold reduces from the hoses' 300 mm, and keeps inside local y 5.3, clear of the airless shop's gantry crane's legs, whose inner faces pass at y 5.43).")
    m4 = "measured on blender's model v2 (the registry's v4, 2026-10-16)"
    swap = {"Feed lock": part("Feed lock", A36, 3311.5 - 576.91 - 337.78 + 351.16 + 199.58, f"two gate valves and a chamber over the fifth hole, the chamber shortened under a square-to-round catch hopper whose rim is the port at z 10.1: bodies, housings, seats and the hopper; {m4} (v3's 3,311.5 kg less its chamber, 576.9, and top flange, 337.8; the new chamber 351.2 and hopper 199.6)"),
            "Off-gas bellows and reducer": part("Off-gas bellows and reducer", "material.stainless-steel-304", 1426.8 - 260.63 - 238.63 + 219.71, f"the bellows 2 m across from the fourth hole's flange, then a reducer to the 1.2 m lined flange at z 9.7; {m4} (v3's 1,426.8 kg less its output flange, 260.6, and reducer, 238.6; the new reducer 219.7)"),
            "Water hoses": part("Water hoses", "material.silicone-rubber", 182.8, f"two flexible hoses, flow and return, 300 mm bore, 4.71 and 4.31 m along their centrelines, to the deck manifold's barrels; reinforced EPDM, silicone rubber standing for it; {m4}")}
    v["built_of"] = [swap.get(x["what"], x) for x in v["built_of"]]
    if "Deck manifold" not in [x["what"] for x in v["built_of"]]:
        v["built_of"] += [part("Off-gas reducer lining", REF, 1531.4, f"the reducer's refractory; {m4}"),
                          part("Deck manifold", A36, 908.4, f"two DN 400 barrels standing on the deck, their closed bottoms, feet, hose reducers and saddles; {m4}"),
                          part("Bus terminal necks", CU, 601.2, f"three copper necks from the flexible cables' fixed ends to the bus joint; {m4}"),
                          part("Bus joint supports", A36, 1059.7, f"the joint's three supports and insulators, the insulators not mass-qualified; {m4}")]
    us["eaf-vessel-80t-sealed"]["ports"] = [
        port("dri-in", "in", "bulk", kit.belt_transfer(0.8), (0.0, 1.8, 10.1), (0, 0, 1), item="good.sponge-iron", rate=DRI, note="the feed lock's top; the belt's head 3 m over it"),
        port("offgas-out", "out", "gas", kit.lined_flange(1.2, 1), (2.0, 1.5, 9.7), (0, 0, 1), item="material.carbon-monoxide", rate=co, pressure=1e5, temperature=SEALED_GAS_T + K),
        port("power-in", "in", "power", kit.bus_joint(V2, I2, 3), (0.0, -6.2, 2.3), (0, -1, 0), power=P, voltage=V2, note=f"{I2 / 1000:.0f} kA a phase"),
        port("water-in", "in", "liquid", kit.flange(dn_main, 10, temp=330.0), (-4.2, 4.95, 0.5), (0, 0, 1), item=WATER, rate=low, temperature=320.0),
        port("water-out", "out", "liquid", kit.flange(dn_main, 10, temp=330.0), (-3.4, 4.95, 0.5), (0, 0, 1), item=WATER, rate=low, temperature=330.0),
    ]
    t = us["furnace-transformer-90mva"]
    L_, W_, H_ = t["dims"]
    if "HV cable box" not in [x["what"] for x in t["built_of"]]:
        m2 = "measured on blender's model v2 (2026-10-16)"
        t["built_of"] = t["built_of"] + [part("HV cable box", A36, 504.1, f"3 x 0.8 x 0.8 m, 6 mm, over the three HV bushings (moved to 0.9 m pitch under it); {m2}"),
                                         part("Cable box supports", A36, 123.8, f"its hollow posts and feet on the tank lid; {m2}"),
                                         part("Cooler water pipework", A36, 35.2 + 56.5, f"the DN 100 stubs turned to face -x (35.2 kg) and their supports (56.5); {m2}")]
        t["desc"] += (f" Fed at 33 kV through a cable box over its HV bushings, its gland plate on top at (0, -0.85, {H_ + 0.8:g}); the secondary's bus joint (SFO 25, the three phases 0.6 m apart) at (0, 2.5, 4) facing +y; its cooler's water in at (-3.76, -2.38, 1.35) and out at (-3.76, -2.38, 2.35), facing -x.")
        t["dims"] = (L_, W_, H_ + 0.8)
    q_t = TRANSFORMER_LOSS * S_MVA / (4186 * 10)
    i_hv = amps(P + TRANSFORMER_LOSS * S_MVA, 33e3, PF)
    t["ports"] = [
        port("hv-in", "in", "power", kit.termination(33000, 630), (0.0, -0.85, H_ + 0.8), (0, 0, 1), power=P + TRANSFORMER_LOSS * S_MVA, voltage=33000, note=f"{math.ceil(i_hv / (630e-6 * kit.J_VACUUM))} circuits of 630 mm2; 33 kV chosen"),
        port("lv-out", "out", "power", kit.bus_joint(V2, I2, 3), (0.0, 2.5, 4.0), (0, 1, 0), power=P, voltage=V2),
        port("water-in", "in", "liquid", kit.flange(dn_t, 10), (-3.76, -2.38, 1.35), (-1, 0, 0), item=WATER, rate=q_t, temperature=320.0),
        port("water-out", "out", "liquid", kit.flange(dn_t, 10), (-3.76, -2.38, 2.35), (-1, 0, 0), item=WATER, rate=q_t),
    ]
    b = us["dri-bin-200t"]
    top = b["dims"][2]
    b["ports"] = [
        port("dri-in", "in", "bulk", kit.chute(0.8), (0.0, 0.0, top), (0, 0, 1), item="good.sponge-iron", rate=DRI, note="a nozzle in its roof's middle, a closed conveyor's head over it"),
        port("dri-out", "out", "bulk", kit.chute(0.8), (0.0, -3.0, 6.0), (0, 0, -1), item="good.sponge-iron", rate=DRI, note="the weigh feeder's outlet, onto a belt's tail"),
        port("power-in", "in", "power", lv(b["needs"]["power"]), (3.0, 0.0, 1.0), (1, 0, 0), power=b["needs"]["power"], voltage=400, note="a terminal box on its east legs' brace, the feeder's cable up the leg"),
    ]
    g = us["offgas-filter-sealed"]
    g["ports"] = [
        port("gas-in", "in", "gas", kit.flange(1.2, 1, temp=FILTER_GAS_T + K), (-6.0, 0.0, 8.0), (-1, 0, 0), item="material.carbon-monoxide", rate=co, pressure=1e5, temperature=FILTER_GAS_T + K),
        port("power-in", "in", "power", lv(g["needs"]["power"]), (5.0, 0.0, 1.0), (1, 0, 0), power=g["needs"]["power"], voltage=400, note="on its frame's east post"),
    ]
    c = us["offgas-cooler-sealed"]
    c["ports"] = [
        port("gas-in", "in", "gas", kit.lined_flange(1.2, 1), (0.0, 0.0, 18.0), (0, 0, 1), item="material.carbon-monoxide", rate=co, pressure=1e5, temperature=SEALED_GAS_T + K),
        port("gas-out", "out", "gas", kit.flange(1.2, 1, temp=FILTER_GAS_T + K), (3.0, 0.0, 8.0), (1, 0, 0), item="material.carbon-monoxide", rate=co, pressure=1e5, temperature=FILTER_GAS_T + K),
        port("steam-out", "out", "gas", kit.flange(dn_steam, 25, temp=485.0), (-3.0, 0.4, 16.5), (-1, 0, 0), item=WATER, rate=q_gas / 1.89e6, pressure=20e5, temperature=485.0),
        port("water-in", "in", "liquid", kit.flange(dn_cond, 25, temp=485.0), (-3.0, -0.4, 16.5), (-1, 0, 0), item=WATER, rate=q_gas / 1.89e6, temperature=480.0),
        port("power-in", "in", "power", lv(c["needs"]["power"]), (2.0, -2.0, 1.0), (0, -1, 0), power=c["needs"]["power"], voltage=400, note="the dust lock's drives"),
    ]




def airless_units(us):
    """The airless furnace shop's own units."""
    out = {}
    v = dict(us["eaf-vessel-80t"])
    v["name"] = "Arc furnace vessel, 80 t, sealed"
    v["desc"] = ("The 80 t arc furnace closed for airless ground: its roof sealed to the shell, glands round the three electrodes, the sponge iron fed through a lock in the fifth hole, and the off-gas taken by a sealed bellows joint from the fourth, so the furnace holds its own gas (the CO its charge gives off, started with argon) at about 1 bar: "
                 "the arc burns as in air, the melt neither boils nor loses its manganese to the vacuum, and the slag foams. It tilts only a few degrees to tap and 15 to deslag, through the bellows. Otherwise as unit.eaf-vessel-80t: " + us["eaf-vessel-80t"]["desc"])
    v["built_of"] = list(us["eaf-vessel-80t"]["built_of"]) + [
        part("Roof seal flanges", A36, 2489.1, "a clamped flange joint round the roof's rim (a sand seal cannot hold 1 bar against vacuum): the shell's and the roof's rings, 250 mm wide and 30 mm thick, and the seal's land; measured on model v3"),
        part("Roof hold-down clamps", A36, 5978.4, "twelve: 1 bar inside against vacuum lifts the 6.6 m roof with 3.4 MN, about 285 kN a clamp, each a 200 x 120 mm A36 bridge from the cage's upright over 0.3 m (about 90 MPa at a 0.15 m lever) on 60 mm pins, with its anchor, base, brace and toe drawn solid; measured on model v3; the roof's panels at 1 bar not checked"),
        part("Electrode glands", A36, 1646.4, "three water-cooled glands with their flanges, 20 mm jackets; measured on model v3"),
        part("Seals", "material.silicone-rubber", 60.0, "the roof's double O-ring, cooled by a tee off the vessel's upper water header, and the glands' packings, from memory; the gap between the O-rings needs no pump on airless ground: a 25 mm line from it, through a pressure gauge that shows a leak, ends at a capped vent beside the vessel's external water ports, open to the vacuum outside"),
        part("Seal services", A36, 35.9, "the cooling tee, the gap's line, gauge and cap; measured on model v3"),
        part("Feed lock", A36, 3311.5, "two gate valves and a chamber over the fifth hole, local z 8.3 to 10.1: bodies, housings and seats; measured on model v3"),
        part("Feed lock drives", "good.electric-motors", 500.0, "from memory"),
        part("Off-gas bellows and reducer", "material.stainless-steel-304", 1426.8, "a bellows 2 m across from the fourth hole's flange (local z 8.5 to 9.1), then a reducer to the layout's 1.2 m duct at z 9.7, with their flanges; measured on model v3")]
    v["needs"] = dict(us["eaf-vessel-80t"]["needs"], atmosphere="any")
    v["basis"] = us["eaf-vessel-80t"]["basis"] + [{"of": ["identity.description"], "tier": "invented", "review": True, "note": "Sealed as closed ferroalloy furnaces are; not a design that has been built for vacuum."}]
    out["eaf-vessel-80t-sealed"] = v
    co = DRI * DRI_C * 28.0 / 12.0
    out["offgas-filter-sealed"] = dict(
        name="Off-gas filter, sealed", size_class=2, fits="bay-s2", dims=(12.0, 6.0, 10.0),
        desc=(f"Takes the furnace's {co:.1f} kg/s of CO with its dust from the steam-cooled duct at about 1 bar and 250 deg C, through ceramic candle filters in a closed vessel, and lets it out past the valve that holds the furnace's pressure, up a short stack: on an airless world the gas simply leaves (or a later unit can compress it as fuel). "
              "The dust is let out through a lock into sealed bins. Gas in at local (-6, 0, 8)."),
        built_of=[part("Vessel", A36, 11152.7, "4 m across, z 2 to 10: hopper, barrel and domed head with their rings, 12 mm (about 17 MPa at 1 bar); measured on model v1"),
                  part("Supports", A36, 11633.3, "its frame (12 mm) and pipe posts (8 mm) with their feet; measured on model v1"),
                  part("Ducts, stack, dust lock and bin", A36, 3195.3, "the gas inlet and its flange, the clean gas outlet and short stack, the dust lock's chamber and the closed bin; measured on model v1"),
                  part("Candles", "material.silicon-carbide", 3000.0, "from memory"),
                  part("Pressure valve and dust valves", "good.pumps", 1500.0, "the valve that holds the furnace's pressure and the dust lock's two valves with their drives, from memory")],
        needs={"power": 50e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": f"From memory; the CO from {DRI_C:.0%} carbon in the sponge iron (from memory) burnt to CO by the melt's oxygen."}])
    out["gantry-crane-20t-moon"] = bridge_crane_unit(20000.0, 24.0, 6000.0, kind="Gantry crane", job="on its own legs over the furnace and its transformer: it sets electrodes, lifts the roof and swaps the shell", gear="a main hoist", g=MOON_G[1], h_min=0.6)
    gc = out["gantry-crane-20t-moon"]
    legs = 4 * box_tube(0.8, 0.8, 0.02, 22.0) * RHO["steel"] + 2 * box_tube(0.8, 0.6, 0.02, 10.0) * RHO["steel"]
    gc["built_of"].append(part("Legs and sill beams", A36, legs, "four box legs 0.8 x 0.8 m, 20 mm, 22 m, and two sill beams"))
    gc["dims"] = (10.0, 25.2, 25.0)
    gc["built_of"] = [dict(p_, mass=12922.6, note=p_["note"] + "; measured on blender's model v1, 12,922.6 kg") if p_["what"] == "Bridge girders" else p_ for p_ in gc["built_of"] if p_["what"] != "Legs and sill beams"]
    gc["built_of"] += [part("Legs and sill beams", A36, 50801.2, "four box legs 0.8 x 0.8 m, 20 mm, from the sills' tops at z 1 to 22, and two sill beams 9.8 m; measured on model v1"),
                       part("Rails, stops, plates, braces and hangers", A36, 11330.3, "the trolley's rails and stops, the end plates, the legs' braces and the festoon hangers; measured on model v1")]
    gc["desc"] = gc["desc"].replace("on the hall's runways", "on its own legs, running on two ground rails 24 m apart").replace("its origin on the runway's rail plane", "its origin on the ground rails' plane, its girders 22 m up").replace("the hook raised to 0.1 m under the rail plane", "the hook raised under the bridge (its heights below)") + f" Sized for {MOON_G[1]:g} m/s2."
    gc["desc"] += " Its bridge datum is the girders' underside at z 22 (their tops 22.8, the trolley's rails 22.88); parked, the hook is raised to z 21.9; the legs (0.8 x 0.8 m, 20 mm) stand from the sill beams' tops at z 1."
    out["crew-post"] = dict(
        name="Crew post, pressurised", size_class=1, fits="bay-s1", dims=(6.0, 4.4, 4.6),
        desc="The shift's control room on airless ground: an aluminium cylinder 4 m across and 4 m long at 1 bar on two saddles, its axis along x 2.3 m up (the shell from x -1 to 3), with a 2 m airlock at its -x end (x -3 to -1), coaxial, its outer door's threshold at (-3, 0, 1.35); both its round doors (1.9 m clear, 110 mm aluminium sandwich leaves) are plugs that open toward the higher pressure, the outer into the lock and the inner into the cabin, so the air holds them shut (about 280 kN) and their eight dogs are only latches; and closed windows on its +y side; air and water kept for its watch. The berm of loose regolith 1 m deep over it, against radiation and the cold, is the layout's, with openings for the airlock and the windows. Drawn as its bare shell, saddles and airlock, closed.",
        built_of=[part("Shell and doors", "material.aluminium-alloy-6061", 1000.7, "the shell 4 m across and 4 m long, 6 mm (1 bar gives 33 MPa), the two ribbed door leaves and the power gland's pad on the +x dome; measured on blender's model v2"),
                  part("Airlock, saddles and frames", A36, 3490.0, "the 6 mm lock, two 10 mm saddles on 400 x 400 x 20 mm foot plates (about 15 kN each under the post and its berm at 2 m/s2), door frames, viewport rings and hardware; measured on blender's model v1"),
                  part("Life support and fit-out", "good.electric-motors", 3000.0, "from memory")],
        needs={"power": 20e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory."}])
    out["radiator-field-330k"] = radiator_unit(330.0, "the panels' and transformer's")
    out["radiator-field-485k"] = radiator_unit(485.0, "the off-gas cooler's evaporative")
    out["offgas-cooler-sealed"] = offgas_cooler_unit()
    return out


def arc_furnace_airless(us, recs):
    """The arc furnace shop on airless ground at about a moon's gravity: no hall (no weather, and no air to carry heat
    off), a sealed furnace, radiators for the tower, sintered regolith for concrete (no water to make it with). Laid of
    runs from port to port (SFO 25)."""
    K = 273.15
    co, q_gas = sealed_offgas()
    rad, hot_rad = "radiator-field-330k", "radiator-field-485k"
    f_low, f_hot = recs[rad]["rejects"]["heat"], recs[hot_rad]["rejects"]["heat"]
    bus = kit.busbar(V2, I2, 3)
    bus_path = [None, (-12.0, -2.0, 4.0), (-12.0, -2.0, 7.3), None]
    bus_loss = kit._recs[bus]["per_m"]["power"] * (1.5 + 3.3 + 1.8)
    low_heat = WATER_SHARE * P + TRANSFORMER_LOSS * S_MVA + bus_loss
    n_low, n_hot = math.ceil(low_heat / f_low), math.ceil(q_gas / f_hot)
    cols = 4
    rad_at = [(-84.0 - 48.0 * (i % cols), 0.0 - 51.0 * (i // cols)) for i in range(n_low)]     # (a 3 m aisle south of each row, for its mains)
    bays = [
        {"id": "V", "mount": "plant-mount.bay-s2", "unit": "unit.eaf-vessel-80t-sealed", "at": [-12.0, 6.0, 5.0], "note": "on the furnace platform, 5 m up, as in the shop for air"},
        {"id": "T", "mount": "plant-mount.bay-s2", "unit": "unit.furnace-transformer-90mva", "at": [-12.0, -6.0, 0.0], "note": "on its slab beside the platform: no fire walls, since nothing burns without air"},
        {"id": "F", "mount": "plant-mount.bay-s1", "unit": "unit.dri-bin-200t", "at": [-55.0, 7.8, 0.0], "turn": 90.0, "note": "west, beyond the gantry crane's reach, turned so its feeder faces east: the feed belt comes in along x between the crane's legs, never across them"},
        {"id": "C", "mount": "plant-mount.bay-s2", "unit": "unit.offgas-cooler-sealed", "at": [14.0, 24.0, 0.0], "note": "north-east, past the crane's east end"},
        {"id": "G", "mount": "plant-mount.bay-s2", "unit": "unit.offgas-filter-sealed", "at": [26.0, 24.0, 0.0], "note": "east of the cooler"},
        {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [18.0, -18.0, 0.0], "note": "south-east, clear of the crane's rails, under its berm, its airlock opening west"},
    ]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{rad}", "at": [x, y, 0.0], "note": f"south-west, in rows of {cols}"} for i, (x, y) in enumerate(rad_at)]
    bays += [{"id": f"RH{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{hot_rad}", "at": [-84.0 - 48.0 * i, 51.0, 0.0], "turn": 180.0, "note": "north-west, turned so its ports face the plant"} for i in range(n_hot)]
    lvf = 400
    lv = lambda w: kit.termination(lvf, kit.area_for(amps(w, lvf)))
    lc = lambda w: kit.cable(lvf, kit.area_for(amps(w, lvf)))
    need = lambda k: recs[k]["needs"]["power"]
    p_fields = n_low * need(rad)
    i_hv = amps(P + TRANSFORMER_LOSS * S_MVA, 33e3, PF)
    drag = kit.drag(0.8, S_RATE * 1.5)
    dh = kit._recs[drag]["outside"][1] / 2
    bin_top = recs["dri-bin-200t"]["ports"][0]["at"][2]
    edge = [
        port("sponge-iron-in", "in", "bulk", kit.chute(0.8), (-55.0, 70.0, 5.188), (0, -1, 0), item="good.sponge-iron", rate=DRI, temperature=T_HOT_DRI + K, note="hot, from the shaft furnace's discharge (its sponge-iron-out), at its height"),
        port("hv-in", "in", "power", kit.termination(33000, 630), (-37.0, -40.0, 0.3), (0, 1, 0), power=P + TRANSFORMER_LOSS * S_MVA, voltage=33000, note="the site's 33 kV substation"),
        port("lv-bin", "in", "power", lv(need("dri-bin-200t")), (-55.0, 14.8, 1.0), (0, -1, 0), power=need("dri-bin-200t"), voltage=lvf),
        port("lv-cooler", "in", "power", lv(need("offgas-cooler-sealed")), (16.0, 18.0, 1.0), (0, 1, 0), power=need("offgas-cooler-sealed"), voltage=lvf),
        port("lv-filter", "in", "power", lv(need("offgas-filter-sealed")), (36.0, 24.0, 1.0), (-1, 0, 0), power=need("offgas-filter-sealed"), voltage=lvf),
        port("lv-post", "in", "power", lv(need("crew-post")), (26.0, -18.0, 2.3), (-1, 0, 0), power=need("crew-post"), voltage=lvf),
        port("lv-fields", "in", "power", lv(p_fields), (-56.4, -3.5, 0.3), (0, -1, 0), power=p_fields, voltage=lvf, note="the site's 400 V distribution, for the cool fields' pumps"),
        port("lv-hot", "in", "power", lv(n_hot * need(hot_rad)), (-50.0, 52.4, 0.3), (-1, 0, 0), power=n_hot * need(hot_rad), voltage=lvf, note="for the hot field's pump"),
    ]
    flow = low_heat / (4186 * 10)
    w_main = kit.pipe(kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi)), 10, liquid=True, temp=330.0)
    assert kit._recs[w_main]["connector"] == recs["eaf-vessel-80t-sealed"]["ports"][3]["connector"], "the vessel's manifold is flanged for the loop's mains"
    _, dn_steam, dn_cond, dn_t = arc_sizes(recs[hot_rad])
    wt = kit.pipe(dn_t, 10, liquid=True)
    L = lambda *a: kit_link(*a[:4], bays, recs, edge, *a[4:])
    belt_rise = (5.0 + 10.1 + 3.0) - (6.0 - kit._recs[kit.belt(0.8, DRI * 1.5)]["outside"][1] / 2)
    links = [
        L("sponge iron belt", kit.belt(0.8, DRI * 1.5), ("F:dri-out", "V:dri-in"), [None, None], (), 1, DRI * MOON_G[1] * belt_rise / 0.5 + 20000,
          f"a chevron-cleated belt from the bin's feeder east along y 7.8, between the gantry crane's legs and under its bridge, rising {math.degrees(math.atan2(belt_rise, 40.0)):.0f} degrees to its head 3 m over the feed lock; open (no gallery), its bearings and grease vacuum-rated"),
        L("hot sponge iron", drag, ("edge:sponge-iron-in", "F:dri-in"), [None, (-55.0, 55.0, 5.188 - dh), (-55.0, 36.0, bin_top + dh), None], (), 1, 150000.0 * MOON_G[1] / G,
          "from the shaft furnace, south along x -55, up at 30 degrees and over the bin's roof"),
        L("secondary bus", bus, ("T:lv-out", "V:power-in"), bus_path, (), 1, bus_loss,
          f"three phases of water-cooled copper tube, {I2 / 1000:.0f} kA each, up from the transformer's joint and north to the vessel's flexible cables; in vacuum their gaps hold no air to break down, but the gas a seal lets out can (Paschen), so the bars are spaced as if in air; their loss, {bus_loss / 1e3:.0f} kW, goes into their water and so to the cool loop"),
        L("transformer supply", kit.cable(33000, 630), ("edge:hv-in", "T:hv-in"), [None, (-37.0, -6.85, 0.3), (-37.0, -6.85, 8.0), (-12.0, -6.85, 8.0), None], (), math.ceil(i_hv / (630e-6 * kit.J_VACUUM)), None,
          "north along x -37, west of the gantry crane's reach, then east 8 m up over the transformer cooling's lines into the cable box"),
        L("off-gas", kit.lined_duct(1.2, 1, temp=SEALED_GAS_T + K), ("V:offgas-out", "C:gas-in"), [None, (-10.0, 7.5, 20.0), (14.0, 7.5, 20.0), (14.0, 24.0, 20.0), None], (), 1, None,
          f"the furnace's {co:.2f} kg/s of CO at about {SEALED_GAS_T:.0f} deg C and 1 bar (no air dilutes it: a fifth of the shop for air's flow), up from the roof's reducer, east 20 m up between the crane's legs and under its bridge, north past its east end at x 14 and down into the cooler"),
        L("cooled off-gas", kit.pipe(1.2, 1, temp=FILTER_GAS_T + K), ("C:gas-out", "G:gas-in"), [None, None], (), 1, None, f"at {FILTER_GAS_T:.0f} deg C; the filter lets the cleaned gas go up its stack"),
        L("steam to the hot field", kit.pipe(dn_steam, 25, lagged=True, temp=485.0), ("C:steam-out", f"RH1:steam-in"),
          [None, (9.0, 24.4, 16.5), (9.0, 24.4, 1.0), (9.0, 30.6, 1.0), (-56.0, 30.6, 1.0), (-56.0, 51.6, 1.0), None], (), 1, None,
          f"{q_gas / 1.89e6:.2f} kg/s of steam at 20 bar from the cooler's drum, down, north past the slag rail's end and west along y 30.6 to the hot field"),
        L("water from the hot field", kit.pipe(dn_cond, 25, liquid=True, temp=485.0), ("RH1:water-out", "C:water-in"),
          [None, (-57.0, 50.4, 1.0), (-57.0, 30.0, 1.0), (8.0, 30.0, 1.0), (8.0, 23.6, 1.0), (8.0, 23.6, 16.5), None], (), 1, None, "the condensate back by the field's pump, on the steam line's left all the way"),
        L("bin supply", lc(need("dri-bin-200t")), ("edge:lv-bin", "F:power-in"), [None, None]),
        L("cooler supply", lc(need("offgas-cooler-sealed")), ("edge:lv-cooler", "C:power-in"), [None, None]),
        L("filter supply", lc(need("offgas-filter-sealed")), ("edge:lv-filter", "G:power-in"), [None, None]),
        L("crew post supply", lc(need("crew-post")), ("edge:lv-post", "P:power-in"), [None, None], (), 1, None, "through the berm"),
        L("hot field supply", lc(n_hot * need(hot_rad)), ("edge:lv-hot", "RH1:power-in"), [None, None]),
    ]
    # The cool loop: the vessel's panels and the transformer, 330 K, to the fields south-west. Flow (hot, from the
    # vessel) and return mains as a parallel pair, the return on the flow's left all the way, so they never cross; off
    # the platform's west edge, south past the transformer, west inside the crane's legs and south to the rows.
    xf, xr, xl = -58.8, -57.6, -56.4
    rows, yf, yr, yl, row_links = radiator_rows(L, bays, recs, rad, rad_at, xf, xr, xl, lvf)
    links += [
        L("loop flow main", w_main, ("V:water-out", "cap"), [None, (-15.4, 10.95, 7.5), (-21.6, 10.95, 7.5), (-21.6, 10.95, 1.0), (-21.6, -1.0, 1.0), (xf, -1.0, 1.0), (xf, yf(rows[0]) - 1.0, 1.0)], (), 1, None,
          f"{flow:,.0f} kg/s at about 330 K from the vessel's deck manifold to the {n_low} cool fields"),
        L("loop return main", w_main, ("cap", "V:water-in"), [(xr, yr(rows[0]) - 1.0, 1.0), (xr, -1.8, 1.0), (-21.0, -1.8, 1.0), (-21.0, 10.95, 1.0), (-21.0, 10.95, 6.5), (-16.2, 10.95, 6.5), None], (), 1, None,
          "back from the fields at about 10 K less"),
        L("radiator pumps' supply", lc(p_fields), ("edge:lv-fields", "cap"), [None, (xl, yl(rows[0]) - 1.0, 0.3)], (), 1, None, "south along the fields' east side, beside the loop's mains"),
    ]
    links += row_links
    links += [
        L("transformer cooling in", wt, ("tee:loop return main", "T:water-in"), [(-25.2, -1.8, 1.0), (-25.2, -8.38, 1.0), (-17.0, -8.38, 1.0), (-17.0, -8.38, 1.35), None], (), 1, None, "off the return main, a balancing orifice in the main between this pair's tees"),
        L("transformer cooling out", wt, ("T:water-out", "tee:loop return main"), [None, (-18.5, -8.38, 2.35), (-18.5, -2.6, 2.35), (-24.0, -2.6, 2.35), (-24.0, -2.6, 1.0), (-24.0, -1.8, 1.0)]),
    ]
    links += [
        link("rail", "ladle rail", (-10.5, 6.0, 0.0), (33.0, 6.0, 0.0), A36, 2 * 60.0, None, "as for air: two 60 kg/m rails at 3 m gauge, on sintered regolith sleepers; on to the ladle station. A ladle standing in vacuum loses its hydrogen: the degasser's work begins here"),
        link("rail", "slag rail", (-16.5, 0.5, 0.0), (-16.5, 23.25, 0.0), A36, 2 * 60.0, None, "as for air: 2.5 m gauge, on sintered regolith sleepers"),
        link("rail", "crane rails", (-30.0, 0.0, 0.0), (6.0, 0.0, 0.0), A36, 2 * 50.0, None, "the gantry crane's two straight rails, 36 m long along x at y -12 and 12 (24 m apart); the link runs on their centre line. No run crosses them: the crane's legs sweep x -35 to 11 along both"),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-7.5, 6.0, 0.0]},
              {"unit": "unit.slag-pot-car", "rides": "slag rail", "at": [-16.5, 6.0, 0.0]},
              {"unit": "unit.gantry-crane-20t-moon", "rides": "crane rails", "at": [-12.0, 0.0, 0.0]}]
    pads = 44.93 / 13
    ground = [
        {"what": "Furnace platform", "item": SINTER, "volume": 612.06, "note": "as for air (the ladle slot, slag tunnel and well), of sintered regolith blocks: there is no water to make concrete; its blocks and top slab; measured on blender's layout (candidate v01)"},
        {"what": "Transformer slab", "item": SINTER, "volume": 86.4, "note": "no fire walls; measured on blender's layout (candidate v01)"},
        {"what": "Crew post berm", "item": "material.regolith", "volume": 584.56, "note": "loose regolith at least 1 m over the post everywhere (its top 4 x 2 m, 5.3 m up), lying at 35 degrees, its foot about 19 x 17 m, a cavity for the post, the ramp cut and the light wells; measured on blender's layout (candidate v01)"},
        {"what": "Crew post entry and light wells", "item": SINTER, "volume": 14.0, "note": "the landing at the airlock's threshold, its ramp, the cut's retaining sides (200 mm) and the light wells' retainers, provisional; measured on blender's layout (candidate v01)"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r((n_low + n_hot) * pads), "note": f"a block under each leg, {pads:.2f} m3 a field as measured on blender's layout (candidate v01)"},
        {"what": "Rail beds", "item": SINTER, "volume": 17.25, "note": "under the ladle, slag and crane rails; measured on blender's layout (candidate v01)"},
        {"what": "Unit pads", "item": SINTER, "volume": 62.5 + 16.9 + 12.0, "note": "under the off-gas filter (62.5) and the bin (16.9), measured on blender's layout (candidate v01); the cooler's 6 x 4 x 0.5 m, from its size"},
        {"what": "Pipe and trestle feet", "item": SINTER, "volume": 21.35, "note": "under the pipes, the duct's stands and the belts' trestles; as measured on blender's layout (candidate v01) before the runs were re-laid, to measure again"},
    ]
    for gr in ground:
        gr["mass"] = r(gr["volume"] * (RHO_SINTER if gr["item"] == SINTER else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used) + bus_loss
    rejects = sum(u.get("rejects", {}).get("heat", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(gr["mass"] for gr in ground)
    total = mass_units + mass_links + mass_ground
    hp_w = low_heat / (0.5 * 485.0 / (485.0 - 330.0))
    hp_fields = math.ceil((low_heat + hp_w + q_gas) / f_hot)
    against = (f"module.arc-furnace: one box, the same on every world. This layout: the same rate; {power / 1e6:.1f} MW; {heat / 1e6:.1f} MW to {n_low + n_hot} radiator fields ({rejects / 1e6:.1f} MW at lunar noon), "
               f"{n_low} at 330 K for the panels, the transformer and the bus and {n_hot} at 485 K for the off-gas cooler's steam, in place of the tower; no water used up (closed loops); no hall, no baghouse (the furnace is sealed); "
               f"{(mass_units + mass_links) / 1000:,.0f} t of plant and {mass_ground / 1000:,.0f} t of sintered regolith. The radiators cover {(n_low + n_hot) * 48 * 48 / 1e4:.1f} ha: the heat that went up a tower's plume as steam must here leave as light, and at 330 K a panel in the noon sun sheds little. "
               f"The sealed furnace's gas is CO alone ({co:.2f} kg/s), so its cooler takes {q_gas / 1e6:.1f} MW, not the {OFFGAS_SHARE * P / 1e6:.1f} MW the shop for air's duct does (that includes the gas burnt in the air drawn in); the hot field is used at {q_gas / (n_hot * f_hot):.0%}. "
               f"A heat pump lifting the panels' heat to 485 K (at half Carnot's) would need about {hp_w / 1e6:.0f} MW more power and {hp_fields} hot fields in place of all {n_low + n_hot} (not fitted); panels cooled by water at 20 bar and 485 K, as some furnaces' are, would need about {math.ceil((low_heat + q_gas) / f_hot)} (not fitted).")
    xs = [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    layout = {
        "identity": {"key": "layout.arc-furnace-airless", "name": "Arc furnace shop, airless",
                     "description": "The 80 t arc furnace on airless ground at about a moon's gravity: sealed, holding its own gas at 1 bar; no hall (no weather to keep off) but a gantry crane on its own legs, which nothing crosses; its heat to fields of radiator panels instead of a cooling tower, one set cool for the panels' water and one hot for the off-gas cooler's steam; its off-gas cooled, filtered and let go; the crew in a pressurised post under a berm; its platform of sintered regolith. Built separately from the shop for air, not reused."},
        "stands_for": "module.arc-furnace",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": 23.7, "mass": r(total)},
        "bays": bays, "ports": edge, "mobile": mobile, "links": links,
        "ground": ground,
        "totals": {"rate": RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": mod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out from the shop for air: the platform, vault and rails where they were; the rest for airless ground, of runs from port to port (SFO 25). The crane's, the ladle car's and the slag car's feeds are not laid yet."},
                  {"of": ["totals.heat"], "tier": "derived", "note": "The units' and the bus's heat; the radiator fields from the rejection a field gives at lunar noon (unit.radiator-field-330k, -485k)."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Volumes as for air, of sintered regolith at 2,500 kg/m3 and loose regolith at 1,500."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, links and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged; it works from the crew post."}],
    }
    print(f"layout.arc-furnace-airless: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to {n_low}+{n_hot} radiator fields ({f_low / 1e6:.1f} and {f_hot / 1e6:.1f} MW each), off-gas {q_gas / 1e6:.2f} MW")
    return layout, bays


def ladle_ports(us):
    """Ports on the units of the airless ladle station (SFO 25)."""
    K = 273.15
    WATER, AR = "good.water", "material.argon"
    lv = lambda w: kit.termination(400, kit.area_for(amps(w, 400)))
    ar_each = 0.5 * 0.001 * L_RATE                    # kg/s: the recipe's 1 kg of argon a tonne, half through the plug on the stand and half under the furnace's roof (the split invented)
    alloys = sum(i["quantity"] for i in lmod["recipes"][0]["inputs"] if i["item"].startswith("element.") and i["item"] != "element.ar") * L_RATE
    flow = (L_WATER * L_P + TRANSFORMER_LOSS * L_MVA) / (4186 * 10)
    dn = kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi))
    lf = us["ladle-furnace-80t-sealed"]
    lf["desc"] += (f" Its ports (SFO 25): the bus joint at the cables' hanger, (0, -5.34, 6), facing -y; the manifold's water, DN {dn * 1000:.0f} facing -y, out at (-4.8, -4.85, 1) and in at (-4.2, -4.85, 1); the alloy chute's mouth at (0, 3, 9), a belt's head over it; "
                   "the fume out of the roof's elbow at (2, 0, 7.5), facing up, DN 200 (the elbow water-cooled, so the gas leaves it under 400 deg C); argon in at (-5.5, -2.5, 1.5) on the west frame, facing -x.")
    lf["ports"] = [
        port("power-in", "in", "power", kit.bus_joint(L_V2, L_I2, 3), (0.0, -5.34, 6.0), (0, -1, 0), power=L_P, voltage=L_V2, note=f"{L_I2 / 1000:.0f} kA a phase"),
        port("water-out", "out", "liquid", kit.flange(dn, 10, temp=330.0), (-4.8, -4.85, 1.0), (0, -1, 0), item=WATER, rate=L_WATER * L_P / (4186 * 10), temperature=330.0),
        port("water-in", "in", "liquid", kit.flange(dn, 10, temp=330.0), (-4.2, -4.85, 1.0), (0, -1, 0), item=WATER, rate=L_WATER * L_P / (4186 * 10), temperature=320.0),
        port("alloy-in", "in", "bulk", kit.chute(0.4), (0.0, 3.0, 9.0), (0, 0, 1), rate=alloys, note="ferromanganese, ferrosilicon and copper (element.mn, si, cu); through the chute's lock"),
        port("fume-out", "out", "gas", kit.flange(0.2, 1, kit.SS, temp=673.0), (2.0, 0.0, 7.5), (0, 0, 1), item=AR, rate=ar_each, pressure=0.5e5, temperature=673.0, note="mostly argon, with the melt's CO"),
        port("argon-in", "in", "gas", kit.flange(0.05, 16, kit.SS), (-5.5, -2.5, 1.5), (-1, 0, 0), item=AR, rate=ar_each, pressure=10e5, note="fed under the roof to about 50 kPa"),
    ]
    t = us["furnace-transformer-14mva"]
    L_, W_, H_ = t["dims"]
    if "HV cable box" not in [x["what"] for x in t["built_of"]]:
        t["built_of"] = t["built_of"] + [part("HV cable box", A36, 2 * (2.0 * 0.6 + 2.0 * 0.5 + 0.6 * 0.5) * 0.006 * RHO["steel"], "2 x 0.6 x 0.5 m, 6 mm, over the three HV bushings: from its dimensions (to be drawn, v2)")]
        t["desc"] += (f" Fed at 33 kV through a cable box over its HV bushings, its gland plate on top at (0, -0.45, {H_ + 0.5:g}); the secondary's bus joint (SFO 25, the phases 0.6 m apart) at (0, {W_ / 2:g}, {H_ * 2 / 3:.2f}) facing +y; its cooler's water in at (-{L_ / 2:g}, -0.3, 1) and out at (-{L_ / 2:g}, 0.3, 1), facing -x.")
        t["dims"] = (L_, W_, H_ + 0.5)
    q_t = TRANSFORMER_LOSS * L_MVA / (4186 * 10)
    t["ports"] = [
        port("hv-in", "in", "power", kit.termination(33000, kit.area_for(amps(L_MVA * PF, 33e3, PF))), (0.0, -0.45, H_ + 0.5), (0, 0, 1), power=L_MVA * PF, voltage=33000, note="33 kV chosen"),
        port("lv-out", "out", "power", kit.bus_joint(L_V2, L_I2, 3), (0.0, W_ / 2, round(H_ * 2 / 3, 2)), (0, 1, 0), power=L_P, voltage=L_V2),
        port("water-in", "in", "liquid", kit.flange(kit.dn_for(math.sqrt(4 * q_t / 1000 / 2.5 / math.pi)), 10), (-L_ / 2, -0.3, 1.0), (-1, 0, 0), item=WATER, rate=q_t, temperature=320.0),
        port("water-out", "out", "liquid", kit.flange(kit.dn_for(math.sqrt(4 * q_t / 1000 / 2.5 / math.pi)), 10), (-L_ / 2, 0.3, 1.0), (-1, 0, 0), item=WATER, rate=q_t),
    ]
    a = us["alloy-bins-8"]
    a["desc"] += " Its ports (SFO 25): the transfer chute's foot kicked 1 m to +x at 45 degrees, to (1, 0, 7), so the layout's belt passes between the centre and east legs (v2); its 400 V box at (6.3, 0, 1) on the east end frame, facing +x."
    a["ports"] = [
        port("alloy-out", "out", "bulk", kit.chute(0.4), (1.0, 0.0, 7.0), (0, 0, -1), rate=alloys, note="onto a belt's tail"),
        port("power-in", "in", "power", lv(a["needs"]["power"]), (6.3, 0.0, 1.0), (1, 0, 0), power=a["needs"]["power"], voltage=400),
    ]
    d = us["degassing-stand"]
    d["desc"] += " Its ports (SFO 25), on the argon cabinet's -x face: argon in at (-3.3, -3.1, 0.6), DN 50; its 400 V at (-3.3, -2.6, 1.2)."
    d["ports"] = [
        port("argon-in", "in", "gas", kit.flange(0.05, 16, kit.SS), (-3.3, -3.1, 0.6), (-1, 0, 0), item=AR, rate=ar_each, pressure=10e5, note="to the ladle's porous plug by the stand's hose"),
        port("power-in", "in", "power", lv(d["needs"]["power"]), (-3.3, -2.6, 1.2), (-1, 0, 0), power=d["needs"]["power"], voltage=400),
    ]
    g = us["offgas-filter-sealed"]
    g["ports"][0]["optional"] = True
    g["ports"].insert(1, dict(port("fume-in", "in", "gas", kit.flange(0.2, 1, kit.SS, temp=673.0), (-6.0, 1.4, 8.0), (-1, 0, 0), item=AR, rate=ar_each, pressure=0.5e5, temperature=673.0, note="a small nozzle beside the main inlet, for a ladle furnace's fume"), optional=True))
    g["desc"] += " Two inlets (SFO 25), each blinded when unused: a DN 1200 flange at (-6, 0, 8) for an arc furnace's gas and a DN 200 nozzle at (-6, 1.4, 8) for a ladle furnace's fume; its 400 V box at (5, 0, 1), facing +x."




def airless_ladle_units(us):
    """The airless ladle station's and casting bay's own units."""
    out = {}
    lf = dict(us["ladle-furnace-80t"])
    lf["name"] = "Ladle furnace, 80 t, sealed"
    lf["desc"] = ("The 80 t ladle furnace closed for airless ground: its roof's skirt seals on the ladle's rim with a soft ring, glands round the electrodes, the alloys in through a lock in the chute, and argon fed under the roof to about 50 kPa, so the arc burns as in air and the manganese it adds is not boiled off into the vacuum (open to vacuum, an 80 t heat at 1,600 deg C would lose about a fifth of its manganese in a quarter hour). "
                 "Its fume, mostly argon, leaves by the roof's elbow to the layout's filter. Otherwise as unit.ladle-furnace-80t: " + us["ladle-furnace-80t"]["desc"])
    lf["built_of"] = list(us["ladle-furnace-80t"]["built_of"]) + [
        part("Electrode glands", A36, 368.7, "insulated water-cooled glands round the electrodes; measured on blender's model v1"),
        part("Rim seal and gland packing", "material.silicon-carbide", 150.0, "a soft ring of ceramic fibre rope on the roof's skirt (a hot rim burns elastomers), which leaks some argon that the feed makes up, and the glands' packing; silicon carbide stands for the ceramic fibre, from memory"),
        part("Roof locks", A36, 1023.4, "50 kPa under the roof over the ladle's 2.9 m mouth lifts it with about 330 kN; the roof's own weight (about 5.8 t, 9 kN at 1.6 m/s2) barely counts against it, so lock collars on the two lift rods take about 165 kN each through twin 40 x 100 mm straps (about 21 MPa) to the cylinders' fixed heads on the gantry; measured on model v1"),
        part("Alloy lock", A36, 2014.5, "two valves and a chamber in the chute; measured on model v1"),
        part("Lock drives", "good.electric-motors", 500.0, "the locks' actuators, from memory")]
    lf["needs"] = dict(us["ladle-furnace-80t"]["needs"])
    lf["basis"] = us["ladle-furnace-80t"]["basis"] + [{"of": ["identity.description"], "tier": "invented", "review": True, "note": "Manganese's loss: its vapour pressure over a 1% melt (about 40 Pa at 1,873 K, from memory) through Hertz-Knudsen over the ladle's 7 m2. Sealed roofs as on closed furnaces; not built for vacuum."}]
    out["ladle-furnace-80t-sealed"] = lf
    out["degassing-stand"] = dict(
        name="Degassing stand", size_class=2, fits="bay-s2", dims=(5.2, 6.0, 8.0),
        desc=("Where the ladle stands on its car in the open vacuum for a quarter hour while argon through its porous plug stirs the melt: the hydrogen and nitrogen leave of themselves, as they do in a degasser's tank on a world with air, and the carbon and oxygen meet as CO. "
              "A refractory heat shield on a swing arm hangs 1 m over the ladle's open top, with gaps for the gas, so the melt loses less heat by radiation (about 4 MW open, 4 degrees a minute); the argon and a sampling lance come from the stand. Before the alloys, which the sealed ladle furnace adds. The car stops at local (0, 0), travelling along x; the stand's one swing post at (0, -3.5), the shield 3.6 m across with its underside 5.4 m up (1 m over the ladle's rim) on an arm at 6.2 m, the sampling lance parked beside the rim up to 8 m, the argon cabinet at (-2.6, -3.5). Its box is the stand as built, not the bay; drawn with the shield over the ladle, and its argon hose parked with a capped coupling at (-2, -2.3, 1). The porous plug and its line are the ladle's (package.ladle), coupled to the hose at the stand; that connection is not yet drawn on the ladle or its car."),
        built_of=[part("Post, arm, shield frame, lance and argon stand", A36, 11174.6, "the swing post on a 1.2 m square, 50 mm baseplate with eight M36 anchors (the shield and arm, about 7 t at 3.5 m, put about 50 kN m on it at 2 m/s2), the arm, the shield's frame, the lance's carriage and the argon cabinet, other baseplates 30 mm; measured on blender's model v1"),
                  part("Heat shield", REF, 4573.1, "3.6 m across, its refractory; measured on model v1"),
                  part("Slew drive, bearings and valves", "good.electric-motors", 1000.0, "from memory")],
        needs={"power": 30e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "From memory."},
               {"of": ["identity.description"], "tier": "derived", "review": True, "note": "Radiation from 7 m2 of melt at 1,873 K at emissivity 0.9; 80 t at 820 J/kg K."}])
    for cap, trolley, kind, job, gear, key in ((150000.0, 37500.0, "Ladle gantry crane", "on its own legs over the teeming rail and the ladle rail: it lifts the full ladle off its car and holds it over each mould in turn", "a main hoist and a ladle beam", "gantry-crane-150t-moon"),
                                                (20000.0, 6000.0, "Stripper gantry crane", "on its own legs over the teeming rail's east half: it lifts each frozen ingot up out of its mould by the hot top, the mould held down by its lugs, and sets the ingots on the ingot car", "ingot tongs whose arms hold the mould down", "stripper-gantry-20t-moon")):
        c = bridge_crane_unit(cap, 24.0, trolley, kind=kind, job=job, gear=gear, g=MOON_G[1], h_min=0.6)
        legs = 4 * box_tube(1.0, 1.0, 0.025, 23.0) * RHO["steel"] if cap > 50000 else 4 * box_tube(0.8, 0.8, 0.02, 23.0) * RHO["steel"]
        c["built_of"].append(part("Legs and sill beams", A36, legs * 1.15, "four box legs and two sill beams, from the legs' size (the sills 15%)"))
        c["dims"] = (10.0, 25.0, 27.0)
        c["desc"] = c["desc"].replace("on the hall's runways", "on its own legs, running on two ground rails 24 m apart").replace("its origin on the runway's rail plane", "its origin on the ground rails' plane, its girders 24 m up").replace("the hook raised to 0.1 m under the rail plane", "the hook raised under the bridge (its heights below)") + f" Sized for {MOON_G[1]:g} m/s2."
        if cap <= 50000:
            c["dims"] = (10.0, 25.5, 27.0)
            c["built_of"] = [dict(p_, mass=12922.6, note=p_["note"] + "; measured on blender's model v1, 12,922.6 kg") if p_["what"] == "Bridge girders" else p_ for p_ in c["built_of"] if p_["what"] != "Legs and sill beams"]
            c["built_of"] += [part("Legs and sill beams", A36, 53559.3, "four box legs 0.8 x 0.8 m, 20 mm, from the sills' tops at z 1 to 24, and two sill beams; measured on model v1"),
                              part("Seats, braces and rails", A36, 19947.5, "the trolley's rails, the legs' braces and seats, the wheel seats; measured on model v1")]
            c["desc"] += " Its bridge datum is the girders' underside at z 24 (0.6 x 0.8 m, 12 mm; their tops 24.8, the trolley's rails 24.88); the legs, 0.8 x 0.8 m, 20 mm, stand from the sill beams' tops at z 1; parked, the stripping tool (as the air stripper's, 3.24 m tall on its ropes) hangs fully raised under the trolley, its bottom at about z 21.2."
        if cap > 50000:
            c["dims"] = (10.0, 25.4, 27.0)
            c["built_of"] = [dict(p_, mass=17703.3, note=p_["note"] + "; measured on blender's model v1, 17,703.3 kg with its closed end plates") if p_["what"] == "Bridge girders" else p_ for p_ in c["built_of"] if p_["what"] != "Legs and sill beams"]
            c["built_of"] += [part("Legs and sill beams", A36, 82738.1, "four box legs 1 x 1 m, 25 mm, from the sills' tops at z 1 to 24, and two sill beams; measured on model v1"),
                              part("Seats, braces, rails and wheel pockets", A36, 21924.6, "the trolley's rails, the legs' braces and seats, the bridges over the wheel pockets; measured on model v1")]
            c["desc"] += " Its bridge datum is the girders' underside at z 24 (their tops about 25.1, the trolley's rails 25.18); the legs, 1 x 1 m, 25 mm, stand from the sill beams' tops at z 1; parked, the hook's ladle beam hangs at z 22.2 under the full rope stack, not packed up to the girders."
        out[key] = c
    return out


def ladle_station_airless(us, recs):
    """The ladle station on airless ground: the ladle degasses on an open stand, then the sealed ladle furnace alloys and
    reheats it under argon; no degasser, no hall, no ladle crane. Laid of runs from port to port (SFO 25)."""
    rad = "radiator-field-330k"
    bus = kit.busbar(L_V2, L_I2, 3)
    bus_path = [None, (3.0, -6.5, round(recs["furnace-transformer-14mva"]["ports"][1]["at"][2], 3)), (3.0, -6.5, 6.0), None]
    bus_loss = kit._recs[bus]["per_m"]["power"] * (0.65 + 3.87 + 1.16)
    lf_heat = recs["ladle-furnace-80t-sealed"]["needs"].get("heat", 0) + recs["furnace-transformer-14mva"]["needs"].get("heat", 0) + bus_loss
    f_low = recs[rad]["rejects"]["heat"]
    n_low = math.ceil(lf_heat / f_low)
    PL = 2.8                                          # m: the alloy bins' plinth, so their chute's foot is level with the furnace's chute
    rad_at = [(-60.0 - 48.0 * i, -32.0) for i in range(n_low)]
    bays = [
        {"id": "D", "mount": "plant-mount.bay-s2", "unit": "unit.degassing-stand", "at": [-15.0, 0.0, 0.0], "note": "first on the rail from the arc furnace shop: degas before alloying"},
        {"id": "L", "mount": "plant-mount.bay-s2", "unit": "unit.ladle-furnace-80t-sealed", "at": [3.0, 0.0, 0.0], "note": "over the ladle rail"},
        {"id": "T", "mount": "plant-mount.bay-s1", "unit": "unit.furnace-transformer-14mva", "at": [3.0, -8.5, 0.0], "note": "on its slab south of the ladle furnace, inside the crane's legs, no fire walls"},
        {"id": "A", "mount": "plant-mount.bay-s3", "unit": "unit.alloy-bins-8", "at": [2.0, 9.0, PL], "note": f"north of the furnace, inside the crane's legs (it passes over them), on a {PL:g} m plinth so a short level belt reaches the furnace's chute"},
        {"id": "S", "mount": "plant-mount.bay-s1", "unit": "unit.stair-tower-12m", "at": [-6.0, 9.0, PL], "turn": 180.0, "note": "against the bins' west end, on their plinth: its top landing gives onto the bins' deck"},
        {"id": "G", "mount": "plant-mount.bay-s2", "unit": "unit.offgas-filter-sealed", "at": [21.0, 5.5, 0.0], "note": "east, inside the crane's legs, north of the ladle rail: the ladle furnace's argon and fume"},
        {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [24.0, -18.0, 0.0], "note": "south-east, under its berm, which stays outside the crane's rails"},
    ]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{rad}", "at": [x, y, 0.0], "note": "south-west, past the crane's reach"} for i, (x, y) in enumerate(rad_at)]
    lvf = 400
    need = lambda k: recs[k]["needs"]["power"]
    lv = lambda w: kit.termination(lvf, kit.area_for(amps(w, lvf)))
    lc = lambda w: kit.cable(lvf, kit.area_for(amps(w, lvf)))
    p_fields = n_low * need(rad)
    ar = 0.001 * L_RATE
    hv = recs["furnace-transformer-14mva"]["ports"][0]
    edge = [
        port("argon-in", "in", "gas", kit.flange(0.05, 16, kit.SS), (-30.0, -4.5, 0.6), (1, 0, 0), item="material.argon", rate=ar, pressure=10e5, note="the site's argon, at 10 bar"),
        port("hv-in", "in", "power", hv["connector"], (18.0, -30.0, 0.3), (0, 1, 0), power=hv["power"], voltage=33000, note="the site's 33 kV substation"),
        port("lv-stand", "in", "power", lv(need("degassing-stand")), (-24.0, -2.6, 1.2), (1, 0, 0), power=need("degassing-stand"), voltage=lvf),
        port("lv-bins", "in", "power", lv(need("alloy-bins-8")), (13.0, 9.0, PL + 1.0), (-1, 0, 0), power=need("alloy-bins-8"), voltage=lvf),
        port("lv-filter", "in", "power", lv(need("offgas-filter-sealed")), (31.0, 5.5, 1.0), (-1, 0, 0), power=need("offgas-filter-sealed"), voltage=lvf),
        port("lv-post", "in", "power", lv(need("crew-post")), (32.0, -18.0, 2.3), (-1, 0, 0), power=need("crew-post"), voltage=lvf),
        port("lv-fields", "in", "power", lv(p_fields), (-32.4, -8.5, 0.3), (0, -1, 0), power=p_fields, voltage=lvf, note="the site's 400 V distribution, for the fields' pumps"),
    ]
    flow = lf_heat / (4186 * 10)
    w_main = kit.pipe(kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi)), 10, liquid=True, temp=330.0)
    assert kit._recs[w_main]["connector"] == recs["ladle-furnace-80t-sealed"]["ports"][1]["connector"], "the furnace's manifold is flanged for the loop's mains"
    q_t = TRANSFORMER_LOSS * L_MVA / (4186 * 10)
    wt = kit.pipe(kit.dn_for(math.sqrt(4 * q_t / 1000 / 2.5 / math.pi)), 10, liquid=True)
    ar_pipe = kit.pipe(0.05, 16, kit.SS)
    belt = kit.belt(0.65, recs["alloy-bins-8"]["ports"][0]["rate"] * 10)
    L = lambda *a: kit_link(*a[:4], bays, recs, edge, *a[4:])
    links = [
        L("alloy belt", belt, ("A:alloy-out", "L:alloy-in"), [None, None], (), 1, 20000.0,
          "level, 6 m: from the bins' chute south between their centre and east legs, under their deck, to the furnace's alloy chute, its head through a hood into the chute's mouth; open, its bearings vacuum-rated"),
        L("secondary bus", bus, ("T:lv-out", "L:power-in"), bus_path, (), 1, bus_loss,
          f"three phases of water-cooled copper tube, {L_I2 / 1000:.0f} kA each, up from the transformer's joint to the furnace's cables; their loss, {bus_loss / 1e3:.1f} kW, to the cool loop"),
        L("transformer supply", kit.cable(33000, kit.area_for(amps(L_MVA * PF, 33e3, PF))), ("edge:hv-in", "T:hv-in"),
          [None, (18.0, -8.95, 0.3), (18.0, -8.95, 4.9), (3.0, -8.95, 4.9), None], (), 1, None, "north along x 18, east of the crane's reach, then west 4.9 m up into the cable box"),
        L("fume to the filter", kit.pipe(0.2, 1, kit.SS, temp=673.0), ("L:fume-out", "G:fume-in"), [None, (5.0, 0.0, 9.5), (5.0, 6.9, 9.5), (13.0, 6.9, 9.5), (13.0, 6.9, 8.0), None], (), 1, None,
          f"{ar / 2:.3f} kg/s of argon and the melt's CO at about 50 kPa: up from the roof's elbow, north out of the furnace's open side and east under the bins' deck to the filter's small inlet; the filter lets it go up its stack (argon is scarce on a moon: a later unit could take it back)"),
        L("argon main", ar_pipe, ("edge:argon-in", "L:argon-in"), [None, (-4.0, -4.5, 0.6), (-4.0, -2.5, 0.6), (-4.0, -2.5, 1.5), None], (), 1, None, "along y -4.5, south of the ladle car's path, to the furnace's west frame"),
        L("argon to the stand", ar_pipe, ("tee:argon main", "D:argon-in"), [(-19.5, -4.5, 0.6), (-19.5, -3.1, 0.6), None]),
        L("stand supply", lc(need("degassing-stand")), ("edge:lv-stand", "D:power-in"), [None, None]),
        L("bins supply", lc(need("alloy-bins-8")), ("edge:lv-bins", "A:power-in"), [None, None], (), 1, None, "along the plinth"),
        L("filter supply", lc(need("offgas-filter-sealed")), ("edge:lv-filter", "G:power-in"), [None, None]),
        L("crew post supply", lc(need("crew-post")), ("edge:lv-post", "P:power-in"), [None, None], (), 1, None, "through the berm"),
    ]
    # The cool loop, 330 K: flow (from the furnace) and return mains as a parallel pair, west inside the crane's legs
    # past its reach, then south to the fields; the return on the flow's left all the way.
    xf, xr, xl = -34.8, -33.6, -32.4
    rows, yf, yr, yl, row_links = radiator_rows(L, bays, recs, rad, rad_at, xf, xr, xl, lvf)
    links += [
        L("loop flow main", w_main, ("L:water-out", "cap"), [None, (-1.8, -6.0, 1.0), (xf, -6.0, 1.0), (xf, yf(rows[0]) - 1.0, 1.0)], (), 1, None, f"{flow:,.1f} kg/s at about 330 K to the {n_low} fields"),
        L("loop return main", w_main, ("cap", "L:water-in"), [(xr, yr(rows[0]) - 1.0, 1.0), (xr, -6.6, 1.0), (-1.2, -6.6, 1.0), None], (), 1, None, "back at about 10 K less"),
        L("radiator pumps' supply", lc(p_fields), ("edge:lv-fields", "cap"), [None, (xl, yl(rows[0]) - 1.0, 0.3)], (), 1, None, "south along the fields' east side, beside the loop's mains"),
    ]
    links += row_links
    links += [
        L("transformer cooling in", wt, ("tee:loop return main", "T:water-in"), [(-4.2, -6.6, 1.0), (-4.2, -8.8, 1.0), None], (), 1, None, "off the return main, a balancing orifice in the main between this pair's tees"),
        L("transformer cooling out", wt, ("T:water-out", "tee:loop return main"), [None, (-3.0, -8.2, 1.0), (-3.0, -6.6, 1.0)]),
        link("rail", "ladle rail", (-30.0, 0.0, 0.0), (30.0, 0.0, 0.0), A36, 2 * 60.0, None, "two 60 kg/m rails at 3 m gauge on sintered regolith sleepers, from the arc furnace shop to the casting bay; the car stops at x -15 (the stand) and 3 (the ladle furnace)"),
        link("rail", "crane rails", (-24.0, 1.0, 0.0), (12.0, 1.0, 0.0), A36, 2 * 50.0, None, "the maintenance gantry crane's two straight rails over the stand, the furnace and the bins, at y -11 and 13 (24 m apart); the link runs on their centre line. No run crosses them: the crane's legs sweep x -29 to 17 along both"),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-15.0, 0.0, 0.0]},
              {"unit": "unit.gantry-crane-20t-moon", "rides": "crane rails", "at": [-6.0, 1.0, 0.0]}]
    plinth = 16.0 * 6.3 * PL
    ground = [
        {"what": "Ladle furnace foundation", "item": SINTER, "volume": 102.0, "note": "as for air, of sintered regolith; measured on blender's layout (candidate v01)"},
        {"what": "Stand and transformer pads", "item": SINTER, "volume": 41.6, "note": "the degassing stand's (20.0) and the transformer's (21.6); measured on blender's layout (candidate v01)"},
        {"what": "Bins' plinth", "item": SINTER, "volume": r(plinth), "note": f"16 x 6.3 m under the alloy bins and the stair tower, {PL:g} m high, sintered regolith blocks, with a ramp for the crews (not counted)"},
        {"what": "Filter pad", "item": SINTER, "volume": 62.5, "note": "measured on blender's layout (candidate v01)"},
        {"what": "Crew post berm", "item": "material.regolith", "volume": 584.56, "note": "loose regolith at least 1 m over the post everywhere, lying at 35 degrees, its foot about 19 x 17 m, a cavity for the post, the ramp cut and the light wells; measured on blender's layout (candidate v01)"},
        {"what": "Crew post entry and light wells", "item": SINTER, "volume": 14.0, "note": "the landing, the ramp, the cut's retaining sides and the light wells' retainers, provisional; measured on blender's layout (candidate v01)"},
        {"what": "Rail sleepers", "item": SINTER, "volume": 12.77, "note": "under the ladle rail (8.0) and the crane's rails (4.8); measured on blender's layout (candidate v01)"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(6.91 / 2 * n_low), "note": "a block under each leg; 3.46 m3 a field, as measured on blender's layout (candidate v01)"},
        {"what": "Feet", "item": SINTER, "volume": 3.05, "note": "under the belt, the pipes' and the cables' supports; as measured on blender's layout (candidate v01) before the runs were re-laid, to measure again"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO_SINTER if g["item"] == SINTER else RHO["steel"] if g["item"] == A36 else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used) + bus_loss
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    against = (f"module.ladle-station: one box, the same on every world. This layout: the same rate; {power / 1e6:.1f} MW; {heat / 1e6:.1f} MW to {n_low} radiator fields at 330 K; no water used up; {ar:.3f} kg/s of argon, let go. "
               f"No degasser: the vacuum degasses the ladle on an open stand, but must not see the alloys, which the sealed ladle furnace adds under argon; no ladle crane (nothing is lifted into a tank) and no hall. "
               f"{(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of sintered regolith.")
    xs = [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    layout = {
        "identity": {"key": "layout.ladle-station-airless", "name": "Ladle station, airless",
                     "description": "The ladle station on airless ground at about a moon's gravity: the heat comes on its car from the arc furnace shop, stands a quarter hour on an open degassing stand where the vacuum takes its gases, then goes under the sealed ladle furnace, which alloys and reheats it under argon, and on to the casting bay. Radiators for the tower, a maintenance gantry crane that nothing crosses, a pressurised crew post. Built separately from the station for air."},
        "stands_for": "module.ladle-station",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": 27.0, "mass": r(total)},
        "bays": bays, "ports": edge, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": L_RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": lmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out for airless ground, of runs from port to port (SFO 25). The crane's and the ladle car's feeds are not laid yet."},
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
        link("rail", "ladle gantry rails", (-33.0, 0.0, 0.0), (0.0, 0.0, 0.0), A36, 2 * 60.0, None, "the ladle gantry's two straight rails over the west half, at y -12 and 12; the link runs on their centre line"),
        link("rail", "stripper gantry rails", (0.0, 0.0, 0.0), (33.0, 0.0, 0.0), A36, 2 * 50.0, None, "the stripper gantry's two straight rails over the east half, at y -12 and 12, beside the ladle gantry's ends; the link runs on their centre line"),
    ]
    mobile = [{"unit": "unit.ladle-car-80t", "rides": "ladle rail", "at": [-15.0, 6.0, 0.0]}]
    mobile += [{"unit": "unit.mould-car-12", "rides": "teeming rail", "at": [round(-27.0 + car_L / 2 + i * car_L, 2), -6.0, 0.0]} for i in range(CARS)]
    mobile += [{"unit": "unit.mould-car-12", "rides": "teeming rail", "at": [round(3.0 + car_L / 2 + i * car_L, 2), -6.0, 0.0]} for i in range(CARS)]
    mobile += [{"unit": "unit.ingot-car-40t", "rides": "ingot rail", "at": [15.0, 6.0, 0.0]},
               {"unit": "unit.gantry-crane-150t-moon", "rides": "ladle gantry rails", "at": [-15.0, 0.0, 0.0]},
               {"unit": "unit.stripper-gantry-20t-moon", "rides": "stripper gantry rails", "at": [15.0, 0.0, 0.0]}]
    bays = [{"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [0.0, -24.0, 0.0], "note": "south, under its berm, which stays clear of the gantries' south rails at y -12"}]
    ground = [
        {"what": "Casting floor", "item": SINTER, "volume": 560.09, "note": "72 x 20 m, 0.4 m of sintered regolith at grade under the three rails (x -36 to 36, y -10 to 10), so spilt steel meets stone; its rails set in 0.4 m channels that take the cars' treads and flanges; measured on blender's layout (candidate v01)"},
        {"what": "Gantry rail sleepers", "item": SINTER, "volume": 132.0, "note": "88 bearing blocks under the gantries' rails; measured on blender's layout (candidate v01)"},
        {"what": "Crew post berm", "item": "material.regolith", "volume": 584.56, "note": "loose regolith at least 1 m over the post everywhere, lying at 35 degrees, a cavity for the post, the ramp cut and the light wells; measured on blender's layout (candidate v01)"},
        {"what": "Crew post entry and light wells", "item": SINTER, "volume": 14.0, "note": "the landing, the ramp, the cut's retaining sides and the light wells' retainers, provisional; measured on blender's layout (candidate v01)"},
    ]
    measured = {"ladle rail": 2879.0, "teeming rail": 7197.0, "ingot rail": 3958.0, "ladle gantry rails": 4539.0, "stripper gantry rails": 4539.0}
    for l in links:
        if l["name"] in measured:
            l["mass"] = r(measured[l["name"]])
            l["note"] = l["note"] + "; its mass measured on blender's layout (candidate v01): " + ("68.8 kg/m gantry rail" if "gantry" in l["name"] else "60 kg/m rail")
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
    d_b = math.ceil(main_dia(mol_flow(GAS), T_GAS, HP_SHAFT) * 10) / 10     # the bore, rounded up to the next 0.1 m
    R_b = Do / 2 + 0.3 + d_b / 2
    side = max(12.0, round(2 * R_b + d_b + 1.0))
    out["shaft-furnace-200th-hp"] = dict(
        name=f"Hydrogen shaft furnace, {S_RATE * 3.6:.0f} t an hour, 8 bar, hot discharge", size_class=2 if side <= 12 else 3, fits="bay-s2" if side <= 12 else "bay-s3", dims=(side, side, round(H, 1)),
        desc=(f"The shaft of unit.shaft-furnace-200th built for airless ground: the gas loop at about {HP_SHAFT / 1e5:.0f} bar, so the shell is a pressure vessel ({hoop / 1e6:.0f} MPa in its {t_sh * 1000:.0f} mm plate) and the charge and discharge go through pressure locks; "
              f"no cooling zone: the sponge iron leaves through a lock at about {T_HOT_DRI:.0f} deg C into a closed hot conveyor to the arc furnace shop, which in vacuum it cannot rust on the way. "
              f"Local frame: the axis at (0, 0); the discharge lock under the cone, its top valve at z {Z_OUT:g} (7.8 to 8), its vessel 6.4 to 7.7, its lower valve 6.2 to 6.4, its outlet at z {Z_OUT - 2:g} onto the hot conveyor; the ring girder at z {z_ring:.0f} (radius {Do / 2 + 0.4:.1f} m); the bustle main at z {z_bustle:.0f}, a ring {2 * R_b:.2f} m across (radius {R_b:.2f} to the pipe's centre) of {d_b:.1f} m clear bore inside 200 mm of lining and a 25 mm shell ({d_b + 0.45:.2f} m outside), its inlet on -y (its flange the same {d_b:.1f} m bore); the top gas outlet at z {z_top - 1:.0f} on +x; the charge lock and hopper z {z_top:.0f} to {H:.0f}. About {burden / 1000:,.0f} t of burden inside."),
        built_of=[part("Shell", A36, 177422.7, f"{Do:.2f} m across, {hot:g} m with its cone, roof and bands, {t_sh * 1000:.0f} mm plate; measured on blender's model v1"),
                  part("Lining", REF, lining, f"{t_ref * 1000:.0f} mm"),
                  part("Bustle main", A36, 43733.0, f"a ring {2 * R_b:.2f} m across, 25 mm plate for the pressure; measured on model v1"),
                  part("Bustle lining", REF, math.pi * (d_b + 0.2) * 0.2 * 2 * math.pi * R_b * RHO["refractory"], "200 mm"),
                  part("Support ring girder", A36, 35860.4, "a hollow ring and its land, as for air; measured on model v1"),
                  part("Struts and seats", A36, 2442.4, "eight 200 x 200 x 12 mm struts and their 40 mm seats; measured on model v1"),
                  part("Charge and discharge pressure locks", A36, 90000.0, "two lock hoppers with their valves, from memory"),
                  part("Burden feeders", "good.electric-motors", 25000.0, "from memory"),
                  part("Drives' terminal box", A36, 350.0, "a 400 V box on the ring girder's west face feeding the locks', valves' and feeders' drives, 0.8 x 0.6 x 1 m; from memory")],
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
        built_of=[part("Shells and heads", "material.stainless-steel-304", 29866.3, "two shells 3 m across, 12 m overall with elliptical heads, 16 mm (about 80 MPa at 8.5 bar); measured on blender's model v1"),
                  part("Tubes", "material.stainless-steel-304", 30000.0, "from memory"),
                  part("Filter and drain pot", A36, 9462.2, "the candle filter 2 m across, 20 mm, and the drain pot 1 m, 12 mm; measured on model v1"),
                  part("Filter candles", "material.silicon-carbide", 2000.0, "from memory"),
                  part("Skirts, supports and feet", A36, 15738.5, "measured on model v1"),
                  part("Piping", A36, 27245.4, "the gas headers and pipes, the coolant pair and the drain, ending plain at the SFO 25 ports (the condensate stub out along +x at (6, 0, 0.5)); measured on blender's model v2")],
        needs={"heat": HP_Q_COND + HP_W_COMP, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "From memory."},
               {"of": ["needs.heat"], "tier": "derived", "note": f"The top gas cooled {T_REC_OUT:.0f} to {T_COND:.0f} deg C and its water condensed down to 6% of the recycle (2.33 MJ/kg); with the compressor's aftercooler."}])
    out["recycle-compressor-8bar"] = dict(
        name=f"Recycle compressor, {HP_W_COMP / 1e6:.1f} MW, 8 bar", size_class=2, fits="bay-s2", dims=(12.0, 6.0, 5.0),
        desc=f"Raises {GAS:.1f} kg/s of hydrogen from {HP_SUCTION / 1e5:.1f} to {HP_SHAFT / 1e5:.1f} bar, the loop's pressure drop only: two centrifugal stages in one casing and a motor. At 8 bar the gas is dense enough that this is a small machine. Suction at local (6, 0, 1.5), discharge at (-6, 0, 1.5); the casing's axis along y at (0, 1.3, 2.5), the motor behind it (y -2.6 to -0.4); the motor's 6.6 kV terminal box on top, its cable entry at (0, -1.5, 5); its water cooler's flanges low on the -x face at (-6, -2.2, 0.6) and (-6, -1.6, 0.6), over the skid's end beam, for the motor's losses (about 4%) to the layout's loop.",
        built_of=[part("Casing, rotor and motor", "material.high-strength-steel-4340", 8000.0 * HP_W_COMP / 1e6, "8 t a MW of drive, from memory (the 13 t of casing and motor walls drawn on blender's model v1 are within it)"),
                  part("Skid and supports", A36, 15631.2, "with two saddle pads; measured on blender's model v2"),
                  part("Gas and coolant piping", A36, 6362.9, "the suction and discharge stubs and the 100 mm cooling pair to the motor's jacket, ending plain at the SFO 25 ports; measured on blender's model v2")],
        needs={"power": HP_W_COMP / 0.96, "atmosphere": "any"},
        basis=[{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": "From memory."},
               {"of": ["needs"], "tier": "derived", "note": f"Isentropic at {ETA_K:.0%}, a motor at 96%; its heat leaves by the condenser's aftercooler."}])
    hh = dict(us[next(k for k in us if k.startswith("gas-heater-"))])
    hh["name"] = f"Hydrogen heater, {HP_Q_HEATER / 1e6:.0f} MW, 8 bar"
    hh["desc"] = hh["desc"].replace(f"from {T_PRE:.0f} to", f"from {HP_T_PRE:.0f} to") + " Its vessels hold the loop's 8 bar (25 mm plate at 4 m: 64 MPa, the shell kept cool by its lining)."
    hh["desc"] = hh["desc"].replace("the cables entering the middle of its +y face at (-11.55, 2.6, 1.575)", "the cables entering the house's -x side, on the unit's face, at (-12, 1.45, 1.575), so they come to it from outside the unit")
    # Its ends reduced to the SFO 25 ports, plain, flanges and caps gone: blender's model v2, the changed steel only.
    v2 = {"Cold header": (1342.6, 811.7, "its inlet a reducer to the 1 m port"), "Hot header and outlet": (4309.0 + 29445.0, 3799.8 + 28841.0, "its outlet a lined reducer to the 2.4 m port, the header opened under it"), "Flanges and blind ends": (2885.6, 0.0, "the inlet's and outlet's flanges and caps gone (the connector is the layout's)")}
    hh["built_of"] = [dict(p, mass=r(p["mass"] - v2[p["what"]][0] + v2[p["what"]][1], 6), note=p.get("note", "") + f"; {v2[p['what']][2]}, measured on blender's model v2") if p["what"] in v2 else p for p in hh["built_of"]]
    hh["needs"] = {"power": HP_Q_HEATER / 0.98}
    out[f"gas-heater-{HP_Q_HEATER / 1e6:.0f}mw-8bar"] = hh
    out[f"radiator-field-{HP_T_LOOP:.0f}k"] = radiator_unit(HP_T_LOOP, "the condenser's")
    rf = out[f"radiator-field-{HP_T_LOOP:.0f}k"]
    assert "its ports at local (-24, -0.6, 1) flow and (-24, 0.6, 1) return." in rf["desc"]
    rf["built_of"] = [dict(q, mass=r(q["mass"] + 137.79, 6), note=q.get("note", "") + "; with the branch, posts and union housings to the south-west ports, 138 kg more, measured on blender's model v3") if q["what"].startswith("Headers") else q for q in rf["built_of"]]
    rf["desc"] = rf["desc"].replace("its ports at local (-24, -0.6, 1) flow and (-24, 0.6, 1) return.", "its ports on its south-west corner facing -y, at local (-23, -24, 1) flow and (-22.4, -24, 1) return, its pump's power entry at (-21.8, -24, 0.3), so a layout's mains run in an aisle beside the field, not under it.")
    return out


# ---------------------------------------------------------------- ports (SFO 25), the trial: the airless shaft furnace's units

def port(pid, flow, medium, connector, at, facing, item=None, rate=None, power=None, pressure=None, temperature=None, voltage=None, note=None):
    d = {"id": pid, "flow": flow, "medium": medium}
    for k, v in (("item", item), ("rate", rate), ("power", power), ("pressure", pressure), ("temperature", temperature), ("voltage", voltage)):
        if v is not None:
            d[k] = r(v) if isinstance(v, float) else v
    d |= {"connector": connector, "at": [float(x) for x in at], "facing": [float(x) for x in facing]}
    if note:
        d["note"] = note
    return d


def amps(watts, volts, pf=0.9):
    return watts / (math.sqrt(3) * volts * pf)


def hp_sizes():
    """The airless shaft furnace's lines, sized once for its units' ports and its layout's runs."""
    d_top = main_dia(mol_flow(H2_TOP, STEAM), T_TOP, HP_SUCTION)
    d_gas = main_dia(mol_flow(GAS), T_COND, HP_SHAFT)
    d_hot = main_dia(mol_flow(GAS), T_GAS, HP_SHAFT)
    return {"top": kit.dn_for(d_top), "gas": kit.dn_for(d_gas), "hot": next(n for n in kit.DN if n - 0.4 >= 0.98 * d_hot), "need": (d_top, d_gas, d_hot)}


def trial_ports(us):
    """Ports on the units of the airless shaft furnace (the SFO 25 trial). Positions in each unit's frame, as its
    description gives them; connectors sized for the 8 bar loop."""
    z = hp_sizes()
    K = 273.15
    T_DIS = (T_COND + K) * (1 + ((HP_SHAFT / HP_SUCTION) ** ((GAMMA - 1) / GAMMA) - 1) / ETA_K)
    top, wet = H2_TOP + STEAM, H2_TOP + _left
    gas = GAS + _left
    g_top = kit.flange(z["top"], 16, sealing="double", temp=T_TOP + K)
    g_wet = kit.flange(z["gas"], 16, sealing="double", temp=T_TOP + K)
    g_hot = kit.lined_flange(z["hot"], 16, sealing="double")
    w_main = kit.flange(0.9, 10)
    rad = f"radiator-field-{HP_T_LOOP:.0f}k"
    q_rad = us[rad]["rejects"]["heat"] / (4186 * 10)
    q_t = us["heater-transformer-131mva"]["needs"]["heat"] / (4186 * 10)
    q_k = 0.04 * HP_W_COMP / (4186 * 10)
    q_s = us["top-gas-condenser-8bar"]["needs"]["heat"] / (4186 * 10)
    H2 = "material.hydrogen"
    WATER = "good.water"
    i_heater = amps(us["gas-heater-119mw-8bar"]["needs"]["power"], 6600, 0.95)
    n_heater = math.ceil(i_heater / (630e-6 * kit.J_VACUUM))
    lv = lambda w: kit.termination(400, kit.area_for(amps(w, 400)))
    us["shaft-furnace-200th-hp"]["ports"] = [
        port("charge-in", "in", "bulk", kit.belt_transfer(1.2), (0, 0, 40), (0, 0, 1), item="good.iron-ore-pellets", rate=PELLETS, note="the charge lock's hopper rim; a belt's head stands 3 m over it"),
        port("top-gas-out", "out", "gas", g_top, (6.0, 0, 31), (1, 0, 0), item=H2, rate=top, pressure=HP_SUCTION, temperature=T_TOP + K, note="hydrogen with the reduction's steam"),
        port("hot-gas-in", "in", "gas", g_hot, (0, -6.5, 16), (0, -1, 0), item=H2, rate=GAS, pressure=HP_SHAFT, temperature=T_GAS + K, note=f"the bustle main's inlet stub; the bustle itself 1.9 m clear, the stub's flange {z['hot'] - 0.4:.1f} m"),
        port("dri-out", "out", "bulk", kit.chute(0.8), (0, 0, 6), (0, 0, -1), item="good.sponge-iron", rate=S_RATE, temperature=T_HOT_DRI + K, note="the discharge lock's outlet, onto a closed conveyor's tail under it"),
        port("power-in", "in", "power", lv(us["shaft-furnace-200th-hp"]["needs"]["power"]), (-4.7, 2.0, 12.0), (-1, 0, 0), power=us["shaft-furnace-200th-hp"]["needs"]["power"], voltage=400, note="the drives' terminal box on the ring girder's west face"),
    ]
    us["recuperator"]["ports"] = [
        port("top-gas-in", "in", "gas", g_top, (0, 0, 18), (0, 0, 1), item=H2, rate=top, pressure=HP_SUCTION, temperature=T_TOP + K),
        port("wet-out", "out", "gas", g_wet, (0, 5.2, 1.5), (0, 1, 0), item=H2, rate=top, pressure=HP_SUCTION, temperature=103.0 + K),
        port("clean-in", "in", "gas", g_wet, (0, -5.2, 1), (0, -1, 0), item=H2, rate=gas, pressure=HP_SHAFT, temperature=T_DIS),
        port("warm-out", "out", "gas", g_wet, (0, -5.2, 16.75), (0, -1, 0), item=H2, rate=gas, pressure=HP_SHAFT, temperature=HP_T_PRE + K),
    ]
    us["recuperator"]["ports_note"] = "flanged for the 8 bar loop of the airless shaft furnace; a layout at lower pressure reduces to them"
    us["top-gas-condenser-8bar"]["ports"] = [
        port("gas-in", "in", "gas", g_wet, (-6, 0, 12), (-1, 0, 0), item=H2, rate=top, pressure=HP_SUCTION, temperature=103.0 + K),
        port("gas-out", "out", "gas", g_wet, (6, 0, 2), (1, 0, 0), item=H2, rate=wet, pressure=HP_SUCTION, temperature=T_COND + K),
        port("condensate-out", "out", "liquid", kit.flange(0.2, 10, kit.SS), (6, 0, 0.5), (1, 0, 0), item=WATER, rate=STEAM - _left, temperature=T_COND + K),
        port("water-in", "in", "liquid", w_main, (-0.6, -4, 1), (0, -1, 0), item=WATER, rate=q_s, temperature=HP_T_LOOP - 10.0),
        port("water-out", "out", "liquid", w_main, (0.6, -4, 1), (0, -1, 0), item=WATER, rate=q_s, temperature=HP_T_LOOP),
    ]
    us["recycle-compressor-8bar"]["ports"] = [
        port("suction", "in", "gas", g_wet, (6, 0, 1.5), (1, 0, 0), item=H2, rate=gas, pressure=HP_SUCTION, temperature=T_COND + K),
        port("discharge", "out", "gas", g_wet, (-6, 0, 1.5), (-1, 0, 0), item=H2, rate=gas, pressure=HP_SHAFT, temperature=T_DIS),
        port("hv", "in", "power", kit.termination(6600, kit.area_for(amps(HP_W_COMP / 0.96, 6600))), (0, -1.5, 5.0), (0, 0, 1), power=HP_W_COMP / 0.96, voltage=6600),
        port("cooling-in", "in", "liquid", kit.flange(0.1, 10), (-6, -2.2, 0.6), (-1, 0, 0), item=WATER, rate=q_k, temperature=HP_T_LOOP - 10.0),
        port("cooling-out", "out", "liquid", kit.flange(0.1, 10), (-6, -1.6, 0.6), (-1, 0, 0), item=WATER, rate=q_k),
    ]
    us["gas-heater-119mw-8bar"]["ports"] = [
        port("gas-in", "in", "gas", g_wet, (-12, -3.3, 2), (-1, 0, 0), item=H2, rate=gas, pressure=HP_SHAFT, temperature=HP_T_PRE + K),
        port("gas-out", "out", "gas", g_hot, (0, 4, 18), (0, 0, 1), item=H2, rate=gas, pressure=HP_SHAFT, temperature=T_GAS + K, note=f"the hot header's stub, reduced to the {z['hot'] * 1000:.0f} mm lined flange"),
        port("power-in", "in", "power", kit.termination(6600, 630), (-12, 1.45, 1.575), (-1, 0, 0), power=us["gas-heater-119mw-8bar"]["needs"]["power"], voltage=6600, note=f"{n_heater} circuits of 630 mm2 into the terminal house"),
    ]
    tp = us["heater-transformer-131mva"]["needs"]["power"] + us["gas-heater-119mw-8bar"]["needs"]["power"]
    us["heater-transformer-131mva"]["ports"] = [
        port("hv-in", "in", "power", kit.termination(132000, kit.area_for(amps(tp, 132000, 0.95))), (0, -0.95625, 7.55), (0, 0, 1), power=tp, voltage=132000, note="the HV cable box's gland plate, over the three bushings; 132 kV chosen"),
        port("lv-out", "out", "power", kit.termination(6600, 630), (0, 3.35, 4.5), (0, 1, 0), power=us["gas-heater-119mw-8bar"]["needs"]["power"], voltage=6600, note=f"the secondary terminals, {n_heater} circuits"),
        port("water-in", "in", "liquid", kit.flange(0.15, 10), (-4.55, -0.6, 1), (-1, 0, 0), item=WATER, rate=q_t, temperature=HP_T_LOOP - 10.0),
        port("water-out", "out", "liquid", kit.flange(0.15, 10), (-4.55, 0.6, 1), (-1, 0, 0), item=WATER, rate=q_t),
    ]
    us["crew-post"]["ports"] = [port("power-in", "in", "power", lv(us["crew-post"]["needs"]["power"]), (3, 0, 2.3), (1, 0, 0), power=us["crew-post"]["needs"]["power"], voltage=400, note="a gland on a pad on its +x end's dome, on the axis; the cable through the berm")]
    us[rad]["ports"] = [
        port("water-in", "in", "liquid", kit.flange(0.15, 10), (-23.0, -24, 1), (0, -1, 0), item=WATER, rate=q_rad, temperature=HP_T_LOOP, note="its south-west corner, facing the aisle"),
        port("water-out", "out", "liquid", kit.flange(0.15, 10), (-22.4, -24, 1), (0, -1, 0), item=WATER, rate=q_rad, temperature=HP_T_LOOP - 10.0),
        port("power-in", "in", "power", lv(us[rad]["needs"]["power"]), (-21.8, -24, 0.3), (0, -1, 0), power=us[rad]["needs"]["power"], voltage=400, note="its circulating pump"),
    ]
    for k in ("recuperator",):
        us[k].pop("ports_note", None)


def world(bay, local):
    a = math.radians(bay.get("turn", 0.0))
    x, y, zz = local
    return (round(bay["at"][0] + x * math.cos(a) - y * math.sin(a), 3), round(bay["at"][1] + x * math.sin(a) + y * math.cos(a), 3), round(bay["at"][2] + zz, 3))


def end_offset(conn_kind, run_rec, which):
    """Where a run's end stands from its port: a belt's head 2 m over the hopper it drops into; a closed conveyor's
    tail under the chute it takes from, by half its casing."""
    if conn_kind == "belt-transfer" and which == 1:
        return (0.0, 0.0, 3.0)
    if conn_kind == "chute":            # (a closed conveyor's tail takes in on top, its head lets out underneath)
        return (0.0, 0.0, -run_rec["outside"][1] / 2 if which == 0 else run_rec["outside"][1] / 2)
    return (0.0, 0.0, 0.0)


KIND_OF_RUN = {"pipe": "pipe", "lined-duct": "duct", "belt": "conveyor", "drag-conveyor": "conveyor", "cable": "cable", "busbar": "busbar"}


def kit_link(name, run, ends, path, bays, recs, edge, taps=(), count=1, power=None, note=None, reversible=False):
    """A layout's link laid of run class `run` (SFO 25). ends: `<bay>:<port>`, `edge:<port>`, `tee:<run>` or `cap`;
    path: its points from end to end, the ends given as None where a port fixes them."""
    rr = kit._recs[run]
    by_id = {b["id"]: b for b in bays}
    pts = list(path)
    n_port = 0
    for which, e in ((0, ends[0]), (1, ends[1])):
        i = 0 if which == 0 else -1
        if e.startswith("edge:"):
            pp = next(q for q in edge if q["id"] == e[5:])
            off = end_offset(kit._recs[pp["connector"]]["kind"], rr, which)
            at = (pp["at"][0] + off[0], pp["at"][1] + off[1], round(pp["at"][2] + off[2], 3))
            n_port += 1
        elif ":" in e and not e.startswith(("tee:", "onto:")):
            bid, pid = e.split(":")
            b = by_id[bid]
            pp = next(q for q in recs[b["unit"].split(".", 1)[1]]["ports"] if q["id"] == pid)
            off = end_offset(kit._recs[pp["connector"]]["kind"], rr, 1 if reversible else which)
            w = world(b, pp["at"])
            at = (w[0] + off[0], w[1] + off[1], round(w[2] + off[2], 3))
            n_port += 1
        else:
            at = None
        if at is not None:
            assert pts[i] is None or math.dist(pts[i], at) < 1e-6, (name, e, pts[i], at)
            pts[i] = at
    assert all(q is not None for q in pts), name
    tap_conn = []
    for t in taps:
        bid, pid = t.split(":")
        tap_conn.append(next(q for q in recs[by_id[bid]["unit"].split(".", 1)[1]]["ports"] if q["id"] == pid)["connector"])
    caps = sum(1 for e in ends if e == "cap") if rr["kind"] in ("pipe", "lined-duct") else 0      # (a belt's or a cable's free end is its tail or its end box, no blind)
    mass, fit, L = kit.lay(run, pts, count, tees=sum(1 for e in ends if e.startswith("tee:")), taps=tap_conn, port_ends=n_port, caps=caps)
    d = {"kind": KIND_OF_RUN[rr["kind"]], "name": name, "run": run, "ends": list(ends), "from": list(pts[0]), "to": list(pts[-1]), "length": r(L),
         "item": rr["per_m"]["built_of"][0]["item"], "mass": r(mass)}
    if len(pts) > 2:
        d["via"] = [list(q) for q in pts[1:-1]]
    if taps:
        d["taps"] = list(taps)
    if reversible:
        d["reversible"] = True
    if count > 1:
        d["count"] = count
    if power:
        d["power"] = r(power)
    d["fittings"] = fit
    if note:
        d["note"] = note
    return d


def radiator_rows(L, bays, recs, rad, rad_at, xf, xr, xl, lvf):
    """A radiator loop's rows (SFO 25), the fields west of the mains: the flow main at x xf, the return at xr and the
    pumps' cable at xl, all going north. Each row of fields has a 3 m aisle on its south side carrying its flow row (1 m
    up), its return row (2 m up, so it crosses over the flow main) and its pumps' cable (0.3 m); each field's ports are
    on its south-west corner facing the aisle, a short branch to each. (rows, yf, yr, yl, links)"""
    links = []
    q_rad = recs[rad]["rejects"]["heat"] / (4186 * 10)
    rows = sorted({y for _, y in rad_at})
    edge_y = lambda y: y - 24.0
    yf, yr, yl = (lambda y: edge_y(y) - 1.65), (lambda y: edge_y(y) - 0.75), (lambda y: edge_y(y) - 2.4)
    wb = kit.pipe(0.15, 10, liquid=True, temp=HP_T_LOOP)
    cb = kit.cable(lvf, kit.area_for(amps(recs[rad]["needs"]["power"], lvf)))
    for k, y in enumerate(rows):
        here = [b for b in bays if b["id"].startswith("RL") and b["at"][1] == y]
        xc = min(b["at"][0] for b in here) - 23.0 - 1.0
        dn = kit.dn_for(math.sqrt(4 * len(here) * q_rad / 1000 / 2.5 / math.pi))
        wr = kit.pipe(dn, 10, liquid=True, temp=HP_T_LOOP)
        e = edge_y(y)
        links += [
            L(f"row {k + 1} flow", wr, ("tee:loop flow main", "cap"), [(xf, yf(y), 1.0), (xc, yf(y), 1.0)], (), 1, None, "in the aisle south of its row, 1 m up"),
            L(f"row {k + 1} return", wr, ("cap", "tee:loop return main"), [(xc, yr(y), 2.0), (xr, yr(y), 2.0), (xr, yr(y), 1.0)], (), 1, None, "in the aisle, 2 m up: over the flow main and the fields' flow branches"),
            L(f"row {k + 1} pumps' supply", kit.cable(lvf, kit.area_for(amps(len(here) * recs[rad]["needs"]["power"], lvf))), ("tee:radiator pumps' supply", "cap"), [(xl, yl(y), 0.3), (xc, yl(y), 0.3)], (), 1, None, "in the aisle, 0.3 m up, under the water rows"),
        ]
        for b in here:
            x = b["at"][0]
            links += [
                L(f"{b['id']} flow branch", wb, (f"tee:row {k + 1} flow", f"{b['id']}:water-in"), [(x - 23.0, yf(y), 1.0), None]),
                L(f"{b['id']} return branch", wb, (f"{b['id']}:water-out", f"tee:row {k + 1} return"), [None, (x - 22.4, e - 0.75, 1.0), (x - 22.4, e - 0.75, 2.0)]),
                L(f"{b['id']} pump supply", cb, (f"tee:row {k + 1} pumps' supply", f"{b['id']}:power-in"), [(x - 21.8, yl(y), 0.3), None]),
            ]
    return rows, yf, yr, yl, links


def shaft_furnace_airless(us, recs):
    """The hydrogen shaft furnace on airless ground at about a moon's gravity."""
    sh_key = "shaft-furnace-200th-hp"
    sh = us[sh_key]
    side, _, H = sh["dims"]
    cell = 3.0
    top = math.ceil((H + 4.0) / cell) * cell
    box = (-12.0, -12.0, 0.0, 12.0, 12.0, top)
    z_top_gas = float(sh["desc"].split("top gas outlet at z ")[1].split(" ")[0])
    z_bustle = float(sh["desc"].split("the bustle main at z ")[1].split(",")[0])
    z_ring = float(sh["desc"].split("ring girder at z ")[1].split(" ")[0])
    openings = [(9.0, -3.0, z_top_gas - 3.0, 12.0, 3.0, top - 3.0), (-3.0, -12.0, z_bustle - 3.0, 3.0, -9.0, z_bustle + 3.0), (-12.0, -3.0, 3.0, -9.0, 3.0, 9.0), (-3.0, -3.0, top - 3.0, 6.0, 3.0, top + 3.0)]   # the last: the roof's framed opening for the charge belt's head, one cell east of the axis where the belt comes up through the roof's depth
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
    beams = 6 * (4 * (box[3] - box[0]) + 8 * 4.5) * box_tube(0.15, 0.1, 0.004, 1.0) * RHO["aluminium"]   # each level's perimeter and eight radial beams, 150 x 100 x 4 mm aluminium box
    ladder = 6 * 6.0 * 40.0                                                         # kg: a caged ladder, 40 kg a metre (from memory)
    deck = 6 * (box[3] - box[0]) * (box[4] - box[1]) * 0.6 * 0.006 * RHO["aluminium"] + beams + ladder
    sized = (f"At {g:g} m/s2 the shaft with its burden, {load / 1e6:.1f} MN, on {n_col} columns of 300 x 300 x 20 mm 4340 box: {col_sigma / 1e6:.0f} MPa each. No wind: the lattice braces the columns and carries six aluminium grating platforms (6 mm, 60% of the plan), "
             f"the kit's lightest aluminium strut ({sec}). Footings not yet checked.")
    heater = next(k for k in us if k.startswith("gas-heater-") and k.endswith("-8bar"))
    rad = f"radiator-field-{HP_T_LOOP:.0f}k"
    loop_heat = recs["top-gas-condenser-8bar"]["needs"]["heat"] + recs["heater-transformer-131mva"]["needs"].get("heat", 0)
    n_rad = math.ceil(loop_heat / recs[rad]["rejects"]["heat"])
    cols = 6
    rad_at = [(-60.0 - 48.0 * (i % cols), 66.0 + 51.0 * (i // cols)) for i in range(n_rad)]     # (a 3 m aisle south of each row, for its mains)
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
    z = hp_sizes()
    flow = loop_heat / (4186 * 10)
    rise = 15.0
    belt_x = H / math.sin(math.radians(rise)) * math.cos(math.radians(rise))
    K = 273.15
    q_rad = recs[rad]["rejects"]["heat"] / (4186 * 10)
    lvf = 400
    p_fields = n_rad * recs[rad]["needs"]["power"]
    # The layout's own ports: what it takes from the site and other layouts and gives them (facing into the layout).
    edge = [
        port("pellets-in", "in", "bulk", kit.belt_transfer(1.2), (round(belt_x, 1), 0, 2.0), (-1, 0, 0), item="good.iron-ore-pellets", rate=PELLETS, note="onto the charge belt's tail, from the pellet plant or its stockpile"),
        port("sponge-iron-out", "out", "bulk", kit.chute(0.8), (-60, 0, 5.188), (1, 0, 0), item="good.sponge-iron", rate=S_RATE, temperature=T_HOT_DRI + K, note="hot, to the arc furnace shop's bin"),
        port("h2-in", "in", "gas", kit.flange(0.25, 16, kit.SS, "double"), (100, 4, 1.5), (-1, 0, 0), item="material.hydrogen", rate=H2_USED, pressure=10e5, note="from the hydrogen plant's electrolysers at 10 bar"),
        port("condensate-out", "out", "liquid", kit.flange(0.2, 10, kit.SS), (100, 24, 0.5), (-1, 0, 0), item="good.water", rate=STEAM - _left, note="back to the electrolysers: on a moon every kilogram comes back"),
        port("mv-in", "in", "power", kit.termination(6600, kit.area_for(amps(HP_W_COMP / 0.96, 6600))), (100, -1.5, 6.0), (-1, 0, 0), power=HP_W_COMP / 0.96, voltage=6600, note="the site's 6.6 kV switchgear"),
        port("hv-in", "in", "power", kit.termination(132000, kit.area_for(amps(recs["heater-transformer-131mva"]["needs"]["power"] + recs[heater]["needs"]["power"], 132000, 0.95))), (100, -24.956, 8.8), (-1, 0, 0), power=recs["heater-transformer-131mva"]["needs"]["power"] + recs[heater]["needs"]["power"], voltage=132000, note="the site's 132 kV substation"),
        port("lv-fields", "in", "power", kit.termination(lvf, kit.area_for(amps(p_fields, lvf))), (-32.4, 30.0, 0.3), (0, 1, 0), power=p_fields, voltage=lvf, note="the site's 400 V distribution, for the radiator fields' pumps"),
        port("lv-furnace", "in", "power", kit.termination(lvf, kit.area_for(amps(recs[sh_key]["needs"]["power"], lvf))), (-40.0, 2.0, 4.0), (1, 0, 0), power=recs[sh_key]["needs"]["power"], voltage=lvf),
        port("lv-post", "in", "power", kit.termination(lvf, kit.area_for(amps(recs["crew-post"]["needs"]["power"], lvf))), (-16.0, -24.0, 2.3), (-1, 0, 0), power=recs["crew-post"]["needs"]["power"], voltage=lvf),
    ]
    g_top = kit.pipe(z["top"], 16, lagged=True, temp=T_TOP + K, sealing="double")
    g_gas = kit.pipe(z["gas"], 16, temp=T_TOP + K, sealing="double")
    g_warm = kit.pipe(z["gas"], 16, lagged=True, temp=HP_T_PRE + K, sealing="double")
    g_hot = kit.lined_duct(z["hot"], 16, sealing="double")
    w_main = kit.pipe(kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi)), 10, liquid=True, temp=HP_T_LOOP)
    assert kit._recs[w_main]["size"] == 0.9, "the mains' path and the condenser's ports are set for DN 900"
    L = lambda *a: kit_link(*a[:4], bays, recs, edge, *a[4:])
    links = [
        L("pellet belt", kit.belt(1.2, PELLETS * 1.5), ("edge:pellets-in", "F:charge-in"), [None, None], (), 1, 300000.0 * MOON_G[1] / G + 50000.0,
          f"an open belt on trestles rising at {math.degrees(math.atan2(H + 1.0, belt_x)):.1f} degrees, its head 3 m over the hopper's rim, up through the tower roof's framed opening; over the compressor its trestles are portals with legs at y -5.5 and 5.5, clear to z 9; its bearings and grease vacuum-rated"),
        L("hot sponge iron", kit.drag(0.8, S_RATE * 1.5), ("F:dri-out", "edge:sponge-iron-out"), [None, None], (), 1, 150000.0,
          f"{S_RATE:.1f} kg/s of sponge iron at about {T_HOT_DRI:.0f} deg C to the arc furnace shop's bin, beyond the layout; the hot charge saves the furnace about {S_RATE * CP_DRI * (T_HOT_DRI - 25) / 1e6:.0f} MW"),
        L("top gas main", g_top, ("F:top-gas-out", "R:top-gas-in"), [None, (15.0, 0.0, 31.0), (15.0, 10.0, 31.0), (24.0, 24.0, 31.0), None], (), 1, None,
          "north along x 15 to y 10 before its diagonal, clear of the pellet belt's trestles; lagged to keep it over its dew point"),
        L("wet gas to the condenser", g_gas, ("R:wet-out", "S:gas-in"), [None, (24.0, 32.0, 1.5), (38.0, 32.0, 1.5), (38.0, 32.0, 12.0), (38.0, 24.0, 12.0), None]),
        L("gas to the compressor", g_gas, ("S:gas-out", "K:suction"), [None, (57.0, 24.0, 2.0), (57.0, 0.0, 1.5), None]),
        L("compressed gas to the recuperator", g_gas, ("K:discharge", "R:clean-in"), [None, (36.0, 0.0, 1.5), (36.0, 14.0, 1.0), (24.0, 14.0, 1.0), None]),
        L("warm gas to the heater", g_warm, ("R:warm-out", "H:gas-in"), [None, (24.0, 16.0, 16.75), (14.0, 16.0, 16.75), (14.0, -15.0, 16.75), (10.0, -15.0, 16.75), (10.0, -15.0, 2.0), (10.0, -27.3, 2.0), None], (), 1, None,
          "round the shaft's tower and the heater's north side, down and into its cold header"),
        L("hot gas to the bustle", g_hot, ("H:gas-out", "F:hot-gas-in"), [None, (24.0, -20.0, 21.0), (24.0, -14.0, 21.0), (0.0, -14.0, 21.0), (0.0, -14.0, 16.0), None], (), 1, None,
          "up from the heater's stub, west along y -14 clear of the hot header, down outside the tower and in through its opening to the bustle"),
        L("hydrogen make-up", kit.pipe(0.25, 16, kit.SS, temp=300.0, sealing="double"), ("edge:h2-in", "tee:gas to the compressor"), [None, (57.0, 4.0, 1.583)], (), 1, None,
          f"{H2_USED:.2f} kg/s teed into the compressor's suction: the electrolysers run at 10 bar, so no make-up compressor"),
        L("condensate to the hydrogen plant", kit.pipe(0.2, 10, kit.SS, liquid=True), ("S:condensate-out", "edge:condensate-out"), [None, None]),
        L("compressor supply", kit.cable(6600, kit.area_for(amps(HP_W_COMP / 0.96, 6600))), ("edge:mv-in", "K:hv"), [None, (48.0, -1.5, 6.0), None]),
        L("transformer supply", kit.cable(132000, kit.area_for(amps(recs["heater-transformer-131mva"]["needs"]["power"] + recs[heater]["needs"]["power"], 132000, 0.95))), ("edge:hv-in", "T:hv-in"), [None, (48.0, -24.956, 8.8), None]),
        L("heater supply", kit.cable(6600, 630), ("T:lv-out", "H:power-in"), [None, (48.0, -17.0, 4.5), (11.3, -17.0, 4.5), (11.3, -22.55, 4.5), (11.3, -22.55, 1.575), None], (), math.ceil(amps(recs[heater]["needs"]["power"], 6600, 0.95) / (630e-6 * kit.J_VACUUM))),
        L("furnace supply", kit.cable(lvf, kit.area_for(amps(recs[sh_key]["needs"]["power"], lvf))), ("edge:lv-furnace", "F:power-in"), [None, (-7.0, 2.0, 4.0), (-7.0, 2.0, 12.0), None], (), 1, None, "in through the tower's west opening and up inside the platforms' opening round the shaft to the ring girder"),
        L("crew post supply", kit.cable(lvf, kit.area_for(amps(recs["crew-post"]["needs"]["power"], lvf))), ("edge:lv-post", "P:power-in"), [None, None], (), 1, None, "through the berm"),
    ]
    # The radiator loop: flow (hot, out of the condenser) and return mains as a parallel pair, the return on the right
    # of the flow all the way, so they never cross. Each row of fields has a 3 m aisle on its south side carrying its
    # flow row (1 m up), its return row (2 m up, so it crosses over the flow main) and its pumps' cable (0.3 m); each
    # field's ports are on its south-west corner facing the aisle, a short branch to each. The coolers' pairs are
    # taken off the return main (62 deg C), a balancing orifice in the main between each pair's tees driving them.
    rows, yf, yr, yl, row_links = radiator_rows(L, bays, recs, rad, rad_at, -34.8, -33.6, -32.4, lvf)
    f_main = [None, (48.6, 17.2, 1.0), (39.4, 17.2, 1.0), (39.4, 35.4, 1.0), (-34.8, 35.4, 1.0), (-34.8, yf(rows[-1]) + 1.0, 1.0)]
    r_main = [(-33.6, yr(rows[-1]) + 1.0, 1.0), (-33.6, 36.6, 1.0), (40.6, 36.6, 1.0), (40.6, 18.4, 1.0), (47.4, 18.4, 1.0), None]
    links += [
        L("loop flow main", w_main, ("S:water-out", "cap"), f_main, (), 1, None, f"{flow:,.0f} kg/s at about {HP_T_LOOP:.0f} K to the {n_rad} radiator fields; the return main on its right all the way"),
        L("loop return main", w_main, ("cap", "S:water-in"), r_main, (), 1, None, "back from the fields at about 10 K less"),
    ]
    lv_main = kit.cable(lvf, kit.area_for(amps(p_fields, lvf)))
    links.append(L("radiator pumps' supply", lv_main, ("edge:lv-fields", "cap"), [None, (-32.4, yl(rows[-1]) + 1.0, 0.3)], (), 1, None, "north along the fields' east side under the loop's mains"))
    links += row_links
    # The coolers' four lines, side by side on the left of the transformer's flow (L0), down under the flow main,
    # round the compressor's west side; the compressor's pair turns off into its -x face.
    def side(path, d):
        """path moved d to its left (in plan), its corners mitred; a vertical leg keeps its neighbours' offset."""
        n = []
        for p_, q_ in zip(path, path[1:]):
            dx, dy = q_[0] - p_[0], q_[1] - p_[1]
            m = math.hypot(dx, dy)
            n.append((-dy / m, dx / m) if m > 1e-9 and abs(q_[2] - p_[2]) < 1e-9 else None)
        out = []
        for i, q in enumerate(path):
            near = [x for x in (n[i - 1] if i > 0 else None, n[i] if i < len(n) else None) if x]
            if not near:
                near = [next(x for x in n[i:] + n[:i][::-1] if x)]
            if len(near) == 2:
                c = 1 + near[0][0] * near[1][0] + near[0][1] * near[1][1]
                ox, oy = d * (near[0][0] + near[1][0]) / c, d * (near[0][1] + near[1][1]) / c
            else:
                ox, oy = d * near[0][0], d * near[0][1]
            out.append((round(q[0] + ox, 3), round(q[1] + oy, 3), q[2]))
        return out
    l0 = [(42.6, 18.4, 1.0), (42.6, 18.4, 0.3), (42.6, 5.8, 0.3), (38.9, 5.8, 0.3), (38.9, -24.6, 0.3), (42.8, -24.6, 0.3), (42.8, -24.6, 1.0), (43.45, -24.6, 1.0)]
    l1 = side(l0, 1.2)
    l2 = side(l0[:5], 1.8)
    l3 = side(l0[:5], 2.4)
    l2[-1] = (l2[-1][0], -2.2, 0.3)
    l3[-1] = (l3[-1][0], -1.6, 0.3)
    wt, wk = kit.pipe(0.15, 10, liquid=True), kit.pipe(0.1, 10, liquid=True)
    links += [
        L("transformer cooling in", wt, ("tee:loop return main", "T:water-in"), l0[:-1] + [None]),
        L("transformer cooling out", wt, ("T:water-out", "tee:loop return main"), [None] + l1[::-1][1:]),
        L("compressor cooling in", wk, ("tee:loop return main", "K:cooling-in"), l2 + [(l2[-1][0], -2.2, 0.6), None]),
        L("compressor cooling out", wk, ("K:cooling-out", "tee:loop return main"), [None, (l3[-1][0], -1.6, 0.6)] + l3[::-1]),
    ]
    ground = [
        {"what": "Tower foundation", "item": SINTER, "volume": r(24 * 24 * 1.5), "note": "a mat of sintered regolith blocks 1.5 m thick (a fifth of the load of 1 g)"},
        {"what": "Unit pads", "item": SINTER, "volume": r(sum(recs[b["unit"].split(".", 1)[1]]["physical"]["length"] * recs[b["unit"].split(".", 1)[1]]["physical"]["width"] * 0.5 for b in bays[1:] if not b["id"].startswith("RL"))), "note": "0.5 m under each unit"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(n_rad * 4 * (44.0 * 44.0 / 72.0) * 0.027)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": BERM, "note": "loose regolith at least 1 m over the post everywhere (its top 4 x 2 m, 5.3 m up), lying at 35 degrees, its foot about 19 x 17 m, a cavity for the post; measured on blender's arc furnace shop; with openings for its windows (light wells on the north) and its airlock (a ramp cut on the west); with openings for its windows and airlock; at the airlock's door a landing of packed regolith 2 x 2 m, its top at the threshold (1.35 m up), and a ramp down to the ground at about 1 in 3"},
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
    xs = [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    layout = {
        "identity": {"key": "layout.shaft-furnace-airless", "name": "Hydrogen shaft furnace, airless",
                     "description": "Direct reduction by hydrogen on airless ground at about a moon's gravity: the gas loop at 8 bar so the top gas's water condenses warm enough for radiators to take its heat, the sponge iron discharged hot through a lock straight to the arc furnace shop, the heat to fields of radiators. Hydrogen leaking into vacuum just leaves (no air, no fire), so the tower is an open lattice for bracing, not for venting. Built separately from the plant for air."},
        "stands_for": "module.shaft-furnace",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": top, "mass": r(total)},
        "bays": bays, "ports": edge, "links": links,
        "frame": {"box": list(box), "cell": cell, "shape": f"An open braced tower {box[3] - box[0]:.0f} x {box[4] - box[1]:.0f} m and {top:.0f} m tall of aluminium kit struts round the shaft; six grating platforms at z 6, 12, 18, 24, 30 and 36, each a 24 m square with a 15.2 m square opening round the shaft (60% of the plan), 6 mm aluminium grating on a perimeter and eight radial 150 x 100 x 4 mm aluminium box beams, with guard rails; a caged ladder up the south-west corner between them; no cladding.",
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
    b["built_of"] = [dict(p_, note=p_["note"].replace("measured on model v1", "as the 3 bar block's, measured on its model v1")) for p_ in us["electrolyser-block-20mw"]["built_of"] if p_["what"] != "Separators"] + [part("Separators", A36, seps, "two vessels 2 m across, 6 m, 25 mm plate for 10 bar (40 MPa)")]
    b["desc"] = ("The electrolyser block built for airless ground: its stacks and separators held at about 10 bar, so the hydrogen goes to the shaft furnace's 8 bar loop with no compressor, standing in the open on a pad with no hall (in vacuum there is no air for a leak to burn in, but the oxygen side is kept apart and its lines are clean). "
                 "Its lye at about 80 deg C gives its heat to the layout's loop through its coolers. Otherwise as unit.electrolyser-block-20mw: " + us["electrolyser-block-20mw"]["desc"])
    b["desc"] = b["desc"].replace("by a busbar to its terminals at local (0, -6, 3)", "by a busbar to its terminals at local (0, -6, 2.4) (one bus joint, + at x -0.3)")
    b["needs"] = dict(us["electrolyser-block-20mw"]["needs"], atmosphere="any")
    out["electrolyser-block-20mw-10bar"] = b
    return out


def h2_ports(us):
    """Ports on the airless hydrogen plant's units (SFO 25), in each unit's frame as its description gives them."""
    K = 273.15
    H2, O2, WATER = "material.hydrogen", "material.oxygen", "good.water"
    blk = us["electrolyser-block-20mw-10bar"]
    q_blk = blk["needs"]["heat"] / (4186 * 10)
    bus = kit.bus_joint(V_STACK, I_DC)
    blk["ports"] = [
        port("dc-in", "in", "power", bus, (0, -6, 2.4), (0, -1, 0), power=P_STACKS, voltage=V_STACK, note=f"{I_DC / 1e3:.1f} kA direct, one bus joint"),
        port("h2-out", "out", "gas", kit.flange(0.15, 16, kit.SS, "double"), (10.5, -2.5, 8), (0, 0, 1), item=H2, rate=H_RATE / N_BLK, pressure=10e5, temperature=80.0 + K, note="the hydrogen separator's top, wet with lye mist"),
        port("o2-out", "out", "gas", kit.flange(0.1, 16, kit.SS, "double"), (10.5, 2.5, 8), (0, 0, 1), item=O2, rate=H_O2 / N_BLK, pressure=10e5, temperature=80.0 + K, note="the oxygen separator's top"),
        port("water-in", "in", "liquid", kit.flange(0.05, 10, kit.SS), (9.8, 6, 1), (0, 1, 0), item=WATER, rate=H_WATER / N_BLK, note="demineralised water, the cells' feed"),
        port("cooling-in", "in", "liquid", kit.flange(0.25, 10), (10.6, 6, 1), (0, 1, 0), item=WATER, rate=q_blk, temperature=HP_T_LOOP - 10.0),
        port("cooling-out", "out", "liquid", kit.flange(0.25, 10), (11.4, 6, 1), (0, 1, 0), item=WATER, rate=q_blk, temperature=HP_T_LOOP),
    ]
    rt = us["rectifier-transformer-21mva"]
    L_t, W_r, H_r = rt["dims"]
    p_in = P_STACKS + rt["needs"]["power"]
    q_rt = rt["needs"]["heat"] / (4186 * 10)
    rt["ports"] = [
        port("hv-in", "in", "power", kit.termination(RT_V, kit.area_for(amps(p_in, RT_V, 0.95))), (0, -1.5, H_r), (0, 0, 1), power=p_in, voltage=RT_V, note="the HV cable box's gland plate, over the three bushings"),
        port("dc-out", "out", "power", bus, (0, W_r / 2, 2.4), (0, 1, 0), power=P_STACKS, voltage=V_STACK, note="the rectifier house's DC bus joint"),
        port("cooling-in", "in", "liquid", kit.flange(0.1, 10), (-L_t / 2, -1.86, 1), (-1, 0, 0), item=WATER, rate=q_rt, temperature=HP_T_LOOP - 10.0, note="the oil cooler's and the heat sinks' pair, and the bus's water through the joint"),
        port("cooling-out", "out", "liquid", kit.flange(0.1, 10), (-L_t / 2, -1.14, 1), (-1, 0, 0), item=WATER, rate=q_rt, temperature=HP_T_LOOP),
    ]
    pu = us["hydrogen-purifier-10bar"]
    q_pu = pu["needs"]["heat"] / (4186 * 10)
    g = kit.flange(0.25, 16, kit.SS, "double")
    pu["ports"] = [
        port("gas-in", "in", "gas", g, (-6, 0, 6), (-1, 0, 0), item=H2, rate=H_RATE, pressure=10e5, temperature=80.0 + K),
        port("gas-out", "out", "gas", g, (6, 0, 2), (1, 0, 0), item=H2, rate=H_RATE, pressure=10e5, note="dry, oxygen-free"),
        port("cooling-in", "in", "liquid", kit.flange(0.2, 10), (0, -4, 1), (0, -1, 0), item=WATER, rate=q_pu, temperature=HP_T_LOOP - 10.0),
        port("cooling-out", "out", "liquid", kit.flange(0.2, 10), (0.6, -4, 1), (0, -1, 0), item=WATER, rate=q_pu, temperature=HP_T_LOOP),
        port("condensate-out", "out", "liquid", kit.flange(0.05, 10, kit.SS), (-6, 3, 0.3), (-1, 0, 0), item=WATER, note="the deoxidiser's water and the knockouts'"),
        port("power-in", "in", "power", kit.termination(400, kit.area_for(amps(pu["needs"]["power"], 400))), (6, -3.5, 0.5), (1, 0, 0), power=pu["needs"]["power"], voltage=400),
    ]
    wt = us["water-treatment-sealed"]
    wt["ports"] = [
        port("raw-in", "in", "liquid", kit.flange(0.2, 10, kit.SS), (6, -1.6, 0.5), (1, 0, 0), item=WATER, rate=20.3 / 3.6, note="the shaft furnace's condensate and melted ice"),
        port("clean-out", "out", "liquid", kit.flange(0.1, 10, kit.SS), (-6, 0, 0.5), (-1, 0, 0), item=WATER, rate=H_WATER),
        port("reject-out", "out", "liquid", kit.flange(0.15, 10, kit.SS), (6, 3.4, 0.3), (1, 0, 0), item=WATER, note="concentrate and backwash, kept"),
        port("power-in", "in", "power", kit.termination(400, kit.area_for(amps(wt["needs"]["power"], 400))), (-6, -3.4, 0.5), (-1, 0, 0), power=wt["needs"]["power"], voltage=400),
    ]


def pellet_ports(us):
    """Ports on the airless pellet plant's units (SFO 25), and the changes laying it from ports asked of them:
    the radiator field's ports to its corner, the transformer's cable boxes, the hall heads' high nozzle and service
    panel, the discs' fines kept, the gas loops' dust binned."""
    K = 273.15
    ORE, WATER, PEL = "good.iron-ore", "good.water", "good.iron-ore-pellets"
    lv = lambda w: kit.termination(400, kit.area_for(amps(w, 400)))
    mv = lambda w: kit.termination(6600, kit.area_for(amps(w, 6600)))
    q = lambda w: w / (4186 * 10)
    # The 330 K field: ports to its south-west corner, as the 345 K field's (radiator_rows).
    rf = us["radiator-field-330k"]
    rf["desc"] = rf["desc"].replace("its ports at local (-24, -0.6, 1) flow and (-24, 0.6, 1) return.", "its ports on its south-west corner facing -y, at local (-23, -24, 1) flow and (-22.4, -24, 1) return, its pump's power entry at (-21.8, -24, 0.3), so a layout's mains run in an aisle beside the field, not under it.")
    rf["built_of"] = [dict(q_, mass=3316.0, note=q_.get("note", "") + "; with the branch, posts and union housings to the south-west ports, 141 kg more, measured on blender's model v2") if q_["what"].startswith("Headers") and "south-west" not in q_.get("note", "") else q_ for q_ in rf["built_of"]]
    rf["ports"] = [
        port("water-in", "in", "liquid", kit.flange(0.15, 10, temp=330.0), (-23.0, -24, 1), (0, -1, 0), item=WATER, rate=q(rf["rejects"]["heat"]), temperature=330.0),
        port("water-out", "out", "liquid", kit.flange(0.15, 10, temp=330.0), (-22.4, -24, 1), (0, -1, 0), item=WATER, rate=q(rf["rejects"]["heat"]), temperature=320.0),
        port("power-in", "in", "power", lv(rf["needs"]["power"]), (-21.8, -24, 0.3), (0, -1, 0), power=rf["needs"]["power"], voltage=400, note="its circulating pump"),
    ]
    # The plant transformer: 33 kV in through a cable box on top, 6.6 kV out through a box on its +y face.
    tkey = next(k for k in us if k.startswith("plant-transformer-"))
    tp = us[tkey]
    L_, W_, H_ = tp["dims"]
    if "HV cable box" not in [x["what"] for x in tp["built_of"]]:
        tp["built_of"] = tp["built_of"] + [part("HV cable box", A36, 503.3, f"measured on blender's model v2; over the three HV bushings on top at x -1.15, 0 and 1.15, y -0.9: 3 x 0.8 x 0.8 m, 6 mm, its gland plate on top at (0, -0.9, {H_ + 0.8:g})"),
                                           part("LV cable box", A36, 677.9, f"measured on blender's model v2; over the three secondary pads on the +y face, 2.9 m up: 4 x 0.5 x 1.2 m, 6 mm, its gland plate facing +y at (0, {W_ / 2 + 0.5:g}, 2.9)"),
                                           part("Cable boxes' supports", A36, 259.2, "their brackets on the tank, and the bushings' insulated feed-throughs' frames; measured")]
        tp["desc"] += f" Fed at 33 kV through a cable box over its HV bushings (gland plate on top at (0, -0.9, {H_ + 0.8:g})); its 6.6 kV leaves through a box over its secondary pads (gland plate facing +y at (0, {W_ / 2 + 0.5:g}, 2.9)); its cooler's water ports at (-{L_ / 2:g}, -0.6, 1) and (-{L_ / 2:g}, 0.6, 1)."
        tp["dims"] = (5.82, 3.95, H_ + 0.8)          # (as drawn: the LV box makes it lopsided across, y -1.6 to 2.35; the box is its extent)
    p_mv = 2 * us["pellet-shaft-63th"]["needs"]["power"] + us["ball-mill-4m4"]["needs"]["power"]
    n_mv = math.ceil(amps(p_mv, 6600) / (630e-6 * kit.J_VACUUM))
    tp["ports"] = [
        port("hv-in", "in", "power", kit.termination(33000, kit.area_for(amps(p_mv + tp["needs"]["power"], 33000, 0.95))), (0, -0.9, H_ + 0.8), (0, 0, 1), power=p_mv + tp["needs"]["power"], voltage=33000),
        port("lv-out", "out", "power", kit.termination(6600, 630), (0, W_ / 2 + 0.5, 2.9), (0, 1, 0), power=p_mv, voltage=6600, note=f"{n_mv} circuits of 630 mm2"),
        port("cooling-in", "in", "liquid", kit.flange(0.1, 10, temp=330.0), (-L_ / 2, -0.6, 1), (-1, 0, 0), item=WATER, rate=q(tp["needs"]["heat"]), temperature=320.0),
        port("cooling-out", "out", "liquid", kit.flange(0.1, 10, temp=330.0), (-L_ / 2, 0.6, 1), (-1, 0, 0), item=WATER, rate=q(tp["needs"]["heat"]), temperature=330.0),
    ]
    # The hall's heads: a second nozzle high on the head, a service panel of pass-throughs (each an outer and an inner
    # port, a spare blinded where a layout leaves it), and the lock pumps' supply.
    hd = us["pressure-hall-30m-end"]
    hall_lv = 4 * us["balling-disc-7m5"]["needs"]["power"] + us["filter-press-2"]["needs"]["power"] + N_SEC * us["pressure-hall-30m"]["needs"]["power"]
    thru = [("w1", -10.2, 10.0, "liquid", kit.flange(0.2, 10, temp=330.0)), ("w2", -9.6, 10.0, "liquid", kit.flange(0.2, 10, temp=330.0)), ("k1", -9.0, 10.0, "liquid", kit.flange(0.05, 10, kit.SS)),
            ("e1", -10.2, 10.8, "power", mv(us["ball-mill-4m4"]["needs"]["power"])), ("e2", -9.4, 10.8, "power", lv(hall_lv))]
    if "high nozzle" not in hd["desc"]:
        hd["desc"] = hd["desc"].replace("and a 3 m belt nozzle on its axis whose bottom is at the deck. ", "a 3 m belt nozzle whose bottom is at the deck, and a second, high nozzle near its crown, each blinded where a layout leaves it unused. ")
        hd["desc"] = hd["desc"].replace("the belt nozzle at (6.9, 0, 9);", "the belt nozzle's face at (7.9, 0, 9), the high nozzle's at (8.0, 0, 14.5);")
        hd["desc"] = hd["desc"].replace("a service panel for pipes and cables at (6.5, -9, 10).", "a service panel for pipes and cables at (6.5, -9, 10): a boxed plate whose pass-throughs' outer faces are at x 6.5 facing +x and inner faces at x 5.0 facing -x, at y -10.2 and -9.6 (200 mm water), -9.0 (50 mm stainless), z 10, and the cables' glands at y -10.2 (6.6 kV) and -9.4 (400 V), z 10.8; the hall's nitrogen make-up in at (6.5, -8.4, 10) and the lock pumps' 400 V supply at (6.5, -8.6, 10.8), outer only.")
    hd["ports"] = [port("nozzle", "both", "bulk", kit.flange(3.0, 1, sealing="double"), (7.9, 0, 9), (1, 0, 0), note="a sealed gallery's flange, 1 m out from the head; the belt passes through", ).__or__({"optional": True}),
                   port("high-nozzle", "both", "bulk", kit.flange(3.0, 1, sealing="double"), (8.0, 0, 14.5), (1, 0, 0), note="a sealed gallery's flange, 0.5 m out from the crown; the belt passes through").__or__({"optional": True})]
    for pid, y, z, med, con in thru:
        hd["ports"] += [port(f"{pid}-out", "both", med, con, (6.5, y, z), (1, 0, 0)).__or__({"optional": True}), port(f"{pid}-in", "both", med, con, (5.0, y, z), (-1, 0, 0)).__or__({"optional": True})]
    hd["ports"] += [port("nitrogen-in", "in", "gas", kit.flange(0.05, 10, kit.SS), (6.5, -8.4, 10), (1, 0, 0), item="material.nitrogen", note="the hall's make-up, let into its gas").__or__({"optional": True}),
                    port("power-in", "in", "power", lv(hd["needs"]["power"]), (6.5, -8.6, 10.8), (1, 0, 0), power=hd["needs"]["power"], voltage=400, note="the locks' pumps")]
    # A hall section: its fan coils' water and their fans' supply.
    hs = us["pressure-hall-30m"]
    hs["ports"] = [
        port("water-in", "in", "liquid", kit.flange(0.15, 10, temp=330.0), (-14, -12, HALL_DECK), (0, 0, 1), item=WATER, rate=q(hs["needs"]["heat"]), temperature=320.0),
        port("water-out", "out", "liquid", kit.flange(0.15, 10, temp=330.0), (-13.2, -12, HALL_DECK), (0, 0, 1), item=WATER, rate=q(hs["needs"]["heat"]), temperature=330.0),
        port("power-in", "in", "power", lv(hs["needs"]["power"]), (-12.4, -12, HALL_DECK), (0, 0, 1), power=hs["needs"]["power"], voltage=400, note="the fan coils' fans, a gland up through the deck"),
    ]
    if "power entry" not in hs["desc"]:
        hs["desc"] = hs["desc"].replace("their faces facing up at the deck's plane", "and their fans' 400 V entry at (-12.4, -12), their faces facing up at the deck's plane")
    # The ore lock: on a plinth in the layout; its ports in its own frame.
    ol = us["ore-lock-hopper"]
    ol["ports"] = [
        port("ore-in", "in", "bulk", kit.belt_transfer(1.0), (0, 0, 17.5), (0, 0, 1), item=ORE, rate=P_ORE, note="the hopper's rim; a belt's head stands 3 m over it"),
        port("ore-out", "out", "bulk", kit.chute(1.0), (0, 0, 10), (0, 0, -1), item=ORE, rate=P_ORE, note="the bottom valve, onto a sealed gallery's belt"),
        port("nitrogen-in", "in", "gas", kit.flange(0.05, 10, kit.SS), (1.7, 0, 13.3), (1, 0, 0), item="material.nitrogen", note="from the hall, to fill the lock"),
        port("power-in", "in", "power", lv(ol["needs"]["power"]), (3, 0, 0.5), (1, 0, 0), power=ol["needs"]["power"], voltage=400),
    ]
    # The mill.
    ml = us["ball-mill-4m4"]
    if "Feed hopper, seal and services" not in [x["what"] for x in ml["built_of"]]:
        ml["built_of"] = ml["built_of"] + [part("Feed hopper, seal and services", A36, 1080.9, "the 1.3 m receiving hopper and its chute into the trunnion, the seal's sleeve, the water and slurry stubs, the 6.6 kV terminal box and their supports; measured on blender's model v2 (2026-10-16)")]
    ml["ports"] = [
        port("ore-in", "in", "bulk", kit.belt_transfer(1.0, sealed=True), (-8, 0, 4), (0, 0, 1), item=ORE, rate=P_ORE, note="the feed chute's hopper at its -x trunnion, flanged for a sealed gallery's drop"),
        port("water-in", "in", "liquid", kit.flange(0.1, 10, kit.SS), (-8, 1.2, 4), (0, 0, 1), item=WATER, note="the presses' filtrate back, into the feed chute"),
        port("slurry-out", "out", "liquid", kit.flange(0.15, 10, temp=330.0), (8, 3, 0.5), (0, 1, 0), item=ORE, note="from the sump's pump"),
        port("power-in", "in", "power", mv(ml["needs"]["power"]), (6, -4.4, 0.5), (0, -1, 0), power=ml["needs"]["power"], voltage=6600, note="the motor's terminal box"),
    ]
    # The presses.
    fp = us["filter-press-2"]
    fp["ports"] = [
        port("slurry-in", "in", "liquid", kit.flange(0.15, 10, temp=330.0), (0, -3.25, 9.5), (0, -1, 0), item=ORE),
        port("filtrate-out", "out", "liquid", kit.flange(0.1, 10, kit.SS), (5, -3.25, 0.5), (0, -1, 0), item=WATER),
        port("cooling-in", "in", "liquid", kit.flange(0.2, 10, temp=330.0), (-5, -3.25, 0.5), (0, -1, 0), item=WATER, rate=q(fp["needs"]["heat"]), temperature=320.0),
        port("cooling-out", "out", "liquid", kit.flange(0.2, 10, temp=330.0), (-4.4, -3.25, 0.5), (0, -1, 0), item=WATER, rate=q(fp["needs"]["heat"]), temperature=330.0),
        port("condensate-in", "in", "liquid", kit.flange(0.05, 10, kit.SS), (4, -3.25, 0.5), (0, -1, 0), item=WATER, rate=P_WATER),
        port("power-in", "in", "power", lv(fp["needs"]["power"]), (2.5, -3.25, 0.5), (0, -1, 0), power=fp["needs"]["power"], voltage=400),
        port("cake-1", "out", "bulk", kit.chute(1.0), (-1.2, 0, 7.775), (0, 0, -1), item=ORE, rate=P_ORE / 2, note="a press's breaker's opening onto the cake belt"),
        port("cake-2", "out", "bulk", kit.chute(1.0), (1.2, 0, 7.775), (0, 0, -1), item=ORE, rate=P_ORE / 2),
    ]
    # The discs: the screen's fines kept on the disc.
    dk = us["balling-disc-7m5"]
    if "fines elevator" not in dk["desc"]:
        dk["desc"] = dk["desc"].replace("the fines drop through into a tray and out at (0, -3.5, 0.65) to the layout's return belt,", "the fines drop through into a tray and an enclosed feeder takes them to a small bucket elevator on the frame's -x side (its casing at (-4.3, -1.0), clear of the pan and of the cake header over the chute), whose head at 10.5 m drops them down a chute into the cake chute at (-0.4, 2, 6.8),")
        dk["dims"] = (dk["dims"][0], dk["dims"][1], 11.0)
        dk["built_of"] = dk["built_of"] + [part("Fines elevator", A36, 1882.1, "its casing, the 6 mm return chute and feeder, the boot and braced posts; measured on blender's model v2"),
                                           part("Fines elevator's machinery", "good.electric-motors", 600.0, "its buckets and chain, the feeder's screw and the drive, not drawn; from memory")]
    dk["ports"] = [
        port("cake-in", "in", "bulk", kit.chute(0.4), (0, 2, 7), (0, 0, 1), item=ORE, rate=P_ORE / 4, note="the cake chute, fed by a plough off the layout's header"),
        port("balls-out", "out", "bulk", kit.chute(0.6), (0, -4.5, 1), (0, -1, 0), item=ORE, rate=P_ORE / 4 / (1 - MOIST), note="the screen's end, onto the layout's collector"),
        port("power-in", "in", "power", lv(dk["needs"]["power"]), (4.5, 1.0, 0.5), (1, 0, 0), power=dk["needs"]["power"], voltage=400),
    ]
    # The shafts.
    sh = us["pellet-shaft-63th"]
    sh["ports"] = [
        port("top-gas-out", "out", "gas", kit.flange(0.7, 10, temp=T_TOPG + K), (0, -3.3, 21.5), (0, -1, 0), item="material.nitrogen", temperature=T_TOPG + K),
        port("cooling-gas-in", "in", "gas", kit.flange(0.6, 10, temp=T_LOOPC + K), (0, -3.3, 7), (0, -1, 0), item="material.nitrogen", temperature=T_LOOPC + K),
        port("firing-in-a", "in", "gas", kit.flange(0.25, 10, temp=T_LOOPC + K), (5.55, 0, 12), (1, 0, 0), item="material.nitrogen", note="the +x torch chamber"),
        port("firing-in-b", "in", "gas", kit.flange(0.25, 10, temp=T_LOOPC + K), (-5.55, 0, 12), (-1, 0, 0), item="material.nitrogen", note="the -x torch chamber"),
        port("water-in", "in", "liquid", kit.flange(0.15, 10, temp=330.0), (-0.6, -3.3, 13), (0, -1, 0), item=WATER, rate=q(sh["needs"]["heat"]), temperature=320.0),
        port("water-out", "out", "liquid", kit.flange(0.15, 10, temp=330.0), (0.6, -3.3, 13), (0, -1, 0), item=WATER, rate=q(sh["needs"]["heat"]), temperature=330.0),
        port("charge-in", "in", "bulk", kit.belt_transfer(0.8, sealed=True), (0, 0, 29.5), (0, 0, 1), item=ORE, note="the charge lock hopper's rim, flanged for a sealed gallery's drop"),
        port("discharge-out", "out", "bulk", kit.chute(0.8), (0, 0, 0.8), (0, 0, -1), item=PEL, rate=P_RATE / 2),
        port("power-in", "in", "power", mv(sh["needs"]["power"]), (0, 5.5, 8), (0, 1, 0), power=sh["needs"]["power"], voltage=6600, note="the torches' supplies"),
    ]
    # The gas loops: the dust into bins; the cold gas out at the shaft's bore.
    gl = us["pellet-gas-loop"]
    if "dust bin" not in gl["desc"]:
        gl["desc"] = gl["desc"].replace("which lets it out by a 200 mm chute at (-9, 0, 3) and (-3, 0, 1.5), facing down onto the layout's closed dust conveyor.", "which holds the loop's 4 bar and lets the dust into a dust-tight 2 m3 bin on the slab under it (5 mm plate, fork runners, a small filtered vent to the vacuum for the lock's pocket gas), the loop's, changed by forklift about once a shift.")
        gl["desc"] = gl["desc"].replace("the cold gas out by a 700 mm flange at (12, 0, 1.5) facing +x;", "the cold gas out by a 600 mm flange at (12, 0, 1.5) facing +x, the shafts' bore;")
        gl["built_of"] = gl["built_of"] + [part("Dust bins", A36, 1153.0, "two dust-tight 2 m3 floor bins of 5 mm plate with fork runners, and the cyclone's fill neck; measured on model v2")]
    gl["ports"] = [
        port("top-gas-in", "in", "gas", kit.flange(0.7, 10, temp=T_TOPG + K), (-9, 0, 13), (0, 0, 1), item="material.nitrogen", temperature=T_TOPG + K),
        port("cold-gas-out", "out", "gas", kit.flange(0.6, 10, temp=T_LOOPC + K), (12, 0, 1.5), (1, 0, 0), item="material.nitrogen", temperature=T_LOOPC + K),
        port("condensate-out", "out", "liquid", kit.flange(0.05, 10, kit.SS), (6, 6, 0.3), (0, 1, 0), item=WATER, rate=P_WATER / 2),
        port("oxygen-in", "in", "gas", kit.flange(0.05, 16, kit.SS, "double"), (9, 6, 1), (0, 1, 0), item="material.oxygen"),
        port("cooling-in", "in", "liquid", kit.flange(0.3, 10, temp=330.0), (-12, -2, 1), (-1, 0, 0), item=WATER, rate=q(gl["needs"]["heat"]), temperature=320.0),
        port("cooling-out", "out", "liquid", kit.flange(0.3, 10, temp=330.0), (-12, -1.4, 1), (-1, 0, 0), item=WATER, rate=q(gl["needs"]["heat"]), temperature=330.0),
        port("power-in", "in", "power", lv(gl["needs"]["power"]), (10, -6, 0.5), (0, -1, 0), power=gl["needs"]["power"], voltage=400, note="the fan"),
    ]


def hydrogen_plant_airless(us, recs):
    """The hydrogen plant on airless ground: the blocks at 10 bar in the open, radiators for the tower, every kilogram
    of water kept. Laid of SFO 25 runs from port to port."""
    K = 273.15
    xs_b = [-48.0, -24.0, 0.0, 24.0, 48.0]
    rad = f"radiator-field-{HP_T_LOOP:.0f}k"
    blk, rt, pu = recs["electrolyser-block-20mw-10bar"], recs["rectifier-transformer-21mva"], recs["hydrogen-purifier-10bar"]
    bus = kit.busbar(V_STACK, I_DC)
    bus_path = lambda x: [None, (x, -19.0, 2.4), (x, -19.0, -1.0), (x, -8.0, -1.0), (x, -8.0, 2.4), None]
    bus_L = sum(math.dist(p_, q_) for p_, q_ in zip([(0, -21.0, 2.4)] + bus_path(0)[1:-1] + [(0, -6.0, 2.4)], bus_path(0)[1:-1] + [(0, -6.0, 2.4)]))
    bus_loss = kit._recs[bus]["per_m"]["power"] * bus_L
    loop = N_BLK * (blk["needs"]["heat"] + rt["needs"]["heat"] + bus_loss) + pu["needs"]["heat"]
    n_rad = math.ceil(loop / recs[rad]["rejects"]["heat"])
    cols = 6
    rad_at = [(-120.0 + 48.0 * (i % cols), 48.0 + 51.0 * (i // cols)) for i in range(n_rad)]     # (a 3 m aisle south of each row, for its mains)
    bays = [{"id": f"E{i + 1}", "mount": "plant-mount.bay-s3", "unit": "unit.electrolyser-block-20mw-10bar", "at": [x, 0.0, 0.0], "note": "on its pad in the open"} for i, x in enumerate(xs_b)]
    bays += [{"id": f"R{i + 1}", "mount": "plant-mount.bay-s2", "unit": "unit.rectifier-transformer-21mva", "at": [x, -24.0, 0.0], "note": "south, no fire walls"} for i, x in enumerate(xs_b)]
    bays += [{"id": "P", "mount": "plant-mount.bay-s2", "unit": "unit.hydrogen-purifier-10bar", "at": [72.0, 0.0, 0.0], "note": "east"},
             {"id": "W", "mount": "plant-mount.bay-s2", "unit": "unit.water-treatment-sealed", "at": [72.0, -24.0, 0.0], "note": "south-east: polishes the shaft furnace's condensate and the ice miners' water"},
             {"id": "Q", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [-72.0, -24.0, 0.0], "note": "south-west, under its berm"}]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{rad}", "at": [x, y, 0.0], "note": f"north, in rows of {cols}"} for i, (x, y) in enumerate(rad_at)]
    lvf = 400
    p_in = P_STACKS + rt["needs"]["power"]
    p_fields = n_rad * recs[rad]["needs"]["power"]
    hv = lambda: kit.termination(RT_V, kit.area_for(amps(p_in, RT_V, 0.95)))
    lane = lambda k: -34.0 - 0.6 * k              # the 33 kV feeders' lanes, the nearest block's on the north
    edge = [
        port("h2-out", "out", "gas", kit.flange(0.25, 16, kit.SS, "double"), (110, 0, 4), (-1, 0, 0), item="material.hydrogen", rate=H_RATE, pressure=10e5, note="to the shaft furnace's make-up at 10 bar"),
        port("o2-out", "out", "gas", kit.flange(0.2, 16, kit.SS, "double"), (110, 5.5, 9.5), (-1, 0, 0), item="material.oxygen", rate=H_O2, pressure=10e5, note="kept: crews' air, the pellet plant's firing"),
        port("water-in", "in", "liquid", kit.flange(0.2, 10, kit.SS), (110, -25.6, 0.5), (-1, 0, 0), item="good.water", rate=20.3 / 3.6, note="the shaft furnace's condensate and the ice plant's make-up"),
        port("reject-out", "out", "liquid", kit.flange(0.15, 10, kit.SS), (110, -20.6, 0.3), (-1, 0, 0), item="good.water", note="back to the ice plant's evaporator"),
    ]
    edge += [port(f"hv-{i + 1}", "in", "power", hv(), (110, lane(N_BLK - 1 - i), 0.3), (-1, 0, 0), power=p_in, voltage=RT_V, note=f"a feeder from the site's {RT_V / 1e3:g} kV switchgear") for i in range(N_BLK)]
    edge += [
        port("lv-purifier", "in", "power", kit.termination(lvf, kit.area_for(amps(pu["needs"]["power"], lvf))), (110, -3.5, 0.5), (-1, 0, 0), power=pu["needs"]["power"], voltage=lvf),
        port("lv-water", "in", "power", kit.termination(lvf, kit.area_for(amps(recs["water-treatment-sealed"]["needs"]["power"], lvf))), (110, -29.5, 0.5), (-1, 0, 0), power=recs["water-treatment-sealed"]["needs"]["power"], voltage=lvf),
        port("lv-post", "in", "power", kit.termination(lvf, kit.area_for(amps(recs["crew-post"]["needs"]["power"], lvf))), (-61.0, -24.0, 2.3), (-1, 0, 0), power=recs["crew-post"]["needs"]["power"], voltage=lvf),
        port("lv-fields", "in", "power", kit.termination(lvf, kit.area_for(amps(p_fields, lvf))), (149.4, 15.0, 0.3), (0, 1, 0), power=p_fields, voltage=lvf, note="the site's 400 V distribution, for the radiator fields' pumps"),
    ]
    L = lambda *a: kit_link(*a[:4], bays, recs, edge, *a[4:])
    flow = loop / (4186 * 10)
    q_s = N_BLK * (rt["needs"]["heat"] + bus_loss) / (4186 * 10) + pu["needs"]["heat"] / (4186 * 10)
    w_main = kit.pipe(kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi)), 10, liquid=True, temp=HP_T_LOOP)
    w_south = kit.pipe(kit.dn_for(math.sqrt(4 * q_s / 1000 / 2.5 / math.pi)), 10, liquid=True, temp=HP_T_LOOP)
    h2 = kit.pipe(0.25, 16, kit.SS, temp=353.0, sealing="double")
    links = []
    # Power: each block's DC bus from its rectifier, in a covered trench under the crane's south rail; a 33 kV feeder to
    # each rectifier in its own lane from the east edge, up and over onto its cable box.
    for i, x in enumerate(xs_b):
        links.append(L(f"DC bus {i + 1}", bus, (f"R{i + 1}:dc-out", f"E{i + 1}:dc-in"), bus_path(x), (), 1, bus_loss,
                       f"down into a covered trench 1.6 m deep under the gantry's south rail (y -12) and up to the block; {bus_loss / 1e3:.0f} kW lost into the bus's water, which the rectifier's cooling pair carries"))
        k = N_BLK - 1 - i
        links.append(L(f"feeder {i + 1}", kit.cable(RT_V, kit.area_for(amps(p_in, RT_V, 0.95))), (f"edge:hv-{i + 1}", f"R{i + 1}:hv-in"), [None, (x, lane(k), 0.3), (x, lane(k), 6.0), (x, -25.5, 6.0), None]))
    links += [
        L("purifier supply", kit.cable(lvf, kit.area_for(amps(pu["needs"]["power"], lvf))), ("edge:lv-purifier", "P:power-in"), [None, None]),
        L("water treatment supply", kit.cable(lvf, kit.area_for(amps(recs["water-treatment-sealed"]["needs"]["power"], lvf))), ("edge:lv-water", "W:power-in"), [None, (64.0, -29.5, 0.5), (64.0, -27.4, 0.5), None], (), 1, None, "round the house's south side to its west face"),
        L("crew post supply", kit.cable(lvf, kit.area_for(amps(recs["crew-post"]["needs"]["power"], lvf))), ("edge:lv-post", "Q:power-in"), [None, None], (), 1, None, "through the berm"),
    ]
    # Gas: the hydrogen header over the blocks' separators to the purifier, and on to the shaft furnace; the oxygen
    # header beside it out to the edge. Each block's outlet rises 1.5 m into its header.
    links += [
        L("hydrogen header", h2, ("cap", "P:gas-in"), [(xs_b[0] + 9.0, -2.5, 9.5), (62.0, -2.5, 9.5), (62.0, 0.0, 9.5), (62.0, 0.0, 6.0), None], (), 1, None, f"stainless at 10 bar, {H_RATE:.3f} kg/s at the purifier"),
        L("hydrogen to the shaft furnace", h2, ("P:gas-out", "edge:h2-out"), [None, (80.0, 0.0, 2.0), (80.0, 0.0, 4.0), None], (), 1, None, "up over the southern loop's return main"),
        L("oxygen header", kit.pipe(0.2, 16, kit.SS, temp=353.0, sealing="double"), ("cap", "edge:o2-out"), [(xs_b[0] + 9.0, 2.5, 9.5), (60.0, 2.5, 9.5), (60.0, 5.5, 9.5), None], (), 1, None, f"the {H_O2:.1f} kg/s of oxygen, past the purifier's north side"),
    ]
    for i, x in enumerate(xs_b):
        links += [
            L(f"E{i + 1} hydrogen", kit.pipe(0.15, 16, kit.SS, temp=353.0, sealing="double"), (f"E{i + 1}:h2-out", "tee:hydrogen header"), [None, (x + 10.5, -2.5, 9.5)]),
            L(f"E{i + 1} oxygen", kit.pipe(0.1, 16, kit.SS, temp=353.0, sealing="double"), (f"E{i + 1}:o2-out", "tee:oxygen header"), [None, (x + 10.5, 2.5, 9.5)]),
        ]
    # Water: in from the edge to the treatment, its clean water west along the blocks' north side (0.5 m up, under their
    # cooling branches), a 50 mm branch rising into each; the purifier's condensate back into the raw water.
    links += [
        L("raw water", kit.pipe(0.2, 10, kit.SS, liquid=True), ("edge:water-in", "W:raw-in"), [None, None]),
        L("reject", kit.pipe(0.15, 10, kit.SS, liquid=True), ("W:reject-out", "edge:reject-out"), [None, None]),
        L("demineralised water", kit.pipe(0.1, 10, kit.SS, liquid=True), ("W:clean-out", "cap"), [None, (62.0, -24.0, 0.5), (62.0, 7.6, 0.5), (xs_b[0] + 8.3, 7.6, 0.5)], (), 1, None, f"{H_WATER:.1f} kg/s to the blocks"),
        L("purifier condensate", kit.pipe(0.05, 10, kit.SS, liquid=True), ("P:condensate-out", "tee:raw water"), [None, (64.5, 3.0, 0.3), (64.5, -18.0, 0.3), (90.0, -18.0, 0.3), (90.0, -18.0, 0.5), (90.0, -25.6, 0.5)]),
    ]
    for i, x in enumerate(xs_b):
        links.append(L(f"E{i + 1} water", kit.pipe(0.05, 10, kit.SS, liquid=True), ("tee:demineralised water", f"E{i + 1}:water-in"), [(x + 9.8, 7.6, 0.5), (x + 9.8, 7.6, 1.0), None]))
    # The radiator loop: the return (cold) main along the blocks' north side 1 m up, the flow (hot) main beside it 3 m up
    # (its branches climb over the return main), both east past the purifier and north at x 147 and 148.2 to the fields,
    # whose rows run west; a southern pair from x 86 for the rectifiers and the purifier.
    xf, xr, xl = 147.0, 148.2, 149.4
    rows, yf, yr, yl, row_links = radiator_rows(L, bays, recs, rad, rad_at, xf, xr, xl, lvf)
    links += [
        L("loop flow main", w_main, ("cap", "cap"), [(xs_b[0] + 9.4, 9.6, 3.0), (xf, 9.6, 3.0), (xf, 9.6, 1.0), (xf, yf(rows[-1]) + 1.0, 1.0)], (), 1, None, f"{flow:,.0f} kg/s at about {HP_T_LOOP:.0f} K from the coolers to the {n_rad} radiator fields"),
        L("loop return main", w_main, ("cap", "cap"), [(xr, yr(rows[-1]) + 1.0, 1.0), (xr, 8.4, 1.0), (xs_b[0] + 8.6, 8.4, 1.0)], (), 1, None, "back from the fields at about 10 K less, to the coolers"),
        L("south return", w_south, ("tee:loop return main", "cap"), [(86.0, 8.4, 1.0), (86.0, -31.0, 1.0), (xs_b[0] - 6.0, -31.0, 1.0)], (), 1, None, "cold water south to the purifier and along the rectifiers' south side"),
        L("south flow", w_south, ("cap", "tee:loop flow main"), [(xs_b[0] - 6.0, -32.0, 3.0), (87.0, -32.0, 3.0), (87.0, 9.6, 3.0)], (), 1, None, "warm water back, 3 m up, over the return"),
        L("radiator pumps' supply", kit.cable(lvf, kit.area_for(amps(p_fields, lvf))), ("edge:lv-fields", "cap"), [None, (xl, yl(rows[-1]) + 1.0, 0.3)], (), 1, None, "north along the fields' east side, under the loop's mains"),
    ]
    links += row_links
    wb, wr_, wp = kit.pipe(0.25, 10, liquid=True, temp=HP_T_LOOP), kit.pipe(0.1, 10, liquid=True, temp=HP_T_LOOP), kit.pipe(0.2, 10, liquid=True, temp=HP_T_LOOP)
    for i, x in enumerate(xs_b):
        links += [
            L(f"E{i + 1} cooling in", wb, ("tee:loop return main", f"E{i + 1}:cooling-in"), [(x + 10.6, 8.4, 1.0), None]),
            L(f"E{i + 1} cooling out", wb, (f"E{i + 1}:cooling-out", "tee:loop flow main"), [None, (x + 11.4, 7.2, 1.0), (x + 11.4, 7.2, 3.0), (x + 11.4, 9.6, 3.0)]),
            L(f"R{i + 1} cooling in", wr_, ("tee:south return", f"R{i + 1}:cooling-in"), [(x - 4.45, -31.0, 1.0), (x - 4.45, -25.86, 1.0), None]),
            L(f"R{i + 1} cooling out", wr_, (f"R{i + 1}:cooling-out", "tee:south flow"), [None, (x - 3.45, -25.14, 1.0), (x - 3.45, -25.14, 3.0), (x - 3.45, -32.0, 3.0)]),
        ]
    links += [
        L("purifier cooling in", wp, ("tee:south return", "P:cooling-in"), [(86.0, -7.0, 1.0), (72.0, -7.0, 1.0), None]),
        L("purifier cooling out", wp, ("P:cooling-out", "tee:south flow"), [None, (72.6, -6.0, 1.0), (72.6, -6.0, 3.0), (87.0, -6.0, 3.0)]),
        link("rail", "crane rails", (-60.0, 0.0, 0.0), (60.0, 0.0, 0.0), A36, 2 * 50.0, None, "the maintenance gantry's two straight rails along the blocks, at y -12 and 12; the link runs on their centre line"),
    ]
    mobile = [{"unit": "unit.gantry-crane-20t-moon", "rides": "crane rails", "at": [0.0, 0.0, 0.0]}]
    trench = N_BLK * (11.0 + 2 * 1.6) * (1.4 + 2 * 1.6) * 0.2
    ground = [
        {"what": "Block pads", "item": SINTER, "volume": r(N_BLK * 24 * 12 * 0.4), "note": "0.4 m of sintered regolith with lye-proof bunds"},
        {"what": "Rectifier and outside pads", "item": SINTER, "volume": r(N_BLK * 12 * 8 * 0.5 + 2 * 12 * 8 * 0.5)},
        {"what": "Bus trenches", "item": SINTER, "volume": r(trench), "note": "a trench 1.4 m wide and 1.6 m deep for each block's bus, under the crane's south rail, lined with 0.2 m of sintered regolith and covered"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(n_rad * 4 * (44.0 * 44.0 / 72.0) * 0.027)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": BERM, "note": "loose regolith at least 1 m over the post everywhere (its top 4 x 2 m, 5.3 m up), lying at 35 degrees, its foot about 19 x 17 m, a cavity for the post; measured on blender's arc furnace shop; with openings for its windows (light wells on the north) and its airlock (a ramp cut on the west); at the airlock's door a landing of packed regolith 2 x 2 m, its top at the threshold (1.35 m up), and a ramp down to the ground at about 1 in 3"},
    ]
    for g in ground:
        g["mass"] = r(g["volume"] * (RHO_SINTER if g["item"] == SINTER else 1500.0))
    used = [recs[b_["unit"].split(".", 1)[1]] for b_ in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used) + N_BLK * bus_loss
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g["mass"] for g in ground)
    total = mass_units + mass_links + mass_ground
    against = (f"module.hydrogen-plant: one box, {H_P / 1e6:.0f} MW. This layout: the same rate; {power / 1e6:.1f} MW; {heat / 1e6:.1f} MW to {n_rad} radiator fields at {HP_T_LOOP:.0f} K ({n_rad * 48 * 48 / 1e4:.1f} ha); "
               f"its {H_WATER:.1f} kg/s of water all comes back from the shaft furnace, none evaporated; no hall; the blocks at 10 bar feed the furnace's loop with no compressor. {(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of sintered regolith.")
    xs = [b_["at"][0] + s_ * unit_dims(recs, b_)[0] / 2 for b_ in bays for s_ in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [b_["at"][1] + s_ * unit_dims(recs, b_)[1] / 2 for b_ in bays for s_ in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    layout = {
        "identity": {"key": "layout.hydrogen-plant-airless", "name": "Hydrogen plant, airless",
                     "description": f"Water split by electricity on airless ground at about a moon's gravity: {N_BLK} electrolyser blocks at 10 bar in the open, their rectifiers beside them, the heat to radiator fields; the water comes back from the shaft furnace, the hydrogen goes to it at its loop's pressure, the oxygen is kept. Built separately from the plant for air."},
        "stands_for": "module.hydrogen-plant",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": 25.0, "mass": r(total)},
        "bays": bays, "ports": edge, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": H_RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": hmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "As the plant for air, for airless ground; laid of SFO 25 runs from port to port."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, runs and ground; the DC buses' loss counted as power and as heat."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Sintered regolith at 2,500 kg/m3, loose at 1,500."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.hydrogen-plant-airless: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to {n_rad} fields, {len(links)} links")
    return layout, bays


# The pellet plant on airless ground (docs/proposals/pellet-plant-airless.md): the wet section in a nitrogen hall at
# 20 kPa, plate presses, two sealed pellet shafts on a closed gas loop at 4 bar, the heat to radiator fields.
HALL_P = 20e3                     # Pa of nitrogen in the wet hall: water stays liquid to about 60 deg C
HALL_D, HALL_SEC, N_SEC = 30.0, 30.0, 3     # m: the hall's tube, its sections' length, how many
HALL_DECK = 7.5                   # m: the deck above the tube's bottom, where it is 26 m wide
PS_P = 4e5                        # Pa: the shafts' gas loop (see the drag check in pellet-shaft-63th)
PS_FLUX = 85.0                    # t a m2 a day through a pellet shaft's section, from memory (they ran 80 to 100)
PS_BED = {"cooling": 6.0, "soaking": 1.0, "firing": 3.0, "preheat": 2.0, "drying": 2.0}   # m of bed, from memory
CP_PEL_P, CP_N2 = 900.0, 1150.0   # J/kg K: fired pellets, nitrogen hot (from memory)
T_FIRE, T_TOPG, T_LOOPC = 1300.0, 120.0, 65.0   # deg C: firing, the top gas, the gas out of the condenser
PS_SAT_65 = 25.0e3                # Pa: water's vapour pressure at 65 deg C (steam tables)


def pellet_airless_units(us):
    out = {}
    g = MOON_G[1]
    # The wet hall: a ring-stiffened steel tube in sections, closed by two heads.
    R = HALL_D / 2
    t_sh = 0.006
    hoop = HALL_P * R / t_sh
    chord = 2 * math.sqrt((R - 0.416) ** 2 - (R - HALL_DECK) ** 2)      # (clear of the rings: 400 mm webs and 16 mm flanges)
    rings = int(HALL_SEC / 3.0)
    fl_t, fl_w = 0.030, 0.3                                    # m: the end flanges, outward rings: the 20 kPa end load is 150 kN a metre, 100 MPa in the ring
    ring_a = 0.4 * 0.012 + 0.2 * 0.016
    gas = HALL_P * 0.028 / (8.314 * 293) * math.pi * R ** 2 * HALL_SEC
    inside = 4 * (90e3 + 15e3) + 100e3                     # the discs' drives and the hall's crane, lights and fans, which the air takes
    out["pressure-hall-30m"] = dict(
        name=f"Pressure hall section, {HALL_D:g} m", size_class=4, fits="bay-s4", dims=(HALL_SEC, HALL_D + 2 * fl_w, HALL_D + 2 * fl_w),
        desc=(f"A {HALL_SEC:g} m length of a steel tube {HALL_D:g} m across lying on its side, flanged at both ends to the next section or a head, for a hall held at {HALL_P / 1e3:.0f} kPa of nitrogen on airless ground (water stays liquid in it to about 60 deg C). "
              f"Its {t_sh * 1000:.0f} mm shell carries the pressure in hoop ({hoop / 1e6:.0f} MPa); {rings} T rings 400 mm deep every 3 m stiffen it and carry the deck and the crane runways; the outside is wrapped in multilayer insulation against the lunar day and night. "
              f"Local frame: the axis along x at z {R:g}, the ends at x -{HALL_SEC / 2:g} and {HALL_SEC / 2:g}, its bottom on the cradle at z 0. A steel deck at z {HALL_DECK:g}, {chord:.1f} m wide (clear of the rings), on posts down to the shell, the gaps to the shell between rings closed by edge plates; the crane runways' rail plane at z {R + 9:g}, y -11.4 and 11.4, the runway boxes under it hung from the rings. "
              f"Each end has an outward flange ring {fl_w * 1000:.0f} x {fl_t * 1000:.0f} mm welded to the shell, its bolt holes at 150 mm; the bolts and the double O-rings between two rings are the layout's (the hall's joints), not the section's. The section stands on a cradle of sintered regolith that is the layout's ground, grooved where the flange rings reach 0.3 m below the tube's bottom (z -0.3 to 30.3). "
              f"Fan coils under the deck take {inside / N_SEC / 1e3:.0f} kW of the machines' heat out of the gas into the layout's radiator loop, by a pair of 150 mm stubs rising through the deck at local (-14, -12) and (-13.2, -12) (clear of the ring at x -13.5), their faces facing up at the deck's plane, z {HALL_DECK:g}; the hall's inner pipes run along the deck's south edge to them. The section holds {gas:,.0f} kg of nitrogen."),
        built_of=[part("Shell", A36, math.pi * HALL_D * HALL_SEC * t_sh * RHO["steel"], f"{t_sh * 1000:.0f} mm plate"),
                  part("Ring stiffeners", A36, rings * math.pi * HALL_D * ring_a * RHO["steel"], f"{rings} rings, 400 x 12 mm web, 200 x 16 mm flange"),
                  part("Deck", A36, chord * HALL_SEC * 0.012 * RHO["steel"] + rings * chord * 150.0 + rings * 3 * HALL_DECK * 100.0, f"12 mm plate on a beam at each ring (150 kg/m) and three posts under each (100 kg/m): sized for about 20 kPa of floor at {g:g} m/s2 (from memory), not checked under the mill's running load"),
                  part("Crane runways", "material.high-strength-steel-4340", 2 * HALL_SEC * (0.4 * 0.4 - 0.35 * 0.35) * RHO["steel"], "two 400 x 400 x 25 mm boxes, hung from the rings"),
                  part("End flanges", A36, 2 * math.pi * HALL_D * fl_w * fl_t * RHO["steel"], f"two outward rings {fl_w * 1000:.0f} x {fl_t * 1000:.0f} mm, drilled for M24 at 150 mm"),
                  part("Runway rails and hangers", A36, 8261.0, "measured on blender's candidate (2026-10-16): the crane rails on the runway boxes and the hangers to the rings, which the boxes above leave out"),
                  part("Fan coil pipes and supports", A36, 1063.0, "measured on blender's candidate: the stubs' pipes and their sleeves through the deck beams"),
                  part("Insulation", "material.polyimide-film", math.pi * HALL_D * HALL_SEC * 1.0, "multilayer blanket, about 1 kg/m2, from memory"),
                  part("Fan coils", "good.pumps", 3000.0, "from memory")],
        holds=gas, needs={"power": 30e3, "heat": inside / N_SEC, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"The shell from its hoop stress, {hoop / 1e6:.0f} MPa against 150; the rings, deck and insulation from memory. Not checked for buckling: the hall is never let down to vacuum with its cradle's load on it."},
               {"of": ["holds"], "tier": "derived", "note": f"Nitrogen at {HALL_P / 1e3:.0f} kPa and 20 deg C."},
               {"of": ["needs.heat"], "tier": "derived", "note": "A third of the discs' drives (90 + 15 kW each) and 100 kW of crane, lights and fans; the mill's and presses' heat leaves in the filtrate instead."}])
    head = 1.084 * HALL_D ** 2 * t_sh * RHO["steel"]       # a 2:1 ellipsoidal head's plate
    out["pressure-hall-30m-end"] = dict(
        name=f"Pressure hall head, {HALL_D:g} m", size_class=4, fits="bay-s4", dims=(14.0, HALL_D + 0.6, HALL_D + 0.6),
        desc=(f"Closes a {HALL_D:g} m pressure hall: a 2:1 ellipsoidal head {R / 2:g} m deep in {t_sh * 1000:.0f} mm plate, flanged to the last section, with a crew airlock (a 2.5 m cylinder, 4 m long, for two in suits) and a vehicle lock (a 7.2 m cylinder, 12 m long, its doors 5 m square, whose corners need 7.07 m) through it at deck level, and a 3 m belt nozzle on its axis whose bottom is at the deck. "
              f"Local frame: the flange plane at x 0, its bottom at z 0, the head bulging to +x (its crown at x {R / 2:g}); the belt nozzle at (6.9, 0, 9); the locks' axes along x: the crew lock centred at (7, -6, 8.4), a flat floor across it at the deck's z 7.5 (1.7 m wide, 2.15 m under its crown), the vehicle lock centred at (8, 8, 10) reaching x 14, its doors' sills at the deck's z 7.5;  a service panel for pipes and cables at (6.5, -9, 10). "
              "Each lock's doors are pressure plugs, seated on their frames by the higher pressure: the inner door opens into the hall, the outer into the lock; drawn closed. The locks' gas is pumped back into the hall before their outer doors open. "
              f"Its flange ring as a section's, {0.3 * 1000:.0f} x {0.03 * 1000:.0f} mm drilled for M24 at 150 mm, reaching 0.3 m outside the shell; the bolts and seals are the layout's."),
        built_of=[part("Head", A36, 41137.1, f"2:1 ellipsoidal, {t_sh * 1000:.0f} mm, less the locks' and nozzle's openings; measured on blender's candidate (2026-10-16; the plate by its area, 1.084 D2, is {head / 1000:.1f} t uncut)"),
                  part("Flange", A36, 6920.0, "an outward ring 300 x 30 mm, as a section's; measured"),
                  part("Deck and its beams", A36, 21972.9, "12 mm at z 7.5 from the flange to the shell, on three transverse hollow beams seated on the shell, the locks' floors tied in; measured"),
                  part("Crew airlock", A36, 6909.2, "10 mm, its floor and two plug doors; measured"),
                  part("Vehicle lock", A36, 44630.3, "8 mm shell, 50 mm end diaphragms (a flat 7.2 m end at 20 kPa needs about 38 mm), 8 mm ribbed plug doors, its floor; measured"),
                  part("Nozzles, service panel and feet", A36, 6171.1 + 263.8, "measured"),
                  part("Lock pumps", "good.pumps", 2000.0, "from memory")],
        needs={"power": 50e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "derived", "review": True, "note": "Measured on blender's candidate (2026-10-16), the lock pumps from memory; local strength provisional."}])
    lock_v = math.pi / 4 * 3.0 ** 2 * 4.0
    out["ore-lock-hopper"] = dict(
        name="Ore lock hopper", size_class=1, fits="bay-s1", dims=(6.0, 6.0, 18.0),
        desc=(f"Takes the dry ore fines from vacuum into the {HALL_P / 1e3:.0f} kPa hall: a hopper on a frame over a lock {3.0:g} m across and 4 m tall ({lock_v:.0f} m3) between two dome valves; a load drops in from the hopper, the top valve shuts, nitrogen fills the lock from the hall, the bottom valve opens onto the hall's ore belt, and the lock is pumped back down before the top opens again. "
              f"About {P_ORE * 3.6 / (lock_v * 0.6 * 2.3):.0f} loads an hour; the nitrogen lost each time is what is left at the pump's 1 kPa, about 0.3 kg. Local frame, on the axis from the bottom up: the bottom dome valve z 10 to 10.3 (DN 1000), a 45 degree cone to the 3 m barrel (z 10.3 to 11.3), the barrel to z 15.3, its head to 15.8, the top valve to 16.1, the hopper flaring from DN 1000 to 3.8 m, its rim at z 17.5 (about 31 m3 of lock with its cone and head). Its connections: the ore belt's head drops into the hopper at the rim (0, 0, 17.5); the lock's outlet at (0, 0, 10) facing down onto the belt in a short gallery to the hall's nozzle; nitrogen from the hall in by a 50 mm stainless stub whose face is at (1.7, 0, 13.3) facing +x, 0.19 m out from the 10 mm shell; the 400 V supply at the frame's foot, (3, 0, 0.5) facing +x."),
        built_of=[part("Lock", A36, 4761.8, "the 3 m barrel in 10 mm, its cone, head and stiffeners; measured on blender's candidate (2026-10-16)"),
                  part("Hopper", A36, 1147.8, "8 mm, DN 1000 to 3.8 m; measured"),
                  part("Frame and pump skid", A36, 16676.9, "four 300 mm square columns, 400 x 300 mm head and radial beams, the ring at the barrel's base, side braces; its -x face a portal (300 x 300 x 12 mm jambs, 400 x 300 x 16 mm headers at 6.7 and 10.3) round the gallery's opening; measured on model v2"),
                  part("Pipes", "material.stainless-steel-304", 170.4, "the nitrogen fill and the pump's suction; measured"),
                  part("Valves and pump", "good.pumps", 3000.0, "from memory")],
        needs={"power": 15e3, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of", "needs"], "tier": "invented", "review": True, "note": "From memory; loads at 60% of the lock filled with ore at 2.3 t/m3."}])
    # The plate presses: two, back to back, each a pack of recessed chamber plates with a squeeze membrane.
    SOLIDS = 0.70
    w_in = P_ORE * (1 - SOLIDS) / SOLIDS
    w_filt = w_in - P_WATER
    plate, a_ch, cake_t, cycle = 2.5, 2.3 ** 2, 0.050, 600.0
    per_cycle = a_ch * cake_t * 0.6 * 4900.0                  # kg of dry ore a chamber holds: its 50 mm of cake, 60% of it ore at 4,900 kg/m3
    n_ch = math.ceil(P_ORE / 2 * cycle / per_cycle)
    close = 1.2 * 15e5 * a_ch                                  # N: the closing cylinder holds the membranes' 15 bar squeeze with a fifth to spare
    beams = 2 * box_tube(0.6, 0.4, 0.025, 6.0) * RHO["steel"]
    slurry_q = (P_ORE + w_in) / (1 / (SOLIDS / 4900.0 + (1 - SOLIDS) / 1000.0))
    p_press = 3 * slurry_q * 6e5 / 0.6 + 2 * 15e3 + 20e3 + 10e3
    q_mill = 18.0 * 3600 * P_ORE
    out["filter-press-2"] = dict(
        name="Filter presses, 2", size_class=2, fits="bay-s2", dims=(12.0, 6.5, 12.0),
        desc=(f"Two membrane plate presses back to back take the water out of the mill's slurry ({SOLIDS:.0%} solids) down to {MOIST:.0%}, pushing it through cloth at 6 bar, which works the same at the hall's {HALL_P / 1e3:.0f} kPa as in air (a vacuum filter would pull against a fifth of an atmosphere): "
              f"each has {n_ch} chambers between {plate:g} m polypropylene plates, filled, squeezed by its membranes at 15 bar and opened every {cycle / 60:.0f} minutes, the two in turn, so the cake falls in slabs about every {cycle / 120:.0f}. "
              f"Their fixed heads stand back to back at local x 0, their plate packs from x 0.5 to 3.5 each way, their closing cylinders ({close / 1e6:.1f} MN) at the outer ends (x ±6), all on a stand whose top is 8 m up, the presses' axis at z 10 (the plates' bottoms 8.75), so the cake drops through a 0.6 m hopper and a 0.4 m breaker; the cake drops through a lump breaker in the stand onto the layout's cake belt along x at y 0, local z 7.6 (x -6 to 6). "
              f"Slurry in by a 150 mm flange on the -y face at (0, -3.25, 9.5), split in the stand to both presses' feed eyes between the heads; each press's cake leaves its own breaker through a 1.5 x 1.1 m opening at (-1.2, 0, 7.775) and (1.2, 0, 7.775), facing down onto the same belt; the filtrate ({w_filt:.1f} kg/s) collects in a tank in the stand and is pumped through a plate cooler, which takes the mill's and presses' heat out of the water ({(q_mill + p_press) / 1e6:.1f} MW: the grinding's power all ends as warm slurry) into the radiator loop, so the water stays under 50 deg C; "
              f"every service on the -y face, 0.5 m up, facing -y: filtrate out (100 mm, stainless) at (5, -3.25, 0.5) to the mill, the radiator loop's 200 mm pair at (-5, -3.25, 0.5) in and (-4.4, -3.25, 0.5) out, the condensate from the shafts in (50 mm, stainless) at (4, -3.25, 0.5), the 400 V supply (95 mm2) at (2.5, -3.25, 0.5)."),
        built_of=[part("Heads and followers", "material.high-strength-steel-4340", 4 * plate ** 2 * 0.25 * RHO["steel"], f"four plates {plate:g} m square, 250 mm, which take the closing force in bending, from memory"),
                  part("Rear crossheads", "material.high-strength-steel-4340", 24531.3, "two 250 mm plates 2.5 m square at x -5.8 and 5.8, where the closing cylinders push from, tied to the heads by the side beams; measured on blender's candidate (2026-10-16)"),
                  part("Side beams", "material.high-strength-steel-4340", 9710.5, f"600 x 400 x 25 mm boxes, 6 m, two a press: each {close / 2 / (0.6 * 0.4 - 0.55 * 0.35) / 1e6:.0f} MPa in tension; with their seats, measured on blender's candidate (2026-10-16)"),
                  part("Plates and cloth", "material.polyethylene-uhmw", 9644.7, f"{2 * (n_ch + 1)} recessed plates 100 mm thick, touching rim to rim, each face recessed 25 mm, with their membranes; polyethylene stands for polypropylene; measured"),
                  part("Stand, supports and hoppers", A36, 45133.9, "the hollow stand, its longitudinals at y -0.9 and 0.9 that keep the cake belt's corridor clear (y -0.75 to 0.75, z 6.9 to 7.75), the cylinders' and pack's seats, the breakers' bearers, the cake hoppers, the pipes' stands and saddles; measured"),
                  part("Lump breakers", "good.pumps", 2000.0, "two breakers' rotors and drives, from memory"),
                  part("Piping", A36, 796.7, "the feed split, the filtrate collectors and the cooling pipes inside the stand; measured"),
                  part("Filtrate cooler", "material.stainless-steel-304", 3000.0, "a plate exchanger, from memory"),
                  part("Cylinders, power packs and pumps", "good.pumps", 12000.0, "from memory")],
        needs={"power": p_press, "heat": q_mill + p_press, "atmosphere": "any"},
        basis=[{"of": ["physical", "built_of", "needs.power"], "tier": "derived", "review": True,
                "note": f"Chambers from the rate: {P_ORE / 2:.1f} kg/s a press, {per_cycle:.0f} kg of ore a chamber a {cycle / 60:.0f} minute cycle (cycle, cake and plate weights from memory); the feed pump at 6 bar and three times the mean flow while filling, hydraulics, squeeze pump and breakers."},
               {"of": ["needs.heat"], "tier": "derived", "note": f"The mill's {q_mill / 1e6:.2f} MW (18 kWh a tonne) and the presses' own power, all of it warm water by the time it reaches the cooler."}])
    # The pellet shaft: sealed, counterflow, on the 4 bar loop.
    rate = P_RATE / 2
    A = rate * 86.4 / PS_FLUX
    D = round(math.sqrt(4 * A / math.pi), 1)
    A = math.pi * D ** 2 / 4
    bed = sum(PS_BED.values())
    m_cool = rate * CP_PEL_P * (T_FIRE - 100.0) / (CP_N2 * (1150.0 - 50.0))
    m_fire = P_FIRE / 2 / (CP_N2 * (1350.0 - T_LOOPC))
    m_gas = m_cool + m_fire
    T_bed = 700.0
    rho = PS_P * 0.028 / (8.314 * (T_bed + 273.15))
    V = m_gas / rho / A
    eps, dp, mu = 0.4, 0.012, 4.0e-5
    ergun = 150 * mu * (1 - eps) ** 2 * V / (eps ** 3 * dp ** 2) + 1.75 * rho * (1 - eps) * V ** 2 / (eps ** 3 * dp)
    weight = (1 - eps) * 3500.0 * MOON_G[0]
    assert ergun * 1.5 <= weight, (ergun, weight)
    rho1 = 1.2e5 * 0.028 / (8.314 * (T_bed + 273.15))
    V1 = m_gas / rho1 / A
    ergun1 = 150 * mu * (1 - eps) ** 2 * V1 / (eps ** 3 * dp ** 2) + 1.75 * rho1 * (1 - eps) * V1 ** 2 / (eps ** 3 * dp)
    t_ref, t_cool_ref, t_s = 0.4, 0.2, 0.020
    Do = D + 2 * t_ref + 2 * t_s
    hoop_s = PS_P * Do / (2 * t_s)
    z_cone, z_bed = 6.0, 6.0 + bed
    hot = bed - PS_BED["cooling"] + 3.0
    burden = A * bed * 2100.0
    torches = math.ceil(P_FIRE / 2 / 0.9 / 1.5e6)
    col_a = 0.3 * 0.3 - 0.26 * 0.26
    shaft_m = cyl_shell(Do, bed + 3.0, t_s, bottom=2.0) * RHO["steel"]
    out["pellet-shaft-63th"] = dict(
        name=f"Pellet shaft furnace, {rate * 3.6:.0f} t an hour, sealed", size_class=2, fits="bay-s2", dims=(11.5, 11.0, 29.5),
        desc=(f"A vertical shaft {D:g} m across inside, sealed, that dries, fires and cools green balls in one bed as they sink through it against a closed loop of nitrogen at {PS_P / 1e5:.0f} bar (with the 4.5% oxygen the recipe takes, from the hydrogen plant's): "
              f"the oldest way of firing pellets, here with no air to draw and none to vent. From the bottom: cooling {PS_BED['cooling']:g} m (the loop's cold gas comes in under it and leaves it at about 1,150 deg C), soaking {PS_BED['soaking']:g}, firing {PS_BED['firing']:g} at about {T_FIRE:.0f} deg C, preheat {PS_BED['preheat']:g}, drying {PS_BED['drying']:g}; "
              f"about {burden / 1000:,.0f} t in the bed, some {burden / rate / 3600:.0f} hours from top to bottom. Two torch chambers on its ±x sides at the firing zone, {torches} plasma torches of 1.5 MW between them, heat a share of the loop's gas ({m_fire:.1f} of its {m_gas:.1f} kg/s) to about 1,350 deg C and blow it in through ports round the shaft; the gas that cooled the pellets carries their heat up into the firing zone, so the new heat is only the module's 0.5 GJ a tonne. "
              f"Why {PS_P / 1e5:.0f} bar: at {MOON_G[0]:g} m/s2 the bed weighs {weight / 1e3:.1f} kPa a metre, and at 1.2 bar the gas's drag through it would be {ergun1 / 1e3:.1f} (the bed would lift and hang); at {PS_P / 1e5:.0f} bar the gas is denser and slower and its drag {ergun / 1e3:.2f}. "
              f"The shell holds the loop ({hoop_s / 1e6:.0f} MPa in {t_s * 1000:.0f} mm plate) and the charge and discharge go through lock hoppers. Local frame: the axis at (0, 0), its foot on the pit floor at z 0; the discharge lock and table feeder z 0 to {z_cone:g}, its outlet at (0, 0, 0.8); the bed z {z_cone:g} to {z_bed:g}; the gas space and top gas outlet ({0.7:.1f} m) at (0, -{Do / 2 + 0.5:.1f}, {z_bed + 1.5:g}); the charge lock hopper z {z_bed + 3:g} to 29.5, its inlet at (0, 0, 29.5); "
              f"the cooling gas in at (0, -{Do / 2 + 0.5:.1f}, {z_cone + 1:g}), the firing gas into each torch chamber at (±5.55, 0, {z_cone + PS_BED['cooling'] + PS_BED['soaking'] - 1:g}) on its outboard face; the torches' cooling water by a pair of flanges at (-0.6, -{Do / 2 + 0.5:.1f}, {z_cone + 7:g}) and (0.6, -{Do / 2 + 0.5:.1f}, {z_cone + 7:g}); the shaft stands on eight columns under a ring girder from z 5.8 to 6.6 (under the cooling gas's stub), holding the shell at z {z_cone:g}. The torch chambers stand upright, centred at x -4.3 and 4.3 from z 11 to 15, a hot duct from each into the firing band. The shell runs from a discharge cone (z 4 to 6, the shell's, over the table feeder and lock at z 0.8 to 4) up the bed to its roof (z 22 to 23). Its connections, plain stubs: the cooling gas 600 mm and each chamber's firing gas 250 mm, facing out; the top gas 700 mm lagged in 25 mm of foil, facing -y (the gas loop's inlet); the torches' water a 150 mm pair facing -y; the charge at the lock hopper's rim (0, 0, 29.5) facing up, where a belt's head drops in; the discharge out at (0, 0, 0.8) through an 800 mm chute facing down; the torches' 6.6 kV supply (400 mm2) into a terminal box at (0, 5.5, 8) facing +y."),
        built_of=[part("Shell", A36, 49830.5, f"{Do:.2f} m across, 19 m with its discharge cone and roof, {t_s * 1000:.0f} mm; measured on blender's candidate (2026-10-16; by formula {shaft_m:,.0f} kg over 17 m)"),
                  part("Lining", REF, math.pi * ((D / 2 + t_ref) ** 2 - (D / 2) ** 2) * hot * RHO["refractory"] + math.pi * ((D / 2 + t_cool_ref) ** 2 - (D / 2) ** 2) * PS_BED["cooling"] * RHO["refractory"], f"{t_ref * 1000:.0f} mm over the hot zones and the gas space ({hot:g} m), {t_cool_ref * 1000:.0f} mm and its backing over the cooling zone"),
                  part("Torch chambers", A36, 7947.7, "two 2.5 m chambers upright, 4 m, 16 mm; measured"),
                  part("Torch chambers' lining", REF, 2 * math.pi * (1.25 ** 2 - 0.95 ** 2) * 4.0 * RHO["refractory"], "300 mm"),
                  part("Hot branches", A36, 3993.3, "two from each chamber into the firing band, 600 mm bore, 8 mm steel; measured"),
                  part("Hot branches' lining", REF, 14.212 * RHO["refractory"], "300 mm, 14.21 m3 measured"),
                  part("Supports and service pipes", A36, 9797.3 + 618.6, "the chambers' legs and braces, the lower lock's legs, the power and water posts and brackets, the service pipes; measured"),
                  part("Plasma torches", "good.electric-motors", torches * 2000.0, f"{torches} torches of 1.5 MW, 2 t each with their supplies, from memory"),
                  part("Charge and discharge locks", A36, 2 * 25000.0, "two lock hoppers with their valves, and the table feeder, from memory"),
                  part("Ring girder and columns", "material.high-strength-steel-4340", 22334.7, f"measured, with the feet; a 600 x 800 x 30 mm box ring and eight 300 x 300 x 20 mm columns: {(burden + 400e3) * MOON_G[1] / 8 / col_a / 1e6:.0f} MPa each at {MOON_G[1]:g} m/s2")],
        holds=burden,
        needs={"power": P_FIRE / 2 / 0.9 + 50e3, "heat": P_FIRE / 2 / 0.9 * 0.1, "gravity": MOON_G[0], "atmosphere": "any"},
        recipes=pmod["recipes"], life=pmod.get("life"),
        basis=[{"of": ["recipes"], "tier": "derived", "note": "The module's recipe (module.pellet-plant), carried here; half the rate."},
               {"of": ["physical", "built_of", "holds"], "tier": "derived", "review": True, "note": f"The section from {PS_FLUX:g} t a m2 a day (from memory); the zones' heights, pellet bulk 2,100 kg/m3 and the locks from memory."},
               {"of": ["needs.gravity", "identity.description"], "tier": "derived", "review": True, "note": f"Ergun's equation for 12 mm pellets, voids 0.4, gas at {T_bed:.0f} deg C: drag {ergun:.0f} Pa/m against the bed's weight {weight:.0f} at {MOON_G[0]:g} m/s2, with half again to spare. The gas flow from the heat balance: the pellets cooled from {T_FIRE:.0f} to 100 deg C by gas warmed from 50 to 1,150, and the torches' share heated from {T_LOOPC:.0f} to 1,350."}])
    # The gas loop for one shaft: cyclone, candle filter, condenser, fan.
    n_n2 = m_gas / 0.028
    steam = P_WATER / 2
    n_left = n_n2 * PS_SAT_65 / PS_P / (1 - PS_SAT_65 / PS_P)
    q_cond = m_gas * 1040.0 * (T_TOPG - T_LOOPC) + steam * 2.35e6 + steam * 1900.0 * (T_TOPG - T_LOOPC)
    dp_loop = ergun * bed + 15e3
    rho_c = PS_P * 0.028 / (8.314 * (T_LOOPC + 273.15))
    w_fan = m_gas / rho_c * dp_loop / 0.75
    lmtd = ((T_TOPG - (330 - 273.15)) - (T_LOOPC - (320 - 273.15))) / math.log((T_TOPG - (330 - 273.15)) / (T_LOOPC - (320 - 273.15)))
    tubes = q_cond / (300.0 * lmtd) / (math.pi * 0.025)
    d_top = math.sqrt(4 * ((n_n2 + steam / 0.018 + n_left) * 8.314 * (T_TOPG + 273.15) / PS_P) / (math.pi * 20.0))
    out["pellet-gas-loop"] = dict(
        name=f"Pellet shaft gas loop, {PS_P / 1e5:.0f} bar", size_class=3, fits="bay-s3", dims=(24.0, 12.0, 14.0),
        desc=(f"Cleans, dries and drives one pellet shaft's {m_gas:.1f} kg/s of nitrogen at {PS_P / 1e5:.0f} bar: the top gas ({T_TOPG:.0f} deg C, about {d_top:.1f} m pipe) through a cyclone and a candle filter kept above its dew point (about 90 deg C), then down through a condenser that keeps the {steam:.1f} kg/s of water the green balls brought (to the presses, and on to the mill: on a moon every kilogram comes back), "
              f"the gas out at {T_LOOPC:.0f} deg C still carrying {n_left * 0.018:.1f} kg/s of vapour round the loop; then a fan ({w_fan / 1e3:.0f} kW, for the bed's {ergun * bed / 1e3:.0f} kPa and the loop's 15) back to the shaft's bottom and its torch chambers. Oxygen make-up joins before the fan. "
              f"Local frame: the cyclone at x -9, the filter at x -3, the condenser's two shells at x 3 and 6, the fan at x 10; Its connections, all plain stubs: the top gas in by a 700 mm lagged flange on the cyclone's top at (-9, 0, 13) facing up (its tangential entry inside the barrel), the cold gas out by a 700 mm flange at (12, 0, 1.5) facing +x; the condensate out (50 mm, stainless) at (6, 6, 0.3) facing +y, the oxygen in (50 mm, stainless, double seal) at (9, 6, 1) facing +y; the radiator loop's 300 mm pair at (-12, -2, 1) in and (-12, -1.4, 1) out, facing -x; the fan's 400 V supply (185 mm2) at (10, -6, 0.5) facing -y. The dust: the cyclone's cone and the filter's hopper each end in a rotary lock, the unit's, which lets it out by a 200 mm chute at (-9, 0, 3) and (-3, 0, 1.5), facing down onto the layout's closed dust conveyor."),
        built_of=[part("Cyclone", A36, 6246.9, f"measured on blender's candidate (2026-10-16); 3 m across, 9 m, 10 mm ({PS_P * 3.0 / 0.02 / 1e6:.0f} MPa)"),
                  part("Candle filter", A36, 14846.7 + 3000.0, "4 m across, 10 m, 16 mm, lagged: the shell measured (14.8 t); the candles 3 t of silicon carbide, from memory"),
                  part("Condenser", "material.stainless-steel-304", 15391.9 + tubes * math.pi * 0.025 * 0.0012 * 7900, f"two shells 2.5 m across, 10 m, 12 mm, and {tubes:,.0f} m of 25 mm tube (1.2 mm): {q_cond / 1e6:.1f} MW at 300 W/m2 K over a {lmtd:.0f} K mean difference; the shells measured (15.4 t), the tubes by their length"),
                  part("Fan", "good.electric-motors", 8000.0 * w_fan / 1e6 + 3000.0, "a centrifugal fan, 8 t a MW and its casing, from memory"),
                  part("Pipework and pots", A36, 7062.7, "measured on model v2, the coolant mains raised to 3.4 m over the bins' lanes"),
                  part("Stands, skirts and supports", A36, 29039.9, "the cyclone's braced stand and support ring, the filter's and condenser's skirts and base rings; measured")],
        needs={"power": w_fan / 0.96, "heat": q_cond + w_fan, "atmosphere": "vacuum"},
        basis=[{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": "Vessels by their pressure; the condenser by its duty; weights of the rest from memory."},
               {"of": ["needs"], "tier": "derived", "note": f"The top gas cooled {T_TOPG:.0f} to {T_LOOPC:.0f} deg C and its {steam:.1f} kg/s of water condensed (2.35 MJ/kg); the vapour left at {T_LOOPC:.0f} deg C from its {PS_SAT_65 / 1e3:.0f} kPa vapour pressure; the fan at 75%."}])
    return out


def pellet_plant_airless(us, recs):
    """The pellet plant on airless ground at about a moon's gravity: the wet hall, the shafts in their pit, radiators.
    Laid of SFO 25 runs from port to port (docs/proposals/factory-kit-assembly.md, the third layout)."""
    tkey = next(k for k in us if k.startswith("plant-transformer-"))
    zd = HALL_DECK
    xe = N_SEC * HALL_SEC / 2
    S_X, S_Y, S_Z = 113.0, 7.0, -10.5
    L_Z = 6.0                                       # m: the ore lock's plinth, so its outlet feeds the gallery at the west head's high nozzle
    sh = recs["pellet-shaft-63th"]
    heat_in = (recs["filter-press-2"]["needs"]["heat"] + N_SEC * recs["pressure-hall-30m"]["needs"]["heat"] + 2 * sh["needs"]["heat"]
               + 2 * recs["pellet-gas-loop"]["needs"]["heat"] + recs[tkey]["needs"]["heat"])
    rad = "radiator-field-330k"
    n_rad = math.ceil(heat_in / recs[rad]["rejects"]["heat"])
    cols = 6
    rad_at = [(-120.0 + 48.0 * (i % cols), 72.0 + 51.0 * (i // cols)) for i in range(n_rad)]     # (a 3 m aisle south of each row, for its mains)
    bays = [{"id": "EW", "mount": "plant-mount.bay-s4", "unit": "unit.pressure-hall-30m-end", "at": [-xe, 0.0, 0.0], "turn": 180.0, "note": "the hall's west head: the ore comes in through its high nozzle, the services through its panel"}]
    bays += [{"id": f"H{i + 1}", "mount": "plant-mount.bay-s4", "unit": "unit.pressure-hall-30m", "at": [-xe + HALL_SEC * (i + 0.5), 0.0, 0.0], "note": "on its cradle"} for i in range(N_SEC)]
    bays += [{"id": "EE", "mount": "plant-mount.bay-s4", "unit": "unit.pressure-hall-30m-end", "at": [xe, 0.0, 0.0], "note": "the east head: the green balls leave through its low nozzle, the condensate comes in through its panel"},
             {"id": "M", "mount": "plant-mount.bay-s3", "unit": "unit.ball-mill-4m4", "at": [-33.0, 0.0, zd], "note": f"on the deck, in {HALL_P / 1e3:.0f} kPa of nitrogen"},
             {"id": "F", "mount": "plant-mount.bay-s2", "unit": "unit.filter-press-2", "at": [-12.0, 0.0, zd], "note": "on the deck"},
             {"id": "B1", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [12.0, -6.0, zd], "turn": 180.0, "note": "two rows of two on the deck, the south pair turned so all four screens face the aisle at y 0"},
             {"id": "B2", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [24.0, -6.0, zd], "turn": 180.0, "note": ""},
             {"id": "B3", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [12.0, 6.0, zd], "note": ""},
             {"id": "B4", "mount": "plant-mount.bay-s2", "unit": "unit.balling-disc-7m5", "at": [24.0, 6.0, zd], "note": ""},
             {"id": "L", "mount": "plant-mount.bay-s1", "unit": "unit.ore-lock-hopper", "at": [-xe - 21.0, 0.0, L_Z], "turn": 180.0, "note": f"outside the west head, on a plinth {L_Z:g} m high, turned so its services are on its far side from the gallery"},
             {"id": "S1", "mount": "plant-mount.bay-s2", "unit": "unit.pellet-shaft-63th", "at": [S_X, -S_Y, S_Z], "note": f"in a pit {-S_Z:g} m deep, so the green balls' gallery climbs less"},
             {"id": "S2", "mount": "plant-mount.bay-s2", "unit": "unit.pellet-shaft-63th", "at": [S_X, S_Y, S_Z], "turn": 180.0, "note": "turned, its gas ports to the north"},
             {"id": "G1", "mount": "plant-mount.bay-s3", "unit": "unit.pellet-gas-loop", "at": [S_X, -27.0, 0.0], "note": "south of the pit"},
             {"id": "G2", "mount": "plant-mount.bay-s3", "unit": "unit.pellet-gas-loop", "at": [S_X, 27.0, 0.0], "note": "north of the pit"},
             {"id": "T", "mount": "plant-mount.bay-s2", "unit": f"unit.{tkey}", "at": [60.0, -36.0, 0.0], "note": "on its slab, no fire walls (nothing to burn in)"},
             {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [-xe - 21.0, -24.0, 0.0], "note": "by the west head's crew lock, under its berm"}]
    bays += [{"id": f"RL{i + 1}", "mount": "plant-mount.bay-s4", "unit": f"unit.{rad}", "at": [x, y, 0.0], "note": f"north, in rows of {cols}"} for i, (x, y) in enumerate(rad_at)]
    by = {b_["id"]: b_ for b_ in bays}
    at = lambda bid, pid: world(by[bid], next(q_ for q_ in recs[by[bid]["unit"].split(".", 1)[1]]["ports"] if q_["id"] == pid)["at"])
    lvf = 400
    lvk = lambda w: kit.cable(lvf, kit.area_for(amps(w, lvf)))
    lvt = lambda w: kit.termination(lvf, kit.area_for(amps(w, lvf)))
    p_fields = n_rad * recs[rad]["needs"]["power"]
    p_hall = 4 * recs["balling-disc-7m5"]["needs"]["power"] + recs["filter-press-2"]["needs"]["power"] + N_SEC * recs["pressure-hall-30m"]["needs"]["power"]
    p_fans = 2 * recs["pellet-gas-loop"]["needs"]["power"]
    tp = recs[tkey]
    p_mv = 2 * sh["needs"]["power"] + recs["ball-mill-4m4"]["needs"]["power"]
    ore_z = 2.0
    x_ore = at("L", "ore-in")[0] - (at("L", "ore-in")[2] + 3.0 - ore_z) / math.tan(math.radians(13.0))
    x_pel = S_X + (2.0 + 3.0 + S_Z + 1.0 - 0.6 - 0.5 - 0.5) * 0 + 78.0
    # The layout's own ports (facing into it).
    edge = [
        port("ore-in", "in", "bulk", kit.belt_transfer(1.0), (round(x_ore, 1), 0, ore_z), (1, 0, 0), item="good.iron-ore", rate=P_ORE, note="dry ore fines from the ore yard, onto the ore belt's tail"),
        port("pellets-out", "out", "bulk", kit.belt_transfer(1.0), (x_pel, S_Y + 7.5, 2.0), (-1, 0, 0), item="good.iron-ore-pellets", rate=P_RATE, note="fired pellets onto the airless shaft furnace's pellet belt or a stockpile"),
        port("o2-in", "in", "gas", kit.flange(0.05, 16, kit.SS, "double"), (160.0, 36.0, 3.5), (-1, 0, 0), item="material.oxygen", note="from the hydrogen plant: the recipe's 4.5%"),
        port("n2-in", "in", "gas", kit.flange(0.05, 10, kit.SS), (-100.0, 12.8, 1.0), (1, 0, 0), item="material.nitrogen", note="make-up for the hall's and the lock's losses"),
        port("hv-in", "in", "power", kit.termination(33000, kit.area_for(amps(p_mv + tp["needs"]["power"], 33000, 0.95))), (60.0, -60.0, 0.3), (0, 1, 0), power=p_mv + tp["needs"]["power"], voltage=33000, note="the site's 33 kV"),
        port("lv-hall", "in", "power", lvt(p_hall), (-100.0, 13.6, 0.3), (1, 0, 0), power=p_hall, voltage=lvf, note="the site's 400 V, for the hall's presses, discs and fan coils"),
        port("lv-west-head", "in", "power", lvt(recs["pressure-hall-30m-end"]["needs"]["power"]), (-100.0, 15.2, 0.3), (1, 0, 0), power=recs["pressure-hall-30m-end"]["needs"]["power"], voltage=lvf),
        port("lv-east-head", "in", "power", lvt(recs["pressure-hall-30m-end"]["needs"]["power"]), (56.0, -60.0, 0.3), (0, 1, 0), power=recs["pressure-hall-30m-end"]["needs"]["power"], voltage=lvf),
        port("lv-lock", "in", "power", lvt(recs["ore-lock-hopper"]["needs"]["power"]), (-71.0, -30.0, 0.3), (0, 1, 0), power=recs["ore-lock-hopper"]["needs"]["power"], voltage=lvf),
        port("lv-fans", "in", "power", lvt(p_fans), (140.0, -50.0, 0.5), (-1, 0, 0), power=p_fans, voltage=lvf, note="the gas loops' fans"),
        port("lv-post", "in", "power", lvt(recs["crew-post"]["needs"]["power"]), (-56.0, -24.0, 4.5), (-1, 0, 0), power=recs["crew-post"]["needs"]["power"], voltage=lvf),
        port("lv-fields", "in", "power", lvt(p_fields), (149.4, 35.0, 0.3), (0, 1, 0), power=p_fields, voltage=lvf, note="the radiator fields' pumps"),
    ]
    L = lambda *a, **k: kit_link(*a[:4], bays, recs, edge, *a[4:], **k)
    b08, b10 = kit.belt(0.8, P_ORE * 1.5), kit.belt(1.0, P_ORE * 1.5)
    g08, g10 = kit.belt(0.8, P_ORE * 1.5, sealed=True), kit.belt(1.0, P_ORE * 1.5, sealed=True)
    links = []
    # Bulk: the ore in by the high nozzle to the mill; the cake from the presses by a riser and a shuttle to the discs'
    # headers; the green balls along the aisle, out by the low nozzle, up to the shafts' shuttle; the pellets out of the pit.
    z_g = at("EE", "nozzle")[2]
    x_g2 = at("EE", "nozzle")[0] + 7.0
    z_sh = at("S1", "charge-in")[2] + 3.0
    links += [
        L("ore belt", b10, ("edge:ore-in", "L:ore-in"), [None, None], (), 1, 100000.0 * MOON_G[1] / G + 30000.0, f"an open belt (dry ore in vacuum), up at 13 degrees to its head 3 m over the lock hopper's rim on its plinth"),
        L("ore gallery", g10, ("L:ore-out", "M:ore-in"), [None, None], ("EW:high-nozzle",), 1, 20000.0, f"sealed, at the hall's {HALL_P / 1e3:.0f} kPa: from under the lock, level through the west head's high nozzle to its head 3 m over the mill's feed hopper"),
        L("cake belt", b08, ("cap", "onto:cake riser"), [(-19.0, 0.0, 14.8), (-3.0, 0.0, 14.8)], ("F:cake-1", "F:cake-2"), 1, 15000.0, "level along x through the presses' corridor, under both breakers' openings"),
        L("cake riser", b08, ("cap", "onto:cake shuttle"), [(-3.0, 0.0, 14.3), (5.58, 0.0, 16.6)], (), 1, 15000.0, "up at 15 degrees to the shuttle"),
        L("cake shuttle", b08, ("onto:south cake header", "onto:north cake header"), [(5.58, -8.6, 16.1), (5.58, 8.6, 16.1)], (), 1, 7500.0, "reversible, along y", reversible=True),
        L("south cake header", b08, ("cap", "cap"), [(4.0, -8.6, 15.6), (26.5, -8.6, 15.6)], ("B1:cake-in", "B2:cake-in"), 1, 10000.0, "ploughs at x 12 and 24 into discs B1 and B2, clear of their fines elevators' return chutes"),
        L("north cake header", b08, ("cap", "cap"), [(4.0, 8.6, 15.6), (26.5, 8.6, 15.6)], ("B3:cake-in", "B4:cake-in"), 1, 10000.0, "ploughs at x 12 and 24 into discs B3 and B4"),
        L("green ball collector", b08, ("cap", "onto:green gallery, hall"), [(7.0, 0.0, 7.95), (40.0, 0.0, z_g + 0.5)], ("B1:balls-out", "B2:balls-out", "B3:balls-out", "B4:balls-out"), 1, 15000.0, "along the aisle under the four screens' ends, rising gently"),
        L("green gallery, hall", g08, ("cap", "onto:green gallery"), [(39.0, 0.0, z_g), (x_g2, 0.0, z_g)], ("EE:nozzle",), 1, 15000.0, "sealed, level out through the east head's low nozzle"),
        L("green gallery", g08, ("cap", "onto:green ball shuttle"), [(x_g2, 0.0, z_g - 0.5), (S_X, 0.0, z_sh + 0.5)], (), 1, 60000.0, f"sealed, up at {math.degrees(math.atan2(z_sh + 1.0 - z_g, S_X - x_g2)):.1f} degrees to the shuttle over the shafts: in vacuum the green balls' water would boil off and freeze in their skins"),
        L("green ball shuttle", g08, ("S1:charge-in", "S2:charge-in"), [None, None], (), 1, 7500.0, "sealed, reversible, its ends 3 m over either shaft's charge lock", reversible=True),
        L("pellet collector", b10, ("cap", "onto:pellet belt"), [(S_X, -S_Y - 2.0, S_Z - 0.6), (S_X, S_Y + 7.5, S_Z - 0.6)], ("S1:discharge-out", "S2:discharge-out"), 1, 15000.0, "along the pit floor in a trench under both discharge locks"),
        L("pellet belt", b10, ("cap", "edge:pellets-out"), [(S_X, S_Y + 7.5, S_Z - 1.1), None], (), 1, 100000.0 * MOON_G[1] / G + 30000.0, "open, east along the pit and up out of its trench"),
    ]
    # Gas: each shaft's loop.
    top = kit.pipe(0.7, 10, lagged=True, temp=T_TOPG + 273.15)
    cold = kit.pipe(0.6, 10, temp=T_LOOPC + 273.15)
    fire = kit.pipe(0.25, 10, temp=T_LOOPC + 273.15)
    for sg, k_ in ((-1, "1"), (1, "2")):
        Sx, G_ = f"S{k_}", f"G{k_}"
        a_, b_ = ("firing-in-a", "firing-in-b") if k_ == "1" else ("firing-in-b", "firing-in-a")     # (the east chamber's port, the west's: S2 is turned)
        y1, y2, y3 = sg * 13.0, sg * 14.0, sg * 15.5
        links += [
            L(f"top gas {k_}", top, (f"{Sx}:top-gas-out", f"{G_}:top-gas-in"), [None, (S_X, y1, at(Sx, "top-gas-out")[2]), (S_X, y1, 17.0), (S_X - 9.0, y1, 17.0), (S_X - 9.0, sg * 27.0, 17.0), None], (), 1, None, "lagged in foil to keep it over its dew point; up over the pit's edge and down onto the cyclone"),
            L(f"cold gas {k_}", cold, (f"{G_}:cold-gas-out", f"{Sx}:cooling-gas-in"), [None, (S_X + 14.0, sg * 27.0, 1.5), (S_X + 14.0, y2, 1.5), (S_X + 6.5, y2, 1.5), (S_X + 6.5, y2, at(Sx, "cooling-gas-in")[2]), (S_X, y2, at(Sx, "cooling-gas-in")[2]), None], (), 1, None, "the fan's gas back, down into the pit to under the cooling zone"),
            L(f"firing gas {k_}, east", fire, (f"tee:cold gas {k_}", f"{Sx}:{a_}"), [(S_X + 9.0, y2, 1.5), (S_X + 9.0, sg * S_Y, 1.5), None]),
            L(f"firing gas {k_}, west", fire, (f"tee:cold gas {k_}", f"{Sx}:{b_}"), [(S_X + 12.0, y2, 1.5), (S_X + 12.0, y3, 1.5), (S_X - 8.0, y3, 1.5), (S_X - 8.0, sg * S_Y, 1.5), None]),
        ]
    o2 = kit.pipe(0.05, 16, kit.SS, sealing="double")
    links += [
        L("oxygen", o2, ("edge:o2-in", "G2:oxygen-in"), [None, (122.0, 36.0, 3.5), (122.0, 36.0, 1.0), None], (), 1, None, "the make-up, from the hydrogen plant"),
        L("oxygen, loop 1", o2, ("tee:oxygen", "G1:oxygen-in"), [(140.0, 36.0, 3.5), (140.0, -17.0, 3.5), (122.0, -17.0, 3.5), (122.0, -17.0, 1.0), None]),
    ]
    # Water kept: both loops' condensate in through the east head's panel to the presses' tank; the presses' filtrate
    # back to the mill; the mill's slurry up to the presses.
    k50 = kit.pipe(0.05, 10, kit.SS, liquid=True)
    links += [
        L("condensate", k50, ("G2:condensate-out", "EE:k1-out"), [None, (119.0, 35.0, 0.3), (119.0, 35.0, 3.0), (135.0, 35.0, 3.0), (135.0, -19.0, 3.0), (55.0, -19.0, 3.0), (55.0, -9.0, 3.0), (55.0, -9.0, 10.0), None], (), 1, None, f"both loops' {P_WATER:.1f} kg/s, kept; 3 m up, over the cables"),
        L("condensate, loop 1", k50, ("G1:condensate-out", "tee:condensate"), [None, (119.0, -19.0, 0.3), (119.0, -19.0, 3.0)]),
        L("condensate, hall", k50, ("EE:k1-in", "F:condensate-in"), [None, (47.0, -9.0, 10.0), (47.0, -9.0, 12.0), (47.0, -12.0, 12.0), (-8.0, -12.0, 12.0), (-8.0, -12.0, 8.0), None], (), 1, None, "along the deck's south edge, high, to the presses' tank"),
        L("filtrate", kit.pipe(0.1, 10, kit.SS, liquid=True), ("F:filtrate-out", "M:water-in"), [None, (-7.0, -5.5, 8.0), (-7.0, -5.5, 9.5), (-43.5, -5.5, 9.5), (-43.5, 1.2, 9.5), (-43.5, 1.2, 13.0), (-41.0, 1.2, 13.0), None], (), 1, None, "the cooled filtrate back into the mill's feed chute"),
        L("slurry", kit.pipe(0.15, 10, liquid=True), ("M:slurry-out", "F:slurry-in"), [None, (-25.0, 5.5, 8.0), (-25.0, 5.5, 18.0), (-20.0, 5.5, 18.0), (-20.0, -5.0, 18.0), (-12.0, -5.0, 18.0), (-12.0, -5.0, 17.0), None], (), 1, None, "rubber-lined, pumped from the mill's sump up to the presses' feed"),
        L("nitrogen", kit.pipe(0.05, 10, kit.SS), ("edge:n2-in", "EW:nitrogen-in"), [None, (-57.0, 12.8, 1.0), (-57.0, 12.8, 9.0), (-57.0, 8.4, 9.0), (-57.0, 8.4, 10.0), None], (), 1, None, "make-up into the hall's gas"),
        L("nitrogen, lock", kit.pipe(0.05, 10, kit.SS), ("tee:nitrogen", "L:nitrogen-in"), [(-72.0, 12.8, 1.0), (-72.0, 12.8, 19.3), (-72.0, 0.0, 19.3), None], (), 1, None, "to fill the lock before its bottom valve opens"),
    ]
    # The radiator loop: the return (cold) main from the fields along y -44 and up the hall's west side into the west
    # head's panel; the flow (warm) main beside it on its inside, 2.5 m up, back out of the panel to the fields.
    xf, xr, xl = 147.0, 148.2, 149.4
    rows, yf, yr, yl, row_links = radiator_rows(L, bays, recs, rad, rad_at, xf, xr, xl, lvf)
    flow = heat_in / (4186 * 10)
    w_main = kit.pipe(kit.dn_for(math.sqrt(4 * flow / 1000 / 2.5 / math.pi)), 10, liquid=True, temp=330.0)
    w_hall = kit.pipe(0.2, 10, liquid=True, temp=330.0)
    links += [
        L("loop return main", w_main, ("cap", "cap"), [(xr, yr(rows[-1]) + 1.0, 1.0), (xr, -44.0, 1.0), (-62.0, -44.0, 1.0)], (), 1, None, f"{flow:,.0f} kg/s back from the {n_rad} radiator fields at about 320 K, along y -44 to the hall's west end"),
        L("loop flow main", w_main, ("cap", "cap"), [(-61.0, -42.8, 2.5), (xf, -42.8, 2.5), (xf, -42.8, 1.0), (xf, yf(rows[-1]) + 1.0, 1.0)], (), 1, None, "warm water back out to the fields, on the return main's inside, 2.5 m up"),
        L("hall supply", w_hall, ("tee:loop return main", "EW:w1-out"), [(-60.0, -44.0, 1.0), (-60.0, 10.2, 1.0), (-60.0, 10.2, 10.0), None], (), 1, None, "north up the hall's west side and in at the west head's panel"),
        L("hall return", w_hall, ("EW:w2-out", "tee:loop flow main"), [None, (-58.8, 9.6, 10.0), (-58.8, 9.6, 2.5), (-58.8, -42.8, 2.5)]),
        L("radiator pumps' supply", kit.cable(lvf, kit.area_for(amps(p_fields, lvf))), ("edge:lv-fields", "cap"), [None, (xl, yl(rows[-1]) + 1.0, 0.3)], (), 1, None, "north along the fields' east side, under the loop's mains"),
    ]
    links += row_links
    br = lambda dn: kit.pipe(dn, 10, liquid=True, temp=330.0)
    def pair(name, dn, cin, cout, x_c, x_h, y_in, y_out, x_port):
        """A cooler's branch pair off the mains along y -44 (return, 1 m up) and y -42.8 (flow, 2.5 m up), north at
        x_c and x_h to ports on a -x face."""
        return [L(f"{name} cooling in", br(dn), ("tee:loop return main", cin), [(x_c, -44.0, 1.0), (x_c, y_in, 1.0), None]),
                L(f"{name} cooling out", br(dn), (cout, "tee:loop flow main"), [None, (x_h, y_out, 1.0), (x_h, y_out, 2.5), (x_h, -42.8, 2.5)])]
    links += pair("transformer", 0.1, "T:cooling-in", "T:cooling-out", 55.0, 54.4, at("T", "cooling-in")[1], at("T", "cooling-out")[1], None)
    links += pair("gas loop 1", 0.3, "G1:cooling-in", "G1:cooling-out", 99.0, 98.4, at("G1", "cooling-in")[1], at("G1", "cooling-out")[1], None)
    links += pair("gas loop 2", 0.3, "G2:cooling-in", "G2:cooling-out", 95.0, 94.4, at("G2", "cooling-in")[1], at("G2", "cooling-out")[1], None)
    for sg, k_, xc, xh in ((-1, "1", 129.0, 130.2), (1, "2", 131.0, 132.2)):
        Sx = f"S{k_}"
        wi, wo = at(Sx, "water-in"), at(Sx, "water-out")
        links += [L(f"torches {k_} cooling in", br(0.15), ("tee:loop return main", f"{Sx}:water-in"), [(xc, -44.0, 1.0), (xc, sg * 12.0, 1.0), (wi[0], sg * 12.0, 1.0), (wi[0], sg * 12.0, wi[2]), None]),
                  L(f"torches {k_} cooling out", br(0.15), (f"{Sx}:water-out", "tee:loop flow main"), [None, (wo[0], sg * 12.6, wo[2]), (xh, sg * 12.6, wo[2]), (xh, -42.8, 2.5)])]
    # Inside the hall: the pair from the panel along the deck's south edge to the presses' cooler and each section's
    # fan coils, the return 8.5 m up, the flow 9.3 m up so its branches climb over it.
    links += [
        L("hall cold", w_hall, ("EW:w1-in", "cap"), [None, (-45.2, 10.2, 10.0), (-45.2, 10.2, 8.5), (-45.2, -11.4, 8.5), (18.0, -11.4, 8.5)], (), 1, None, "inside, east of the west head's vehicle lock, along the deck's south edge"),
        L("hall warm", w_hall, ("cap", "EW:w2-in"), [(18.0, -10.8, 9.3), (-45.8, -10.8, 9.3), (-45.8, 9.6, 9.3), (-45.8, 9.6, 10.0), None]),
        L("presses cooling in", w_hall, ("tee:hall cold", "F:cooling-in"), [(-17.0, -11.4, 8.5), (-17.0, -11.4, 8.0), None]),
        L("presses cooling out", w_hall, ("F:cooling-out", "tee:hall warm"), [None, (-16.4, -10.8, 8.0), (-16.4, -10.8, 9.3)]),
    ]
    for i in range(N_SEC):
        H = f"H{i + 1}"
        xi, xo = at(H, "water-in")[0], at(H, "water-out")[0]
        links += [L(f"{H} fan coils in", br(0.15), ("tee:hall cold", f"{H}:water-in"), [(xi, -11.4, 8.5), (xi, -12.0, 8.5), None]),
                  L(f"{H} fan coils out", br(0.15), (f"{H}:water-out", "tee:hall warm"), [None, (xo, -12.0, 9.3), (xo, -10.8, 9.3)])]
    # Power: 33 kV to the transformer; 6.6 kV from it by a main with joint boxes to the shafts' torches and, through the
    # west head's panel, the mill; 400 V from the site: the hall (one main inside, branched), the heads, the lock, the fans,
    # the post.
    links += [L("transformer supply", kit.cable(33000, kit.area_for(amps(p_mv + tp["needs"]["power"], 33000, 0.95))), ("edge:hv-in", "T:hv-in"), [None, (60.0, -46.0, 0.3), (60.0, -46.0, 7.0), (at("T", "hv-in")[0], at("T", "hv-in")[1], 7.0), None])]
    n_mv = math.ceil(amps(p_mv, 6600) / (630e-6 * kit.J_VACUUM))
    lo = at("T", "lv-out")
    links += [
        L("6.6 kV main", kit.cable(6600, 630), ("T:lv-out", "cap"), [None, (lo[0], -30.0, lo[2]), (lo[0], -30.0, 5.0), (lo[0], -17.0, 5.0), (126.0, -17.0, 5.0), (126.0, 2.0, 5.0)], (), n_mv, None, "on trays 5 m up, over the loop's pipes"),
        L("torches 1 supply", kit.cable(6600, kit.area_for(amps(sh["needs"]["power"], 6600))), ("tee:6.6 kV main", "S1:power-in"), [(126.0, -0.3, 5.0), (S_X, -0.3, 5.0), (S_X, -0.3, at("S1", "power-in")[2]), None]),
        L("torches 2 supply", kit.cable(6600, kit.area_for(amps(sh["needs"]["power"], 6600))), ("tee:6.6 kV main", "S2:power-in"), [(126.0, 0.3, 5.0), (S_X, 0.3, 5.0), (S_X, 0.3, at("S2", "power-in")[2]), None]),
        L("mill supply", kit.cable(6600, kit.area_for(amps(recs["ball-mill-4m4"]["needs"]["power"], 6600))), ("tee:6.6 kV main", "EW:e1-out"), [(lo[0], -20.0, 5.0), (-56.0, -20.0, 5.0), (-56.0, 12.0, 5.0), (-56.0, 12.0, 12.5), (-54.5, 12.0, 12.5), (-54.5, 10.2, 12.5), (-54.5, 10.2, 10.8), None], (), 1, None, "west past the hall to the west head's panel, down onto it from above the water"),
        L("mill supply, hall", kit.cable(6600, kit.area_for(amps(recs["ball-mill-4m4"]["needs"]["power"], 6600))), ("EW:e1-in", "M:power-in"), [None, (-45.5, 10.2, 10.8), (-45.5, -6.5, 10.8), (-27.0, -6.5, 10.8), (-27.0, -6.5, 8.0), None]),
        L("hall 400 V", lvk(p_hall), ("edge:lv-hall", "EW:e2-out"), [None, (-55.0, 13.6, 0.3), (-55.0, 13.6, 12.5), (-53.5, 13.6, 12.5), (-53.5, 9.4, 12.5), (-53.5, 9.4, 10.8), None]),
        L("hall 400 V, inside", lvk(p_hall), ("EW:e2-in", "cap"), [None, (-46.5, 9.4, 10.8), (-46.5, -12.6, 10.8), (31.0, -12.6, 10.8)], (), 1, None, "along the shell's south side over the deck's edge, a joint box at each branch"),
        L("presses supply", lvk(recs["filter-press-2"]["needs"]["power"]), ("tee:hall 400 V, inside", "F:power-in"), [(-9.5, -12.6, 10.8), (-9.5, -6.0, 10.8), (-9.5, -6.0, 8.0), None]),
    ]
    for i in range(N_SEC):
        H = f"H{i + 1}"
        xp = at(H, "power-in")[0]
        links.append(L(f"{H} fans supply", lvk(recs["pressure-hall-30m"]["needs"]["power"]), ("tee:hall 400 V, inside", f"{H}:power-in"), [(xp, -12.6, 10.8), (xp, -12.0, 10.8), None]))
    for bid, xt, yt in (("B1", 5.5, -7.0), ("B2", 18.5, -7.0), ("B3", 18.0, 7.0), ("B4", 30.0, 7.0)):
        pp = at(bid, "power-in")
        links.append(L(f"{bid} supply", lvk(recs["balling-disc-7m5"]["needs"]["power"]), ("tee:hall 400 V, inside", f"{bid}:power-in"), [(xt, -12.6, 10.8), (xt, yt, 10.8), (xt, yt, pp[2]), None]))
    links += [
        L("west head supply", lvk(recs["pressure-hall-30m-end"]["needs"]["power"]), ("edge:lv-west-head", "EW:power-in"), [None, (-58.0, 15.2, 0.3), (-58.0, 15.2, 12.5), (-52.5, 15.2, 12.5), (-52.5, 8.6, 12.5), (-52.5, 8.6, 10.8), None]),
        L("east head supply", lvk(recs["pressure-hall-30m-end"]["needs"]["power"]), ("edge:lv-east-head", "EE:power-in"), [None, (56.0, -8.6, 0.3), (56.0, -8.6, 10.8), None]),
        L("lock supply", lvk(recs["ore-lock-hopper"]["needs"]["power"]), ("edge:lv-lock", "L:power-in"), [None, (-71.0, 0.0, 0.3), (-71.0, 0.0, at("L", "power-in")[2]), None], (), 1, None, "up the plinth's side"),
        L("fans supply", lvk(p_fans), ("edge:lv-fans", "cap"), [None, (121.0, -50.0, 0.5)], (), 1, None, "a joint box to each fan"),
        L("fan 1 supply", lvk(recs["pellet-gas-loop"]["needs"]["power"]), ("tee:fans supply", "G1:power-in"), [(123.0, -50.0, 0.5), None]),
        L("fan 2 supply", lvk(recs["pellet-gas-loop"]["needs"]["power"]), ("tee:fans supply", "G2:power-in"), [(133.5, -50.0, 0.5), (133.5, 18.0, 0.5), (123.0, 18.0, 0.5), None]),
        L("crew post supply", lvk(recs["crew-post"]["needs"]["power"]), ("edge:lv-post", "P:power-in"), [None, (-62.0, -24.0, 4.5), (-62.0, -24.0, 2.3), None], (), 1, None, "through the berm"),
    ]
    mobile = [{"unit": "unit.mill-crane-30t", "rides": "crane rails", "at": [-30.0, 0.0, HALL_D / 2 + 9.0]}]
    links.append(link("rail", "crane rails", (-xe + 1.0, 0.0, HALL_D / 2 + 9.0), (xe - 1.0, 0.0, HALL_D / 2 + 9.0), A36, 2 * 50.0, None, "the crane's two straight rails on the sections' runways, at y -11.4 and 11.4 (22.8 m apart); the link runs on their centre line"))
    R = HALL_D / 2
    arc = 2 * math.pi * R / 3
    wedge = (HALL_D * (R / 2) - (R ** 2 * math.acos(0.5) - (R / 2) * math.sqrt(R ** 2 - (R / 2) ** 2))) - arc * 0.5
    pit = (16.0, 2 * 17.0, -S_Z)
    ground = [
        {"what": "Hall bedding", "item": SINTER, "volume": r(arc * 0.5 * N_SEC * HALL_SEC), "note": f"0.5 m of sintered regolith under the lower third of the tube, x -{xe:g} to {xe:g}: the tube bears on it all along, grooved under the sections' and heads' flange rings"},
        {"what": "Hall cradle", "item": "material.regolith", "volume": r(wedge * N_SEC * HALL_SEC), "note": f"loose regolith packed against both sides of the tube up to z {R / 2:g}"},
        {"what": "Head pads", "item": SINTER, "volume": r(2 * 14 * 20 * 0.5), "note": "under the two heads and their locks"},
        {"what": "Lock plinth", "item": SINTER, "volume": r(8.0 * 8.0 * L_Z), "note": f"sintered blocks {L_Z:g} m high under the ore lock"},
        {"what": "Shaft pit", "item": SINTER, "volume": r(2 * (pit[0] + pit[1]) * pit[2] * 1.0 + pit[0] * pit[1] * 1.0), "note": f"a pit {pit[0]:g} x {pit[1]:g} m and {pit[2]:g} deep, x {S_X - 8:g} to {S_X + 8:g}, y -17 to 17: walls and floor of sintered blocks 1 m thick, a trench in its floor for the pellet collector"},
        {"what": "Pellet belt trench", "item": SINTER, "volume": r(4.0 * 0.5 * 60.0 + 2 * 60.0 * 5.0 * 0.5), "note": "the belt's ramp out of the pit, east: floor and walls"},
        {"what": "Unit pads", "item": SINTER, "volume": r(2 * 24 * 12 * 0.5 + 12 * 8 * 0.5), "note": "0.5 m under the gas loops and the transformer"},
        {"what": "Radiator legs' pads", "item": SINTER, "volume": r(n_rad * 4 * (44.0 * 44.0 / 72.0) * 0.027)},
        {"what": "Crew post berm", "item": "material.regolith", "volume": BERM, "note": "loose regolith at least 1 m over the post everywhere (its top 4 x 2 m, 5.3 m up), lying at 35 degrees, its foot about 19 x 17 m, a cavity for the post; measured on blender's arc furnace shop; with openings for its windows (light wells on the north) and its airlock (a ramp cut on the west); at the airlock's door a landing of packed regolith 2 x 2 m, its top at the threshold (1.35 m up), and a ramp down to the ground at about 1 in 3"},
    ]
    for g_ in ground:
        g_["mass"] = r(g_["volume"] * (RHO_SINTER if g_["item"] == SINTER else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used) + sum(l.get("power", 0) for l in links)
    heat = sum(u.get("needs", {}).get("heat", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g_["mass"] for g_ in ground)
    total = mass_units + mass_links + mass_ground
    mp = pmod["physical"]
    air = yaml.safe_load(open(S + "layouts/pellet-plant-air.yaml"))["totals"] if os.path.exists(S + "layouts/pellet-plant-air.yaml") else {"power": 0}
    against = (f"module.pellet-plant: one box {mp['length']:g} x {mp['width']:g} x {mp['height']:g} m, {P_P / 1e6:.0f} MW. The layout for air draws {air['power'] / 1e6:.1f} MW. "
               f"This layout: the same {P_RATE * 3.6:.0f} t an hour; {power / 1e6:.1f} MW, of which {P_FIRE / 0.9 / 1e6:.1f} the shafts' torches (the same firing heat; the presses and the shafts' fans take less than the disc filters' vacuum and the grate's fans); "
               f"{heat / 1e6:.1f} MW to {n_rad} radiator fields at 330 K ({n_rad * 48 * 48 / 1e4:.1f} ha), most of it the water the green balls bring, condensed out of the shafts' gas and kept; no water used up. "
               f"{(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of regolith, sintered and loose.")
    xs = [b["at"][0] + s * unit_dims(recs, b)[0] / 2 for b in bays for s in (-1, 1)] + [p[0] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    ys = [b["at"][1] + s * unit_dims(recs, b)[1] / 2 for b in bays for s in (-1, 1)] + [p[1] for l in links for p in (l["from"], l["to"], *l.get("via", []))]
    layout = {
        "identity": {"key": "layout.pellet-plant-airless", "name": "Pellet plant, airless",
                     "description": (f"Fine ore made into fired pellets on airless ground at about a moon's gravity: the wet section (ball mill, plate presses, four balling discs) in a steel tube hall {HALL_D:g} m across held at {HALL_P / 1e3:.0f} kPa of nitrogen so the water stays liquid; "
                                     f"the green balls by a sealed gallery to two pellet shaft furnaces in a pit, sealed and fired by plasma torches on a closed gas loop at {PS_P / 1e5:.0f} bar, which keeps the balls' water; the heat to radiator fields. Built separately from the plant for air (docs/proposals/pellet-plant-airless.md); laid of SFO 25 runs from port to port.")},
        "stands_for": "module.pellet-plant",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(ys) - min(ys)), "height": HALL_D, "mass": r(total)},
        "bays": bays, "ports": edge, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": P_RATE, "power": r(power), "heat": r(heat), "water": 0.0, "mass": r(total), "against_module": against},
        "staff": pmod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out for airless ground from the proposal, of SFO 25 runs; the hall's joints' bolts and seals not yet counted (about half a tonne a joint, from memory)."},
                  {"of": ["totals"], "tier": "derived", "review": True, "note": "Summed from the units and runs; the radiators from the heat the hall, shafts, gas loops and transformer reject."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": "Sintered regolith at 2,500 kg/m3, loose at 1,500; the shapes in the notes."},
                  {"of": ["physical"], "tier": "derived", "note": "Summed from the units, runs and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.pellet-plant-airless: {total / 1000:,.0f} t, {power / 1e6:.1f} MW, {heat / 1e6:.1f} MW to {n_rad} fields, {len(links)} links")
    return layout, bays


# The yards on airless ground: no water, no timber, no hall; the ingots cool by radiation to the sky.
STEEL_CRATE = yaml.safe_load(open(S + "packages/crate-steel.yaml"))
T_SAW = 150.0 + 273.15                 # K: an ingot cool enough for the saws (from memory)


def yard_airless_units(us):
    out = {}
    g = MOON_G[1]
    c = bridge_crane_unit(10000.0, 22.8, 3000.0, kind="Ingot gantry", job="on its own legs over the cooling beds, the tipper and the saws: it lifts hot ingots off the ingot car onto the beds, later sets each on the tipper, lays it on a saw's table and the cropped ingot in a steel crate on the crating line", gear="scissor ingot tongs, about 1.5 t of the trolley's", g=g, h_min=0.6)
    legs = 4 * box_tube(0.5, 0.5, 0.012, 12.0) * RHO["steel"] + 2 * box_tube(0.5, 0.4, 0.012, 8.0) * RHO["steel"]
    c["built_of"].append(part("Legs and sill beams", A36, legs, "four box legs 0.5 x 0.5 m, 12 mm, 12 m, and two sill beams"))
    c["built_of"].append(part("Knee braces, wheel seats and trolley rails", A36, 7651.1, "measured on blender's candidate (2026-10-16): the bracing between legs and girders, the bearing seats and the trolley's rails, which the sizing above left out"))
    c["dims"] = (8.0, 25.0, 14.0)
    c["desc"] = c["desc"].replace("on the hall's runways", "on its own legs, running on two ground rails 22.8 m apart").replace("its origin on the runway's rail plane", "its origin on the ground rails' plane, its girders 12 m up").replace("the hook raised to 0.1 m under the rail plane", "the hook raised under the bridge") + f" Sized for {g:g} m/s2. Its scissor tongs as the ingot crane's (unit.ingot-crane-10t)."
    out["ingot-gantry-10t-moon"] = c
    lab = dict(us["sampling-lab"])
    lab["name"] = "Sampling laboratory, sealed"
    lab["dims"] = (10.2, 4.0, 4.3)
    lab["size_class"], lab["fits"] = 2, "bay-s2"
    lab["desc"] = ("The sampling laboratory for airless ground: an aluminium cylinder 4 m across and 8 m long at 1 bar under a berm of loose regolith, its staff airlock at -x; the coupon comes in through a small sample lock beside it instead of a hatch. Local frame: the shell's axis along x at z 2.3, from x -2 to 6; one floor at z 1.4 through shell and lock (3.57 m wide in the shell, 1.73 m in the lock); the staff lock a 2.5 m cylinder from x -4 to -2, its axis at y -0.2, z 2.3, its doors 0.8 x 1.9 m for a crew in suits; the sample lock through the -x end head beside it, centred at y 1.55, z 2.3, from x -2.5 to -0.8, 0.6 m square clear (0.7 outside), its sill at z 2.0, reached from the layout's step platform (a coupon 0.48 m long fits only square). Steps up to the lock's door are the layout's. "
                   "Inside as unit.sampling-lab: an abrasive cutter, a grinder and a spark emission spectrometer, its spark under argon as always. " + us["sampling-lab"]["desc"].split(": no ingot comes in. ")[1].split(" Inside,")[0])
    lab["built_of"] = [part("Shell", "material.aluminium-alloy-6061", 1510.7, "4 m across, 8 m over its domed heads, 6 mm (1 bar gives 33 MPa), less its openings; measured on blender's candidate (2026-10-16)"),
                       part("Locks and floors", "material.aluminium-alloy-6061", 1245.5, "the staff lock's and sample lock's 6 mm shells, the floors and the four ribbed plug doors' skins; measured"),
                       part("Saddles, floor webs and door hardware", A36, 2357.6, "10 mm saddle webs on 12 mm sole plates, the floors' 100 x 12 mm webs, the doors' seats, hinges and latches; measured"),
                       part("Life support and fit-out", A36, 1500.0, "air and heat, benches and cabinets, not drawn; from memory"),
                       part("Cutter, grinder and spectrometer", "good.electric-motors", 3000.0, "from memory")]
    lab["needs"] = {"power": 80e3, "atmosphere": "vacuum"}
    out["sampling-lab-sealed"] = lab
    k = dict(us["crating-line"])
    k["name"] = "Crating line, steel crates"
    k["desc"] = us["crating-line"]["desc"].replace("an empty heavy crate", "an empty steel crate (package.crate-steel)").replace("the nailer's portal at x 3.5, where the lid is nailed on", "the latching portal at x 3.5, where the lid is clamped and latched").replace("label printer", "label marker (a stencil and paint that hold in vacuum)") + " For airless ground: no timber; its bearings and drives rated for vacuum and the lunar night."
    k["desc"] = k["desc"].replace("A roller table along x, its rollers' tops 0.8 m up", "A roller table along x, its rollers' tops 0.8 m up (100 mm rollers at 150 mm, so each of the crate's 160 mm skids, which run across y, always rests on one)").replace("its lid on top", "its hinged lid shut").replace("where the lid is lifted aside", "where a lid arm beside the table (on its -y side) swings the crate's hinged lid open and holds it up").replace("the latching portal at x 3.5, where the lid is clamped and latched", "the latching portal at x 3.5, the lid arm having swung the lid shut, where it is clamped and latched")
    k["built_of"] = [part("Roller table, scale, lid arm and latching portal", A36, 12000.0, "from memory")]
    k["needs"] = dict(us["crating-line"]["needs"], atmosphere="vacuum")
    out["crating-line-steel"] = k
    return out


def ore_yard_airless(us, recs):
    """The ore yard on airless ground: as for air, without the sprays; sintered regolith for concrete."""
    air, _ = ore_yard_air(us, recs)
    d = yaml.safe_load(yaml.safe_dump(air))
    d["links"] = [l for l in d["links"] if not l["name"].startswith("dust sprays")]
    for l in d["links"]:
        if l["item"] == CONC:
            l["item"], l["mass"] = SINTER, r(l["mass"] * RHO_SINTER / RHO["concrete"])
            l["note"] = l["note"].replace("paved", "of sintered regolith")
    for g_ in d["ground"]:
        if g_["item"] == CONC:
            g_["item"], g_["mass"] = SINTER, r(g_["volume"] * RHO_SINTER)
            g_["note"] = g_.get("note", "").replace("; measured on blender's layout (candidate v01)", " (the volume as measured on blender's layout for air)").replace("sloped to drains", "level")
    mass_units = sum(recs[b["unit"].split(".", 1)[1]]["physical"]["mass"] for b in d["bays"]) + sum(recs[m_["unit"].split(".", 1)[1]]["physical"]["mass"] for m_ in d["mobile"])
    total = mass_units + sum(l["mass"] for l in d["links"]) + sum(g_["mass"] for g_ in d["ground"])
    d["identity"] = {"key": "layout.ore-yard-airless", "name": "Ore yard, airless",
                     "description": ("Two stockpile beds of ore either side of the bucket wheel stacker-reclaimer's rail, on airless ground at about a moon's gravity: as the yard for air, with the same machines, but no water sprays (with no air, dust thrown up falls back at once instead of drifting) and sintered regolith for concrete. "
                                     "The machines' bearings, lubricants and belts are of kinds rated for vacuum and the lunar night's cold (rubber stiffens below about -50 deg C), not checked. Built separately from the yard for air.")}
    d["site"] = {"atmosphere": "none", "gravity": list(MOON_G)}
    d["physical"]["mass"] = r(total)
    d["totals"]["water"] = 0.0
    d["totals"]["mass"] = r(total)
    d["totals"]["against_module"] = d["totals"]["against_module"].split(" t of plant and supports, ")[0].rsplit("; ", 1)[0] + f"; no water used; {total / 1000:,.0f} t in all, its ground of sintered regolith."
    d["basis"] = [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "As the yard for air, without the sprays."},
                  {"of": ["ground", "physical"], "tier": "derived", "review": True, "note": "The beds as for air (iron ore's angle of repose barely depends on gravity); sintered regolith at 2,500 kg/m3 in place of concrete."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}]
    d.pop("visual", None)
    print(f"layout.ore-yard-airless: {total / 1000:,.0f} t")
    return d, d["bays"]


def ingot_yard_airless(us, recs):
    """The ingot yard on airless ground: no hall; a gantry over cooling beds open to the sky, steel crates four high."""
    ing_rate = C_RATE * SOUND / ING_M
    # Radiation alone, to a black sky: an ingot standing on a bed, 0.2 of its view to its neighbours.
    area = 4 * (ING_A + ING_B) / 2 * ING_L + ING_A ** 2
    mc = (ING_M + HOT_TOP) * 600.0
    eps = 0.8 * 0.8
    t_cool = mc / (3 * area * eps * SIGMA) * (1 / T_SAW ** 3 - 1 / (700.0 + 273.15) ** 3)
    on_beds = math.ceil(ing_rate * t_cool)
    pitch = 1.2
    bed_x, bed_y = 22.0, 18.0
    per_bed = int(bed_x / pitch) * int(bed_y / pitch)
    n_beds = math.ceil(on_beds / per_bed)
    beds = [(-26.0 + i * (bed_x + 2.0), -9.0) for i in range(n_beds)]
    x_e = beds[-1][0] + bed_x
    saws = 2
    bays = [{"id": "T", "mount": "plant-mount.bay-s1", "unit": "unit.ingot-tipper", "at": [r(x_e + 4.5), -6.0, 0.0], "note": "east of the beds, under the gantry"},
            {"id": "C1", "mount": "plant-mount.bay-s2", "unit": "unit.crop-saws", "at": [r(x_e + 16.0), -3.0, 0.0], "note": "cutting dry: no coolant in vacuum, so at about half the rate for air (from memory): two sets"},
            {"id": "C2", "mount": "plant-mount.bay-s2", "unit": "unit.crop-saws", "at": [r(x_e + 30.0), -3.0, 0.0], "note": ""},
            {"id": "K", "mount": "plant-mount.bay-s2", "unit": "unit.crating-line-steel", "at": [r(x_e + 30.0), 7.0, 0.0], "note": "under the gantry's east end, out along +x"},
            {"id": "S", "mount": "plant-mount.bay-s2", "unit": "unit.sampling-lab-sealed", "at": [r(x_e + 6.0), -20.0, 0.0], "note": "outside the gantry, south, under its berm"},
            {"id": "P", "mount": "plant-mount.bay-s1", "unit": "unit.crew-post", "at": [r(x_e + 18.0), -20.0, 0.0], "note": "south, under its berm"}]
    x_g = x_e + 40.0
    crates = math.ceil(imod["capacity"]["holds"] / (ING_M + STEEL_CRATE["physical"]["mass"]))
    lanes = math.ceil(crates / STEEL_CRATE["stack"] / 10)
    lane_w, lane_d = STEEL_CRATE["physical"]["width"] + 0.1, 10 * STEEL_CRATE["physical"]["length"]
    rows = 4
    per_row = math.ceil(lanes / (2 * rows))
    yard_L, yard_W = per_row * lane_w, rows * (2 * lane_d + 4.5)
    yx0 = x_g + 10.0
    aisle_y = [r(-yard_W / 2 + lane_d + 2.25 + i * (2 * lane_d + 4.5)) for i in range(rows)]
    aisle_path = []
    for i, y in enumerate(aisle_y):
        ends = (yx0, yx0 + yard_L) if i % 2 == 0 else (yx0 + yard_L, yx0)
        aisle_path += [(r(ends[0]), y, 0.0), (r(ends[1]), y, 0.0)]
    links = [
        link("rail", "ingot rail", (-40.0, 6.0, 0.0), (-28.0, 6.0, 0.0), A36, 2 * 50.0, None, "from the casting bay, its car stopping at x -34 by the beds' west end, under the gantry"),
        link("rail", "gantry rails", (-36.0, 0.0, 0.0), (r(x_g), 0.0, 0.0), A36, 2 * 60.0, None, "the ingot gantry's two straight rails over the beds, the tipper, the saws and the crating line, at y -11.4 and 11.4 (the gantry's wheels); the link runs on their centre line"),
        link("road", "yard aisles", aisle_path[0], aisle_path[-1], SINTER, 4.5 * 0.25 * RHO_SINTER, None, f"{rows} forklift aisles 4.5 m wide, {yard_L:.0f} m long, joined end to end, of sintered regolith 0.25 m", via=aisle_path[1:-1]),
        link("road", "truck lane", (r(x_g), -3.0, 0.0), (r(yx0 + yard_L + 20.0), -3.0, 0.0), SINTER, 7.0 * 0.3 * RHO_SINTER, None, "7 m wide, sintered regolith 0.3 m: trucks load crates at the yard's east end, beyond the layout"),
    ]
    mobile = [{"unit": "unit.ingot-car-40t", "rides": "ingot rail", "at": [-34.0, 6.0, 0.0]},
              {"unit": "unit.ingot-gantry-10t-moon", "rides": "gantry rails", "at": [0.0, 0.0, 0.0]}]
    mobile += [{"unit": "unit.forklift-2t5", "rides": "yard aisles", "at": [r(yx0 + 10.0 + 20.0 * i), aisle_y[i % rows], 0.0]} for i in range(4)]
    ground = [{"what": f"Cooling bed {i + 1}", "item": CAST_IRON, "volume": r(bed_x * bed_y * 0.05), "note": f"cast iron floor plates 50 mm thick over x {x:g} to {x + bed_x:g}, y {y:g} to {y + bed_y:g}, on sintered regolith: {per_bed} places, the ingots standing big end up at {pitch:g} m each way, open to the sky"} for i, (x, y) in enumerate(beds)]
    ground += [{"what": "Work floor", "item": SINTER, "volume": r((x_g - x_e) * 24.0 * 0.4), "note": "0.4 m of sintered regolith under the tipper, saws and crating line"},
               {"what": "Gantry rail sleepers", "item": SINTER, "volume": r(2 * (x_g + 36.0) * 1.0 * 0.6)},
               {"what": "Stack yard", "item": SINTER, "volume": r((yard_L * yard_W - rows * yard_L * 4.5) * 0.25), "note": f"{yard_L:.0f} x {yard_W:.0f} m of sintered regolith 0.25 m: {crates:,} steel crates four high in {lanes} lanes ten deep (no tarpaulins: no weather)"},
               {"what": "Berms", "item": "material.regolith", "volume": r(2 * BERM), "note": "over the crew post and the sealed lab, each as a crew post's: 1 m over the crest at 35 degrees (the lab's a little longer, not counted)"}]
    for g_ in ground:
        g_["mass"] = r(g_["volume"] * (7200.0 if g_["item"] == CAST_IRON else RHO_SINTER if g_["item"] == SINTER else 1500.0))
    used = [recs[b["unit"].split(".", 1)[1]] for b in bays] + [recs[m_["unit"].split(".", 1)[1]] for m_ in mobile]
    power = sum(u.get("needs", {}).get("power", 0) for u in used)
    mass_units = sum(u["physical"]["mass"] for u in used)
    mass_links = sum(l["mass"] for l in links)
    mass_ground = sum(g_["mass"] for g_ in ground)
    total = mass_units + mass_links + mass_ground
    heat = C_RATE * SOUND * 600.0 * (700.0 - 150.0)
    against = (f"module.ingot-yard: one box, holding {imod['capacity']['holds'] / 1e6:.0f} kt. This layout: {crates:,} steel crates four high (a moon's gravity and the steel crate allow it) on a yard {yard_L:.0f} x {yard_W:.0f} m; "
               f"no hall: {on_beds} ingots stand {t_cool / 3600:.0f} hours on {n_beds} beds open to the sky, radiating {heat / 1e6:.1f} MW to it (with no air, light alone takes their heat; on a lunar day the sun adds about a tenth near the end); "
               f"the saws cut dry, two sets; {power / 1e6:.2f} MW; no water; one gantry makes about four moves an ingot ({4 * ing_rate * 3600:.0f} an hour), more than one crane can at a minute or so a move: its count is not yet worked out; {(mass_units + mass_links) / 1000:,.0f} t of plant, {mass_ground / 1000:,.0f} t of beds and ground.")
    xs = [-40.0, yx0 + yard_L + 20.0]
    layout = {
        "identity": {"key": "layout.ingot-yard-airless", "name": "Ingot yard, airless",
                     "description": "The ingots on airless ground at about a moon's gravity: they come hot on their car from the casting bay; a gantry crane on its own legs stands them on cast iron beds open to the black sky, where they cool by radiation alone; it tips them, the saws crop them dry, a sealed lab reads each heat, and they go one to a steel crate (no timber here), stacked four high by forklifts. No hall, no water. Built separately from the yard for air."},
        "stands_for": "module.ingot-yard",
        "site": {"atmosphere": "none", "gravity": list(MOON_G)},
        "physical": {"length": r(max(xs) - min(xs)), "width": r(max(yard_W, 48.0)), "height": 14.0, "mass": r(total)},
        "bays": bays, "mobile": mobile, "links": links, "ground": ground,
        "totals": {"rate": 0.0, "power": r(power), "heat": 0.0, "water": 0.0, "mass": r(total), "against_module": against},
        "staff": imod.get("staff"),
        "basis": [{"of": ["bays", "links", "mobile"], "tier": "invented", "review": True, "note": "Laid out for airless ground from the yard for air."},
                  {"of": ["ground"], "tier": "derived", "review": True, "note": f"The cooling time from radiation alone: an ingot's {area:.1f} m2 at emissivity 0.8, a fifth of its view to its neighbours, from 700 deg C to {T_SAW - 273.15:.0f} (from memory, for the saws): {t_cool / 3600:.1f} h, so {on_beds} on the beds; sintered regolith at 2,500 kg/m3, cast iron 7,200."},
                  {"of": ["totals", "physical"], "tier": "derived", "note": "Summed from the units, links and ground."},
                  {"of": ["staff"], "tier": "derived", "note": "The module's watch, unchanged."}],
    }
    print(f"layout.ingot-yard-airless: {total / 1000:,.0f} t, {on_beds} ingots on {n_beds} beds ({t_cool / 3600:.1f} h), {crates:,} crates, yard {yard_L:.0f} x {yard_W:.0f} m")
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
    hu["cooling-tower-h2"] = cooling_unit(N_BLK * (hu["electrolyser-block-20mw"]["needs"]["heat"] + hu["rectifier-transformer-21mva"]["needs"]["heat"]) + hu["hydrogen-purifier"]["needs"]["heat"], "the electrolysers', rectifiers' and purifier's")
    us.update(hu)
    us.update(pellet_units())
    us.update(yard_units())
    us.update(airless_units(us))
    us.update(airless_ladle_units(us))
    us.update(shaft_airless_units(us))
    us.update(hydrogen_airless_units(us))
    us.update(pellet_airless_units(us))
    us.update(yard_airless_units(us))
    trial_ports(us)
    h2_ports(us)
    pellet_ports(us)
    arc_ports(us)
    ladle_ports(us)
    recs = {slug: unit_record(slug, u) for slug, u in us.items()}
    layouts = {"arc-furnace-air": arc_furnace_air(us, recs), "ladle-station-air": ladle_station_air(us, recs), "casting-bay-air": casting_bay_air(us, recs), "shaft-furnace-air": shaft_furnace_air(us, recs), "hydrogen-plant-air": hydrogen_plant_air(us, recs), "pellet-plant-air": pellet_plant_air(us, recs), "ore-yard-air": ore_yard_air(us, recs), "ingot-yard-air": ingot_yard_air(us, recs), "arc-furnace-airless": arc_furnace_airless(us, recs),
               "ladle-station-airless": ladle_station_airless(us, recs), "casting-bay-airless": casting_bay_airless(us, recs),
               "shaft-furnace-airless": shaft_furnace_airless(us, recs), "hydrogen-plant-airless": hydrogen_plant_airless(us, recs),
               "pellet-plant-airless": pellet_plant_airless(us, recs), "ore-yard-airless": ore_yard_airless(us, recs), "ingot-yard-airless": ingot_yard_airless(us, recs)}
    mounts = mounts_for([b for _, bays in layouts.values() for b in bays], recs)
    for slug, d in mounts.items():
        write(S + f"plant-mounts/{slug}.yaml", d)
    for slug, d in recs.items():
        write(S + f"units/{slug}.yaml", d)
    for slug, (d, _) in layouts.items():
        cover_model(S + f"layouts/{slug}.yaml", d)
        write(S + f"layouts/{slug}.yaml", d)
    for folder, slug, d in kit.records():
        write(S + f"{folder}/{slug}.yaml", d)
    print(f"{len(mounts)} mounts, {len(recs)} units, {len(layouts)} layouts")


if __name__ == "__main__":
    main()
