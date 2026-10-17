"""Plant connectors and runs (SFO 25): the made parts a layout is snapped together from.

A layout asks for what it needs (`pipe(...)`, `flange(...)`, ...); each call sizes the class from its pressure, size and
material and returns its key. `records()` gives every class asked for, to be written to connectors/ and runs/.
`lay(run, points, ...)` counts a run's straights, elbows, tees, supports and connectors along a path and weighs it.

Sizes come from one series of nominal bores (EN 1092 DN). Walls from pressure (Barlow, B31.3's allowable stresses);
flanges by a simple bolt-and-ring rule, not to a flange code; supports from the span a pipe full of its fluid bears at
1 g. Figures from memory are said so in each record's basis.
"""

import math

G = 9.80665
DN = [0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.6, 2.8, 3.0, 3.2, 3.6, 4.0]
PN = {1: 1.0e5, 10: 1.0e6, 16: 1.6e6, 25: 2.5e6}
AREAS = [16, 25, 35, 50, 70, 95, 120, 150, 185, 240, 300, 400, 500, 630]      # mm2, cable conductors
BOLTS = [0.016, 0.020, 0.024, 0.027, 0.030, 0.036, 0.042, 0.048, 0.056, 0.064]   # m, bolt diameters
A36, SS, BOLT, CU, AL = "material.structural-steel-a36", "material.stainless-steel-304", "material.high-strength-steel-4340", "material.copper", "material.aluminium-alloy-6061"
REF, MLI, PE, RUBBER, SILICA = "good.refractory-brick", "material.multi-layer-insulation", "material.polyethylene-uhmw", "material.silicone-rubber", "material.silica-aerogel"
RHO = {A36: 7850, SS: 7900, BOLT: 7850, CU: 8960, AL: 2700, REF: 3000, MLI: 50, PE: 940, RUBBER: 1075, SILICA: 250}
STRESS = {A36: 133e6, SS: 115e6}        # Pa allowable (B31.3, A36 and 304 to about 340 deg C; from memory)
E_WELD = 0.85
TRANSFER_DROP = 0.5                     # m: a belt's head over the belt it drops onto, in a hood (SFO 25 `onto:` ends)
J_VACUUM = 2.5e6                        # A/m2 in a cable's copper on airless ground: radiation only, about half what air allows (from memory)

_recs = {}


def r(x, nd=1):
    return round(float(x), nd)


def dn_for(d):
    """The nominal bore a flow needing `d` m is laid in: the next size up, allowing 2% under."""
    return next(n for n in DN if n >= 0.98 * d)


def _tag(x):
    return f"{round(x * 1000):d}"


def _volt(v):
    return f"{v / 1000:g}kv".replace(".", "p") if v >= 1000 else f"{v:g}v"


def _part(what, item, mass, note=""):
    d = {"what": what, "item": item, "mass": r(mass)}
    if note:
        d["note"] = note
    return d


def wall(dn, p, item, temp=None):
    """m of wall a pipe of nominal bore dn holds p in: Barlow at the allowable stress, a weld factor, 1.5 mm for
    corrosion on carbon steel; at least 3 mm plus dn/200 to be handled. Rounded up to 0.5 mm."""
    t = p * dn / (2 * STRESS[item] * E_WELD) + (0.0015 if item == A36 else 0.0)
    t = max(t, 0.003 + dn / 200)
    return math.ceil(t * 2000) / 2000


# ---------------------------------------------------------------- connectors

def flange(dn, pn, item=A36, sealing="single", temp=373.0):
    """A bolted flange pair on a pipe of nominal bore dn, class pn: bolts carry 1.5 times the end force on the gasket
    (1.8 with two seals), the ring's thickness from bending about the bolt circle."""
    key = f"connector.flange-dn{_tag(dn)}-pn{pn}" + ("-ss" if item == SS else "") + ("-double" if sealing == "double" else "")
    if key in _recs:
        rt = _recs[key]["rating"]
        rt["temperature"] = max(rt["temperature"], temp)       # (rated for the hottest line that asks for it, whoever asks first)
        return key
    p = PN[pn]
    dg = dn + 0.05
    w = (1.8 if sealing == "double" else 1.5) * p * math.pi / 4 * dg ** 2
    for db in BOLTS:
        k = dg + 2 * (1.5 * db + 0.01) + (0.03 if sealing == "double" else 0.0)
        a_s = math.pi / 4 * (0.9 * db) ** 2
        n = max(8, math.ceil(max(w / (170e6 * a_s), math.pi * k / (7 * db)) / 4) * 4)
        if math.pi * k / n >= 2.5 * db:
            break
    h = (k - dg) / 2
    t = max(0.02, math.sqrt(6 * max(w, 1.0) * h / (math.pi * dg * 140e6)))
    t = math.ceil(t * 500) / 500
    D = k + 3 * db
    wp = wall(dn, p, item)
    ri, ro, rh = dn / 2, dn / 2 + wp, dn / 2 + 4 * wp                     # (the neck: a hub one ring thick, tapering from 4 walls to the pipe's)
    neck = math.pi * t * ((rh * rh + rh * ro + ro * ro) / 3 - ri * ri)
    ring = 2 * (math.pi / 4 * (D ** 2 - dn ** 2) * t + neck) * RHO[item]
    lb = 2 * t + 0.006 + 2 * db
    bolts = n * RHO[BOLT] * (math.pi / 4 * db ** 2 * lb + 2 * (0.866 * (1.5 * db) ** 2 - math.pi / 4 * db ** 2) * 0.8 * db)
    gw = 0.015 + 0.01 * dn
    gasket = (2 if sealing == "double" else 1) * math.pi * dg * gw * 0.0045 * 0.6 * RHO[SS]
    ring_note = f"two weld-neck rings {D:.3f} m across, {t * 1000:.0f} mm thick, their necks tapering over {t * 1000:.0f} mm from {4 * wp * 1000:.0f} mm to the pipe's wall; along the pipe the joint is {(2 * t + 0.006 + max(4 * db, 2 * t)) * 1000:.0f} mm with the necks and the bolts' and nuts' ends"
    if key in FLANGE_MEASURED:          # (a joint whose pipe is not the pipe rule's: a gallery's 4 mm tube)
        ring, ring_note = FLANGE_MEASURED[key]
    built = [_part("Flanges", item, ring, ring_note),
             _part("Bolts and nuts", BOLT, bolts, f"{n} of M{db * 1000:.0f}, {lb * 1000:.0f} mm, on a {k:.3f} m circle"),
             _part("Gaskets" if sealing == "double" else "Gasket", SS, gasket, "spiral wound, stainless windings" + ("; two, the space between them piped to a leak watch" if sealing == "double" else ""))]
    _recs[key] = {"identity": {"key": key, "name": f"Flange DN {dn * 1000:.0f} PN {pn}" + (", stainless" if item == SS else "") + (", double seal" if sealing == "double" else ""),
                               "description": f"A bolted pair of weld-neck flanges for a pipe of {dn * 1000:.0f} mm nominal bore at up to {p / 1e5:.0f} bar inside (on airless ground, the whole of it against vacuum)."},
                  "kind": "flange", "size": dn, "rating": {"pressure": p, "temperature": temp}, "sealing": sealing,
                  "physical": {"length": r(D, 3), "width": r(D, 3), "height": r(2 * t + 0.006 + max(4 * db, 2 * t), 3), "mass": r(sum(b["mass"] for b in built))}, "built_of": built,
                  "basis": [{"of": ["built_of", "physical"], "tier": "derived", "review": True, "note": f"Sized by a simple rule, not to a flange code: the end force on the gasket's mean circle ({dg:.2f} m) at the class's pressure, bolts at 170 MPa, the ring's thickness from its bending about the bolt circle at 140 MPa."}]}
    return key


