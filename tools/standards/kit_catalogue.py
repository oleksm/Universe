"""The catalogue kit families (docs/proposals/connectables.md, packages 4 and 9 of the SFO 26 plan): what joins things
that layouts do not yet lay, in preferred sizes, each family drawn by one template. Writes runs, connectors, their
interfaces and templates; then a stock record for every kit item, laid or not (how it is sold and hauled: a coil, a
length, a piece in a crate).

    python3 tools/standards/kit_catalogue.py           # write them (idempotent)

Families: hose (rubber, PN 16), hydraulic hose (two steel braids), metal hose (corrugated stainless, braided),
flexible cable (trailing and reeling), conductor rail, festoon; cam-lock and flat-face quick couplings, IEC 60309 plugs
and sockets, single-pole 400 A connectors, an 11 kV coupler, collectors, ball, butterfly, gate, check and relief valves,
expansion joints, ISO 1161 corner castings and twistlocks, cable reels. Figures from memory unless a basis says
otherwise; every record says so.
"""
import glob, math, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plant_kit as pk  # noqa: E402
from packages import FORMS as PACKAGE_OF  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata")
HEAD = "# Written by tools/standards/kit_catalogue.py: change it there.\n"

A36, SS, AL, CU, RUB, PE, BR = ("material.structural-steel-a36", "material.stainless-steel-304", "material.aluminium-alloy-6061",
                               "material.copper", "material.epdm-rubber", "material.polyethylene-uhmw", "material.bearing-bronze-c93200")
RHO = {A36: 7850, SS: 7900, AL: 2700, CU: 8960, RUB: 1150, PE: 940}
MEMORY = "From memory of makers' catalogues; to be sourced."


def r(x, nd=1):
    return round(float(x), nd)


def tag(x):
    return f"{round(x * 1000):d}"


def part(what, item, mass, note=""):
    d = {"what": what, "item": item, "mass": r(mass, 3 if mass < 1 else 2 if mass < 10 else 1)}
    if note:
        d["note"] = note
    return d


def tube_mass(od, t, rho):
    return math.pi * (od - t) * t * rho


OUT = []          # (folder, slug, record)
IFS = {}


def run(slug, d):
    OUT.append(("runs", slug, d))


def conn(slug, d, iface):
    d["interface"] = "interface." + iface["slug"]
    OUT.append(("connectors", slug, d))
    i = IFS.setdefault(iface["slug"], {"identity": {"key": "interface." + iface["slug"], "name": iface["name"],
                                                    "description": iface.get("description", f"The mating face of the {iface['name']}: any maker's half built to it mates with any other's.")},
                                       "domain": iface["domain"], "kind": iface["kind"], "envelope": {}, "media": iface["media"],
                                       "sealing": iface.get("sealing", "air"), "halves": iface["halves"], "make": iface["make"],
                                       "face": {k: d["physical"][k] for k in ("length", "width", "height")}})
    if d.get("size"):
        i["size"] = d["size"]
    for k, v in (d.get("rating") or {}).items():
        if k in ("pressure", "temperature", "voltage", "current", "force"):
            i["envelope"][k] = max(i["envelope"].get(k, 0), v)
    if iface.get("standard"):
        i["standard"] = iface["standard"]
    i["basis"] = [{"of": ["envelope", "face"], "tier": "derived", "note": "The most the products built to it are rated for, and their joint's outside."}]


# ------------------------------------------------------------------ hoses and their couplings
CAM = {0.025: 0.6, 0.05: 1.27, 0.1: 3.6, 0.15: 7.5}            # kg a cam-lock pair (A and D): DN 50 as blender modelled it (1.166 kg of cast aluminium, about 0.1 kg of pins and gasket, 2026-10-17), the others scaled by it from memory
CAM_P = {0.025: 1.7e6, 0.05: 1.7e6, 0.1: 1.0e6, 0.15: 0.7e6}   # Pa they are rated for (EN 14420-7, falling with size; from memory)
for dn in (0.025, 0.05, 0.1, 0.15):
    t = {0.025: 0.006, 0.05: 0.008, 0.1: 0.012, 0.15: 0.015}[dn]
    od = dn + 2 * t
    rub = tube_mass(od, t, RHO[RUB]) * 0.8
    steel = tube_mass(od, t, RHO[RUB]) * 0.25
    ck = f"camlock-dn{tag(dn)}"
    run(f"hose-dn{tag(dn)}-pn16", {
        "identity": {"key": f"run.hose-dn{tag(dn)}-pn16", "name": f"Hose, DN {tag(dn)}, PN 16",
                     "description": f"A rubber hose for water, air and oils, a steel wire spiral in its wall: laid loose or on the ground, bent by hand, its ends cam-lock couplings."},
        "kind": "hose", "size": dn, "bore": dn, "outside": [r(od, 3), r(od, 3)],
        "rating": {"pressure": 1.6e6, "temperature": 373.0},
        "per_m": {"mass": r(rub + steel, 2), "built_of": [part("Liner, cover and plies", RUB, rub, "EPDM rubber"), part("Wire spiral", A36, steel, "a steel helix against kinking and vacuum")]},
        "elbow": {"radius": r(6 * od, 3), "mass": r((rub + steel) * 6 * od * math.pi / 2, 2)},
        "connector": f"connector.{ck}", "carries": "liquid", "template": "template.hose",
        "basis": [{"of": ["per_m", "elbow", "rating"], "tier": "invented", "review": True, "note": f"Wall {t * 1000:g} mm, rubber at 1,150 kg/m3 and a quarter as much steel spiral; least bend six outsides; {MEMORY}"}]})
    conn(ck, {
        "identity": {"key": f"connector.{ck}", "name": f"Cam-lock coupling, DN {tag(dn)}", "description": "A hose coupling made and broken by hand: a grooved adapter (male) and a coupler (female) whose two cam arms pull it home on its gasket."},
        "kind": "quick-coupling", "size": dn, "rating": {"pressure": CAM_P[dn], "temperature": 373.0},
        "physical": {"length": r(dn * 1.2 + 0.07, 3), "width": r(dn + 0.036, 3), "height": r(dn * 1.9 + 0.06, 3), "mass": CAM[dn]},
        "built_of": [part("Adapter and coupler", AL, CAM[dn] * 0.95, "cast aluminium; stainless pins"), part("Gasket", RUB, CAM[dn] * 0.05)],
        "provides": "both", "template": "template.camlock",
        "basis": [{"of": ["physical", "rating"], "tier": "invented", "review": True, "note": MEMORY}]},
        {"slug": ck, "name": f"Cam-lock DN {tag(dn)}", "domain": "fluid", "kind": "quick-coupling", "media": ["gas", "liquid"], "halves": "male-female", "make": "manual", "standard": "EN 14420-7 (cam-lock) sizes"})

