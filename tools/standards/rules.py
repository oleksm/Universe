"""Say it once (registry audit, step 2): the notes copied into hundreds of records become rules, stated once under
standards/SFO/metadata/rules/, and the records' basis entries point at them (`rule: rule.<slug>`), keeping as `note` only what
is the record's own.

    python3 tools/standards/rules.py

Idempotent: a basis line already pointing at a rule is left alone. Run from the repository root; then the build.
"""
import glob, os, re
import yaml

S = "standards/SFO/"; R = S + "metadata/rules/"
q = lambda s: '"' + s.replace('"', '\\"') + '"'


def edit(path, fn):
    s = open(path, encoding="utf-8").read(); t = fn(s)
    if t != s:
        if path.endswith(".yaml"): yaml.safe_load(t)
        open(path, "w", encoding="utf-8").write(t)


def schemas():
    def common(s):
        if "rule: { type: string, x-ref: [rule]" in s: return s
        s = s.replace("      required: [of, tier]\n      properties:\n        of: { type: array, minItems: 1, items: { type: string } }\n        tier: { $ref: \"dictionary.schema.yaml#/definitions/tier\" }\n",
                      "      required: [of]\n      anyOf: [{ required: [tier] }, { required: [rule] }]\n      properties:\n        of: { type: array, minItems: 1, items: { type: string } }\n        tier: { $ref: \"dictionary.schema.yaml#/definitions/tier\" }\n        rule: { type: string, x-ref: [rule], description: \"A rule stated once (SFO/metadata/rules) that this entry rests on: its tier, source and review are the rule's unless given here. The record's note says only what is its own.\" }\n")
        return s
    edit("standards/common.schema.yaml", common)
    if not os.path.exists(S + "schema/rule.schema.yaml"):
        open(S + "schema/rule.schema.yaml", "w").write('''$schema: http://json-schema.org/draft-07/schema#
title: Rule
x-kind: [rule]
description: "standards/SFO/metadata/rules/<name>.yaml: a rule of the registry stated once, that many records' figures rest on: a service life, a cutting allowance, how a generated part is fitted, a margin. A record's basis entry names it (`rule:`) instead of repeating its text; the rule carries the tier, the source and the review flag for all of them. Change the rule and every record that rests on it has changed."
type: object
additionalProperties: false
required: [identity, statement, tier]
properties:
  identity:
    type: object
    additionalProperties: false
    required: [key, name, about]
    properties:
      key: { $ref: "../../common.schema.yaml#/definitions/key" }
      name: { type: string }
      about: { type: string, description: "What it decides, in a sentence." }
  statement: { type: string, description: "The rule itself, as the records' notes used to say it." }
  value: { type: number, description: "Its figure, where it has one (a life in seconds, a share)." }
  unit: { type: string, description: "The figure's unit." }
  tier: { $ref: "../../dictionary.schema.yaml#/definitions/tier" }
  source: { type: string, description: "Where the figure comes from, for a sourced or derived rule; a file in standards/sources or a named document." }
  review: { type: boolean, description: "Chosen, to be reviewed." }
  revision:
    $ref: "../../common.schema.yaml#/definitions/record_revision"
    default: { status: review, owner: registry, on_conflict: propose, because: "A rule many records rest on; change it with a reason and every record follows.", ask: docs/registry-ssot-request.md }
''')