def lined_flange(dn, pn, temp=473.0, sealing="single"):
    key = f"connector.lined-flange-dn{_tag(dn)}-pn{pn}" + ("-double" if sealing == "double" else "")
    if key in _recs:
        return key
    had = set(_recs)
    plain = flange(dn, pn, A36, sealing, temp)
    base = _recs[plain] if plain in had else _recs.pop(plain)  # the plain pair is its base, not a record unless asked for
    pack = math.pi * (dn - 0.2) * 0.2 * 0.05 * RHO[SILICA]
    built = [dict(b) for b in base["built_of"]] + [_part("Lining joint", SILICA, pack, "the refractory's joint packed with 50 mm of silica fibre; silica aerogel stands for it")]
    _recs[key] = {"identity": {"key": key, "name": f"Lined flange DN {dn * 1000:.0f} PN {pn}" + (", double seal" if sealing == "double" else ""), "description": f"The flange pair of a refractory-lined hot gas duct whose steel shell is {dn * 1000:.0f} mm nominal bore, the lining's joint packed; the shell kept cool by the lining."},
                  "kind": "lined-flange", "size": dn, "rating": {"pressure": PN[pn], "temperature": temp}, "sealing": sealing,
                  "physical": dict(base["physical"], mass=r(sum(b["mass"] for b in built))), "built_of": built,
                  "basis": base["basis"] + [{"of": ["built_of"], "tier": "invented", "review": True, "note": "The lining's joint from memory."}]}
    return key


def chute(dn, item=SS):
    """A sealed bolted chute flange for bulk out of a lock into a closed conveyor: no pressure across it (the lock
    lets the bulk down to the conveyor's), sealed against dust."""
    key = f"connector.chute-dn{_tag(dn)}"
    if key in _recs:
        return key
    t = 0.02
    D = dn + 0.2
    ring = 2 * math.pi / 4 * (D ** 2 - dn ** 2) * t * RHO[item]
    n = max(8, math.ceil(math.pi * (dn + 0.1) / 0.15 / 4) * 4)
    bolts = n * 0.185          # kg an M16 bolt and nut, measured on blender's DN 800 chute (20 of them, 3.7 kg)
    built = [_part("Flanges", item, ring, f"two plate rings {D:.2f} m across, 20 mm"), _part("Bolts", BOLT, bolts, f"{n} of M16"),
             _part("Gasket", "material.fused-silica", math.pi * (dn + 0.05) * 0.03 * 0.006 * 2200.0, "a 6 mm silica-fibre dust gasket, for the sponge iron's 700 deg C (rubber would not take it); fused silica stands for it")]
    _recs[key] = {"identity": {"key": key, "name": f"Chute flange DN {dn * 1000:.0f}", "description": f"A sealed bolted flange where bulk falls through a {dn * 1000:.0f} mm chute into a closed conveyor."},
                  "kind": "chute", "size": dn, "rating": {"pressure": 0.0, "temperature": 973.0}, "sealing": "single",
                  "physical": {"length": r(D, 3), "width": r(D, 3), "height": r(2 * t + 0.006 + 4 * 0.016, 3), "mass": r(sum(b["mass"] for b in built))}, "built_of": built,
                  "basis": [{"of": ["built_of"], "tier": "invented", "review": True, "note": "Plate rings and M16 bolts at a 150 mm pitch, from memory; along the chute the rings, gasket and the bolts' and nuts' ends, as for pipe flanges."}]}
    return key


HOOD_WATER_MEASURED = {2.0: 261.36}     # kg in a hood's ring and plenum, measured on blender's DN 2000 hood (2026-10-16)


def hood(dn):
    """A catch hood: a fixed water-cooled ring over a moving outlet (a tilting furnace's elbow), not touching it: the
    gap draws in air, which burns the outlet's CO, and lets the outlet move. No pressure across it."""
    key = f"connector.hood-dn{_tag(dn)}"
    if key in _recs:
        return key
    D, h, t = dn + 0.4, 0.5, 0.010
    ring = math.pi * D * h * t * RHO[A36] * 2 + math.pi / 4 * (D ** 2 - dn ** 2) * t * RHO[A36]
    water = math.pi * D * h * 0.05 * 1000.0
    built = [_part("Hood ring", A36, ring, f"a double-walled ring {D:.2f} m across and {h:g} m deep, 10 mm, its water jacket 50 mm, and its top plate"),
             _part("Jacket water", "good.water", HOOD_WATER_MEASURED.get(dn, water), "in the ring's jacket and the plenum in its top 100 mm, open to the duct's jacket: no feed of its own" + (", measured on blender's model" if dn in HOOD_WATER_MEASURED else ""))]
    _recs[key] = {"identity": {"key": key, "name": f"Catch hood DN {dn * 1000:.0f}", "description": f"The water-cooled mouth of a {dn * 1000:.0f} mm duct facing a moving outlet's, {GAP_HOOD:g} m clear of it: air drawn in through the gap burns the gas's CO. It is a sleeve on the duct's end, drawn back {HOOD_STROKE:g} m along its axis before the outlet moves (a furnace tilting to tap or deslag, its arc off), the outlet's mouth then sliding across its plane. The port it serves is the outlet's mouth; the duct's end stands {GAP_HOOD:g} m from it, the way it faces."},
                  "kind": "hood", "size": dn, "rating": {"pressure": 0.0, "temperature": 1873.0}, "sealing": "single",
                  "physical": {"length": r(D, 3), "width": r(D, 3), "height": h, "mass": r(sum(b["mass"] for b in built))}, "built_of": built,
                  "basis": [{"of": ["built_of", "physical"], "tier": "invented", "review": True, "note": "Sizes from memory of arc furnace off-gas hoods; the gap from memory, adjustable on real plant."}]}
    return key


GAP_HOOD = 0.15                  # m between a moving outlet's mouth and the hood facing it (from memory of arc furnace ducts)
HOOD_STROKE = 1.0                # m the hood's sleeve is drawn back before the outlet moves (invented)


CLAMP_JAWS_MEASURED = {1.2: 173.0}       # kg of a clamp coupling's C jaws, measured on blender's DN 1200 (2026-10-16)
CLAMP_LINING_MEASURED = {1.2: 167.6}     # kg of its spool's refractory taper, 1.2 to 0.8 m clear


def clamp_coupling(dn):
    """A sealed clamped coupling between a moving outlet and a fixed duct: a flange pair held by a quick clamp ring,
    a slide gate on the outlet's side, and a 0.15 m spool on the duct's side that draws back to open the joint before
    the outlet moves (a sealed furnace tilting; its gate shut, so it stays sealed)."""
    key = f"connector.clamp-coupling-dn{_tag(dn)}"
    if key in _recs:
        return key
    D, t = dn + 0.4, 0.04
    rings = 2 * math.pi / 4 * (D ** 2 - dn ** 2) * t * RHO[A36]
    clamp = math.pi * D * 0.15 * 0.02 * RHO[A36]
    spool = math.pi * (dn + 0.012) * 0.012 * 0.15 * RHO[A36]
    built = [_part("Flange rings", A36, rings, f"two rings {D:.2f} m across, {t * 1000:.0f} mm, with a double O-ring seal"),
             _part("Clamp ring and its drives", A36, CLAMP_JAWS_MEASURED.get(dn, clamp) + 150.0, "a segmented clamp ring of C jaws round the rings" + (f" ({CLAMP_JAWS_MEASURED[dn]:g} kg measured on blender's model)" if dn in CLAMP_JAWS_MEASURED else "") + " and two actuators (150 kg, from memory); it opens in two steps, the jaws out and then the spool back"),
             _part("Sliding spool", A36, spool, f"a 0.15 m spool on the duct's side, its stroke {GAP_HOOD:g} m, sliding over the duct's end"),
             _part("Spool lining", REF, math.pi / 4 * (dn ** 2 - (dn - 0.4) ** 2) * 0.15 / 2 * RHO[REF] if dn not in CLAMP_LINING_MEASURED else CLAMP_LINING_MEASURED[dn], "the refractory tapering from the furnace's clear bore to the lined duct's within the spool" + (", measured on blender's model" if dn in CLAMP_LINING_MEASURED else "")),
             _part("Seals", RUBBER, math.pi * (dn + 0.1) * 2 * 0.01 * 0.01 * RHO[RUBBER], "the O-rings; silicone rubber stands for them")]
    _recs[key] = {"identity": {"key": key, "name": f"Clamp coupling DN {dn * 1000:.0f}", "description": f"A sealed, clamped joint between a moving outlet and a fixed {dn * 1000:.0f} mm duct. To let the outlet move (a sealed furnace tilting to tap or deslag), its slide gate shuts, the clamp opens and the duct's spool draws back {GAP_HOOD:g} m. The outlet's face then slides across the duct's, never into it. The gate is the outlet's; its mass is the unit's."},
                  "kind": "clamp-coupling", "size": dn, "rating": {"pressure": 1.0e5, "temperature": 773.0}, "sealing": "double",
                  "physical": {"length": r(D, 3), "width": r(D, 3), "height": r(2 * t + 0.15, 3), "mass": r(sum(b["mass"] for b in built))}, "built_of": built,
                  "basis": [{"of": ["built_of", "physical"], "tier": "invented", "review": True, "note": "Sizes and the clamp's drives from memory of furnace off-gas couplings."}]}
    return key


