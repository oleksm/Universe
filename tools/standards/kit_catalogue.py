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
    for k in ("keys", "contacts"):         # (what a wire's snap checks: a port's key and its pins' contacts)
        if iface.get(k):
            i[k] = iface[k]
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
        "conductors": [{"id": c, "role": "earth" if c == "PE" else "neutral" if c == "N" else "line", **({"line": c} if c[0] == "L" else {}), "size": mm2 * 1e-6} for c in (["L1", "L2", "L3", "N"][:n] if n == 4 else ["L1", "L2", "L3"])],
        "per_m": {"mass": r(cu + sheath, 2), "built_of": [part("Conductors", CU, cu, f"{n} fine-stranded cores of {mm2} mm2"), part("Insulation and sheath", RUB, sheath, "EPDM and a tough rubber sheath")]},
        "elbow": {"radius": r(od * 7.5, 3), "mass": r((cu + sheath) * od * 7.5 * math.pi / 2, 2)},
        "connector": f"connector.{ck}", "carries": "power", "template": "template.cable-flexible",
        "basis": [{"of": ["per_m", "rating", "outside"], "tier": "invented", "review": True, "note": f"Reeling cable figures; least bend 7.5 outsides on a reel; {MEMORY}"}]})

# ------------------------------------------------------------------ small wiring: a building's circuits, a ship's harness
PTFE = "material.ptfe"
RHO[PTFE] = 2200
conn("wiring-box-lv", {
    "identity": {"key": "connector.wiring-box-lv", "name": "Wiring box, low voltage", "description": "A small steel box with terminal blocks inside and glands in its sides, where a building's installation cables end: at an outlet, a light, a switch, a machine's isolator or a distribution board's way."},
    "kind": "termination", "size": 16e-6, "rating": {"voltage": 400, "current": 63},
    "physical": {"length": 0.1, "width": 0.1, "height": 0.06, "mass": 0.35},
    "built_of": [part("Box and lid", A36, 0.25, "1 mm steel"), part("Terminal blocks and glands", PE, 0.07, "polyamide, as UHMW polyethylene here"), part("Clamps", CU, 0.03, "brass, as copper here")],
    "provides": "both", "template": "template.wiring-box",
    "basis": [{"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": MEMORY}]},
    {"slug": "wiring-box-lv", "name": "Wiring box, low voltage", "domain": "power", "kind": "termination", "media": ["power"], "halves": "male-female", "make": "bolted"})
INSTALL = [  # (volts, cores, mm2, amps clipped in air)
    (230, 3, 1.5, 16), (230, 3, 2.5, 20), (230, 3, 4, 27), (230, 3, 6, 34), (230, 3, 10, 46),
    (400, 5, 2.5, 18), (400, 5, 6, 30), (400, 5, 16, 57)]
for v, n, mm2, amps in INSTALL:
    a = mm2 * 1e-6
    dc = 1.15 * math.sqrt(4 * a / math.pi)
    ti = 0.0007 if mm2 <= 2.5 else 0.0008 if mm2 <= 6 else 0.001
    core = dc + 2 * ti
    od = core * (2.15 if n == 3 else 2.7) + 2 * 0.0012
    cu = n * a * RHO[CU]
    pe = (math.pi / 4 * od ** 2 - n * math.pi / 4 * dc ** 2) * RHO[PE] * 0.85      # (insulation, filler and sheath: the section less the copper, 15% voids)
    slug = f"cable-installation-{v}v-{n}x{tag(mm2 / 1000) if mm2 >= 1 and mm2 == int(mm2) else str(mm2).replace('.', 'p')}"
    run(slug, {
        "identity": {"key": f"run.{slug}", "name": f"Installation cable, {v} V, {n} x {mm2:g} mm2",
                     "description": f"A sheathed cable of {n} solid copper cores ({'line, neutral and earth' if n == 3 else 'three lines, neutral and earth'}) clipped along a building's frame or laid in its floors and walls: its circuits to outlets, lights and small machines, {amps} A clipped in air."},
        "kind": "cable", "size": a, "outside": [r(od, 4), r(od, 4)], "rating": {"voltage": v, "current": amps},
        "conductors": [{"id": c, "role": "earth" if c == "PE" else "neutral" if c == "N" else "line", **({"line": c} if c[0] == "L" else {}), "size": a} for c in (["L", "N", "PE"] if n == 3 else ["L1", "L2", "L3", "N", "PE"])],
        "per_m": {"mass": r(cu + pe, 3), "built_of": [part("Conductors", CU, cu, f"{n} solid cores of {mm2:g} mm2"), part("Insulation and sheath", PE, pe, "PVC or LSZH, as UHMW polyethylene here")]},
        "elbow": {"radius": r(od * 4, 3), "mass": r((cu + pe) * od * 4 * math.pi / 2, 4)},
        "connector": "connector.wiring-box-lv", "carries": "power", "template": "template.cable-installation",
        "basis": [{"of": ["per_m", "rating", "outside"], "tier": "invented", "review": True, "note": f"Wiring regulations' clipped-direct ratings and usual insulation thicknesses; least bend 4 outsides; {MEMORY}"}]})