RULES = {
    "life-ship-system": ("Life of a ship's system", "How long a piece of ship equipment serves before it is replaced.", "Fifteen years of service for a ship's system, as a ship's machinery is written off on Earth; then it is replaced, and its stock comes back as scrap. Chosen; the user's rule that everything made has a life.", 473364000, "s", "invented", None, True),
    "life-plant": ("Life of industrial plant", "How long an industrial module serves before it is rebuilt.", "Twenty-five years for industrial plant, as plant is depreciated on Earth; then it is rebuilt. Chosen; the user's rule that everything made has a life.", 788940000, "s", "invented", None, True),
    "life-building": ("Life of a building", "How long a building's structure is designed to stand.", "50 years: the working life building structures are designed for (EN 1990, category 4).", 1577880000, "s", "sourced", "standards/sources/research_people_plant.json", False),
    "life-hull": ("Life of a hull", "How long a ship's frame serves before it is scrapped.", "Forty years for a hull, a ship's frame; then it is scrapped. Chosen; the user's rule that everything made has a life.", 1262304000, "s", "invented", None, True),
    "cut-loss": ("Cutting loss", "How much of its stock a part cut from sheet, plate or tube wastes.", "A part is taken as cut from its stock with 15% lost: the allowance for nesting and kerf. Nesting on a cutting table uses 60 to 70% of a sheet by hand and 82 to 90% automated (research_yard.json, sheet nesting); 15% sits at the better end, chosen for a yard with automated nesting. What a part is made of is fitted to what the registry can describe: a real one is of more materials.", 0.15, "1", "invented", "standards/sources/research_yard.json", True),
    "fitted-within": ("A generated part's box", "How a part written as a share of its whole gets its size.", "A part generated as a share of its whole is fitted within it: its box is the whole's, scaled by the cube root of its share of the weight, in the same proportions, so that the parts fit the whole. Its own shape is not drawn.", None, None, "derived", None, True),
    "first-design-parts": ("A first design in a few parts", "How the first equipment records were broken into parts.", "A first design in a few parts, their shares chosen.", None, None, "invented", None, True),
    "discovery-ii-engine-shares": ("Fusion engine shares, Discovery II", "How a fusion engine's weight is split into parts.", "Its share of the engine, as in NASA's Discovery II fusion engine (NASA/TM-2005-213559, 2005), brought to this product's weight.", None, None, "derived", "standards/sources/research_ship_equipment.json", True),
    "discovery-ii-reactor-shares": ("Fusion plant shares, Discovery II", "How a fusion power plant's weight is split into parts.", "Its share of reactor and power conversion, as in NASA's Discovery II (NASA/TM-2005-213559, 2005), brought to this product's weight.", None, None, "derived", "standards/sources/research_ship_equipment.json", True),
    "bulk-density-typical": ("Bulk density, typical", "Where a good's bulk density comes from when no source was looked up.", "A typical figure as stowed, from memory: no source was looked up. To be reviewed.", None, None, "invented", None, True),
    "plant-weights-from-memory": ("Plant weights, from memory", "Where the weight and count of an industrial module's parts come from.", "A guess, to be reviewed: its weight and how many, from memory of real plant of this kind, not from a source. The measured masses of plant of these kinds are in research_plant_weights.json, research_yard.json and research_modules.json; the records are to be reconciled with them, kind by kind (the basis note names the file's entry where one matches).", None, None, "invented", "standards/sources/research_plant_weights.json", True),
    "mounts-margin": ("Mount margins", "How the first mounts (SFO 19) were set round the equipment.", "The most of each among the equipment of this slot and class today, with a tenth more room and a quarter more weight, power and thrust. The cooling a propulsion mount gives follows the heat rule (lib.jet_heat, SFO 22). The nozzle opening is four fifths of the mount's width. A first standard: it describes what is, not what a hull should give. The attachment: its pattern by kind; each point takes the mount's weight at 3 g (a design acceleration, chosen) plus its thrust, landing or recoil load, with the margin; the same in tension for a reversal; shear half.", None, None, "invented", None, True),
    "early-list": ("The game's early equipment", "Where the outdated equipment records' figures come from.", "The game's early list (content/base/modules.ron), a rough first guess: in service, not to be balanced against; a current design is to replace it (design stage outdated).", None, None, "invented", None, True),
}

PLANT_KEYS = {"alumina-refinery": "08_alumina_refinery_bayer", "arc-furnace": "05_electric_arc_furnace", "arc-furnace-metal": "05_electric_arc_furnace", "bar-mill": "14_bar_mill", "casthouse": "10_aluminium_casthouse",
              "cold-rolling-mill": "13_cold_rolling_mill_tandem", "cutting-table": "17_laser_cutting_and_plate_rolls", "forging-press": "16_forging_press", "fusion-power-station": "21_fusion_power_plant", "general-warehouse": "22_warehouse_tanks_pumps",
              "hot-rolling-mill": "12_hot_strip_plate_mill", "hydrogen-plant": "03_water_electrolysis_hydrogen_plant", "ladle-station": "06_ladle_furnace", "machining-centre": "19_large_5_axis_machining_centre", "ore-yard": "01_bulk_ore_stockyard",
              "pellet-plant": "02_iron_ore_pellet_plant", "piercing-mill": "15_seamless_tube_plant", "potline": "09_aluminium_potline", "reheat-furnace": "11_reheat_furnace_walking_beam", "shaft-furnace": "04_direct_reduction_shaft_furnace",
              "tank-farm": "22_warehouse_tanks_pumps", "welding-bay": "18_welding_gantries_and_cranes", "ingot-yard": "07_ingot_casting", "loading-dock": "23_port_cranes"}