FLANGE_MEASURED = {"connector.flange-dn3000-pn1-double": (333.18, "two weld-neck rings 3.196 m across, 20 mm thick, their necks tapering over 20 mm from 16 mm to the gallery's 4 mm wall at 3 m outside, 2.992 m clear; measured on blender's model v1")}
SEALED_TRANSFER_MEASURED = {0.8: (806.35, 59.02), 1.0: (948.71, 70.05)}     # kg of steel and liner, the sealed transfers measured
TRANSFER_MEASURED = {1.2: (1652.31, 146.40), 1.0: (1395.23, 123.10), 0.8: (1176.03, 103.72), 0.65: (959.32, 84.67)}     # kg of plate and liner, by belt width (blender's models)


def _by_width(table, width):
    """A measured pair at this width, or between (or beyond) the measured ones in a straight line: the hood's length
    is the same for every width, so the plate goes nearly as the width, not its square."""
    if width in table:
        return table[width]
    (w0, a), (w1, b_) = sorted(table.items())[:2] if width < min(table) else sorted(table.items())[-2:] if width > max(table) else next(((x, y) for x, y in zip(sorted(table.items()), sorted(table.items())[1:]) if x[0] <= width <= y[0]))
    t = (width - w0) / (w1 - w0)
    return tuple(p + t * (q - p) for p, q in zip(a, b_))


def belt_transfer(width, sealed=False):
    """Where a belt's head drops into a hopper or onto another belt. sealed: from a gallery's sealed head box, the
    drop chute closed and bolted at both ends, so the gas in the gallery stays in it."""
    key = f"connector.belt-transfer-{_tag(width)}" + ("-sealed" if sealed else "")
    if key in _recs:
        return key
    b = width + 0.3
    steel, liner = _by_width(TRANSFER_MEASURED, width)
    src = f"measured on blender's model v1 of the {width * 1000:.0f} mm transfer" if width in TRANSFER_MEASURED else "between the measured transfers, by the chute's width"
    built = [_part("Hood, drop chute and mounting tabs", A36, steel, f"a hood over the belt's head and a chute {b:.1f} m square, 3 m down, 10 mm plate, on four 12 mm tabs seated on the head frame's rails (not hung from the pulley's bearings); {src}"),
             _part("Wear liner", PE, liner, f"10 mm UHMW polyethylene; {src}")]
    if sealed:          # (no hood: the head box is the hood; a straight closed chute from its floor down to the port)
        h_ = 3.0 - 1.525
        built = [_part("Chute", A36, 4 * b * h_ * 0.010 * RHO[A36], f"a straight chute {b:.1f} m square, {h_:.3f} m long, 10 mm plate"),
                 _part("Wear liner", PE, 4 * b * h_ * 0.010 * RHO[PE], "10 mm UHMW polyethylene"),
                 _part("Flanges and seals", A36, 2 * 4 * (b + 0.1) * 0.1 * 0.02 * RHO[A36] + 20.0, f"a bolted flange {b + 0.2:.1f} m outside at each end, with their seals; from memory")]
        if width in SEALED_TRANSFER_MEASURED:
            st, li = SEALED_TRANSFER_MEASURED[width]
            built = [_part("Chute, flanges and bolts", A36, st, f"a straight chute {b:.1f} m square, 10 mm, its four flange rings and 24 bolts; measured on blender's model v1"),
                     _part("Wear liner", PE, li, "10 mm UHMW polyethylene; measured on blender's model v1")]
    desc = (f"Where a {width * 1000:.0f} mm belt's head drops into a hopper or onto another belt: the hood and the drop chute (3 m). A receiving belt's skirt is its tail's. Its origin is the head pulley's centre: the hood over it to 0.55 m up, inside the pulley's bearings, the chute flaring below it to its square and down to 3 m under the pulley, where the port is."
            if not sealed else f"Where a sealed gallery's {width * 1000:.0f} mm belt drops out of its head box: a closed chute from the box's bottom outlet (1.525 m under the head pulley's centre, the origin) down to 3 m under it, where the port is, bolted and sealed at both ends; the hood is the head box's.")
    _recs[key] = {"identity": {"key": key, "name": f"Belt transfer, {width * 1000:.0f} mm" + (", sealed" if sealed else ""), "description": desc},
                  "kind": "belt-transfer", "size": width, "sealing": "double" if sealed else "open",
                  "physical": ({"length": r(b + 0.2, 2), "width": r(b + 0.2, 2), "height": 1.475, "mass": r(sum(x["mass"] for x in built))} if sealed else {"length": r(b, 2), "width": r(width + 0.6, 2), "height": 3.55, "mass": r(sum(x["mass"] for x in built))}), "built_of": built,
                  "basis": [{"of": ["built_of"], "tier": "invented", "review": True, "note": "Plate and liner by the chute's faces, from memory."}]}
    return key