CIRC = [  # (shell, outside m, mated length m, kg a pair, contacts, mm2 a contact at most)
    (11, 0.022, 0.05, 0.05, 7, 1.0), (17, 0.03, 0.06, 0.1, 7, 2.5), (25, 0.04, 0.075, 0.2, 4, 16), (33, 0.052, 0.095, 0.4, 2, 50)]
CIRC_IDS = "ABCD"       # contact ids: the seven-contact inserts numbered 1 to 7 (6 on the circle, 7 at the centre), the others lettered (invented)
CIRC_KG = (17, 0.06268, 0.04627, 0.00703, 0.00938)     # blender's shell 17 (2026-10-18): kg in all, aluminium, copper, inserts and seals; the other shells scaled by the old ratio to it
for sh, od, ll, kg, nc, mm2 in CIRC:
    ref = next(k for s_, _o, _l, k, *_r in CIRC if s_ == CIRC_KG[0])
    split = [x * kg / ref for x in CIRC_KG[2:]]
    kg = round(sum(split), 4)
    conn(f"circular-shell-{sh}", {
        "identity": {"key": f"connector.circular-shell-{sh}", "name": f"Circular connector, shell {sh}", "description": f"A bayonet-coupled circular connector, plug and receptacle, an aluminium shell round an insert of {nc} crimped contacts for wires up to {mm2:g} mm2: where a harness meets a unit, a ship's equipment or a bulkhead. Sealed; keyed so that only its own mates."},
        "kind": "plug-socket", "size": mm2 * 1e-6, "rating": {"voltage": 600, "current": {1.0: 8, 2.5: 15, 16: 55, 50: 120}[mm2]},
        "physical": {"length": od, "width": od, "height": ll, "mass": kg},
        "built_of": [part("Shells and coupling ring", AL, split[0], "anodised aluminium"), part("Contacts", CU, split[1], "gold-plated copper alloy, as copper here"), part("Inserts and seals", RUB, split[2], "silicone, as EPDM here")],
        "provides": "both", "template": "template.circular-connector",
        "basis": [{"of": ["physical", "built_of"], "tier": "derived" if sh == CIRC_KG[0] else "invented", "review": True, "note": "From blender's model (2026-10-18): 30 x 30 x 60 mm mated, seven contacts (six on a 10 mm circle and one at the centre), 2 mm pins, three bayonet pins." if sh == CIRC_KG[0] else f"Scaled from the shell {CIRC_KG[0]}'s measured mass by the old ratio; {MEMORY}"},
                  {"of": ["rating"], "tier": "invented", "review": True, "note": f"Of the order of military circular connectors (MIL-DTL-38999 class); {MEMORY}"}]},
        {"slug": f"circular-shell-{sh}", "name": f"Circular shell {sh}", "domain": "power", "kind": "plug-socket", "media": ["power"], "halves": "male-female", "make": "manual",
         "contacts": [{"id": c, "size": mm2 * 1e-6} for c in (CIRC_IDS[:nc] if nc != 7 else [str(j) for j in range(1, 8)])]})