FF = {0.00635: (0.15, 34.5e6), 0.0127: (0.42, 31.5e6), 0.01905: (0.95, 27.5e6), 0.0254: (1.8, 25.0e6)}   # kg a pair, Pa (ISO 16028; from memory)
FF_STEEL = 0.8225       # blender's DN 12.7 model (2026-10-17): the halves in stainless; the seals an allowance on top; the others scaled by the old ratios to it
FF_SEALS = 0.013
FF_NOTE = "From blender's model (2026-10-17): 51.2 x 51.2 x 121 mm, 12.7 mm bore, 36 mm body and hex, 51 mm release sleeve over a 36.4 mm cavity, 0.8225 kg of stainless; seals, springs and balls an allowance. No pressure qualification claimed."
for dn, (mass, p) in FF.items():
    steel = round(mass * FF_STEEL / 0.42, 3)
    seals = round(FF_SEALS * mass / 0.42, 3)
    mass = round(steel + seals, 3)
    od = {0.00635: 0.0141, 0.0127: 0.0213, 0.01905: 0.0293, 0.0254: 0.038}[dn]
    pm = {0.00635: 0.23, 0.0127: 0.53, 0.01905: 0.83, 0.0254: 1.26}[dn]
    ck = f"flat-face-dn{tag(dn)}"
    run(f"hose-hydraulic-dn{tag(dn)}", {
        "identity": {"key": f"run.hose-hydraulic-dn{tag(dn)}", "name": f"Hydraulic hose, DN {dn * 1000:g}", "description": "A two-braid hydraulic hose (SAE 100R2 class): oil at working pressure to cylinders and motors that move, its ends flat-face quick couplings."},
        "kind": "hose", "size": dn, "bore": dn, "outside": [od, od], "rating": {"pressure": p, "temperature": 373.0},
        "per_m": {"mass": pm, "built_of": [part("Tube and cover", RUB, pm * 0.45, "nitrile, as EPDM here"), part("Two steel braids", A36, pm * 0.55)]},
        "elbow": {"radius": r(od * 7, 3), "mass": r(pm * od * 7 * math.pi / 2, 3)},
        "connector": f"connector.{ck}", "carries": "liquid", "template": "template.hose-hydraulic",
        "basis": [{"of": ["per_m", "outside", "rating"], "tier": "invented", "review": True, "note": f"Two-braid hose figures; {MEMORY}"}]})
    conn(ck, {
        "identity": {"key": f"connector.{ck}", "name": f"Flat-face coupling, DN {dn * 1000:g}", "description": "A hydraulic quick coupling with flat valve faces: made and broken by hand under no load, spilling next to nothing."},
        "kind": "quick-coupling", "size": dn, "rating": {"pressure": p, "temperature": 373.0},
        "physical": {"length": r(od * 2.4, 3), "width": r(od * 2.4, 3), "height": r(dn * 4 + 0.07, 3), "mass": mass},
        "built_of": [part("Male and female halves, sleeve and valves", SS, steel), part("Seals", RUB, seals, "allowance")],
        "provides": "both", "template": "template.flat-face",
        "basis": [{"of": ["physical", "built_of"], "tier": "derived" if dn == 0.0127 else "invented", "review": True, "note": FF_NOTE if dn == 0.0127 else f"Scaled from the DN 12.7's measured {FF_STEEL:g} kg by the old catalogue ratios; {MEMORY}"}, {"of": ["rating"], "tier": "invented", "review": True, "note": MEMORY}]},
        {"slug": ck, "name": f"Flat-face DN {dn * 1000:g}", "domain": "fluid", "kind": "quick-coupling", "media": ["liquid"], "halves": "male-female", "make": "manual", "standard": "ISO 16028 (flat-face) sizes"})

for dn in (0.025, 0.05, 0.1, 0.2):
    od = dn * 1.32 + 0.006
    corr = tube_mass(dn * 1.15, 0.0004 * (1 if dn < 0.1 else 1.5), RHO[SS]) * 2.6        # (the corrugation's developed length about 2.6 times)
    braid = corr * 0.9
    run(f"hose-metal-dn{tag(dn)}", {
        "identity": {"key": f"run.hose-metal-dn{tag(dn)}", "name": f"Metal hose, DN {tag(dn)}, PN 25", "description": "A corrugated stainless hose in a stainless braid: where rubber will not do (hot, cold, vacuum, airless ground, gas that rubber lets through). Its ends weld-neck flanges."},
        "kind": "hose", "size": dn, "bore": dn, "outside": [r(od, 3), r(od, 3)], "rating": {"pressure": 2.5e6, "temperature": 823.0},
        "per_m": {"mass": r(corr + braid, 2), "built_of": [part("Corrugated tube", SS, corr), part("Braid", SS, braid)]},
        "elbow": {"radius": r(od * 5, 3), "mass": r((corr + braid) * od * 5 * math.pi / 2, 2)},
        "connector": f"connector.flange-dn{tag(dn)}-pn25-ss" if dn >= 0.1 else f"connector.hose-union-dn{tag(dn)}",
        "carries": "gas", "template": "template.hose-metal",
        "basis": [{"of": ["per_m", "elbow"], "tier": "invented", "review": True, "note": f"0.4 mm stainless corrugation (0.6 mm from DN 100) and a braid of nine tenths its weight; least bend radius five outsides for a hose that flexes now and then (static: about two); {MEMORY}"}]})