def termination(v, area_mm2):
    key = f"connector.termination-{_volt(v)}-{area_mm2}mm2"
    if key in _recs:
        return key
    a = area_mm2 * 1e-6
    dc = 1.1 * math.sqrt(4 * a / math.pi)
    od = dc + 2 * (0.0014 if v <= 1000 else 0.0034 if v <= 12000 else 0.018) + 2 * (0.0025 if v <= 12000 else 0.004)
    R = max(0.3, 15 * od)
    fan = math.sqrt(4 * R * 0.04 - 0.04 ** 2)
    lugs = 3 * a * 0.12 * RHO[CU] * 1.5
    if v > 36000:          # (high voltage: three screened plug-in terminations into sockets in the unit's box, not open lugs)
        phase = 0.5
        fan = math.sqrt(4 * R * (phase - max(0.06, od + 0.017)) - (phase - max(0.06, od + 0.017)) ** 2)
        built = [_part("Plug-in terminations", "material.fused-silica", 3 * 30.0, "three screened plug-in terminations, 0.7 m into the box's sockets, their epoxy insulators 30 kg each; fused silica stands for filled epoxy (from memory)"),
                 _part("Stress cones", RUBBER, 3 * 6.0, "a silicone stress cone on each core (from memory)"),
                 _part("Connectors", CU, 3 * 3.0, "the plug's contact on each conductor"),
                 _part("Gland plate", AL, 1.5 * 0.5 * 0.01 * RHO[AL], "1.5 x 0.5 m, 10 mm, the three entries 0.5 m apart")]
        _recs[key] = {"identity": {"key": key, "name": f"Cable termination, {v / 1000:g} kV, {area_mm2} mm2, plug-in", "description": f"Three screened plug-in terminations for {area_mm2} mm2 cores at {v / 1000:g} kV, their phases 0.5 m apart on a 1.5 x 0.5 m gland plate, each 0.7 m into a socket of the unit's box (the box's sockets and its leads to the bushings are the unit's); before the plate the cores fan out from the tray's pitch to the phases' over {fan:.1f} m."},
                      "kind": "termination", "size": a, "rating": {"voltage": v, "current": r(a * J_VACUUM)},
                      "physical": {"length": 1.5, "width": 0.5, "height": r(0.7 + fan, 3), "mass": r(sum(x["mass"] for x in built))}, "built_of": built,
                      "basis": [{"of": ["built_of", "physical"], "tier": "invented", "review": True, "note": "Provisional: phase spacing, plug length and masses from memory of plug-in high-voltage terminations; dielectric design not checked."}]}
        return key
    cones = 3 * (0.3 + 5.0 * (v / 1e4) ** 1.2) if v > 1000 else 0.0
    built = [_part("Lugs", CU, lugs, "three crimped lugs"), _part("Gland plate", AL, 5.0, "the box's gland plate and glands")]
    if cones:
        built.append(_part("Stress cones", RUBBER, cones, "a stress cone on each core, sized with the voltage (from memory); silicone rubber"))
    _recs[key] = {"identity": {"key": key, "name": f"Cable termination, {v / 1000:g} kV, {area_mm2} mm2", "description": f"A three-phase cable's end into a terminal box: lugs, stress control and the gland, for {area_mm2} mm2 conductors at {v / 1000:g} kV."},
                  "kind": "termination", "size": a, "rating": {"voltage": v, "current": r(a * J_VACUUM)},
                  "physical": {"length": 0.4, "width": 0.4, "height": r(0.5 + fan, 3), "mass": r(sum(x["mass"] for x in built))}, "built_of": built,
                  "basis": [{"of": ["built_of"], "tier": "invented", "review": True, "note": "From memory."},
                            {"of": ["physical.height"], "tier": "derived", "note": f"Along the cable: 0.5 m of cones and lugs inside the box, and the fan-out from the tray's spacing to the box's (0.04 m aside each way) in two arcs at the cable's least bend radius ({R:.2f} m): {fan:.2f} m."}]}
    return key


# ---------------------------------------------------------------- runs

def _span(do, di, w):
    """m between supports, from deflection (2.5 mm at mid span, 1 g, full) and stress (a third of yield), 3 to 12 m."""
    i = math.pi / 64 * (do ** 4 - di ** 4)
    z = 2 * i / do
    by_defl = (0.0025 * 384 * 200e9 * i / (5 * w)) ** 0.25
    by_stress = math.sqrt(8 * z * 83e6 / w)
    return max(3.0, min(12.0, by_defl, by_stress))


PIPE_SUPPORT_MEASURED = {"run.pipe-dn150-pn25-mli": 29.19}     # kg a stand's head (its split saddle and base, the silica pad apart), measured on blender's kits (2026-10-16)


def pipe(dn, pn, item=A36, lagged=False, liquid=False, temp=373.0, sealing="single"):
    """A welded pipe of nominal bore dn, class pn; lagged in multi-layer insulation (25 mm), which in vacuum is the
    lagging that works; liquid: full of water when its supports are sized."""
    key = f"run.pipe-dn{_tag(dn)}-pn{pn}" + ("-ss" if item == SS else "") + ("-mli" if lagged else "") + ("-double" if sealing == "double" else "")
    if key in _recs:
        return key
    t = wall(dn, PN[pn], item, temp)
    steel = math.pi * (dn + t) * t * RHO[item]
    built = [_part("Pipe", item, steel, f"{dn * 1000:.0f} mm bore, {t * 1000:.1f} mm wall (Barlow at {PN[pn] / 1e5:.0f} bar, {STRESS[item] / 1e6:.0f} MPa, weld factor {E_WELD}" + (", 1.5 mm for corrosion" if item == A36 else "") + ")")]
    do = dn + 2 * t
    out = do
    if lagged:
        built.append(_part("Lagging", MLI, math.pi * (do + 0.025) * 0.025 * RHO[MLI], "25 mm of multi-layer insulation"))
        out = do + 0.05
    per_m = sum(b["mass"] for b in built)
    w = (per_m + (math.pi / 4 * dn ** 2 * 1000 if liquid else 0.0)) * G
    span = _span(do, dn, w)
    R = 1.5 * dn
    _recs[key] = {"identity": {"key": key, "name": f"Pipe DN {dn * 1000:.0f} PN {pn}" + (", stainless" if item == SS else "") + (", lagged" if lagged else "") + (", double-sealed joints" if sealing == "double" else ""),
                               "description": f"Welded pipe, {dn * 1000:.0f} mm bore, for up to {PN[pn] / 1e5:.0f} bar; welded between the units' flanges, so a run has a flange pair only at its ends." + (" Lagged in multi-layer insulation, 25 mm." if lagged else "")},
                  "kind": "pipe", "size": dn, "bore": dn, "outside": [r(out, 3), r(out, 3)], "rating": {"pressure": PN[pn], "temperature": temp},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "elbow": {"radius": r(R, 3), "mass": r(per_m * math.pi / 2 * R)},
                  "tee": {"mass": r(per_m * max(0.0, 1.5 * dn - out / 2))},
                  "cap": cap(dn, PN[pn], item, flange(dn, pn, item, sealing, temp)),
                  "support": {"spacing": r(span), "mass": PIPE_SUPPORT_MEASURED.get(key, r(10 + 60 * out)), "per_m_height": r(4 * (0.1 + 0.1 * out) * 0.005 * RHO[A36]), "item": A36},
                  "connector": flange(dn, pn, item, sealing, temp), "carries": "liquid" if liquid else "gas",
                  "basis": [{"of": ["per_m"], "tier": "derived", "note": "Wall from pressure; mass from material and dimensions."},
                            {"of": ["elbow", "tee"], "tier": "derived", "note": "A long-radius elbow (1.5 bores) is its arc of straight; a tee is its branch stub over the straight it sits in, from the main's outside to 1.5 bores off its axis (checked on blender's DN 200 kit)."},
                            {"of": ["support"], "tier": "derived", "review": True, "note": f"Spacing from the span it bears full at 1 g (2.5 mm sag, a third of yield), 3 to 12 m: {span:.1f} m. The stand's head and its leg by the metre from memory."}]}
    return key


LINED_SUPPORT_MEASURED = {1.2: (84.85, 42.42)}     # kg a stand's head and kg a metre of its legs, measured on blender's DN 1200 kit (2026-10-16)