WIRE = [  # (mm2, amps single in vacuum, connector shell, amps single in free air at 40 C)
    (0.5, 5, 11, 11), (1.0, 8, 11, 16), (2.5, 15, 17, 30), (6.0, 28, 25, 55), (16.0, 55, 25, 105), (50.0, 120, 33, 220)]
BUNDLE = 0.7            # a bundle of up to six in free air carries this share each (invented, of the order of wiring rules' grouping factors)
for mm2, amps, sh, amps_air in WIRE:
    a = mm2 * 1e-6
    dc = 1.2 * math.sqrt(4 * a / math.pi)          # (stranded)
    ti = 0.0003 if mm2 <= 1 else 0.0005 if mm2 <= 6 else 0.0008
    od = dc + 2 * ti
    cu = a * RHO[CU]
    ins = math.pi * (dc + ti) * ti * RHO[PTFE]
    slug = f"wire-{str(mm2).replace('.0', '').replace('.', 'p')}mm2"
    run(slug, {
        "identity": {"key": f"run.{slug}", "name": f"Hook-up wire, {mm2:g} mm2, 600 V",
                     "description": f"A single stranded, plated copper wire in thin fluoropolymer, light and good in vacuum: laid in bundles as a vehicle's or a ship's harness, or inside a unit's cabinets. {amps} A alone in vacuum; less in a bundle."},
        "kind": "cable", "size": a, "outside": [r(od, 4), r(od, 4)], "rating": {"voltage": 600, "current": amps, "current_air": amps_air, "current_air_bundle": round(amps_air * BUNDLE)},
        "conductors": [{"id": "1", "size": a}],
        "per_m": {"mass": r(cu + ins, 4), "built_of": [part("Conductor", CU, cu, f"{mm2:g} mm2 stranded, silver-plated (as copper here)"), part("Insulation", PTFE, ins, f"{ti * 1000:g} mm of PTFE")]},
        "elbow": {"radius": r(max(0.005, od * 10), 3), "mass": r((cu + ins) * max(0.005, od * 10) * math.pi / 2, 5)},
        "connector": f"connector.circular-shell-{sh}", "carries": "power", "template": "template.wire",
        "basis": [{"of": ["per_m", "rating", "outside"], "tier": "invented", "review": True, "note": f"Of the order of aerospace hook-up wire (SAE AS22759 class): `current` a single wire's in vacuum, radiation only; `current_air` alone in free air at 40 deg C, a bundle's at {BUNDLE:g} of it; least bend 10 outsides; {MEMORY}"}]})

MULTI = [  # (mm2 a core, cores, connector shell): a harness cable for one low-bus way (ships, 2026-10-18: a laid wire fills a socket once, so one cable carries the way's every contact)
    (1.0, 3, 11), (2.5, 3, 17), (6.0, 3, 25), (16.0, 3, 25), (50.0, 2, 33)]
MULTI_SHARE = 0.8        # a core of a multicore carries this share of a single wire's amps (invented: three cores in one jacket run warmer)
JACKET = 0.0003          # m of FEP jacket over the laid-up cores (invented)
WIRE_AMPS = {mm2: (amps, air) for mm2, amps, _sh, air in WIRE}


def jacketed(cores):
    """Outside and kg/m of the jacket over cores laid up together (each core (dc, ti)): a circle round them."""
    d1 = max(dc + 2 * ti for dc, ti in cores)
    od = d1 * {1: 1.0, 2: 2.0, 3: 2.155, 4: 2.414}[len(cores)] + 2 * JACKET
    return od, math.pi * (od - JACKET) * JACKET * RHO[PTFE]


