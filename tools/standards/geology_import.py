#!/usr/bin/env python3
"""Brings the planet simulation's own words into the registry: its deposit types and rock units
(planet-sim: docs/vocabulary.json, made by its tools/vocabulary.py) become the records under
standards/Celestial/metadata/deposit-types/ and rock-units/.

The simulation says what each is, in its own keys and units. The registry adds what is its own to
say: a record's key and name, what forms it (one of the dictionary's events), the good its ore is,
and each commodity as an element or a good with its grade in kg for each kg. Those are kept from the
record that is there; a type or unit the registry has not met stops the run until it is given them
below. Where each deposit lies is the simulation's, and is not brought in.

    python3 tools/standards/geology_import.py [path to vocabulary.json]
"""
import json, os, sys
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CEL = os.path.join(ROOT, "standards", "Celestial", "metadata")
SRC = os.path.expanduser(sys.argv[1] if len(sys.argv) > 1 else "~/git/planet-sim/docs/vocabulary.json")
voc = json.load(open(SRC, encoding="utf-8"))
SOURCE = f"the planet simulation: docs/vocabulary.json of {voc['version']} (planet-sim, tools/vocabulary.py)"
q = lambda s: '"' + str(s).replace('"', '\\"') + '"'


def num(v):
    t = repr(float(f"{v:.4g}"))
    return t if "e" not in t else (t if "." in t.split("e")[0] else t.replace("e", ".0e"))


# The simulation's key for a type, and the registry's name for its record.
TYPE = {"porphyry": "porphyry", "epithermal": "epithermal", "vms": "massive-sulphide", "orogenic_gold": "orogenic-gold", "tin_tungsten": "tin-tungsten", "nickel_sulphide": "nickel-sulphide",
        "chromite": "chromite", "iron_formation": "iron-formation", "mvt": "carbonate-zinc-lead", "sedex": "shale-zinc-lead", "sediment_copper": "sediment-copper", "diamond": "kimberlite",
        "layered_cr_pge": "layered-intrusion", "layered_vti": "magnetite-seams", "sed_manganese": "sedimentary-manganese", "magnesite": "magnesite", "volcanic_sulphur": "volcanic-sulphur",
        "halite": "rock-salt", "potash": "potash", "evaporite_sulphur": "salt-dome-sulphur", "phosphorite": "phosphorite", "carbonatite": "carbonatite", "laterite_nickel": "laterite-nickel",
        "bauxite": "bauxite", "heavy_mineral_sands": "beach-sands", "lithium_brine": "salt-flat-brine", "mn_nodules": "manganese-nodules"}
UNIT = {"basement": "gneiss-basement", "arc_plutonic": "granodiorite", "carbonate": "limestone", "arc_volcanic": "andesite", "granite": "granite", "clastic": "sandstone-shale", "flood_basalt": "flood-basalt",
        "schist": "schist", "ophiolite": "ophiolite", "rift": "rift-basalt", "greenstone": "greenstone", "iron_formation": "iron-formation", "morb": "ocean-floor-basalt"}
# A unit the registry had not met: what it is, and what lays it down.
NEW_UNIT = {"greenstone": ("Old lavas of a young, hot mantle, squeezed and turned green.", ["arc"]), "iron-formation": ("Banded iron laid down in seas before the air held oxygen.", ["sea"]),
            "ocean-floor-basalt": ("The floor of the oceans, made at the ridges.", ["ridge"])}
