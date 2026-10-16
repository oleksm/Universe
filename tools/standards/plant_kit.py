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
PN = {10: 1.0e6, 16: 1.6e6, 25: 2.5e6}
AREAS = [16, 25, 35, 50, 70, 95, 120, 150, 185, 240, 300, 400, 500, 630]      # mm2, cable conductors
BOLTS = [0.016, 0.020, 0.024, 0.027, 0.030, 0.036, 0.042, 0.048, 0.056, 0.064]   # m, bolt diameters
A36, SS, BOLT, CU, AL = "material.structural-steel-a36", "material.stainless-steel-304", "material.high-strength-steel-4340", "material.copper", "material.aluminium-alloy-6061"
REF, MLI, PE, RUBBER, SILICA = "good.refractory-brick", "material.multi-layer-insulation", "material.polyethylene-uhmw", "material.silicone-rubber", "material.silica-aerogel"
RHO = {A36: 7850, SS: 7900, BOLT: 7850, CU: 8960, AL: 2700, REF: 3000, MLI: 50, PE: 940, RUBBER: 1075, SILICA: 250}
STRESS = {A36: 133e6, SS: 115e6}        # Pa allowable (B31.3, A36 and 304 to about 340 deg C; from memory)
E_WELD = 0.85
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
    built = [_part("Flanges", item, ring, f"two weld-neck rings {D:.3f} m across, {t * 1000:.0f} mm thick, their necks tapering over {t * 1000:.0f} mm from {4 * wp * 1000:.0f} mm to the pipe's wall; along the pipe the joint is {(2 * t + 0.006 + max(4 * db, 2 * t)) * 1000:.0f} mm with the necks and the bolts' and nuts' ends"),
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
    base = _recs[flange(dn, pn, A36, sealing, temp)]
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
    bolts = n * 0.6
    built = [_part("Flanges", item, ring, f"two plate rings {D:.2f} m across, 20 mm"), _part("Bolts", BOLT, bolts, f"{n} of M16"),
             _part("Seal", RUBBER, math.pi * (dn + 0.05) * 0.03 * 0.01 * RHO[RUBBER], "a dust seal; silicone rubber, which takes the heat")]
    _recs[key] = {"identity": {"key": key, "name": f"Chute flange DN {dn * 1000:.0f}", "description": f"A sealed bolted flange where bulk falls through a {dn * 1000:.0f} mm chute into a closed conveyor."},
                  "kind": "chute", "size": dn, "rating": {"pressure": 0.0, "temperature": 973.0}, "sealing": "single",
                  "physical": {"length": r(D, 3), "width": r(D, 3), "height": 0.05, "mass": r(sum(b["mass"] for b in built))}, "built_of": built,
                  "basis": [{"of": ["built_of"], "tier": "invented", "review": True, "note": "Plate rings and bolts at a 150 mm pitch, from memory."}]}
    return key


def belt_transfer(width):
    key = f"connector.belt-transfer-{_tag(width)}"
    if key in _recs:
        return key
    b = width + 0.3
    built = [_part("Hood and drop chute", A36, 4 * b * 3.0 * 0.01 * RHO[A36], f"a hood over the belt's head and a chute {b:.1f} m square, 3 m down, 10 mm plate"),
             _part("Wear liner", PE, 4 * b * 3.0 * 0.01 * RHO[PE], "10 mm UHMW polyethylene"), _part("Skirt", RUBBER, 2 * 3.0 * 0.3 * 0.015 * RHO[RUBBER], "the receiving side's skirt, 3 m a side")]
    _recs[key] = {"identity": {"key": key, "name": f"Belt transfer, {width * 1000:.0f} mm", "description": f"Where a {width * 1000:.0f} mm belt's head drops into a hopper or onto another belt: the hood, the drop chute (3 m) and the skirt. Its port is where the head pulley stands."},
                  "kind": "belt-transfer", "size": width, "sealing": "open",
                  "physical": {"length": r(b, 2), "width": r(b, 2), "height": 3.0, "mass": r(sum(x["mass"] for x in built))}, "built_of": built,
                  "basis": [{"of": ["built_of"], "tier": "invented", "review": True, "note": "Plate and liner by the chute's faces, from memory."}]}
    return key


def termination(v, area_mm2):
    key = f"connector.termination-{_volt(v)}-{area_mm2}mm2"
    if key in _recs:
        return key
    a = area_mm2 * 1e-6
    lugs = 3 * a * 0.12 * RHO[CU] * 1.5
    cones = 3 * (0.3 + 5.0 * (v / 1e4) ** 1.2) if v > 1000 else 0.0
    built = [_part("Lugs", CU, lugs, "three crimped lugs"), _part("Gland plate", AL, 5.0, "the box's gland plate and glands")]
    if cones:
        built.append(_part("Stress cones", RUBBER, cones, "a stress cone on each core, sized with the voltage (from memory); silicone rubber"))
    _recs[key] = {"identity": {"key": key, "name": f"Cable termination, {v / 1000:g} kV, {area_mm2} mm2", "description": f"A three-phase cable's end into a terminal box: lugs, stress control and the gland, for {area_mm2} mm2 conductors at {v / 1000:g} kV."},
                  "kind": "termination", "size": a, "rating": {"voltage": v, "current": r(a * J_VACUUM)},
                  "physical": {"length": 0.4, "width": 0.4, "height": 0.6, "mass": r(sum(x["mass"] for x in built))}, "built_of": built,
                  "basis": [{"of": ["built_of"], "tier": "invented", "review": True, "note": "From memory."}]}
    return key


# ---------------------------------------------------------------- runs

def _span(do, di, w):
    """m between supports, from deflection (2.5 mm at mid span, 1 g, full) and stress (a third of yield), 3 to 12 m."""
    i = math.pi / 64 * (do ** 4 - di ** 4)
    z = 2 * i / do
    by_defl = (0.0025 * 384 * 200e9 * i / (5 * w)) ** 0.25
    by_stress = math.sqrt(8 * z * 83e6 / w)
    return max(3.0, min(12.0, by_defl, by_stress))


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
                  "support": {"spacing": r(span), "mass": r(10 + 60 * out), "per_m_height": r(4 * (0.1 + 0.1 * out) * 0.005 * RHO[A36]), "item": A36},
                  "connector": flange(dn, pn, item, sealing, temp), "carries": "liquid" if liquid else "gas",
                  "basis": [{"of": ["per_m"], "tier": "derived", "note": "Wall from pressure; mass from material and dimensions."},
                            {"of": ["elbow", "tee"], "tier": "derived", "note": "A long-radius elbow (1.5 bores) is its arc of straight; a tee is its branch stub over the straight it sits in, from the main's outside to 1.5 bores off its axis (checked on blender's DN 200 kit)."},
                            {"of": ["support"], "tier": "derived", "review": True, "note": f"Spacing from the span it bears full at 1 g (2.5 mm sag, a third of yield), 3 to 12 m: {span:.1f} m. The stand's head and its leg by the metre from memory."}]}
    return key


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
                  "support": {"spacing": r(span), "mass": r(30 + 80 * do), "per_m_height": r(4 * (0.1 + 0.1 * do) * 0.006 * RHO[A36]), "item": A36},
                  "cap": (lambda c: dict(c, mass=r(c["mass"] + math.pi / 4 * (dn - 2 * lining) ** 2 * lining * RHO[REF])))(cap(dn, PN[pn], A36, lined_flange(dn, pn, sealing=sealing))),
                  "connector": lined_flange(dn, pn, sealing=sealing), "carries": "gas",
                  "basis": [{"of": ["per_m"], "tier": "derived", "note": "Shell from pressure, lining from its thickness; mass from material and dimensions."},
                            {"of": ["support", "elbow"], "tier": "derived", "review": True, "note": f"Spacing from the span it bears at 1 g, {span:.1f} m; mitred bends a tenth heavier than their arc. Stands from memory."}]}
    return key