for mm2, n, sh in MULTI:
    a = mm2 * 1e-6
    dc = 1.2 * math.sqrt(4 * a / math.pi)
    ti = 0.0003 if mm2 <= 1 else 0.0005 if mm2 <= 6 else 0.0008
    od, jkt = jacketed([(dc, ti)] * n)
    cu = n * a * RHO[CU]
    ins = n * math.pi * (dc + ti) * ti * RHO[PTFE]
    amps, air = WIRE_AMPS[mm2]
    slug = f"cable-dc-600v-{n}x{str(mm2).replace('.0', '').replace('.', 'p')}mm2"
    roles = ["plus", "minus", "earth"][:n]
    run(slug, {
        "identity": {"key": f"run.{slug}", "name": f"Harness cable, 600 V DC, {n} x {mm2:g} mm2",
                     "description": f"One low-voltage DC way in one jacket: {n} stranded, plated copper cores in PTFE ({', '.join(roles)} by colour) under a thin FEP jacket, a circular shell {sh} at each end. {round(amps * MULTI_SHARE)} A in vacuum; {round(air * BUNDLE)} A in free air."},
        "kind": "cable", "size": a, "outside": [r(od, 4), r(od, 4)],
        "rating": {"voltage": 600, "current": round(amps * MULTI_SHARE), "current_air": round(air * BUNDLE), "current_air_bundle": round(air * BUNDLE * BUNDLE)},
        "conductors": [{"id": c, "role": c, "size": a} for c in roles],
        "per_m": {"mass": r(cu + ins + jkt, 4), "built_of": [part("Conductors", CU, cu, f"{n} cores of {mm2:g} mm2"), part("Insulation and jacket", PTFE, ins + jkt, f"{ti * 1000:g} mm PTFE a core, {JACKET * 1000:g} mm FEP jacket (as PTFE)")]},
        "elbow": {"radius": r(max(0.01, od * 8), 3), "mass": r((cu + ins + jkt) * max(0.01, od * 8) * math.pi / 2, 5)},
        "connector": f"connector.circular-shell-{sh}", "carries": "power", "template": "template.wire",
        "basis": [{"of": ["per_m", "outside"], "tier": "derived", "note": "Each core as the hook-up wire of its size, laid up in a circle, the jacket round them: calculated, not a single wire's mass multiplied."},
                  {"of": ["rating"], "tier": "invented", "review": True, "note": f"A core at {MULTI_SHARE:g} of a single wire's amps in vacuum and at the bundle factor {BUNDLE:g} in air (the cores share one jacket); {MEMORY}"}]})
# a CAN-class data cable: a twisted pair, a return, a braid screen
a = 0.5e-6
dc = 1.2 * math.sqrt(4 * a / math.pi)
ti = 0.0003
od, jkt = jacketed([(dc, ti)] * 3)
braid = math.pi * od * 0.0001 * RHO[CU] * 0.6        # a 0.1 mm braid, 60% cover
od += 0.0004
cu = 3 * a * RHO[CU]
ins = 3 * math.pi * (dc + ti) * ti * RHO[PTFE]
run("cable-data-can", {
    "identity": {"key": "run.cable-data-can", "name": "Data cable, control bus, 2 x 0.5 mm2 pair, screened",
                 "description": "A ship's control-bus cable (can-a): a twisted pair of 0.5 mm2 cores (signal-high, signal-low), a 0.5 mm2 return, under a braid screen and an FEP jacket; a circular shell 11 at each end. A unit's data-1 to the next's data-2: the bus daisy-chains."},
    "kind": "cable", "size": a, "outside": [r(od, 4), r(od, 4)], "rating": {"voltage": 60, "current": 1},
    "conductors": [{"id": "H", "role": "signal-high", "size": a}, {"id": "L", "role": "signal-low", "size": a}, {"id": "G", "role": "return", "size": a}, {"id": "S", "role": "screen"}],
    "per_m": {"mass": r(cu + braid + ins + jkt, 4), "built_of": [part("Conductors and braid", CU, cu + braid, "three 0.5 mm2 cores and a tinned braid"), part("Insulation and jacket", PTFE, ins + jkt, "PTFE cores, FEP jacket (as PTFE)")]},
    "elbow": {"radius": r(max(0.01, od * 8), 3), "mass": r((cu + braid + ins + jkt) * max(0.01, od * 8) * math.pi / 2, 5)},
    "connector": "connector.circular-shell-11", "carries": "signal", "template": "template.wire",
    "basis": [{"of": ["per_m", "outside", "rating"], "tier": "invented", "review": True, "note": f"Of the order of aerospace CAN and data-bus cable; {MEMORY}"}]})