UNION_KG = 1.375         # blender's DN 25 model (2026-10-18): 304, captive hex nut, metal cone seat; the DN 50 scaled by the old ratio
for dn in (0.025, 0.05):
    m = round({0.025: 0.6, 0.05: 1.8}[dn] * UNION_KG / 0.6, 2)
    ck = f"hose-union-dn{tag(dn)}"
    conn(ck, {
        "identity": {"key": f"connector.{ck}", "name": f"Hose union, DN {tag(dn)}", "description": "A screwed union with a metal seat: a small metal hose's end, made with spanners."},
        "kind": "union", "size": dn, "rating": {"pressure": 2.5e6, "temperature": 823.0},
        "physical": {"length": r(dn * 2.2 + 0.03, 3), "width": r(dn * 2.2 + 0.03, 3), "height": r(dn * 2 + 0.04, 3), "mass": m},
        "built_of": [part("Union nut, tail and seat", SS, m)], "provides": "both", "template": "template.union",
        "basis": [{"of": ["physical"], "tier": "derived" if dn == 0.025 else "invented", "review": True, "note": "From blender's model (2026-10-18): 73.6 mm hex across flats (85 across corners) x 90 mm, round DN 25 bore, captive nut and metal cone seat, 1.375 kg of 304." if dn == 0.025 else f"Scaled from the DN 25's measured {UNION_KG:g} kg by the old ratio; {MEMORY}"}]},
        {"slug": ck, "name": f"Union DN {tag(dn)}", "domain": "fluid", "kind": "union", "media": ["gas", "liquid"], "halves": "male-female", "make": "bolted"})
for dn in (0.1, 0.2):          # (PN 25 stainless flanges for the bigger metal hoses, as the plant kit makes its own)
    key = pk.flange(dn, 25, item=pk.SS, temp=823.0)
    d = dict(pk._recs[key], template="template.flange")
    i = pk.interface_key(key)
    d["provides"] = "both"
    conn(key.split(".", 1)[1], d, {"slug": i.split(".", 1)[1], "name": d["identity"]["name"].replace(", stainless", ""), "domain": "fluid", "kind": "flange", "media": ["gas", "liquid"], "halves": "androgynous", "make": "bolted", "standard": pk.STANDARD["flange"]})

# ------------------------------------------------------------------ flexible cable, plugs, feeds to moving things
FLEX = [  # (slug, volts, cores, mm2, amps in air, outside m, connector)
    ("690v-4x16", 690, 4, 16, 80, 0.026, "plug-iec60309-125a"),
    ("690v-4x35", 690, 4, 35, 125, 0.034, "plug-iec60309-125a"),
    ("690v-4x95", 690, 4, 95, 240, 0.05, "powerlock-400a"),
    ("11kv-3x95", 11000, 3, 95, 260, 0.062, "coupler-11kv-400a")]
for slug, v, n, mm2, amps, od, ck in FLEX:
    cu = n * mm2 * 1e-6 * RHO[CU]
    sheath = math.pi / 4 * od * od * RHO[RUB] * 0.55
    run(f"cable-flexible-{slug}", {
        "identity": {"key": f"run.cable-flexible-{slug}", "name": f"Flexible cable, {v / 1000:g} kV, {n} x {mm2} mm2" if v >= 1000 else f"Flexible cable, {v} V, {n} x {mm2} mm2",
                     "description": f"A rubber-sheathed cable of fine-stranded copper that bends all its life: on a reel, a festoon or laid to a machine that moves. {amps} A in air; about half that on airless ground, where only radiation cools it."},
        "kind": "cable", "size": mm2 * 1e-6, "outside": [od, od], "rating": {"voltage": v, "current": amps},
        "per_m": {"mass": r(cu + sheath, 2), "built_of": [part("Conductors", CU, cu, f"{n} fine-stranded cores of {mm2} mm2"), part("Insulation and sheath", RUB, sheath, "EPDM and a tough rubber sheath")]},
        "elbow": {"radius": r(od * 7.5, 3), "mass": r((cu + sheath) * od * 7.5 * math.pi / 2, 2)},
        "connector": f"connector.{ck}", "carries": "power", "template": "template.cable-flexible",
        "basis": [{"of": ["per_m", "rating", "outside"], "tier": "invented", "review": True, "note": f"Reeling cable figures; least bend 7.5 outsides on a reel; {MEMORY}"}]})

PLUGS = [  # (slug, name, amps, volts, pins, kg a pair, l, w, h, interface name, standard)
    ("plug-iec60309-16a", "Plug and socket, 16 A, 400 V", 16, 400, 5, 0.45, 0.095, 0.095, 0.19, "IEC 60309 16 A 5-pin", "IEC 60309-2"),
    ("plug-iec60309-32a", "Plug and socket, 32 A, 400 V", 32, 400, 5, 0.65, 0.11, 0.11, 0.21, "IEC 60309 32 A 5-pin", "IEC 60309-2"),
    ("plug-iec60309-63a", "Plug and socket, 63 A, 400 V", 63, 400, 5, 1.9, 0.14, 0.14, 0.28, "IEC 60309 63 A 5-pin", "IEC 60309-2"),
    ("plug-iec60309-125a", "Plug and socket, 125 A, 690 V", 125, 690, 5, 3.6, 0.17, 0.17, 0.33, "IEC 60309 125 A 5-pin", "IEC 60309-2"),
    ("powerlock-400a", "Single-pole connectors, 400 A, set of five", 400, 1000, 5, 6.5, 0.3, 0.12, 0.25, "Single-pole 400 A", "single-pole 400 A connectors (the registry's; of the order of the Powerlock family)"),
    ("coupler-11kv-400a", "Coupler, 11 kV, 400 A", 400, 11000, 3, 28.0, 0.22, 0.22, 0.6, "Coupler 11 kV 400 A", "a mining-type 11 kV plug coupler (the registry's)")]