def lined_duct(dn, pn, lining=0.2, temp=1173.0, sealing="single"):
    key = f"run.lined-duct-dn{_tag(dn)}-pn{pn}" + ("-double" if sealing == "double" else "")
    if key in _recs:
        return key
    t = wall(dn, PN[pn], A36)
    shell = math.pi * (dn + t) * t * RHO[A36]
    ref = math.pi * (dn - lining) * lining * RHO[REF]
    built = [_part("Shell", A36, shell, f"{dn * 1000:.0f} mm, {t * 1000:.1f} mm (Barlow at {PN[pn] / 1e5:.0f} bar, the shell kept under 200 deg C by its lining)"),
             _part("Lining", REF, ref, f"{lining * 1000:.0f} mm of refractory, {dn - 2 * lining:.2f} m clear bore"),
             _part("Anchors", SS, 0.02 * ref / RHO[REF] * RHO[SS] * 0.1, "stainless anchors holding the lining, about 0.2% of its volume")]
    per_m = sum(b["mass"] for b in built)
    do = dn + 2 * t
    span = _span(do, dn, per_m * G)
    R = 1.0 * dn
    _recs[key] = {"identity": {"key": key, "name": f"Lined duct DN {dn * 1000:.0f} PN {pn}", "description": f"A steel shell {dn * 1000:.0f} mm nominal bore lined with {lining * 1000:.0f} mm of refractory, for hot gas at up to {temp - 273.15:.0f} deg C and {PN[pn] / 1e5:.0f} bar: {dn - 2 * lining:.2f} m clear. Its bends are mitred, one bore in radius."},
                  "kind": "lined-duct", "size": dn, "bore": r(dn - 2 * lining, 3), "outside": [r(do, 3), r(do, 3)], "rating": {"pressure": PN[pn], "temperature": temp},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "elbow": {"radius": r(R, 3), "mass": r(per_m * math.pi / 2 * R * 1.1)},
                  "tee": {"mass": r(per_m * max(0.0, 1.5 * (dn - 2 * lining) - do / 2))},
                  "support": {"spacing": r(span), "mass": LINED_SUPPORT_MEASURED.get(dn, (r(30 + 80 * do),))[0], "per_m_height": LINED_SUPPORT_MEASURED.get(dn, (0, r(4 * (0.1 + 0.1 * do) * 0.006 * RHO[A36])))[1], "item": A36},
                  "cap": (lambda c: dict(c, mass=r(c["mass"] + math.pi / 4 * (dn - 2 * lining) ** 2 * lining * RHO[REF])))(cap(dn, PN[pn], A36, lined_flange(dn, pn, sealing=sealing))),
                  "connector": lined_flange(dn, pn, sealing=sealing), "carries": "gas",
                  "basis": [{"of": ["per_m"], "tier": "derived", "note": "Shell from pressure, lining from its thickness; mass from material and dimensions."},
                            {"of": ["support", "elbow"], "tier": "derived", "review": True, "note": f"Spacing from the span it bears at 1 g, {span:.1f} m; mitred bends a tenth heavier than their arc. Stands from memory."}]}
    return key


BELT_HEIGHT_MEASURED = {0.65: 0.91}     # m: an open belt's full height about its axis, the rigid return roll under it included (0.454 below, 0.426 above), measured on blender's 650 kit (2026-10-16)


def jacketed_duct(dn, heat, temp=1873.0):
    """A water-jacketed steel duct for hot gas at about 1 bar: an inner shell, a 100 mm water gap and an outer shell.
    Its jacket's water comes by pipes ending on it (`jacket:` ends) and carries `heat` away."""
    key = f"run.jacketed-duct-dn{_tag(dn)}"
    if key in _recs:
        _recs[key]["jacket"]["heat"] = max(_recs[key]["jacket"]["heat"], r(heat))
        return key
    t, gap, to = 0.010, 0.10, 0.006
    inner = math.pi * (dn + t) * t * RHO[A36]
    outer = math.pi * (dn + 2 * t + 2 * gap + to) * to * RHO[A36]
    water = math.pi / 4 * ((dn + 2 * t + 2 * gap) ** 2 - (dn + 2 * t) ** 2) * 1000.0
    built = [_part("Inner shell", A36, inner, f"{dn * 1000:.0f} mm bore, {t * 1000:.0f} mm plate, the hot face"),
             _part("Outer shell", A36, outer, f"{to * 1000:.0f} mm, round a {gap * 1000:.0f} mm water gap"),
             _part("Jacket water", "good.water", water, "in the gap, flowing")]
    per_m = sum(b["mass"] for b in built)
    do = dn + 2 * t + 2 * gap + 2 * to
    span = _span(do, dn, per_m * G)
    R = 1.0 * dn
    stub = dn_for(math.sqrt(4 * heat / (4186 * 10) / 1000 / 2.5 / math.pi))     # (the jacket's water pipes' bore, at 2.5 m/s)
    _recs[key] = {"identity": {"key": key, "name": f"Water-jacketed duct DN {dn * 1000:.0f}", "description": f"A {dn * 1000:.0f} mm duct for a furnace's hot gas at about 1 bar, its steel kept cool by water in a {gap * 1000:.0f} mm jacket round it; the jacket's water comes and goes by pipes ending on it. Its bends are mitred, one bore in radius."},
                  "kind": "jacketed-duct", "size": dn, "bore": dn, "outside": [r(do, 3), r(do, 3)], "rating": {"pressure": 1.0e5, "temperature": temp},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "jacket": {"heat": r(heat), "rise": 10.0},
                  "jacket_stub": {"bore": stub, "mass": r(math.pi * (stub + 0.01) * 0.01 * 0.4 * RHO[A36] + math.pi / 4 * ((stub + 0.2) ** 2 - stub ** 2) * 0.012 * RHO[A36])},
                  "jacket_end": {"mass": r(math.pi / 4 * ((dn + 2 * t + 2 * gap) ** 2 - (dn + 2 * t) ** 2) * 0.012 * RHO[A36])},
                  "elbow": {"radius": r(R, 3), "mass": r(per_m * math.pi / 2 * R * 1.0132)},
                  "support": {"spacing": r(span), "mass": r(30 + 80 * do), "per_m_height": r(4 * (0.1 + 0.1 * do) * 0.006 * RHO[A36]), "item": A36},
                  "cap": cap(dn, 1.0e5, A36, flange(dn, 1, temp=523.0)),
                  "connector": flange(dn, 1, temp=523.0), "carries": "gas",
                  "basis": [{"of": ["per_m"], "tier": "derived", "review": True, "note": "Shells and water from their dimensions; the plate and gap from memory of furnace off-gas ducts."},
                            {"of": ["support", "elbow", "jacket", "jacket_stub", "jacket_end"], "tier": "derived", "review": True, "note": f"Spacing from the span it bears full at 1 g, {span:.1f} m; a bend of four mitres 1.3% longer along its centreline than the arc (3.183 m for the DN 2000's, measured on blender's kit); the jacket's water 10 K warmer out than in. A jacket stub is a 10 mm nozzle 0.4 m long through the outer shell, its bore the water pipe's, on a 12 mm pad 0.1 m wider; a jacket end the 12 mm annular plate closing the water gap where the duct meets a flange or hood. Stands from memory."}]}
    return key