# note text -> (rule, a function of (path, entry) giving the record's own note, or None)
MATCH = [
    (re.compile(r"^Fifteen years of service for a ship's system.*everything made has a life\.$"), "life-ship-system", None),
    (re.compile(r"^Twenty-five years for industrial plant.*everything made has a life\.$"), "life-plant", None),
    (re.compile(r"^50 years: the working life building structures are designed for \(EN 1990, category 4\)\.$"), "life-building", None),
    (re.compile(r"^Forty years for a hull.*everything made has a life\.$"), "life-hull", None),
    (re.compile(r"^Taken as cut from this stock with 15% lost\.( What it is made of is fitted to what the registry can describe: a real one is of more materials\.)?$"), "cut-loss", None),
    (re.compile(r"^Fitted within the .*: its share of the room by weight, in the same proportions, so that the parts fit the whole\. Its own shape is not drawn\.$"), "fitted-within", None),
    (re.compile(r"^A first design in a few parts, their shares chosen\.$"), "first-design-parts", None),
    (re.compile(r"^Its share of the engine, as in NASA's Discovery II fusion engine \(2005\), brought to this product's weight\.$"), "discovery-ii-engine-shares", None),
    (re.compile(r"^Its share of reactor and power conversion, as in NASA's Discovery II \(2005\), brought to this product's weight\.$"), "discovery-ii-reactor-shares", None),
    (re.compile(r"^A typical figure( as stowed)?, from memory: no source was looked up\. To be reviewed\.$"), "bulk-density-typical", None),
    (re.compile(r"^A guess, to be reviewed: (its weight and )?how many, from memory of real plant of this kind, not from a source\.$"), "plant-weights-from-memory", lambda path, en: (lambda k: f"research_plant_weights.json: {k}" if k else None)(PLANT_KEYS.get(path.split(os.sep)[-2]))),
    (re.compile(r"^The most of each among the equipment of this slot and class today, with a tenth more room and a quarter more weight.*shear half\.$"), "mounts-margin", None),
    (re.compile(r"^The game's early list \(content/base/modules\.ron\), a rough first guess: to be reviewed and brought in properly\. Outdated, kept to be salvaged\.$"), "early-list", None),
]


def records():
    os.makedirs(R, exist_ok=True)
    for slug, (name, about, statement, value, unit, tier, source, review) in RULES.items():
        f = R + slug + ".yaml"
        body = f"# yaml-language-server: $schema=../../schema/rule.schema.yaml\nidentity:\n  key: rule.{slug}\n  name: {q(name)}\n  about: {q(about)}\nstatement: {q(statement)}\n"
        body += f"tier: {tier}\n"
        if source: body += f"source: {q(source)}\n"
        if review: body += "review: true\n"
        if not os.path.exists(f) or open(f).read() != body:
            open(f, "w").write(body)


LINE = re.compile(r"^(\s*)- (\{.*\})\s*$")


def flow(en):
    parts = [f"of: [{', '.join(en['of'])}]"]
    if "rule" in en: parts.append(f"rule: {en['rule']}")
    if "tier" in en: parts.append(f"tier: {en['tier']}")
    if "review" in en and "rule" not in en: parts.append(f"review: {'true' if en['review'] else 'false'}")
    if "source" in en: parts.append(f"source: {q(en['source'])}")
    if "note" in en: parts.append(f"note: {q(en['note'])}")
    return "{ " + ", ".join(parts) + " }"


def rewrite(path):
    s = open(path, encoding="utf-8").read()
    if "\nbasis:" not in s: return 0
    out, n, in_basis = [], 0, False
    for line in s.split("\n"):
        if re.match(r"^\S", line): in_basis = line.startswith("basis:")
        m = LINE.match(line) if in_basis else None
        if m:
            try: en = yaml.safe_load(m.group(2))
            except Exception: en = None
            if isinstance(en, dict) and "note" in en and "rule" not in en:
                for rx, rule, own in MATCH:
                    if rx.match(en["note"]):
                        new = {"of": en["of"], "rule": f"rule.{rule}"}
                        if en.get("source") and en["source"] != RULES[rule][6]: new["source"] = en["source"]
                        note = own(path, en) if own else None
                        if note: new["note"] = note
                        line = f"{m.group(1)}- {flow(new)}"; n += 1
                        break
        out.append(line)
    if n:
        t = "\n".join(out); yaml.safe_load(t); open(path, "w", encoding="utf-8").write(t)
    return n


def main():
    schemas(); records()
    total = files = 0
    for f in glob.glob("standards/**/*.yaml", recursive=True):
        if "/schema/" in f or f.endswith(".schema.yaml") or "/rules/" in f: continue
        n = rewrite(f)
        if n: total += n; files += 1
    print(f"{total} basis entries in {files} records now rest on {len(RULES)} rules")


if __name__ == "__main__":
    main()