BOUND = {  # blender's measured parts, where the model changed what it is made of
    "coupler-11kv-400a": ([("Contacts", CU, 4.86, "pins and sockets"), ("Body", RUB, 14.95, "EPDM-moulded body"), ("Shroud, coupling ring and bolts", A36, 3.33, None)],
                          "blender's model (2026-10-17): EPDM body, three phase pins on a 90 mm circle, pilot and earth at the centre, steel shroud and ring"),
    "powerlock-400a": ([("Contacts", CU, 1.421, "16 mm pins, 22 mm socket sleeves; silver-plated brass, as copper here"), ("Bodies, glands and bands", RUB, 2.005, "rubber-moulded body")],
                       "blender's model (2026-10-18): five bored pairs, the 5 mm steel panel plate; hidden cable terminations not drawn")}
PANEL = {"powerlock-400a": round((0.3 * 0.12 - 5 * math.pi * 0.025 ** 2) * 0.005 * RHO[A36], 2)}   # the receivers' common plate: 5 mm steel (invented), five 50 mm cut-outs
for slug, name, amps, v, pins, kg, l, w, h, iname, std in PLUGS:
    cu = min(kg * 0.3, pins * amps * 4e-6 * 0.08 * RHO[CU] + 0.05)
    if slug in BOUND:
        kg = round(sum(part(*p)["mass"] for p in BOUND[slug][0]), 2)
    plate = [part("Panel plate", A36, PANEL[slug], "5 mm, the receivers' common mounting plate, four corner holes")] if slug in PANEL else []
    conn(slug, {
        "identity": {"key": f"connector.{slug}", "name": name, "description": "The plug and receiver a crew or a player hooks a machine up with: made and broken by hand, keyed so that only its own rating mates, the earth pin first in and last out."},
        "kind": "plug-socket", "size": amps / 1e6, "rating": {"voltage": v, "current": amps},
        "physical": {"length": l, "width": w, "height": h, "mass": round(kg + sum(q["mass"] for q in plate), 2)},
        "built_of": ([part(*p) for p in BOUND[slug][0]] if slug in BOUND else [part("Contacts", CU, cu, "silver-plated brass, as copper here"), part("Housings and seals", PE if v < 1000 else RUB, kg - cu, "polyamide housings, as UHMW polyethylene here" if v < 1000 else "rubber-moulded body")]) + plate,
        "provides": "both", "template": "template.plug-socket" if v < 1000 and pins == 5 and amps <= 125 else "template.single-pole" if slug.startswith("powerlock") else "template.coupler",
        "basis": [{"of": ["physical"], "tier": "derived", "review": True, "note": f"From {BOUND[slug][1]}."} if slug in BOUND else {"of": ["physical"], "tier": "invented", "review": True, "note": MEMORY}]},
        {"slug": slug, "name": iname, "domain": "power", "kind": "plug-socket", "media": ["power"], "halves": "male-female", "make": "manual", "standard": std})

RAILS = [(100, 25, 2.6), (250, 60, 4.3), (500, 125, 7.1), (1000, 250, 12.6)]     # (A, mm2 copper a pole, kg/m)
for amps, mm2, kgm in RAILS:
    cu = 4 * mm2 * 1e-6 * RHO[CU]
    ck = f"collector-{amps}a"
    run(f"conductor-rail-{amps}a", {
        "identity": {"key": f"run.conductor-rail-{amps}a", "name": f"Conductor rail, {amps} A, 690 V", "description": f"Four copper conductors in an insulating housing hung along a crane's or a car's path; a collector trolley on the machine slides along them. For tight spaces and many machines on one track."},
        "kind": "conductor-rail", "size": amps / 1e6, "outside": [0.09 if amps < 500 else 0.12, 0.16 if amps < 500 else 0.2],
        "rating": {"voltage": 690, "current": amps},
        "per_m": {"mass": kgm, "built_of": [part("Conductors", CU, cu, f"four poles of {mm2} mm2"), part("Housing", PE, kgm - cu - 0.5, "PVC, as UHMW polyethylene here"), part("Hangers' share", A36, 0.5)]},
        "support": {"spacing": 1.5, "mass": 1.2, "per_m_height": 2.0, "item": A36},
        "connector": f"connector.{ck}", "carries": "power", "template": "template.conductor-rail",
        "basis": [{"of": ["per_m", "support"], "tier": "invented", "review": True, "note": f"About 1 to 4 A/mm2 in the housing; hangers every 1.5 m; {MEMORY}"}]})
    m = {100: 2.5, 250: 3.5, 500: 6.0, 1000: 11.0}[amps]
    conn(ck, {
        "identity": {"key": f"connector.{ck}", "name": f"Collector trolley, {amps} A", "description": "The sliding shoes a moving machine draws power by from a conductor rail, sprung against the conductors, on a towing arm."},
        "kind": "collector", "size": amps / 1e6, "rating": {"voltage": 690, "current": amps},
        "physical": {"length": 0.35 if amps < 500 else 0.6, "width": 0.15, "height": 0.25, "mass": m},
        "built_of": [part("Shoes", CU, m * 0.15, "copper-graphite, as copper here"), part("Trolley and arm", A36, m * 0.6), part("Insulators", PE, m * 0.25)],
        "provides": "both", "template": "template.collector",
        "basis": [{"of": ["physical"], "tier": "invented", "review": True, "note": MEMORY}]},
        {"slug": f"conductor-rail-{amps}a", "name": f"Conductor rail {amps} A", "description": f"A {amps} A four-pole conductor rail's face: any collector built to it runs on any rail built to it.", "domain": "power", "kind": "conductor-rail", "media": ["power"], "halves": "male-female", "make": "automatic"})