# The simulation's commodity, as the registry has it: the item, and what to multiply its figure by to
# get kg of that item in each kg of ore (an oxide's share that is the element; % and g/t to a share).
PCT, GT = 1e-2, 1e-6
ITEM = {"Cu %": ("element.cu", PCT), "Mo %": ("element.mo", PCT), "Au g/t": ("element.au", GT), "Ag g/t": ("element.ag", GT), "Zn %": ("element.zn", PCT), "Pb %": ("element.pb", PCT),
        "Ni %": ("element.ni", PCT), "Co %": ("element.co", PCT), "Sn %": ("element.sn", PCT), "W %": ("element.w", PCT), "Fe %": ("element.fe", PCT), "Mn %": ("element.mn", PCT),
        "S %": ("element.s", PCT), "Li %": ("element.li", PCT), "K %": ("element.k", PCT), "Mg %": ("element.mg", PCT),
        "Cr2O3 %": ("element.cr", PCT * 103.992 / 151.99), "TiO2 %": ("element.ti", PCT * 47.867 / 79.866), "V2O5 %": ("element.v", PCT * 101.883 / 181.88), "MgO %": ("element.mg", PCT * 24.305 / 40.304),
        "ZrO2 %": ("element.zr", PCT * 91.224 / 123.218), "K2O %": ("element.k", PCT * 78.197 / 94.196), "P2O5 %": ("element.p", PCT * 61.948 / 141.945), "Nb2O5 %": ("element.nb", PCT * 185.813 / 265.81),
        "REO %": ("element.ce", PCT * 0.814), "Al2O3 %": ("good.alumina", PCT), "NaCl %": ("good.salt", PCT), "diamond ct/t": ("good.diamonds", 0.2e-6)}
METHOD = {"ip": "induced polarisation", "em": "electromagnetics"}
REACH = {m["key"]: m["reach_m"] for m in voc["survey_methods"]}
YIELD = {"aggregate": "good.aggregate", "limestone": "good.limestone", "quartz sand": "good.silica-sand", "clay": "good.clay"}


def was(folder, slug):
    p = os.path.join(CEL, folder, slug + ".yaml")
    return (yaml.safe_load(open(p, encoding="utf-8")) or {}) if os.path.exists(p) else {}


def basis(of, note):
    return f"  - {{ of: [{', '.join(of)}], tier: sourced, review: true, source: {q(SOURCE)}, note: {q(note)} }}\n"


n = 0
for t in voc["deposit_types"]:
    slug = TYPE.get(t["key"]) or sys.exit(f"{t['key']}: a deposit type the registry has not met: give it a name in TYPE")
    old = was("deposit-types", slug)
    if not old.get("formed_by") or not old.get("ore"):
        sys.exit(f"{slug}: the registry's own part is missing (formed_by, ore): write its record first")
    name = old["identity"]["name"]
    s = f"# yaml-language-server: $schema=../../schema/deposit-type.schema.yaml\n# Brought in by tools/standards/geology_import.py from the planet simulation's vocabulary: change it there, or the\n# registry's own part (name, about, formed_by, ore, counted) here.\nidentity:\n  key: deposit-type.{slug}\n  name: {q(name)}\n" + (f"  about: {q(old['identity']['about'])}\n" if old["identity"].get("about") else "")
    s += f"sim: {{ key: {t['key']}, code: {t['code']} }}\nformed_by: {old['formed_by']}\ncarries:\n"
    split = t.get("pge_split") or {}
    for said, v in t["grades_typical"].items():
        if said == "PGE g/t":       # (the platinum metals, each by its share of them; all as platinum where the simulation gives no split)
            for el, share in (split or {"Pt": 1.0}).items():
                s += f"  - {{ item: element.{el.lower()}, grade: {num(v * GT * share)}, said_as: {q('PGE g/t, ' + el + ' ' + format(share, '.0%') if split else 'PGE g/t')} }}\n"
            continue
        if said == "REO %" and t.get("ree_split"):       # (the rare earths, each by its share of their oxides; "others" is left out)
            for el, share in t["ree_split"].items():
                if el != "others":
                    s += f"  - {{ item: element.{el.lower()}, grade: {num(v * PCT * 0.83 * share)}, said_as: {q('REO %, ' + el + ' ' + format(share, '.1%'))} }}\n"
            continue
        item, k = ITEM.get(said) or sys.exit(f"{said}: a commodity the registry has not met: give it an item in ITEM")
        s += f"  - {{ item: {item}, grade: {num(v * k)}, said_as: {q(said)} }}\n"
    if old.get("offshore"):
        s += "offshore: true\n"
    s += f"ore: {old['ore']}\nshape: {t['shape']}\nsize:\n  median: {num(t['tonnage_median_mt'] * 1e9)}\n  spread: {num(t['tonnage_spread_ln'])}\n  cap: {num(t['tonnage_cap_mt'] * 1e9)}\n"
    s += f"footprint: {{ length: {num(t['footprint_m']['length'])}, width: {num(t['footprint_m']['width'])} }}\nforms_at: {num(t['form_depth_m'])}\n"
    d = t["district"]
    s += f"cluster:\n  deposits: {num(d['deposits_mean'])}\n  along: {num(d['spread_along_grain_km'] * 1000)}\n  across: {num(d['spread_across_km'] * 1000)}\n"
    if t["survey"]:
        s += "seen_by:\n" + "".join(f"  - {{ method: {METHOD.get(m, m)}, reach: {num(REACH[m])} }}\n" for m in t["survey"])
    if old.get("counted"):
        s += "counted:\n" + "".join(f"  {k}: {v}\n" for k, v in old["counted"].items())
    s += "basis:\n" + basis(["sim", "carries", "shape", "size", "footprint", "forms_at", "cluster"] + (["seen_by"] if t["survey"] else []), "As the simulation has it. A grade is its figure put as kg in each kg; an oxide's as the element's; the rare earths and the platinum metals each by the simulation's own split, where it gives one (a rare-earth oxide is taken as 83% metal; the rare earths as cerium where there is no split).")
    if old.get("counted"):
        s += f"  - {{ of: [counted], tier: sourced, review: true, source: \"the planet simulation: brief 'World Geology' v0.3 of 2026-10-05; test world E4\", note: \"Deposits in one test world, against about how many are known on Earth.\" }}\n"
    yaml.safe_load(s)
    open(os.path.join(CEL, "deposit-types", slug + ".yaml"), "w", encoding="utf-8").write(s)
    n += 1