# ------------------------------------------------------------------ a ship's high-voltage DC bus (ships, 2026-10-18: 3 kV DC from the plants and batteries to fans, drives and lasers)
J_VAC = 2.5e6           # A/m2: copper in vacuum, radiation only (as plant_kit.J_VACUUM)
HVDC_POLE = [  # (amps, outside m, mated length m, kg a pair, mm2 at most)
    (150, 0.04, 0.12, 0.4, 50), (400, 0.055, 0.16, 1.0, 150), (650, 0.07, 0.2, 1.8, 240), (1000, 0.085, 0.24, 3.0, 400)]
HVDC_KG = (400, 0.30575, 0.13510, 0.24021)       # blender's 400 A pole (2026-10-18): kg of aluminium, copper and PTFE; the other sizes scaled by the old ratio to it
for amps, od, ll, kg, mm2 in HVDC_POLE:
    ref = next(k for a_, _o, _l, k, _m in HVDC_POLE if a_ == HVDC_KG[0])
    split = [x * kg / ref for x in HVDC_KG[1:]]
    kg = round(sum(split), 3)
    conn(f"hvdc-pole-{amps}a", {
        "identity": {"key": f"connector.hvdc-pole-{amps}a", "name": f"HV DC pole connector, {amps} A, 3 kV", "description": f"A single-pole plug and receptacle for a 3 kV DC feeder, its crimped contact up to {mm2} mm2 (copper, or aluminium on a bimetallic contact) inside a long creepage insulator and an earthed aluminium shell, bayonet-locked, keyed by polarity: two of them, plus and minus, where a feeder meets a plant, a battery, a bus box or a load."},
        "kind": "plug-socket", "size": mm2 * 1e-6, "rating": {"voltage": 3000, "current": amps},
        "physical": {"length": od, "width": od, "height": ll, "mass": kg},
        "built_of": [part("Shell and coupling ring", AL, split[0], "anodised aluminium, earthed"), part("Contact", CU, split[1], "silver-plated copper"), part("Insulator and seals", PTFE, split[2], "PTFE creepage body; silicone seals as PTFE here")],
        "provides": "both", "template": "template.hvdc-pole",
        "basis": [{"of": ["physical", "built_of"], "tier": "derived" if amps == HVDC_KG[0] else "invented", "review": True, "note": "From blender's model (2026-10-18): 55 x 55 x 160 mm mated, a 14 mm pin in a 14.2 mm socket, a long PTFE body, keyed 44 mm aluminium shells, three bayonet pins." if amps == HVDC_KG[0] else f"Scaled from the {HVDC_KG[0]} A pole's measured mass by the old ratio; {MEMORY}"},
                  {"of": ["rating"], "tier": "invented", "review": True, "note": f"Of the order of single-pole high-voltage DC connectors for electric vehicles and aircraft; {MEMORY}"}]},
        {"slug": f"hvdc-pole-{amps}a", "name": f"HV DC pole {amps} A", "domain": "power", "kind": "plug-socket", "media": ["power"], "halves": "male-female", "make": "manual",
         "keys": ["plus", "minus"], "contacts": [{"id": "1", "size": mm2 * 1e-6}]})