def belt(width, rate, sealed=False):
    """An open troughed belt; sealed: in a gallery tube 3 m across that holds a hall's gas round it (green balls or
    ore taken through vacuum from one sealed space to another)."""
    key = f"run.belt-{_tag(width)}" + ("-sealed" if sealed else "")
    if key in _recs:            # (rated for the most any layout asks of it)
        _recs[key]["rating"]["rate"] = max(_recs[key]["rating"]["rate"], r(rate))
        return key
    built = [_part("Belt", RUBBER, 2 * width * 0.018 * RHO[RUBBER], "both runs, 18 mm; a vacuum-rated belt, silicone rubber standing for it"),
             _part("Idlers", A36, 39.0 / 1.2 + 20.0 / 3.0, "a three-roll troughing set every 1.2 m (39 kg) and a return roll every 3 m (20 kg), sealed bearings with vacuum grease; from memory"),
             _part("Stringers", A36, 2 * 18.0, "two 150 x 75 mm channels")]
    if sealed:
        built += [_part("Gallery tube", A36, math.pi * 3.0 * 0.004 * RHO[A36], "a 3 m tube of 4 mm plate round the belt, holding about 20 kPa of nitrogen"),
                  _part("Gallery rings", A36, 66.5 / 3.0, "an inside stiffening ring every 3 m, 90 x 6 mm, with the stringers' seats, 66.5 kg (a repeated piece; measured), its share a metre"),
                  _part("Insulation", MLI, math.pi * 3.0 * 0.025 * RHO[MLI], "25 mm of multilayer foil over the tube, against the lunar day and night")]
    per_m = sum(b["mass"] for b in built)
    BOX = 4069.5          # kg: a gallery's sealed head or tail box, 6 mm plate on 150 x 75 x 6 mm ribs at 0.5 m, its throat, posts and feet; measured on blender's 0.8 m candidate (the tube's the same for every width)
    out = [3.05, 3.05] if sealed else [r(width + 0.6, 2), BELT_HEIGHT_MEASURED.get(width, r(max(0.8, width), 2))]     # (a belt's height by its width: 1.2 m for the 1.2 m belt; a gallery's tube with its foil)
    _recs[key] = {"identity": {"key": key, "name": f"Belt conveyor, {width * 1000:.0f} mm" + (", in a sealed gallery" if sealed else ""), "description": f"An open troughed belt {width * 1000:.0f} mm wide on idlers and stringers, carried on trestles; it runs straight (a turn is a transfer to another belt). Its drive is at its head." + (" It runs in a sealed gallery tube 3 m across, its axis the tube's, at the hall's gas pressure; where the tube meets a hall's nozzle the joint is the nozzle's flange." if sealed else "")},
                  "kind": "belt", "size": width, "outside": out, "rating": {"rate": r(rate)},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "support": {"spacing": 9.0, "mass": 416.3, "per_m_height": 75.0, "item": A36} if sealed else {"spacing": 18.0, "mass": 300.0, "per_m_height": 75.0, "item": A36},
                  "repeat": [{"piece": "idler", "pitch": 1.2}, {"piece": "return_roll", "pitch": 3.0}] + ([{"piece": "ring", "pitch": 3.0}] if sealed else []),
                  "terminals": {"head": {"mass": 4500.0 + (BOX if sealed else 0.0), "transition": 1.0, "note": "a 0.8 m lagged drive pulley, its bearings and frame, a gearbox and a motor of about 400 kW (from memory)" + ("; in a sealed box the tube's size, 3 m long, 6 mm plate, the motor along x inside it, an outlet in its floor at the origin" if sealed else "")},
                                "tail": {"mass": 2230.0 + (BOX if sealed else 0.0), "transition": 1.0, "note": "a 0.63 m tail pulley on a screw take-up, its bearings and frame, and the loading skirt (rubber, 3 m a side) where a chute drops onto it (from memory)" + ("; in a sealed box the tube's size, 3 m long, 6 mm plate, an inlet in its roof at the origin" if sealed else "")}},
                  "connector": belt_transfer(width, sealed), "carries": "bulk",
                  "basis": [{"of": ["per_m", "support"], "tier": "invented", "review": True, "note": "Belt, idlers and stringers from memory of 1.2 m belts; trestles every 18 m, a 300 kg cross beam and two 200 mm box legs (75 kg/m)."}]}
    return key


def drag(width, rate, temp=923.0):
    key = f"run.drag-conveyor-{_tag(width)}"
    if key in _recs:
        _recs[key]["rating"]["rate"] = max(_recs[key]["rating"]["rate"], r(rate))
        return key
    h = 0.75 * width
    built = [_part("Casing", SS, 2 * (width + h) * 0.006 * RHO[SS], f"a closed box {width:.2f} x {h:.2f} m, 6 mm stainless"),
             _part("Insulation", SILICA, 2 * (width + h + 0.2) * 0.1 * RHO[SILICA], "100 mm of microporous silica round it; silica aerogel stands for it"),
             _part("Chain and flights", SS, 4 * 25.0 + 40.0 / 0.6, "two strands of forged chain, both runs (25 kg/m each), a flight every 0.6 m (40 kg)")]
    per_m = sum(b["mass"] for b in built)
    _recs[key] = {"identity": {"key": key, "name": f"Drag chain conveyor, {width * 1000:.0f} mm, hot", "description": f"Flights on two chains drawn through a closed insulated casing {width * 1000:.0f} mm wide, for hot bulk up to {temp - 273.15:.0f} deg C; it bends gently in the vertical."},
                  "kind": "drag-conveyor", "size": width, "outside": [r(width + 0.2 + 0.012, 3), r(h + 0.2 + 0.012, 3)], "rating": {"rate": r(rate), "temperature": temp},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "elbow": {"radius": 6.0, "mass": r(per_m * math.pi / 2 * 6.0)},
                  "support": {"spacing": 6.0, "mass": 80.0, "per_m_height": 25.0, "item": A36},
                  "terminals": {"head": {"mass": 3000.0, "transition": 1.2, "note": "the drive sprocket, its shaft seals, a gearbox and a motor of about 150 kW, and the outlet nozzle under the casing (from memory)"},
                                "tail": {"mass": 1800.0, "transition": 1.2, "note": "the tail sprocket on a take-up, its shaft seals, and the inlet nozzle on top of the casing (from memory)"}},
                  "connector": chute(width), "carries": "bulk",
                  "basis": [{"of": ["per_m", "support", "elbow"], "tier": "invented", "review": True, "note": "Casing and insulation from their dimensions; chain and flights, bends and stands from memory."}]}
    return key


def cable(v, area_mm2):
    key = f"run.cable-{_volt(v)}-{area_mm2}mm2"
    if key in _recs:
        return key
    a = area_mm2 * 1e-6
    dc = 1.1 * math.sqrt(4 * a / math.pi)
    ti = 0.0014 if v <= 1000 else 0.0034 if v <= 12000 else 0.018
    ts = 0.0025 if v <= 12000 else 0.004
    ins = 3 * (math.pi * (dc + ti) * ti + math.pi * (dc + 2 * ti + ts) * ts) * RHO[PE]
    built = [_part("Conductors", CU, 3 * a * RHO[CU], f"three cores of {area_mm2} mm2"),
             _part("Insulation and sheaths", PE, ins, f"{ti * 1000:.1f} mm of insulation and a {ts * 1000:.1f} mm sheath on each core; UHMW polyethylene stands for XLPE")]
    if v > 1000:
        built.append(_part("Screens", CU, 3 * 35e-6 * RHO[CU], "a 35 mm2 copper screen on each core"))
    built.append(_part("Tray", AL, 4.0, "its share of a ladder tray, with 4 mm bridges over the cores at each rung that hold them on risers"))
    per_m = sum(b["mass"] for b in built)
    od = dc + 2 * ti + 2 * ts
    _recs[key] = {"identity": {"key": key, "name": f"Cable, {v / 1000:g} kV, {area_mm2} mm2", "description": f"Three single-core cables of {area_mm2} mm2 at {v / 1000:g} kV on a ladder tray: {a * J_VACUUM:,.0f} A a circuit on airless ground, where only radiation cools them."},
                  "kind": "cable", "size": a, "outside": [r(max(0.3, 3 * od + 0.1), 3), r(0.1 + od + 0.004, 3)], "rating": {"voltage": v, "current": r(a * J_VACUUM)},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "pitch": r(max(0.06, od + 0.017), 3),
                  "elbow": {"radius": r(max(0.3, 15 * od) + max(0.06, od + 0.017), 3), "mass": r(per_m * math.pi / 2 * (max(0.3, 15 * od) + max(0.06, od + 0.017)))},
                  "riser": {"radius": r(max(0.3, 15 * od), 3), "mass": r(per_m * math.pi / 2 * max(0.3, 15 * od))},
                  "tee": {"mass": 25.0},
                  "support": {"spacing": 3.0, "mass": 8.0, "per_m_height": 6.0, "item": AL},
                  "repeat": [{"piece": "rung", "pitch": 0.3}],
                  "connector": termination(v, area_mm2), "carries": "power",
                  "basis": [{"of": ["per_m"], "tier": "derived", "review": True, "note": f"Copper from its area; insulation by the voltage's usual thickness (from memory); the rating at {J_VACUUM / 1e6:g} A/mm2, radiation only."},
                            {"of": ["support", "tee", "elbow", "riser", "pitch"], "tier": "invented", "review": True, "note": "Tray posts every 3 m; a tee is a joint box; the cores' least bend radius 15 cable diameters (from memory): a riser bends all three at it, an elbow in plan has its axis one core's pitch further out so the inner core keeps it."}]}
    return key