for slug, v, n, mm2, amps, od, ck in FLEX[:3]:
    cu = n * mm2 * 1e-6 * RHO[CU]
    cable = cu + math.pi / 4 * od * od * RHO[RUB] * 0.55
    run(f"festoon-{slug}", {
        "identity": {"key": f"run.festoon-{slug}", "name": f"Festoon, {n} x {mm2} mm2", "description": "A flexible cable hung in loops from trolleys on a C-track along a crane's or a carriage's travel: the loops gather as the machine comes near and pay out as it goes. For short travels up to about 100 m."},
        "kind": "festoon", "size": mm2 * 1e-6, "outside": [r(0.06 + od, 3), r(1.2 + od, 3)], "rating": {"voltage": v, "current": amps},
        "per_m": {"mass": r(3.2 + 1.4 + cable * 1.3, 2), "built_of": [part("C-track", A36, 3.2, "galvanised 40 x 40 mm section"), part("Cable trolleys' share", A36, 1.4, "a trolley every 1.5 m of track"), part("Cable, with its loops", CU, cable * 1.3, "1.3 m of cable a metre of track")]},
        "support": {"spacing": 2.0, "mass": 1.02, "per_m_height": 3.0, "item": A36},
        "connector": f"connector.{ck}", "carries": "power", "template": "template.festoon",
        "basis": [{"of": ["per_m", "outside"], "tier": "invented", "review": True, "note": f"Loops 1.2 m deep; {MEMORY}"},
                  {"of": ["support"], "tier": "derived", "review": True, "note": "Head from blender's model (2026-10-17): a 6 mm clasp and threaded seat, 1.02 kg; the rod 22 mm, 2.95 kg/m (kept at 3)."}]})

for slug, v, n, mm2, amps, od, ck in FLEX[1:]:
    L = {"690v-4x35": 100, "690v-4x95": 200, "11kv-3x95": 300}[slug]
    d0 = max(1.0, od * 30)
    D = math.sqrt(d0 * d0 + 4 * L * od / math.pi)
    cu = n * mm2 * 1e-6 * RHO[CU]
    cable = (cu + math.pi / 4 * od * od * RHO[RUB] * 0.55) * L
    frame = 150 + 52.5 * D * D          # (blender's 690 V 4 x 95 reel, 2026-10-17: 786 kg of cheeks, core, A-frames, shaft and stiffeners on a 3.87 m drum; 150 kg of drive, bearings and slip rings invented)
    rk = f"cable-reel-{slug}-{L}m"
    conn(rk, {
        "identity": {"key": f"connector.{rk}", "name": f"Cable reel, {n} x {mm2} mm2, {L} m", "description": f"A motor-driven monospiral reel on a moving machine, holding {L} m of its flexible cable: it pays out and winds in as the machine travels, its slip rings passing the power from the turning drum. For long travels."},
        "kind": "cable-reel", "size": mm2 * 1e-6, "rating": {"voltage": v, "current": amps},
        "physical": {"length": r(D, 2), "width": r(od + 0.35, 2), "height": r(D + 0.2, 2), "mass": r(frame + cable)},
        "built_of": [part("Drum, frame, drive and slip rings", A36, frame, f"a {D:.2f} m drum on a {d0:.2f} m core"), part("Its cable", CU, cable, f"{L} m of the flexible cable, wound")],
        "provides": "both", "template": "template.cable-reel",
        "basis": [{"of": ["physical"], "tier": "derived", "review": True, "note": f"A monospiral drum: D^2 = core^2 + 4 L outside / pi; core thirty cable outsides, at least 1 m; structure 52.5 kg/m2 of D^2 (blender's measured reel), drive, bearings and slip rings 150 kg (invented)."}]},
        {"slug": f"cable-reel-{slug}", "name": f"Reel feed {n} x {mm2} mm2", "description": "The fixed feed point a cable reel's cable is plugged into at the travel's centre.", "domain": "power", "kind": "cable-reel", "media": ["power"], "halves": "male-female", "make": "bolted"})

# ------------------------------------------------------------------ valves and expansion joints (between the plant kit's PN 10 flanges)
VALVES = [  # (type, DN, kg, face to face m, height m, what)
    ("ball", 0.05, 9.0, 0.15, 0.2, "a bored ball turned a quarter by its lever"),
    ("ball", 0.1, 28.0, 0.23, 0.3, "a bored ball turned a quarter by its gearbox"),
    ("butterfly", 0.15, 12.0, 0.056, 0.42, "a disc on a shaft across the bore, a quarter turn"),
    ("butterfly", 0.3, 45.0, 0.078, 0.7, "a disc on a shaft across the bore, a quarter turn by gearbox"),
    ("butterfly", 0.6, 230.0, 0.154, 1.25, "a double-offset disc, actuated"),
    ("butterfly", 1.0, 900.0, 0.216, 1.95, "a double-offset disc, actuated"),
    ("gate", 0.1, 40.0, 0.19, 0.55, "a wedge drawn up out of the bore by a handwheel's stem"),
    ("gate", 0.3, 280.0, 0.33, 1.5, "a wedge drawn up out of the bore by a handwheel's stem"),
    ("check", 0.1, 30.0, 0.3, 0.25, "a swing disc that lets flow one way only"),
    ("check", 0.3, 220.0, 0.5, 0.55, "a swing disc that lets flow one way only"),
    ("relief", 0.05, 20.0, 0.25, 0.45, "a spring-held disc that lifts when the pressure passes its set"),
    ("relief", 0.1, 60.0, 0.35, 0.75, "a spring-held disc that lifts when the pressure passes its set")]