def belt(width, rate):
    key = f"run.belt-{_tag(width)}"
    if key in _recs:
        return key
    built = [_part("Belt", RUBBER, 2 * width * 0.018 * RHO[RUBBER], "both runs, 18 mm; a vacuum-rated belt, silicone rubber standing for it"),
             _part("Idlers", A36, 39.0 / 1.2 + 20.0 / 3.0, "a three-roll troughing set every 1.2 m (39 kg) and a return roll every 3 m (20 kg), sealed bearings with vacuum grease; from memory"),
             _part("Stringers", A36, 2 * 18.0, "two 150 x 75 mm channels")]
    per_m = sum(b["mass"] for b in built)
    _recs[key] = {"identity": {"key": key, "name": f"Belt conveyor, {width * 1000:.0f} mm", "description": f"An open troughed belt {width * 1000:.0f} mm wide on idlers and stringers, carried on trestles; it runs straight (a turn is a transfer to another belt). Its drive is at its head."},
                  "kind": "belt", "size": width, "outside": [r(width + 0.6, 2), 1.2], "rating": {"rate": r(rate)},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "support": {"spacing": 18.0, "mass": 300.0, "per_m_height": 75.0, "item": A36},
                  "repeat": [{"piece": "idler", "pitch": 1.2}, {"piece": "return_roll", "pitch": 3.0}],
                  "terminals": {"head": {"mass": 4500.0, "transition": 1.0, "note": "a 0.8 m lagged drive pulley, its bearings and frame, a gearbox and a motor of about 400 kW (from memory)"},
                                "tail": {"mass": 2200.0, "transition": 1.0, "note": "a 0.63 m tail pulley on a screw take-up, its bearings and frame (from memory)"}},
                  "connector": belt_transfer(width), "carries": "bulk",
                  "basis": [{"of": ["per_m", "support"], "tier": "invented", "review": True, "note": "Belt, idlers and stringers from memory of 1.2 m belts; trestles every 18 m, a 300 kg cross beam and two 200 mm box legs (75 kg/m)."}]}
    return key