J_BUS = 4.0e6                           # A/m2 in a water-cooled copper bus (from memory: 3 to 6)
RHO_E_CU = 2.0e-8                       # ohm m, copper at about 60 deg C
TUBE = (0.060, 0.010)                   # m: the bus's hollow conductor, outside diameter and wall (from memory)


def _bus(amps):
    """A direct current bus's pole: hollow copper tubes in rows of four, 20 mm apart. (tubes, rows, width, height)."""
    od, t = TUBE
    a = math.pi / 4 * (od ** 2 - (od - 2 * t) ** 2)
    n = math.ceil(amps / J_BUS / a)
    rows = math.ceil(n / 4)
    return n, a, rows, 4 * od + 3 * 0.02, rows * od + (rows - 1) * 0.02


BUS_PLATES_MEASURED = {"connector.bus-joint-ac-27ka": 106.16}     # kg of a joint's copper plates, holes cut, measured on blender's family generator (2026-10-16)


def bus_joint(v, amps, phases=2):
    """phases 2: a direct current bus's two poles; 3: an alternating current bus's three phases."""
    ac = phases == 3
    key = f"connector.bus-joint-{'ac' if ac else 'dc'}-{math.ceil(amps / 1000):d}ka"
    if key in _recs:
        return key
    n, a, rows, w, h = _bus(amps)
    ph = h + 0.11                                   # the plate's height: two rows of M20 at 50 mm from the tubes' centres, 35 mm in from its edges
    pw = w + 0.04                                   # and wide enough for the outer bolt columns' 30 mm edge distance
    plate = pw * ph * 0.03 * RHO[CU]
    pole = "phase" if ac else "pole"
    plates = BUS_PLATES_MEASURED.get(key, phases * 2 * plate)
    built = [_part("Joint plates", CU, plates, f"a pair of 30 mm copper plates a {pole}, {pw * 1000:.0f} x {ph * 1000:.0f} mm, the tubes brazed into one, bolted to the unit's terminal pad" + ("; measured on blender's model with its tube bores and bolt holes cut, " + f"{phases * 2 * plate:,.1f} kg as solid plates" if key in BUS_PLATES_MEASURED else "")),
             _part("Bolts", BOLT, phases * 16 * 0.4, f"sixteen M20 a {pole} in two rows of eight, above and below the tubes, with spring washers"),
             _part("Braze lands", CU, phases * n * 0.68, "a copper land round each tube where it is brazed into the plate; measured on blender's model (9.5 kg for 14 tubes, 2026-10-16)"),
             _part("Water couplings and hoses", RUBBER, phases * n * 0.5, "an insulating hose from each tube's end to the unit's cooling manifold behind its pad")]
    what = f"{amps / 1000:.1f} kA three-phase alternating current bus" if ac else f"{amps / 1000:.1f} kA direct current bus"
    lay_ = "the three phases side by side across y" if ac else "the poles side by side across y, + at -y"
    _recs[key] = {"identity": {"key": key, "name": f"Bus joint, {'AC' if ac else 'DC'}, {math.ceil(amps / 1000):d} kA", "description": f"Where a {what} meets a unit's terminals: each {pole}'s tubes brazed into copper plates, bolted to the unit's pads, the {pole}s' centres {w + 0.3:g} m apart ({0.3:g} m clear between them), and the tubes' water handed by insulating hoses to the unit's cooling manifold. At the plane: {lay_}, each {pole} {w:.2f} m wide and {h:.2f} m tall about the axis."},
                  "kind": "bus-joint", "size": r(n * a, 5), "rating": {"voltage": v, "current": r(amps)},
                  "physical": {"length": 0.3, "width": r(phases * w + (phases - 1) * 0.3 + 0.1, 3), "height": r(ph + 0.01, 3), "mass": r(sum(x["mass"] for x in built))}, "built_of": built,
                  "basis": [{"of": ["built_of", "physical"], "tier": "invented", "review": True, "note": "Plates, bolting and hoses from memory of electrolysis and furnace bus joints; the bolts' edge distance 35 mm (1.6 bolt diameters)."}]}
    if ac and phases * n > 14:
        _recs[key]["triangle_budget"] = 22000       # (three phases of brazed tubes and their bolting; blender, 2026-10-16)
    return key


def busbar(v, amps, phases=2):
    """Water-cooled hollow copper on post insulators. phases 2: the two poles from a rectifier to its cells; 3: the
    three phases of an arc furnace's secondary, from its transformer to its flexible cables. In vacuum a bare bus can
    only radiate, and tens of kA would need plates metres wide; water takes its heat to the unit's loop."""
    ac = phases == 3
    key = f"run.busbar-{'ac' if ac else 'dc'}-{math.ceil(amps / 1000):d}ka"
    if key in _recs:
        return key
    od, t = TUBE
    n, a, rows, w, h = _bus(amps)
    loss = phases * amps ** 2 * RHO_E_CU / (n * a)
    water = phases * n * math.pi / 4 * (od - 2 * t) ** 2 * 1000
    pole = "phase" if ac else "pole"
    built = [_part("Conductors", CU, phases * n * a * RHO[CU], f"{'three phases' if ac else 'two poles'} of {n} tubes {od * 1000:.0f} x {t * 1000:.0f} mm, {n * a * 1e6:,.0f} mm2 a {pole}, in {rows} rows of four"),
             _part("Cooling water", "good.water", water, "inside the tubes, half of them flow and half return, turned at the far joint"),
             _part("Clamps and spacers", A36, phases * rows * 6.0, "a clamp a row every metre, from memory")]
    per_m = sum(b["mass"] for b in built)
    R = R_RISE = 0.6
    if ac:          # (the least tube's bend 0.6 m: the axis's radius is that plus the outermost tube's offset; blender, 2026-10-16)
        R = 0.6 + (phases - 1) / 2 * (w + 0.3) + (w - od) / 2
        R_RISE = 0.6 + (h - od) / 2
    what = (f"A three-phase alternating current bus, its phases' centres {w + 0.3:g} m apart (0.3 m clear), each {n} hollow copper tubes {od * 1000:.0f} mm across, water-cooled, on post insulators: {amps / 1000:.1f} kA a phase at {v:g} V. The tubes' {t * 1000:.0f} mm wall is about the skin depth of copper at 50 Hz, so the current fills it" if ac else
            f"A direct current bus of two poles, their centres {w + 0.3:g} m apart (0.3 m clear), each {n} hollow copper tubes {od * 1000:.0f} mm across, water-cooled, on post insulators: {amps / 1000:.1f} kA at {v:g} V")
    _recs[key] = {"identity": {"key": key, "name": f"Busbar, {'AC' if ac else 'DC'}, {math.ceil(amps / 1000):d} kA", "description": f"{what}; it loses {loss:,.0f} W a metre into its water, which the unit at its source carries away."},
                  "kind": "busbar", "size": r(n * a, 5), "outside": [r(phases * w + (phases - 1) * 0.3 + 0.1, 3), r(h + 0.1, 3)], "rating": {"voltage": v, "current": r(amps)},
                  "per_m": {"mass": r(per_m), "power": r(loss), "built_of": built},
                  "elbow": {"radius": r(R, 3), "mass": r(per_m * math.pi / 2 * R)},
                  "riser": {"radius": r(R_RISE, 3), "mass": r(per_m * math.pi / 2 * R_RISE)},
                  "support": {"spacing": 3.0, "mass": 60.0 if not ac else 75.0, "per_m_height": 25.0, "item": A36},
                  "connector": bus_joint(v, amps, phases), "carries": "power",
                  "basis": [{"of": ["per_m"], "tier": "derived", "review": True, "note": f"Copper at {J_BUS / 1e6:g} A/mm2 water-cooled (from memory); its loss by its resistance at 60 deg C" + (" (the AC resistance taken as the DC's: the wall is about one skin depth)." if ac else ".")},
                            {"of": ["elbow", "riser", "support"], "tier": "invented", "review": True, "note": ("Each tube bent at 0.6 m or more, so the bus's axis bends at that plus its outermost tube's offset; a steel post with three insulators, one a phase, every 3 m (from memory)." if ac else "Bent tubes at 0.6 m; a steel post with two insulators every 3 m (from memory).")}]}
    if ac and phases * n > 14:
        _recs[key]["triangle_budget"] = 28000       # (three phases of tubes: 12-sided tubes and their bends need it; blender, 2026-10-16)
    return key