FL_OD = {0.05: 0.165, 0.1: 0.22, 0.15: 0.285, 0.3: 0.445, 0.6: 0.78, 1.0: 1.23}       # m, PN 10 flange outsides (EN 1092-1; from memory)


def fl_od(dn):
    """The PN 10 flange's outside it bolts to: the plant kit's flange interface (its joint's face) where there is one, else EN 1092-1's."""
    f = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "standards", "SFO", "metadata", "interfaces", f"flange-dn{tag(dn)}-pn10.yaml")
    return float(yaml.safe_load(open(f))["face"]["length"]) if os.path.exists(f) else FL_OD[dn]


ROD_LUGS = 0.06     # m the limit rods' lugs stand out past the flanges, both sides together (invented)
for vt, dn, kg, ff, h, what in VALVES:
    slug = f"valve-{vt}-dn{tag(dn)}-pn10"
    conn(slug, {
        "identity": {"key": f"connector.{slug}", "name": f"{vt.capitalize()} valve, DN {tag(dn)}, PN 10", "description": f"A flanged {vt} valve set in a line between two flange joints: {what}."},
        "kind": "valve", "size": dn, "valve": vt, "rating": {"pressure": 1.0e6, "temperature": 473.0},
        "physical": {"length": fl_od(dn), "width": r(fl_od(dn) / 2 + max(h, fl_od(dn) / 2), 3), "height": r(ff, 3), "mass": kg},     # (width: from the flange's far rim to the actuator's top, off the axis along +Z)
        "built_of": [part("Body and bonnet", A36, kg * 0.75, "cast carbon steel"), part("Trim", SS, kg * 0.2, "stainless seats, stem, disc or ball"), part("Packing and seats", RUB if vt != "gate" else SS, kg * 0.05)],
        "provides": "both", "template": f"template.valve-{vt}",
        "basis": [{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": f"Face to face as the EN 558 series run (from memory); its height along the line; the lever, gearbox or actuator out to {h} m from the axis along +Z, inside its width; {MEMORY}"}]},
        {"slug": f"flange-dn{tag(dn)}-pn10", "name": f"Flange DN {tag(dn)} PN 10", "domain": "fluid", "kind": "flange", "media": ["gas", "liquid"], "halves": "androgynous", "make": "bolted", "standard": "EN 1092-1 bores and pressure classes (its faces and bolting the registry's)"})
EJ_KG = {0.3: 49.48}       # blender's DN 300 model (2026-10-17): 36.15 kg A36, 13.34 kg 304; the others scaled by the old catalogue's ratios to it
EJ_SS = 13.34 / 49.48
EJ_NOTE = "From blender's model (2026-10-17): 526 x 526 x 350 mm, six formed convolutions of 3 mm multi-ply 304 on a 300 mm bore, A36 flanges at the kit's 466 mm, limit rods stopping 40 mm each way."
for dn, kg, ll, mv in ((0.1, 15.0, 0.25, 0.025), (0.3, 70.0, 0.35, 0.04), (0.6, 220.0, 0.45, 0.06), (1.0, 560.0, 0.55, 0.08)):
    kg = round(kg * EJ_KG[0.3] / 70.0, 1)
    slug = f"expansion-joint-dn{tag(dn)}-pn10"
    conn(slug, {
        "identity": {"key": f"connector.{slug}", "name": f"Expansion joint, DN {tag(dn)}, PN 10", "description": f"A stainless bellows between two flanges, set in a line to take its growth as it heats: {mv * 1000:g} mm each way along the line, a little sideways. Limit rods outside it stop its travel at that much; they do not hold the pressure's end load, the pipe's anchors either side do."},
        "kind": "expansion-joint", "size": dn, "rating": {"pressure": 1.0e6, "temperature": 673.0}, "movement": mv,
        "physical": {"length": r(fl_od(dn) + ROD_LUGS, 3), "width": r(fl_od(dn) + ROD_LUGS, 3), "height": ll, "mass": kg},
        "built_of": [part("Bellows and necks", SS, kg * EJ_SS, "six formed convolutions, 3 mm multi-ply 304"), part("Flanges, limit rods and lugs", A36, kg * (1 - EJ_SS))],
        "provides": "both", "template": "template.expansion-joint",
        "basis": [{"of": ["physical", "built_of"], "tier": "derived" if dn == 0.3 else "invented", "review": True, "note": EJ_NOTE if dn == 0.3 else f"Scaled from the DN 300's measured {EJ_KG[0.3]:g} kg by the old catalogue ratios; {MEMORY}"}, {"of": ["movement"], "tier": "invented", "review": True, "note": MEMORY}]},
        {"slug": f"flange-dn{tag(dn)}-pn10", "name": f"Flange DN {tag(dn)} PN 10", "domain": "fluid", "kind": "flange", "media": ["gas", "liquid"], "halves": "androgynous", "make": "bolted", "standard": "EN 1092-1 bores and pressure classes (its faces and bolting the registry's)"})

# ------------------------------------------------------------------ container locks
TL = {"slug": "twistlock-iso1161", "name": "ISO 1161 corner", "description": "A container's corner: the ISO 1161 corner casting's oval hole and the twistlock turned a quarter in it. How containers stack and lock to trucks, trains, ships and pads.",
      "domain": "load", "kind": "twistlock", "media": ["load"], "halves": "male-female", "make": "manual", "standard": "ISO 1161 corner fittings; twistlocks as ISO 3874"}
conn("corner-casting-iso1161", {
    "identity": {"key": "connector.corner-casting-iso1161", "name": "Corner casting, ISO 1161", "description": "The cast steel box at each of a container's eight corners, an oval hole in three faces: the female half every lock, lashing and spreader takes hold of."},
    "kind": "twistlock", "size": 0.178, "rating": {"force": 250000.0},
    "physical": {"length": 0.178, "width": 0.162, "height": 0.118, "mass": 10.5},
    "built_of": [part("Casting", A36, 10.5, "cast steel")], "provides": "female", "template": "template.corner-casting",
    "basis": [{"of": ["physical"], "tier": "sourced", "review": True, "source": "ISO 1161 outside (178 x 162 x 118 mm), from memory", "note": "Mass and the 250 kN a lock from memory of the standard's lift and lashing loads."}]}, TL)
conn("twistlock-iso1161", {
    "identity": {"key": "connector.twistlock-iso1161", "name": "Twistlock, ISO 1161", "description": "The cone a quarter turn locks into a corner casting's hole, by hand lever or by itself as the box comes down: what holds a container to the next."},
    "kind": "twistlock", "size": 0.178, "rating": {"force": 250000.0},
    "physical": {"length": 0.16, "width": 0.15, "height": 0.2, "mass": 6.46},     # an inter-box lock: 160 x 90 mm spacer, the lever out to +Z (invented)
    "built_of": [part("Cones, shaft and lever", "material.high-strength-steel-4340", 6.46, "forged steel; a double-ended inter-box lock")], "provides": "male", "template": "template.twistlock",
    "basis": [{"of": ["physical"], "tier": "derived", "review": True, "note": "From blender's model (2026-10-18): solid 160 x 90 x 28 mm spacer with bearing and lever slot, 36 mm shaft, 118 x 58 mm tapered heads, 12 mm lever; turns 0 to 90 degrees in the casting clear."}, {"of": ["rating"], "tier": "invented", "review": True, "note": MEMORY}]}, TL)

# ------------------------------------------------------------------ templates
TEMPLATES = {
    "hose": ("hose", "sweep", "a round rubber hose of the record's outside", ["outside"], "Rubber hoses, PN 16"),
    "hose-hydraulic": ("hose", "sweep", "a round hose of the record's outside, its ferrules at the ends", ["outside"], "Two-braid hydraulic hoses"),
    "hose-metal": ("hose", "sweep", "a corrugated, braided round hose of the record's outside", ["outside"], "Corrugated stainless metal hoses"),
    "cable-flexible": ("cable", "sweep", "a round rubber-sheathed cable of the record's outside", ["outside"], "Flexible trailing and reeling cables"),
    "conductor-rail": ("conductor-rail", "sweep-and-pieces", "the housing's section, its width and height the record's outside", ["outside"], "Enclosed conductor rails"),
    "festoon": ("festoon", "sweep-and-pieces", "the C-track swept; trolleys every 1.5 m, the cable hung in loops between them", ["outside"], "Cable festoons on C-track"),
    "camlock": ("quick-coupling", "model", None, ["physical"], "Cam-lock hose couplings"),
    "flat-face": ("quick-coupling", "model", None, ["physical"], "Flat-face hydraulic couplings"),
    "union": ("union", "model", None, ["physical"], "Screwed hose unions"),
    "plug-socket": ("plug-socket", "model", None, ["physical"], "IEC 60309 plugs and sockets"),
    "single-pole": ("plug-socket", "model", None, ["physical"], "Single-pole power connector sets"),
    "coupler": ("plug-socket", "model", None, ["physical"], "High-voltage plug couplers"),
    "collector": ("collector", "model", None, ["physical"], "Conductor rail collector trolleys"),
    "cable-reel": ("cable-reel", "model", None, ["physical"], "Motor-driven cable reels"),
    "valve-ball": ("valve", "model", None, ["physical"], "Flanged ball valves"),
    "valve-butterfly": ("valve", "model", None, ["physical"], "Flanged butterfly valves"),
    "valve-gate": ("valve", "model", None, ["physical"], "Flanged gate valves"),
    "valve-check": ("valve", "model", None, ["physical"], "Swing check valves"),
    "valve-relief": ("valve", "model", None, ["physical"], "Relief and safety valves"),
    "expansion-joint": ("expansion-joint", "model", None, ["physical"], "Flanged bellows expansion joints"),
    "corner-casting": ("twistlock", "model", None, ["physical"], "ISO 1161 corner castings"),
    "twistlock": ("twistlock", "model", None, ["physical"], "Twistlocks"),
}


def templates():
    members = {}
    for folder, slug, d in OUT:
        if d.get("template"):
            members.setdefault(d["template"], []).append(d["identity"]["key"])
    for t, (fam, draws, profile, scales, name) in TEMPLATES.items():
        ms = members.get(f"template.{t}", [])
        sized = sorted(ms, key=lambda k: next((d.get("size") or 0) for _, _, d in OUT if d["identity"]["key"] == k))
        d = {"identity": {"key": f"template.{t}", "name": name, "description": f"{name}: one look for every size of the family, scaled to each record's figures (the owner, 2026-10-17: never one model per size)."},
             "family": fam, "draws": draws, "scales_by": scales, "members": len(ms)}
        if draws != "sweep" and sized:
            d["reference"] = sized[(len(sized) - 1) // 2]
        if profile:
            d["profile"] = profile
        if draws == "sweep-and-pieces":
            d["pieces"] = [{"name": "support", "how": "at each support.spacing along the run, scaled to its outside"}]
        if fam == "conductor-rail":   # hung from above: the head clasps the housing, the game stretches the rod up to the ceiling
            d["pieces"] = [{"name": "support_head", "how": "a hanger clasping the housing at each support.spacing, the slot underneath clear; its child leg_top is the rod's seat above, envelope at the reference rail (invented)", "length": 0.06, "width": 0.11, "height": 0.2, "mass": 1.2},
                           {"name": "support_leg", "how": "a threaded rod 1 m long from leg_top up to y = 1; the game stretches it to the ceiling (support.per_m_height)"}]
        if fam == "festoon":          # the same 40 x 40 C-track for every member: its pieces drawn at their own size, never by outside (its height is the loops' depth)
            d["pieces"] = [{"name": "support_head", "how": "a bracket clasping the C-track from above at each support.spacing, the slot underneath clear; its child leg_top is the rod's seat above (blender's model)", "length": 0.08, "width": 0.12, "height": 0.1, "mass": 1.02},
                           {"name": "support_leg", "how": "a rod 1 m long from leg_top up to y = 1; the game stretches it to the ceiling (support.per_m_height)"},
                           {"name": "trolley", "how": "every 1.5 m along the track, four wheels inside it, a neck down through the slot to a cable saddle; its child cable_anchor is where the game hangs each loop's end (invented)", "length": 0.12, "width": 0.1, "height": 0.15, "mass": 2.1}]
        d["basis"] = [{"of": ["draws", "scales_by"], "tier": "derived", "note": "The owner, 2026-10-17: one template a family, scaled; the catalogue families (kit_catalogue.py)."}]
        yield "templates", t, d


# ------------------------------------------------------------------ stock: how every kit item is sold and hauled
COIL_KINDS = {"hose", "cable", "festoon"}


def kit_stock():
    """A stock record for every run and connector in the registry (laid by plants.py or catalogued here)."""
    recs = {}
    for f in glob.glob(os.path.join(M, "runs", "*.yaml")) + glob.glob(os.path.join(M, "connectors", "*.yaml")):
        d = yaml.safe_load(open(f)) or {}
        recs[d["identity"]["key"]] = d
    for folder, slug, d in OUT:
        if folder in ("runs", "connectors"):
            recs[d["identity"]["key"]] = d
    for key, d in sorted(recs.items()):
        kind, slug = key.split(".", 1)
        mats = []
        for p_ in (d.get("per_m") or d).get("built_of") or []:
            it = p_.get("item")
            if it and it.startswith("material.") and it not in mats:
                mats.append(it)
        name = d["identity"]["name"]
        if kind == "run":
            k = d["kind"]
            if k in COIL_KINDS:
                L = 500.0 if k == "cable" and d["per_m"]["mass"] < 25 else 100.0 if k in ("cable", "festoon") else 20.0
                form, how = "coil", f"a {L:g} m {'drum' if k == 'cable' else 'coil'}"
            else:
                L = 6.0
                form, how = "length", "a 6 m length"
            mass = d["per_m"]["mass"] * L
            o = d.get("outside") or [0, 0]
            size = {"length": L, "diameter": o[0]} if o[0] == o[1] else {"length": L, "width": o[0], "height": o[1]}
            code = "KIT-" + slug.upper().replace(".", "P")
            st = {"identity": {"key": "stock." + code.lower(), "code": code, "name": f"{name}, {how}", "traded_as": "market.machinery", "form": form},
                  "made_from": [{"item": m_} for m_ in mats] or [{"item": A36}], "size": size, "supplies": {"item": key, "length": L},
                  "physical": {"mass": r(mass)},
                  "basis": [{"of": ["size", "physical.mass"], "tier": "derived", "note": f"{how} of {key}: its mass a metre times the length sold."}]}
        else:
            ph = d["physical"]
            code = "KIT-" + slug.upper().replace(".", "P")
            st = {"identity": {"key": "stock." + code.lower(), "code": code, "name": name, "traded_as": "market.machinery", "form": "piece"},
                  "made_from": [{"item": m_} for m_ in mats] or [{"item": A36}], "size": {"length": ph["length"], "width": ph["width"], "height": ph["height"]},
                  "supplies": {"item": key}, "physical": {"mass": ph["mass"]},
                  "basis": [{"of": ["size", "physical.mass"], "tier": "derived", "note": f"One {key}, as made."}]}
        st = {"identity": st["identity"], "package": "package." + PACKAGE_OF[st["identity"]["form"]], **{k: v for k, v in st.items() if k != "identity"}}
        yield "mill-stock", code, st


KEEP = ("visual", "model_requests")      # (what the model installer and the asset tools write into a record: kept across a rerun)


def write(folder, slug, d, schema):
    path = os.path.join(M, folder, slug + ".yaml")
    if os.path.exists(path):
        old = yaml.safe_load(open(path, encoding="utf-8")) or {}
        d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
    text = f"# yaml-language-server: $schema=../../schema/{schema}.schema.yaml\n" + HEAD + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
    if not os.path.exists(path) or open(path).read() != text:
        open(path, "w").write(text)
        return 1
    return 0


def main():
    n = 0
    for folder, slug, d in OUT:
        n += write(folder, slug, d, {"runs": "run", "connectors": "connector"}[folder])
    for slug, d in IFS.items():
        if os.path.exists(os.path.join(M, "interfaces", slug + ".yaml")) and "kit_catalogue" not in open(os.path.join(M, "interfaces", slug + ".yaml")).read():
            continue        # (an interface the plant kit already writes: a flange's)
        n += write("interfaces", slug, d, "interface")
    for folder, slug, d in templates():
        n += write(folder, slug, d, "template")
    k = 0
    for folder, slug, d in kit_stock():
        k += write(folder, slug, d, "mill-stock")
    print(f"{len(OUT)} kit records, {len(IFS)} interfaces, {len(TEMPLATES)} templates; {n} written; stock {k} written")


if __name__ == "__main__":
    main()