m = 0
for u in voc["rock_units"]:
    slug = UNIT.get(u["key"]) or sys.exit(f"{u['key']}: a rock unit the registry has not met: give it a name in UNIT")
    old = was("rock-units", slug)
    about, made_by = (old.get("identity") or {}).get("about") or NEW_UNIT[slug][0], old.get("made_by") or NEW_UNIT[slug][1]
    name = (old.get("identity") or {}).get("name") or u["name"].split(" (")[0].capitalize()
    s = f"# yaml-language-server: $schema=../../schema/rock-unit.schema.yaml\n# Brought in by tools/standards/geology_import.py from the planet simulation's vocabulary.\nidentity:\n  key: rock-unit.{slug}\n  name: {q(name)}\n  about: {q(about)}\nsim: {{ key: {u['key']} }}\nmade_by: [{', '.join(made_by)}]\n"
    s += f"physical:\n  density: {num(u['density_kg_m3'])}\n  magnetic_susceptibility: {num(u['susceptibility_si'])}\n  erodibility: {num(u['erodibility'])}\n"
    ys = [y for y in u["bulk_yields"] if y["material"] in YIELD]
    also = [y for y in u["bulk_yields"] if y["material"] not in YIELD]
    quality = lambda y: f"{y['range'][0]:g} to {y['range'][1]:g} {y['measure']}" if y.get("range") else ""
    if ys:
        s += "yields:\n" + "".join(f"  - {{ item: {YIELD[y['material']]}" + (f", quality: {q(quality(y))}" if quality(y) else "") + " }\n" for y in ys)
    if also:
        s += "also:\n" + "".join(f"  - {q(y['material'] + ' for ' + y['use'] + (' (' + quality(y) + ')' if quality(y) else ''))}\n" for y in also)
    s += "basis:\n" + basis(["sim", "physical"] + (["yields"] if ys else []) + (["also"] if also else []), "As the simulation has it.")
    yaml.safe_load(s)
    open(os.path.join(CEL, "rock-units", slug + ".yaml"), "w", encoding="utf-8").write(s)
    m += 1
print(f"{n} deposit types and {m} rock units from {SRC}")