def drag(width, rate, temp=923.0):
    key = f"run.drag-conveyor-{_tag(width)}"
    if key in _recs:
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
    built.append(_part("Tray", AL, 4.0, "its share of a ladder tray"))
    per_m = sum(b["mass"] for b in built)
    od = dc + 2 * ti + 2 * ts
    _recs[key] = {"identity": {"key": key, "name": f"Cable, {v / 1000:g} kV, {area_mm2} mm2", "description": f"Three single-core cables of {area_mm2} mm2 at {v / 1000:g} kV on a ladder tray: {a * J_VACUUM:,.0f} A a circuit on airless ground, where only radiation cools them."},
                  "kind": "cable", "size": a, "outside": [r(max(0.3, 3 * od + 0.1), 3), r(0.1 + od, 3)], "rating": {"voltage": v, "current": r(a * J_VACUUM)},
                  "per_m": {"mass": r(per_m), "built_of": built},
                  "elbow": {"radius": r(max(0.3, 15 * od), 3), "mass": r(per_m * math.pi / 2 * max(0.3, 15 * od))},
                  "tee": {"mass": 25.0},
                  "support": {"spacing": 3.0, "mass": 8.0, "per_m_height": 6.0, "item": AL},
                  "connector": termination(v, area_mm2), "carries": "power",
                  "basis": [{"of": ["per_m"], "tier": "derived", "review": True, "note": f"Copper from its area; insulation by the voltage's usual thickness (from memory); the rating at {J_VACUUM / 1e6:g} A/mm2, radiation only."},
                            {"of": ["support", "tee", "elbow"], "tier": "invented", "review": True, "note": "Tray posts every 3 m; a tee is a joint box; bends at 15 cable diameters. From memory."}]}
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
    m += sum(_recs[c]["physical"]["mass"] for c in taps) * count
    m += m_sup + count * sum(t["mass"] for t in rec.get("terminals", {}).values())
    fit = {"elbows": n_elb, "tees": tees + len(taps), "supports": n_sup, "connectors": port_ends + caps + len(taps)}
    return m, fit, L


def records():
    """Every connector and run asked for: (folder, slug, record)."""
    for key, d in sorted(_recs.items()):
        kind, slug = key.split(".", 1)
        yield {"connector": "connectors", "run": "runs"}[kind], slug, d