AIR_240 = 800.0          # A: a 240 mm2 copper core in PTFE alone in free air at 40 deg C (the conductor to about 150 deg C); others by area^0.625 (invented, of the order of cable tables)
RHO_E = {CU: 1.72e-8, AL: 2.82e-8}      # ohm m at 20 deg C
FEEDERS = [(CU, mm2) for mm2 in (25, 50, 95, 150, 240)] + [(AL, mm2) for mm2 in (50, 95, 150, 240, 400)]
for metal, mm2 in FEEDERS:
    a = mm2 * 1e-6
    k = (RHO_E[CU] / RHO_E[metal]) ** 0.5          # (the same heat a metre from the same section: aluminium carries 1/1.28 of copper's amps)
    dc = 1.15 * math.sqrt(4 * a / math.pi)
    ti, tj = 0.002, 0.0005           # m: 2 mm of PTFE for 3 kV DC with margin for vacuum's partial discharge, a 0.5 mm braid-and-jacket
    od = dc + 2 * (ti + tj)
    cond = a * RHO[metal]
    ins = math.pi * (dc + ti) * ti * RHO[PTFE] + math.pi * (od - tj) * tj * 2500     # (the jacket a tinned copper braid in FEP, about 2,500 kg/m3 together)
    amps = round(a * J_VAC * k)
    air = round(AIR_240 * (mm2 / 240) ** 0.625 * k)
    ck = next(f"hvdc-pole-{x}a" for x, *_r, m in HVDC_POLE if m >= mm2 and x >= amps)
    al = metal == AL
    slug = f"feeder-3kv-dc-{'al-' if al else ''}{mm2}mm2"
    run(slug, {
        "identity": {"key": f"run.{slug}", "name": f"Feeder, 3 kV DC, {mm2} mm2{' aluminium' if al else ''}",
                     "description": f"One pole of a ship's high-voltage DC bus: a single stranded {'aluminium' if al else 'copper'} core in 2 mm of PTFE under an earthed braid and jacket, laid in pairs (plus and minus) along the frame. {amps} A in vacuum, where only radiation cools it; {air} A alone in free air."
                     + (" Half a copper feeder's mass for the same amps, a size and a half fatter; its ends crimped on bimetallic contacts." if al else "")},
        "kind": "cable", "size": a, "outside": [r(od, 4), r(od, 4)], "rating": {"voltage": 3000, "current": amps, "current_air": air, "current_air_bundle": round(air * BUNDLE)},
        "conductors": [{"id": "1", "size": a}],
        "per_m": {"mass": r(cond + ins, 3), "built_of": [part("Conductor", metal, cond, f"{mm2} mm2 stranded" + (", 1350 grade, as 6061 here" if al else "")), part("Insulation, screen and jacket", PTFE, ins, "2 mm PTFE, a braid screen and a thin jacket (the braid's copper counted here)")]},
        "elbow": {"radius": r(od * 8, 3), "mass": r((cond + ins) * od * 8 * math.pi / 2, 4)},
        "connector": f"connector.{ck}", "carries": "power", "template": "template.feeder",
        "basis": [{"of": ["per_m", "outside"], "tier": "invented", "review": True, "note": f"Insulation from memory of medium-voltage aerospace and traction cable; least bend 8 outsides; {MEMORY}"},
                  {"of": ["rating.current"], "tier": "derived", "note": f"{J_VAC / 1e6:g} A/mm2 in copper, radiation only (plant_kit.J_VACUUM), aluminium at sqrt of the resistivities' ratio of it (the same heat a metre): one core alone; a pair laid together runs warmer."},
                  {"of": ["rating.current_air", "rating.current_air_bundle"], "tier": "invented", "review": True, "note": f"{AIR_240:g} A for 240 mm2 copper alone in free air at 40 deg C, others by area^0.625, aluminium as above; a bundle of up to six at {BUNDLE:g}; {MEMORY}"}]})

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
        {"slug": slug, "name": iname, "domain": "power", "kind": "plug-socket", "media": ["power"], "halves": "male-female", "make": "manual", "standard": std,
         **({"keys": ["L1", "L2", "L3", "N", "PE"], "contacts": [{"id": "1"}]} if slug.startswith("powerlock") else
            {"contacts": [{"id": c} for c in (["L1", "L2", "L3", "N", "PE"] if pins == 5 else ["L1", "L2", "L3"])]})})

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
VALVE_BOUND = {  # blender's measured references: (DN, A36, 304, EPDM kg, note); the type's other sizes scaled by the old ratio
    "ball": (0.05, 16.786, 1.084, 0.019, "blender's model (2026-10-18): the kit's 216 x 20 mm flanges, 8 M16 on 168 mm, a 110 mm body round an 80 mm chamber, bored 304 ball and stem, EPDM seats, A36 lever"),
    "check": (0.1, 48.260, 2.449, 0.011, "blender's model (2026-10-18): the kit's 266 x 20 mm end rings, 8 M16 on 218 mm, a 220 mm body round a 190 mm chamber, bolted bonnet, closed 304 swing disc"),
    "relief": (0.05, 26.997, 0.830, 0.0, "blender's model (2026-10-18): the kit's flanges, inline chambers meeting only at an 18 mm-radius seat, a closed 304 poppet, stem, spring and cap adjuster; metal-seated, no elastomer")}