def area_for(amps):
    return next(a for a in AREAS if a * 1e-6 * J_VACUUM >= amps)


# ---------------------------------------------------------------- laying a run

def cap(dn, p, item, conn_key):
    """A blind end: a flat plate welded into the pipe's end (runs are welded; no flange, nothing to leak), clamped at its
    edge, 0.75 p r2 / t2 within the allowable stress, rounded up to 2 mm."""
    t = math.ceil(dn / 2 * math.sqrt(0.75 * p / STRESS[item]) * 500) / 500
    return {"thickness": t, "mass": r(math.pi / 4 * dn ** 2 * t * RHO[item])}


def corners(pts):
    """The corners a path turns at: (index, angle in degrees)."""
    out = []
    for i in range(1, len(pts) - 1):
        a = [q - p for p, q in zip(pts[i - 1], pts[i])]
        b = [q - p for p, q in zip(pts[i], pts[i + 1])]
        la, lb = math.hypot(*a), math.hypot(*b)
        c = max(-1.0, min(1.0, sum(x * y for x, y in zip(a, b)) / (la * lb)))
        ang = math.degrees(math.acos(c))
        if ang > 1.0:
            out.append((i, ang))
    return out


def lay(run_key, pts, count=1, tees=0, taps=(), port_ends=0, caps=0):
    """A run laid along pts: (mass, fittings, length). tees: its ends that branch off another run; taps: the
    connectors of the ports taken off it; port_ends: its ends at a port (a connector each); caps: blind ends."""
    rec = _recs[run_key]
    L = sum(math.dist(p, q) for p, q in zip(pts, pts[1:]))
    cs = corners(pts)
    n_elb = len(cs)
    el = rec.get("elbow")
    m_elb = 0.0
    if el:          # a bend of angle a weighs its share of the 90 degree elbow, less the straight it replaces (2R tan a/2)
        m_elb = sum(el["mass"] * ang / 90.0 - rec["per_m"]["mass"] * 2 * el["radius"] * math.tan(math.radians(ang) / 2) for _, ang in cs)
    sup = rec.get("support")
    n_sup, m_sup = 0, 0.0
    if sup:
        for p, q in zip(pts, pts[1:]):
            leg = math.dist(p, q)
            if leg < 1e-9:
                continue
            k = 1 if abs(q[2] - p[2]) > 0.5 * leg else max(1, math.ceil(leg / sup["spacing"] - 1e-9))
            z = max(p[2], q[2]) if k == 1 and abs(q[2] - p[2]) > 0.5 * leg else (p[2] + q[2]) / 2
            n_sup += k
            m_sup += k * (sup["mass"] + sup["per_m_height"] * max(0.0, z))
    conn = _recs[rec["connector"]]["physical"]["mass"]
    m = count * (rec["per_m"]["mass"] * L + m_elb + port_ends * conn + caps * rec.get("cap", {}).get("mass", conn / 2))
    m += (tees + len(taps)) * rec.get("tee", {}).get("mass", 0.0) * count
    m += (port_ends + caps) * rec.get("jacket_end", {}).get("mass", 0.0) * count
    m += sum(_recs[c]["physical"]["mass"] for c in taps) * count
    m += m_sup + count * sum(t["mass"] for t in rec.get("terminals", {}).values())
    fit = {"elbows": n_elb, "tees": tees + len(taps), "supports": n_sup, "connectors": port_ends + caps + len(taps)}
    return m, fit, L


# ---------------------------------------------------------------- interfaces (SFO 26)

IFACE = {    # connector kind: (domain, halves, make, sealing by default)
    "flange": ("fluid", "androgynous", "bolted", "air"), "lined-flange": ("fluid", "androgynous", "bolted", "air"),
    "chute": ("bulk", "androgynous", "bolted", "air"), "belt-transfer": ("bulk", "male-female", "open", "open"),
    "termination": ("power", "male-female", "bolted", "air"), "bus-joint": ("power", "androgynous", "bolted", "open"),
    "hood": ("fluid", "male-female", "open", "open"), "clamp-coupling": ("fluid", "androgynous", "automatic", "double"),
}
STANDARD = {"flange": "EN 1092-1 bores and pressure classes (its faces and bolting the registry's)", "termination": "IEC 60038 voltages"}


def interface_key(conn_key):
    """The interface a connector is built to: its mating face, whatever its maker's metal (a stainless flange pair and a
    carbon steel one of the same bore, class and sealing mate)."""
    slug = conn_key.split(".", 1)[1]
    if slug.startswith("flange-"):
        slug = slug.replace("-ss", "")
    return "interface." + slug


def half(conn_key, flow):
    """Which half of its interface a port built to this connector carries."""
    kind = _recs[conn_key]["kind"]
    if IFACE[kind][1] == "androgynous":
        return "androgynous"
    if kind == "belt-transfer":
        return "female" if flow == "in" else "male"
    if kind == "termination":
        return "female"            # (the unit's box and terminals; the cable brings its lugs)
    return "male"                  # (a hood serves the outlet's mouth, which is the port)


def interfaces():
    """An interface record for every connector's mating face; each connector names its own."""
    out = {}
    for key, d in sorted(_recs.items()):
        if not key.startswith("connector."):
            continue
        kind = d["kind"]
        ik = interface_key(key)
        dom, halves, make, seal = IFACE[kind]
        d["interface"] = ik
        d["provides"] = "both"
        rt = d.get("rating") or {}
        rec = out.get(ik) or {"identity": {"key": ik, "name": d["identity"]["name"].replace(", stainless", ""),
                                           "description": f"The mating face of the {d['identity']['name'].replace(', stainless', '')} joint: any maker's half built to it mates with any other's, whatever its metal."},
                              "domain": dom, "kind": kind, "envelope": {}, "media": [], "sealing": "double" if d.get("sealing") == "double" else seal,
                              "halves": halves, "make": make, "face": {k: d["physical"][k] for k in ("length", "width", "height")}}
        if kind == "termination":
            rec["area"] = d["size"]
        elif d.get("size"):
            rec["size"] = d["size"]
        for k in ("pressure", "temperature", "voltage", "current"):
            if rt.get(k) is not None:
                rec["envelope"][k] = max(rec["envelope"].get(k, 0), rt[k])
        med = {"fluid": ["gas", "liquid"], "bulk": ["bulk"], "power": ["power"]}[dom]
        rec["media"] = sorted(set(rec["media"]) | set(med))
        if kind in STANDARD:
            rec["standard"] = STANDARD[kind]
        rec["basis"] = [{"of": ["envelope", "face"], "tier": "derived", "note": "The most the connection products built to it are rated for, and their joint's outside (SFO 25's connectors)."}]
        out[ik] = rec
    return out


def records():
    """Every connector, run and interface asked for: (folder, slug, record)."""
    ifs = interfaces()
    for key, d in sorted(_recs.items()):
        kind, slug = key.split(".", 1)
        yield {"connector": "connectors", "run": "runs"}[kind], slug, d
    for key, d in sorted(ifs.items()):
        yield "interfaces", key.split(".", 1)[1], d