for vt, dn, kg, ff, h, what in VALVES:
    slug = f"valve-{vt}-dn{tag(dn)}-pn10"
    vb = VALVE_BOUND.get(vt)
    if vb:
        ref = next(k for t, d, k, *_ in VALVES if t == vt and d == vb[0])
        f = (vb[1] + vb[2] + vb[3]) / ref
        parts = [round(x * kg / ref, 3) for x in vb[1:4]]
        kg = round(kg * f, 2)
    conn(slug, {
        "identity": {"key": f"connector.{slug}", "name": f"{vt.capitalize()} valve, DN {tag(dn)}, PN 10", "description": f"A flanged {vt} valve set in a line between two flange joints: {what}."},
        "kind": "valve", "size": dn, "valve": vt, "rating": {"pressure": 1.0e6, "temperature": 473.0},
        "physical": {"length": fl_od(dn), "width": r(fl_od(dn) / 2 + max(h, fl_od(dn) / 2), 3), "height": r(ff, 3), "mass": kg},     # (width: from the flange's far rim to the actuator's top, off the axis along +Z)
        "built_of": [part("Body, bonnet and flanges", A36, parts[0], "cast carbon steel, with its lever or hinge"), part("Trim", SS, parts[1], "stainless ball, disc and stem"), part("Seats", RUB, parts[2])][:3 if parts[2] else 2] if vb else [part("Body and bonnet", A36, kg * 0.75, "cast carbon steel"), part("Trim", SS, kg * 0.2, "stainless seats, stem, disc or ball"), part("Packing and seats", RUB if vt != "gate" else SS, kg * 0.05)],
        "provides": "both", "template": f"template.valve-{vt}",
        "basis": [{"of": ["physical", "built_of"], "tier": "derived", "review": True, "note": f"From {vb[4]}."} if vb and dn == vb[0] else {"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": f"Scaled from the DN {tag(vb[0])}'s measured mass by the old ratio; {MEMORY}"} if vb else {"of": ["physical", "built_of"], "tier": "invented", "review": True, "note": f"Face to face as the EN 558 series run (from memory); its height along the line; the lever, gearbox or actuator out to {h} m from the axis along +Z, inside its width; {MEMORY}"}]},
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
    "cable-installation": ("cable", "sweep", "a round sheathed cable of the record's outside", ["outside"], "Installation cables for buildings"),
    "wire": ("cable", "sweep", "a round insulated wire of the record's outside", ["outside"], "Hook-up wires for harnesses"),
    "feeder": ("cable", "sweep", "a round screened single-core cable of the record's outside", ["outside"], "3 kV DC feeders"),
    "hvdc-pole": ("plug-socket", "model", None, ["physical"], "HV DC pole connectors"),
    "wiring-box": ("termination", "model", None, ["physical"], "Low-voltage wiring boxes"),
    "circular-connector": ("plug-socket", "model", None, ["physical"], "Circular harness connectors"),
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
    from mounting import support      # (how each run is held: rule.run-support)
    for folder, slug, d in OUT:
        if folder == "runs":
            d = dict(d, support=support(d))
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
