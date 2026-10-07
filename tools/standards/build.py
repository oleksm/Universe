#!/usr/bin/env python3
"""The standards, from their YAML, checked and built.

    python3 tools/standards/build.py

Reads `standards/`: a folder per body (named after its prefix) with `<PREFIX>.yaml`, and its
standards beside it, flat, one file each: `NNNN-<slug>.yaml`, the number its permanent id
(`SFO 12`: never reused, never changed, whatever it's later filed under). A standard's `topics`
are free tags describing it; how they'll be classified is left until patterns show.

Writes:
- `standards/index.html`: browse by topic or by number (rerun and refresh after an edit);
- (no game content: the game reads the records through crates/registry; this build writes only the page and the trackers)
- formerly: `content/base/*.ron` (generated:
  edit the YAML, not these; the game's tree is the topics for now).

Exits non-zero with every problem listed if anything's wrong (the page still shows them).
See docs/standards.md.
"""
import html
import glob
import json
import math
import os
import re
import sys

import sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import jet_heat, plant_waste, JET_KINDS

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TREE = os.path.join(ROOT, "standards")
CONTENT = os.path.join(ROOT, "content", "base")

STATUSES = ["draft", "published", "superseded", "withdrawn"]
CHECKS = ["at_most", "at_least", "equals", "fits_within", "provides"]
BODY_FIELDS = {"key", "name", "prefix", "seat", "address", "note", "kind", "purpose", "details", "founded_by", "about"}
BODY_KINDS = ["consortium", "independent", "authority", "corporation", "players"]
STANDARD_FIELDS = {"version", "title", "parent", "records", "purpose", "details", "status", "topics", "scope", "sections", "refs", "params", "requires", "text", "licence", "published"}

problems = []


def problem(where, what):
    problems.append(f"{os.path.relpath(where, ROOT)}: {what}")


# Every record has one key, <kind>.<name> (see validate.py and standards/common.schema.yaml). Until
# the game loads records by it, this build still works by file names and the game's old keys, so a
# key written at a record's top is taken off as it is read and kept here; a company's and the
# standards body's stand as the game still has them (brand.x, body.x).
REGISTRY_KEY = {}
OLD_KEY = {"company": "brand", "band": "brand", "standards_body": "body"}        # (an organisation's old key, by its kind)

# Records name each other by key (a property marked x-ref in its schema). This build still works by
# what they named each other by before: a file name, a part's code, an element's symbol, a body's
# name, brand.x. So each key is turned back as a record is read. REGISTRY: every record, by its key.
import validate as V
REGISTRY = {}
for _dp, _dns, _fns in os.walk(TREE):
    _dns[:] = [d for d in _dns if d not in ("schema", "sources", "logos", "icons")]
    for _fn in _fns:
        _rel = os.path.relpath(os.path.join(_dp, _fn), TREE)
        if _fn.endswith(".yaml") and not _fn.endswith(".schema.yaml") and os.sep in _rel and V.schema_named(os.path.join(_dp, _fn)):
            try:
                _rec = yaml.safe_load(open(os.path.join(_dp, _fn), encoding="utf-8")) or {}
            except yaml.YAMLError:
                continue
            if isinstance(V.key_in(_rec), str):
                REGISTRY[V.key_in(_rec)] = (_rel, _rec)


REGISTRY_SYSTEM = {rec["identity"]["name"]: key for key, (_r, rec) in REGISTRY.items() if key.startswith("system.")}


def old_name(key, rel, at):
    """What a record was named by before keys, for the record at `rel` naming it at `at`."""
    if key not in REGISTRY:
        return key
    krel, rec = REGISTRY[key]
    kind, _, rest = key.partition(".")
    idn = rec.get("identity") if isinstance(rec.get("identity"), dict) else rec
    stem = os.path.basename(krel)[:-5]
    if kind == "element":
        return idn.get("symbol")
    if kind == "org":
        return OLD_KEY[rec["kind"]] + "." + rest.replace("-", "_") if rec.get("kind") in OLD_KEY else stem
    if kind in ("body", "system"):
        return idn.get("name")
    if kind == "rock-class":
        return (idn.get("label") or stem).lower() if at == "rocks.class" or os.sep + "bodies" + os.sep in rel else stem   # (the game's own go by its label)
    if kind == "standard":
        return "SFO " + rest.split(".")[-1]
    if kind in ("settlement", "rig"):
        return krel.split(os.sep)[3] + "/" + stem
    if kind == "parcel":
        return rec.get("number")
    return stem


# Records hold every value in SI (kg, m, s, W, N, Pa; angles in degrees), each property's unit in its
# schema as x-unit. This build and the page still work in the units people read (t, km, hours, AU):
# reading_units.yaml says which, for each property, and a value is turned to it as a record is read.
READING = yaml.safe_load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "reading_units.yaml"), encoding="utf-8"))


def reading(v, path, tab):
    if isinstance(v, dict):
        return {k: reading(x, f"{path}.{k}" if path else str(k), tab) for k, x in v.items()}
    if isinstance(v, list):
        return [reading(x, path + "[]" if isinstance(x, dict) else path, tab) for x in v]
    if path in tab and isinstance(v, (int, float)) and not isinstance(v, bool):
        per = tab[path]["per"]
        r = float(f"{v / per:.12g}")
        return int(r) if r == int(r) and abs(r) < 1e15 and (isinstance(v, int) or per < 1 or per != int(per)) else r
    return v


def flatten_schema(node, path):
    """A schema with every object's `allOf` shapes flattened into its properties and required, as the validators and the
    generator read them (a shape's own `$ref`s are left relative to its file; this build reads names and descriptions)."""
    if isinstance(node, list):
        return [flatten_schema(x, path) for x in node]
    if not isinstance(node, dict):
        return node
    node = {k: flatten_schema(v, path) for k, v in node.items()}
    shapes = node.pop("allOf", None)
    if shapes and (node.get("type") == "object" or "properties" in node):
        import copy
        props = dict(node.get("properties") or {}); required = list(node.get("required") or [])
        for shape in shapes:
            spath = path
            if isinstance(shape, dict) and "$ref" in shape:
                shape, spath = V.resolve(shape["$ref"], path)
                shape = copy.deepcopy(shape)
            shape = flatten_schema(shape, spath)
            for k, pv in (shape.get("properties") or {}).items():
                if k in props or not isinstance(pv, dict) or not any(x in pv for x in ("type", "$ref", "oneOf", "enum", "properties", "const")):
                    continue
                props[k] = pv
            for r in shape.get("required") or []:
                if r not in required and r in props:
                    required.append(r)
        node["properties"] = props
        if required:
            node["required"] = required
    elif shapes:
        node["allOf"] = shapes
    return node


def read_schema(path):
    """A schema, its descriptions in the units the page reads."""
    sch = flatten_schema(yaml.safe_load(open(path, encoding="utf-8")), path)
    props = sch.get("properties") or {}
    import copy
    for g in ("physical", "made_from", "making"):
        # (Groups every made thing shares, in common.schema.yaml: read here in full.)
        if "$ref" in (props.get(g) or {}):
            props[g] = copy.deepcopy(V.resolve(props[g]["$ref"], path)[0])
    for at, how in (READING.get(os.path.relpath(path, TREE)) or {}).items():
        node = sch
        for step in at.split("."):
            node = (node.get("properties") or {}).get(step.replace("[]", "")) if isinstance(node, dict) else None
            if step.endswith("[]") and isinstance(node, dict):
                node = node.get("items")
        if isinstance(node, dict):
            node["description"] = how["reads"]
    kind = os.path.basename(path)[:-12]
    if kind == "equipment":
        for g in ("function", "needs", "size_class"):
            props.pop(g, None)
        props["performance"] = {"type": "object", "additionalProperties": False, "properties": {k: {"description": v} for k, v in EQUIPMENT_READS.items()}}
        sch["properties"] = props = {k: props[k] for k in ("identity", "physical", "performance", "built_of", "making", "fits", "life", "revision", "basis") if k in props}
    # (This build and the page still take what a thing is made from as one entry, a mill stock's form and
    # temper with it, and one `process` where there is one.)
    if "made_from" in props and kind in ("part", "mill-stock"):
        one = props["made_from"]["items"]
        one["description"] = props["made_from"].get("description", "")
        if kind == "mill-stock":
            idn = props["identity"]["properties"]
            one["properties"] = {"material": one["properties"]["item"], "form": idn.pop("form"), "temper": idn.pop("temper")}
        props["made_from"] = one
    # (And a module's one recipe as its own figures, as they were before recipes: rate, inputs, outputs, needs.)
    if kind == "module" and "recipes" in props:
        props.pop("throughput", None)
        R, cap = props.pop("recipes")["items"]["properties"], props.pop("capacity")["properties"]
        group = lambda d: {"type": "object", "additionalProperties": False, "properties": d}
        was = {"rate": group({"throughput": R["rate"], "batch": R["batch"], **cap, "product": R["makes"], "power": props.pop("generation")["properties"]["supplies"]}),
               "inputs": group({"materials": {**R["inputs"], "description": "What goes in, t per t of its product."}}),
               "outputs": group({"by_products": {**R["outputs"], "description": "What else comes out, t per t of its product."}}),
               "needs": group({"power": R["power"]})}
        new = {}
        for k, v in props.items():
            if k == "needs":
                new.update(was)
            else:
                new[k] = v
        sch["properties"] = props = new
    # (And a hull's and a module's size and mass as they were grouped before: `size`, and a hull's `mass`.)
    if kind in ("hull", "module") and "physical" in props:
        ph, cap = props["physical"]["properties"], (props.get("capacity") or {}).get("properties") or {}
        group = lambda d: {"type": "object", "additionalProperties": False, "properties": d}
        size = {k: ph[k] for k in ("length", "width", "height")}
        new = {}
        for k, v in props.items():
            if k == "physical":
                new["size"] = group(size if kind == "module" else {**size, "volume": ph["volume"], "hold_volume": cap["hold_volume"]})
                if kind == "hull":
                    new["mass"] = group({"frame": ph["mass"], "fuel": cap["fuel"], "hold": cap["hold"]})
            elif k != "capacity":
                new[k] = v
        sch["properties"] = new
    return sch


# A piece of equipment's `function` (its kind, and that kind's figures in SI), its `needs` and its size
# class, as this build and the page still take them: one `performance` group, in the units people read.
EQUIPMENT_READS = {
    "kind": "What kind of device it is.", "size_class": "Its size class, 1 to 4: it fits a slot at least as big.",
    "power_draw": "kW, working", "output": "MW it supplies", "efficiency": "the share of its fuel's or its draw's energy it turns to use",
    "thrust": "kN, of one nozzle at full share", "exhaust_speed": "km/s", "burns": "What it burns.", "capacity": "kg it holds (a capacitor: J)", "rate": "W it can take in or give out",
    "holds": "What it holds.", "seats": "how many it seats", "top_speed": "times the speed of light", "turn_rate": "rad/s it can turn the ship at", "roll_rate": "rad/s it can roll the ship at",
    "range": "km", "capture": "km, how far it hears", "link": "km, how far it reaches another", "lag": "s to pass a message on", "messages": "messages an hour",
    "features": "What it can do.", "interlock": "m it holds the ship from a body's ground in hyperdrive", "governor": "1/s, its hyperdrive governor", "cadence": "s between its throws",
    "resolves": "a survey makes out a rock at this many times its size", "survey_range": "m, the farthest a survey reaches",
    "muzzle_speed": "m/s", "slug_mass": "kg", "magazine": "rounds", "beam_power": "W on the target", "focus": "m its beam holds together", "burn": "s of firing to too hot", "cool": "s to cool",
    "excavator_power": "W it cuts with", "throughput": "kg/s of spoil at most", "anchor_reach": "m", "anchor_speed": "m/s it holds below",
    "stroke": "m its strut compresses over", "sink_rate": "m/s, the touchdown it is designed for", "extended": "m, mount to pad, gear down",
    "volume": "m3 it holds, heaped", "fill_density": "kg/m3 of broken rock its capacity is reckoned at", "reset": "the share of too hot it cools to before firing again", "heat_to_hull": "the share of the jet's power that reaches the hull as heat", "water_recovery": "the share of water recovered", "air_recovery": "the share of oxygen won back",
    "persons": "how many it cycles at once", "cycle": "s a cycle (an airlock); how it makes its jet (an engine)", "passage": "m, the clear way", "air_lost": "kg of air lost a cycle", "load": "kg it bears or lifts", "width": "m", "height": "m", "reach": "m", "travel": "m", "opens_in": "s to open",
    "propellant": "What it throws.", "throttle": "the least thrust it holds, as a share", "gimbal": "rad it steers its jet", "isotropic_loss": "the share of its power leaving in every direction", "shield_pass": "the share of that its shield lets through", "reaction_offset": "m aft to the reaction", "turns": "rad it turns through", "bears": "N it carries", "slew": "rad/s it turns at",
    "g_rating": "m/s2 its seats are rated to", "facing": "which way the seats face", "cooling": "W of cabin heat it carries away", "protects": "m3 of room one unit covers", "accuracy": "m, how closely it reads", "field_of_view": "rad across its picture", "endurance": "s of air a suit carries",
    "standard": "the docking standard", "rejects": "W of heat thrown off", "temperature": "K, its working surface", "area": "m2", "transfers": "W passed", "carries": "W", "flow": "kg/s", "stores": "J", "head": "Pa", "pressure": "Pa", "torque": "N m", "momentum": "N m s",
}


def equipment_view(rec):
    f, needs = rec.pop("function", None) or {}, rec.pop("needs", None) or {}
    kind = f.get("kind")
    perf = {"kind": (kind or "").replace("_", " ")}
    if "size_class" in rec:
        perf["size_class"] = rec.pop("size_class")
    if "power" in needs:
        perf["power_draw"] = needs["power"] / 1e3
    g = lambda v, per: float(f"{v / per:.12g}")
    for k, v in f.items():
        if k == "kind":
            continue
        if k == "output":
            perf["output"] = g(v, 1e6)
        elif k == "thrust":
            perf["thrust"] = g(v, 1e3)
        elif k == "exhaust":
            perf["exhaust_speed"] = g(v, 1e3)
        elif k == "top_speed":
            perf["top_speed"] = g(v, 299792458.0)
        elif k in ("range", "capture", "link"):
            perf[k] = g(v, 1e3)
        elif k == "capacity" and kind in ("comm", "gate_relay", "hyper_relay"):
            perf["messages"] = g(v * 3600, 1)
        else:
            perf[k] = v
    new = {}
    for k, v in rec.items():
        new[k] = v
        if k == "physical":
            new["performance"] = perf
    if "performance" not in new:
        new["performance"] = perf
    for b in new.get("basis") or []:
        if isinstance(b, dict) and isinstance(b.get("of"), list):
            b["of"] = list(dict.fromkeys({"needs": "performance", "function": "performance", "size_class": "performance"}.get(x, x) for x in b["of"]))
    return new


def old_groups(rec, rel):
    """A hull's or a module's `physical` and `capacity`, as this build still takes them: `size` and `mass`."""
    kind = rel.split(os.sep)[2] if rel.count(os.sep) >= 3 else ""
    if kind not in ("hulls", "modules") or not isinstance(rec, dict):
        return rec
    ph, cap = rec.get("physical") or {}, rec.get("capacity") or {}
    size = {k: v for k, v in ph.items() if k != "mass"}
    if "hold_volume" in cap:
        size["hold_volume"] = cap["hold_volume"]
    mass = {**({"frame": ph["mass"]} if "mass" in ph else {}), **{k: cap[k] for k in ("fuel", "hold") if k in cap}}
    new = {}
    for k, v in rec.items():
        if k in ("physical", "capacity"):
            if "size" not in new and size:
                new["size"] = size
            if "mass" not in new and mass and kind == "hulls":
                new["mass"] = mass
        else:
            new[k] = v
    back = {"physical": "size", "physical.mass": "mass.frame", "capacity.hold_volume": "size.hold_volume", "capacity.fuel": "mass.fuel", "capacity.hold": "mass.hold"}
    for b in new.get("basis") or []:
        if isinstance(b, dict) and isinstance(b.get("of"), list):
            b["of"] = [back.get(x, "size" + x[8:] if x.startswith("physical.") else x) for x in b["of"]]
    rec.clear()
    rec.update(new)
    return rec


def old_names(rec, path, schema=None):
    rel = os.path.relpath(os.path.abspath(path), TREE)
    sp = schema or (V.schema_named(os.path.abspath(path)) if os.sep in rel and os.path.exists(path) else None)
    if sp and isinstance(rec, dict) and os.path.relpath(sp, TREE) in READING:
        for k, x in reading(rec, "", READING[os.path.relpath(sp, TREE)]).items():
            rec[k] = x
    if sp and isinstance(rec, dict):
        for holder, i, _kinds, at in list(V.refs(rec, V.schema(sp), sp)):
            if isinstance(holder[i], str):
                holder[i] = old_name(holder[i], rel, at)
        # (A material's part that is no record is written `name`; this build reads `part`.)
        for c in (rec.get("identity") or {}).get("composition") or [] if rel.startswith(os.path.join("SFO", "metadata", "materials")) else []:
            if "name" in c and "part" not in c:
                c["part"] = c.pop("name")
    if rel.startswith(os.path.join("Celestial", "metadata", "seeding")) and isinstance(rec, dict):
        # (A seeding record, as this build still takes it: its settings, with no identity; the galaxy's flat.)
        idn = rec.pop("identity", {})
        if "galaxy" in rec:
            flat = {**rec.pop("galaxy"), **({"note": idn["about"]} if "about" in idn else {})}
            rec.update(flat)
    kind = rel.split(os.sep)[2] if rel.count(os.sep) >= 3 else ""
    if isinstance(rec, dict) and kind == "equipment":
        rec = equipment_view(rec)
    if isinstance(rec, dict) and kind == "modules" and "generation" in rec:
        # (One that makes power, as this build still takes it: a recipe that supplies, its fuel as inputs.)
        gen = rec.pop("generation")
        rec = {**{k: v for k, v in rec.items() if k != "basis"}, "recipes": [{"supplies": gen["supplies"], "inputs": [{"item": b["item"], "quantity": b["rate"]} for b in gen.get("burns") or []]}], **({"basis": rec["basis"]} if "basis" in rec else {})}
        for b in rec.get("basis") or []:
            if isinstance(b, dict) and isinstance(b.get("of"), list):
                b["of"] = [{"generation": "recipes", "generation.supplies": "recipes.supplies", "generation.burns": "recipes.inputs"}.get(x, x) for x in b["of"]]
    if isinstance(rec, dict) and kind == "modules" and "throughput" in rec:
        # (A shop module, as this build still takes it: one recipe by weight, for parts (or hulls) as a whole.)
        tp = rec.pop("throughput")
        what = "hulls" if "hull" in str((rec.get("identity") or {}).get("step", "")).lower() else "parts"
        rec = {**{k: v for k, v in rec.items() if k != "basis"}, "recipes": [{"makes": what, "rate": tp["rate"], **({"power": tp["power"]} if "power" in tp else {})}], **({"basis": rec["basis"]} if "basis" in rec else {})}
        for b in rec.get("basis") or []:
            if isinstance(b, dict) and isinstance(b.get("of"), list):
                b["of"] = [y for x in b["of"] for y in {"throughput": ["recipes.rate", "recipes.power"], "throughput.rate": ["recipes.rate"], "throughput.power": ["recipes.power"]}.get(x, [x])]
    if isinstance(rec, dict) and kind == "modules" and ("recipes" in rec or "capacity" in rec):
        # (A module's recipe, as this build still takes it: the module's own rate, inputs, outputs and power.
        # It takes the first; none has more than one yet.)
        r = (rec.get("recipes") or [{}])[0]
        k_ = 3.6e9 / 1000 / (r["supplies"] * 1e6) if "supplies" in r else 1          # (kg/s at full output, back to t per MWh; its power is read in MW by now)
        amt = lambda xs: [{"item": x["item"], "amount": float(f"{x['quantity'] * k_:.12g}") if "supplies" in r else x["quantity"]} for x in xs]
        # (All its recipes, each as the build works with one: what it makes, t/h, what goes in and comes out for each t, MW.)
        every = [{"product": q.get("makes"), "does": q.get("does"), "throughput": q.get("rate"), "batch": q.get("batch"), "power": q.get("power", 0), "supplies": q.get("supplies"),
                  "inputs": [{"item": x["item"], "amount": x["quantity"]} for x in q.get("inputs") or []] if "supplies" not in q else amt(q.get("inputs") or []),
                  "outputs": [{"item": x["item"], "amount": x["quantity"]} for x in q.get("outputs") or []]} for q in rec.get("recipes") or []]
        rate = {**({"throughput": r["rate"]} if "rate" in r else {}), **({"batch": r["batch"]} if "batch" in r else {}), **(rec.get("capacity") or {}),
                **({"product": r["makes"]} if "makes" in r else {}), **({"power": r["supplies"]} if "supplies" in r else {})}
        was = {"rate": rate, "inputs": {"materials": amt(r["inputs"])} if "inputs" in r else None, "outputs": {"by_products": amt(r["outputs"])} if "outputs" in r else None,
               "needs": {"power": r["power"]} if "power" in r else rec.get("needs")}
        new, done = {}, False
        for k, v in rec.items():
            if k in ("capacity", "recipes", "needs"):
                if not done:
                    new.update({a: b for a, b in was.items() if b})
                    done = True
            else:
                new[k] = v
        back = {"capacity": "rate", "recipes.rate": "rate.throughput", "recipes.batch": "rate.batch", "recipes.makes": "rate.product", "recipes.supplies": "rate.power",
                "recipes.inputs": "inputs", "recipes.outputs": "outputs", "recipes.power": "needs"}
        for b in new.get("basis") or []:
            if isinstance(b, dict) and isinstance(b.get("of"), list):
                b["of"] = [back.get(x, x) for x in b["of"]] + [x for x in b["of"] if x.startswith("recipes.")]
        new["recipes"] = every
        rec.clear()
        rec.update(new)
    if isinstance(rec, dict) and kind in ("parts", "mill-stock", "hulls", "gates"):
        # (What it is made from, and how, as this build still takes them.)
        if isinstance(rec.get("made_from"), list) and rec["made_from"]:
            one = dict(rec["made_from"][0])
            if kind == "mill-stock":
                idn = rec.get("identity") or {}
                one = {"material": one.get("item"), **{k: idn.pop(k) for k in ("form", "temper") if k in idn}}
            rec["made_from"] = one
    return old_groups(rec, rel) if os.sep in rel else rec


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return load_text(f.read(), path)
    except yaml.YAMLError as e:
        problem(path, f"not valid YAML: {e}")
        return {}


def load_rec(e):
    """A record read again from where `load` found it: its file, or the text it was derived from."""
    return load_text(e["_text"], os.path.join(TREE, e["file"]), os.path.join(TREE, "SFO", "schema", "part.schema.yaml")) if e.get("_text") else load(os.path.join(TREE, e["file"]))


def load_text(text, path, schema=None):
    """A record from its text, as `load` reads a file (`path` names where it is, or would be; `schema` its schema where no file names one)."""
    try:
        rec = yaml.safe_load(text) or {}
    except yaml.YAMLError as e:
        problem(path, f"not valid YAML: {e}")
        return {}
    # (A record whose key and name were at its top until they moved into `identity`: this build still
    # takes them there. Local Administration's, a standard, an organisation.)
    idn = rec.get("identity") if isinstance(rec, dict) else None
    if isinstance(idn, dict) and str(idn.get("key", "")).split(".")[0] in ("settlement", "rig", "zone", "parcel", "street", "power-line", "facility", "standard", "org"):
        rec = {**idn, **{k: v for k, v in rec.items() if k != "identity"}}
    if isinstance(rec, dict) and isinstance(rec.get("key"), str) and "." in rec["key"]:
        kind, _, rest = rec["key"].partition(".")
        REGISTRY_KEY[os.path.abspath(path)] = rec["key"]
        if kind == "org":
            # (One organisation schema; this build still reads a company, the standards body and an
            # administration each in its old shape.)
            org = rec.pop("kind", None)
            if org in OLD_KEY:
                rec["key"] = OLD_KEY[org] + "." + rest.replace("-", "_")
            else:
                del rec["key"]
            if "form" in rec:
                rec["kind"] = rec.pop("form")
        else:
            del rec["key"]
    return old_names(rec, path, schema)


def check_standard(s, ids):
    where = os.path.join(TREE, s["file"])
    for k in s:
        if k not in STANDARD_FIELDS | {"id", "body", "number", "file"}:
            problem(where, f"unknown field '{k}'")
    for k in ["title"]:
        if k not in s:
            problem(where, f"no {k}")
    topics = s.get("topics") or []
    for t in topics if isinstance(topics, list) else []:
        if not isinstance(t, str) or not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*", t):
            problem(where, f"topic '{t}': lower case words joined by -")
    if not isinstance(s.get("version", 1), int) or s.get("version", 1) < 1:
        problem(where, "version: a whole number from 1")
    if s.get("status", "published") not in STATUSES:
        problem(where, f"status: one of {', '.join(STATUSES)}")
    lic = s.get("licence", "open")
    if not (lic == "open" or (isinstance(lic, dict) and set(lic) == {"fee"} and isinstance(lic["fee"], (int, float)) and lic["fee"] >= 0)):
        problem(where, "licence: open, or {fee: credits}")
    if "parent" in s:
        if s["parent"] not in ids:
            problem(where, f"parent {s['parent']}: no such record")
        seen, up = {s["id"]}, s["parent"]
        while up in ids:
            if up in seen:
                problem(where, "parent: goes round in a circle")
                break
            seen.add(up)
            up = next((x.get("parent") for x in standards if x["id"] == up), None)
    for r in s.get("refs", []) or []:
        if r not in ids:
            problem(where, f"refers to {r}: no such standard (ids are like 'SFO 12')")
        if r == s["id"]:
            problem(where, "refers to itself")
    if "purpose" in s and not isinstance(s["purpose"], str):
        problem(where, "purpose: a paragraph")
    # Details: text and/or a table, as a section is.
    if "details" in s and not isinstance(s["details"], dict):
        problem(where, "details: text and/or table")
    # Sections: titled, each paragraphs and/or a table (columns, rows of as many cells).
    for sec in ([s["details"]] if isinstance(s.get("details"), dict) else []) + (s.get("sections", []) or []):
        if not isinstance(sec, dict):
            problem(where, "a section is title, text and/or table")
            continue
        sec.setdefault("title", "")
        for extra in set(sec) - {"title", "text", "table"}:
            problem(where, f"section '{sec['title']}': unknown field '{extra}' (title, text, table)")
        text = sec.get("text", [])
        if not isinstance(text, (str, list)):
            problem(where, f"section '{sec['title']}': text is a paragraph or a list of them")
        t = sec.get("table")
        if t is not None:
            cols = t.get("columns") if isinstance(t, dict) else None
            rows = t.get("rows") if isinstance(t, dict) else None
            if not isinstance(cols, list) or not isinstance(rows, list):
                problem(where, f"section '{sec['title']}': a table has columns and rows")
            else:
                for r in rows:
                    if not isinstance(r, list) or len(r) != len(cols):
                        problem(where, f"section '{sec['title']}': a row of {len(r) if isinstance(r, list) else '?'} cells for {len(cols)} columns")
    keys = set()
    for p in s.get("params", []) or []:
        k = str(p.get("key", ""))
        if not re.fullmatch(r"[A-Za-z0-9_]+(\.[A-Za-z0-9_]+)?", k):
            problem(where, f"parameter key '{k}': a name, or ROW.column")
        if k in keys:
            problem(where, f"parameter {k} twice")
        keys.add(k)
        v = p.get("value")
        if isinstance(v, list):
            if len(v) != 2 or not all(isinstance(x, (int, float)) for x in v):
                problem(where, f"parameter {k}: a range is [at least, at most]")
            elif v[1] < v[0]:
                problem(where, f"parameter {k}: range upside down")
        elif not isinstance(v, (int, float, str)) or isinstance(v, bool):
            problem(where, f"parameter {k}: a number, a range or text")
        for extra in set(p) - {"key", "value", "unit", "note"}:
            problem(where, f"parameter {k}: unknown field '{extra}'")
    for r in s.get("requires", []) or []:
        if r.get("check") not in CHECKS:
            problem(where, f"requirement on {r.get('subject')}: check one of {', '.join(CHECKS)}")
        param = r.get("param", "")
        if not any(k == param or k.endswith("." + param) for k in keys):
            problem(where, f"requirement on {r.get('subject')}: no parameter '{param}'")
        if not r.get("subject"):
            problem(where, "a requirement without its subject")


# ---------------------------------------------------------------- reading
# Maker House: the makers, one file each (MakerHouse/metadata/makers/<name>.yaml), to
# MakerHouse/schema/company.schema.yaml. The game's
# brands.ron is written from them.
# 0. Dogma: the laws everything runs on (standards/Dogma). The engine makes its constants from these
# records when it is built; the constants this build works things out with are these too, by name.
dogma = []
_ddir = os.path.join(TREE, "Dogma", "metadata")


for _fn in sorted(os.listdir(_ddir)) if os.path.isdir(_ddir) else []:
    if not _fn.endswith(".yaml"):
        continue
    sec = load(os.path.join(_ddir, _fn))
    sec["slug"], sec["file"], sec["laws"] = _fn[:-5], os.path.relpath(os.path.join(_ddir, _fn), TREE), []
    for _ln in sorted(os.listdir(os.path.join(_ddir, _fn[:-5]))) if os.path.isdir(os.path.join(_ddir, _fn[:-5])) else []:
        law = load(os.path.join(_ddir, _fn[:-5], _ln))
        law["slug"], law["file"] = _ln[:-5], os.path.relpath(os.path.join(_ddir, _fn[:-5], _ln), TREE)
        dogma.append(sec) if sec not in dogma else None
        sec["laws"].append(law)
    if sec not in dogma:
        dogma.append(sec)
dogma.sort(key=lambda s_: (s_["identity"].get("order", 99), s_["slug"]))
_law = {l["slug"]: l["value"] for s_ in dogma for l in s_["laws"]}
C_LIGHT, SIGMA, G_N = _law["speed-of-light"], _law["stefan-boltzmann"], _law["gravitation"]
G0, AU_M, LY, DAY_S, YEAR_S = _law["standard-gravity"], _law["astronomical-unit"], _law["light-year"], _law["day"], _law["year"]
SUN_KG, SUN_W = _law["sun-mass"], _law["sun-luminosity"]

HOUSE = "MakerHouse"
house = load(os.path.join(TREE, HOUSE, "metadata", HOUSE + ".yaml"))
house = {**{k: v for k, v in house.items() if k != "identity"}, "name": (house.get("identity") or {}).get("name", house.get("name"))}      # (the page reads its name plainly)
makers = []
makers_dir = os.path.join(TREE, HOUSE, "metadata", "makers")
for name in sorted(os.listdir(makers_dir)):
    full = os.path.join(makers_dir, name)
    if not re.fullmatch(r"[a-z0-9-]+\.yaml", name):
        problem(full, "a maker's file is named <name>.yaml (lower case, words joined by -)")
        continue
    m = load(full)
    for k in ["key", "name", "ticker", "note"]:
        if k not in m:
            problem(full, f"no {k}")
    for k in m:
        if k not in {"key", "name", "ticker", "business", "address", "note", "who", "what", "story", "slug", "file", "insurance", "trade_bans", "fleet", "banking", "exchange"}:
            problem(full, f"unknown field '{k}'")
    if not re.fullmatch(r"[A-Z]{2,4}", str(m.get("ticker", ""))):
        problem(full, "ticker: 2 to 4 capital letters")
    if any(x.get("ticker") == m.get("ticker") for x in makers):
        problem(full, f"ticker {m.get('ticker')} twice")
    if not re.fullmatch(r"brand\.[a-z0-9_]+", str(m.get("key", ""))):
        problem(full, "key: brand.<name>")
    if any(x.get("key") == m.get("key") for x in makers):
        problem(full, f"{m.get('key')} twice")
    m["file"] = os.path.relpath(full, TREE)
    m["slug"] = name[:-5]
    makers.append(m)
BRANDS = {m.get("key"): m.get("name") for m in makers}

# Local Administration: each settled system's, one file each
# (LocalAdministration/metadata/administrations/<name>.yaml), to its administration.schema.yaml.
LOCAL = "LocalAdministration"
ZONE_USES = ["port", "industrial", "commercial", "civic", "residential", "agricultural", "mining"]
# What zone each kind of facility needs.
FACILITY_ZONE = {"foundry": "industrial", "mine": "mining", "mill": "industrial", "yard": "industrial", "power": "industrial", "warehouse": "port", "farm": "agricultural", "food works": "industrial", "store": "port", "utility": "industrial"}
# The game's spaceport, for the map of a settlement: its pads and its hangar (crates/world/src/spaceport.rs).
_port = open(os.path.join(ROOT, "crates", "world", "src", "spaceport.rs"), encoding="utf-8").read()
PORT = {
    "grid": int(re.search(r"pub const GRID: usize = (\d+);", _port).group(1)),
    "spacing": float(re.search(r"pub const PAD_SPACING: f64 = ([0-9.]+);", _port).group(1)),
    "pad": float(re.search(r"pub const PAD_SIZE: f64 = ([0-9.]+);", _port).group(1)),
    "radius": float(re.search(r"pub const PAD_RADIUS: f64 = ([0-9.]+);", _port).group(1)),
}


# The game's kinds of goods: each one's name and how much a cubic metre of it weighs as stowed
# (content/base/goods.ron).
# (The game no longer has ores.ron or goods.ron: it reads the goods and the market's categories from
# the registry. Both are taken from the records here, under the names the rest of this build uses.)
_sfo_meta = os.path.join(TREE, "SFO", "metadata")
# (What is sold is stock: a good's market category is its stock's.)
SOLD_AS = {}
_sdir = os.path.join(TREE, "SFO", "metadata", "stock")
for _f in sorted(os.listdir(_sdir)) if os.path.isdir(_sdir) else []:
    _s = yaml.safe_load(open(os.path.join(_sdir, _f), encoding="utf-8")) or {}
    for _m in _s.get("made_from") or []:
        SOLD_AS[str(_m.get("item", "")).split(".", 1)[-1]] = str((_s.get("identity") or {}).get("traded_as", ""))
ORES = {m for f in sorted(os.listdir(os.path.join(_sfo_meta, "goods"))) for m in re.findall(r"^\s*ore: (ore\.[a-z_]+)", open(os.path.join(_sfo_meta, "goods", f), encoding="utf-8").read(), re.M)}
GOODS_KINDS = {}
for _mf in sorted(os.listdir(os.path.join(_sfo_meta, "markets"))):
    _mk = yaml.safe_load(open(os.path.join(_sfo_meta, "markets", _mf), encoding="utf-8"))
    GOODS_KINDS["goods." + _mf[:-5].replace("-", "_")] = {"name": _mk["identity"]["name"].title(), "density": (_mk.get("bulk_density") or 0) / 1000}


TIERS = ["sourced", "derived", "invented"]


# (The rules many records rest on, stated once: a basis entry's `rule` gives its tier, source and review flag.)
RULES = {}
for _rf in sorted(glob.glob(os.path.join(TREE, "SFO", "metadata", "rules", "*.yaml"))):
    _r = yaml.safe_load(open(_rf, encoding="utf-8")) or {}
    RULES[_r["identity"]["key"]] = _r
    RULES[_r["identity"]["key"].split(".", 1)[1]] = _r   # (the old view reads references without their kind prefix)


def basis_tier(en):
    """A basis entry's tier: its own, or its rule's."""
    return en.get("tier") or RULES.get(en.get("rule"), {}).get("tier")


def basis_review(en):
    return bool(en.get("review")) if "review" in en else bool(RULES.get(en.get("rule"), {}).get("review"))


def check_basis(rec, where):
    """A record's `basis`: entries of what they cover, a tier, and for a sourced one its source."""
    bs = rec.get("basis")
    if bs is None:
        return
    if not isinstance(bs, list):
        problem(where, "basis: a list of entries (of, tier, source, note)")
        return
    for en in bs:
        if not isinstance(en, dict) or not isinstance(en.get("of"), list) or (en.get("rule") is None and en.get("tier") not in TIERS):
            problem(where, f"basis: each entry names what it is of and a tier ({', '.join(TIERS)}) or a rule")
        elif set(en) - {"of", "tier", "source", "note", "review", "rule"}:
            problem(where, "basis: unknown field")
        elif en.get("rule") is not None and en["rule"] not in RULES:
            problem(where, f"basis: no rule '{en['rule']}' (standards/SFO/metadata/rules)")
        elif basis_tier(en) == "sourced" and not (en.get("source") or RULES.get(en.get("rule"), {}).get("source")):
            problem(where, "basis: a sourced entry names its source")


def area(o):
    """An outline's area (m2)."""
    return abs(sum(o[i][0] * o[(i + 1) % len(o)][1] - o[(i + 1) % len(o)][0] * o[i][1] for i in range(len(o)))) / 2


def strictly_within(c, o):
    """Is the point c inside the outline o, not on its edge?"""
    x, y = c
    for i in range(len(o)):
        (x1, y1), (x2, y2) = o[i], o[(i + 1) % len(o)]
        if min(x1, x2) <= x <= max(x1, x2) and min(y1, y2) <= y <= max(y1, y2) and abs((x2 - x1) * (y - y1) - (y2 - y1) * (x - x1)) < 1e-9:
            return False
    return within(c, o)


def overlap(p, q):
    """Do two outlines share ground (more than an edge)? By corners and centres: enough for plain shapes."""
    mid = lambda o: [sum(c[0] for c in o) / len(o), sum(c[1] for c in o) / len(o)]
    return any(strictly_within(c, q) for c in p + [mid(p)]) or any(strictly_within(c, p) for c in q + [mid(q)])


def within(c, o):
    """Is the point c inside the outline o, or on its edge?"""
    x, y = c
    inside = False
    for i in range(len(o)):
        (x1, y1), (x2, y2) = o[i], o[(i + 1) % len(o)]
        if min(x1, x2) <= x <= max(x1, x2) and min(y1, y2) <= y <= max(y1, y2) and abs((x2 - x1) * (y - y1) - (y2 - y1) * (x - x1)) < 1e-9:
            return True
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            inside = not inside
    return inside

local = load(os.path.join(TREE, LOCAL, "metadata", LOCAL + ".yaml"))
local = {**{k: v for k, v in local.items() if k != "identity"}, "name": (local.get("identity") or {}).get("name", local.get("name"))}      # (the page reads its name plainly)
administrations = []
adm_dir = os.path.join(TREE, LOCAL, "metadata", "administrations")
for name in sorted(os.listdir(adm_dir)) if os.path.isdir(adm_dir) else []:
    full = os.path.join(adm_dir, name)
    if os.path.isdir(full):
        # (An administration's bodies: read with it, below.)
        continue
    if not re.fullmatch(r"[a-z0-9-]+\.yaml", name):
        problem(full, "an administration's file is named <name>.yaml (lower case, words joined by -)")
        continue
    ad = load(full)
    # (Its bodies: one file each, in the folder named after it.)
    ad["bodies"] = []
    bodies_dir = full[:-5]
    for bn in sorted(os.listdir(bodies_dir)) if os.path.isdir(bodies_dir) else []:
        bfull = os.path.join(bodies_dir, bn)
        if os.path.isdir(bfull):
            # (A settlement's zones: read with it, below.)
            continue
        if not re.fullmatch(r"[a-z0-9-]+\.yaml", bn):
            problem(bfull, "a body's file is named <name>.yaml (lower case, words joined by -)")
            continue
        x = load(bfull)
        for k in ["name", "kind"]:
            if k not in x:
                problem(bfull, f"no {k}")
        for k in x:
            if k not in {"name", "kind", "at", "gravity", "position", "about", "story", "zones", "parcels", "facilities", "streets", "power_lines", "gate", "population", "structure", "resupply", "census", "independents", "buildings"} | ({"owner", "processes", "lines", "modules", "spin"} if x.get("kind") == "rig" else set()):
                problem(bfull, f"unknown field '{k}'")
        x["slug"] = bn[:-5]
        x["file"] = os.path.relpath(bfull, TREE)
        # (Its zones and parcels: <settlement>/zones/<name>.yaml and <settlement>/parcels/parcel-<n>.yaml,
        # each an outline in metres east and north of the settlement's position.)
        # (Its streets: <settlement>/streets/<name>.yaml, a line on the same ground.)
        streets = []
        st_dir = os.path.join(bfull[:-5], "streets")
        for fn in sorted(os.listdir(st_dir)) if os.path.isdir(st_dir) else []:
            ffull = os.path.join(st_dir, fn)
            if not re.fullmatch(r"[a-z0-9-]+\.yaml", fn):
                problem(ffull, "a street's file is named <name>.yaml (lower case, words joined by -)")
                continue
            st = load(ffull)
            for k in ["name", "line"]:
                if k not in st:
                    problem(ffull, f"no {k}")
            for k in st:
                if k not in {"name", "line"}:
                    problem(ffull, f"unknown field '{k}'")
            ln = st.get("line")
            if not (isinstance(ln, list) and len(ln) >= 2 and all(isinstance(c, list) and len(c) == 2 and all(isinstance(v, (int, float)) for v in c) for c in ln)):
                problem(ffull, "line: two or more points, each [east, north] in metres")
                continue
            st["length"] = sum(((ln[i + 1][0] - ln[i][0]) ** 2 + (ln[i + 1][1] - ln[i][1]) ** 2) ** 0.5 for i in range(len(ln) - 1))
            st["slug"] = fn[:-5]
            st["file"] = os.path.relpath(ffull, TREE)
            streets.append(st)
        if streets:
            x["streets"] = streets
        zones, plots = [], []
        for sub, into in (("zones", zones), ("parcels", plots)):
            sub_dir = os.path.join(bfull[:-5], sub)
            for fn in sorted(os.listdir(sub_dir)) if os.path.isdir(sub_dir) else []:
                ffull = os.path.join(sub_dir, fn)
                r = load(ffull)
                need, pattern = (["name", "use", "outline"], r"[a-z0-9-]+\.yaml") if sub == "zones" else (["number", "outline"], r"parcel-([0-9]+)\.yaml")
                allowed = need + (["address", "owner"] if sub == "parcels" else [])
                m = re.fullmatch(pattern, fn)
                if not m:
                    problem(ffull, "a zone's file is named <name>.yaml" if sub == "zones" else "a parcel's file is named parcel-<number>.yaml")
                    continue
                for k in need:
                    if k not in r:
                        problem(ffull, f"no {k}")
                for k in r:
                    if k not in allowed:
                        problem(ffull, f"unknown field '{k}'")
                o = r.get("outline")
                if not (isinstance(o, list) and len(o) >= 3 and all(isinstance(c, list) and len(c) == 2 and all(isinstance(v, (int, float)) for v in c) for c in o)):
                    problem(ffull, "outline: three or more corners, each [east, north] in metres")
                    continue
                if x.get("kind") != "settlement" or "position" not in x:
                    problem(ffull, "zones and parcels belong to a settlement on a surface (one with a position)")
                r["area"] = area(o)
                r["slug"] = fn[:-5]
                r["file"] = os.path.relpath(ffull, TREE)
                if sub == "zones" and r.get("use") not in ZONE_USES:
                    problem(ffull, f"use: one of {', '.join(ZONE_USES)}")
                if sub == "parcels":
                    if r.get("number") != int(m.group(1)):
                        problem(ffull, f"number {r.get('number')} in a file numbered {m.group(1)}")
                    if "owner" in r and r["owner"] not in BRANDS and not str(r["owner"]).startswith("body."):
                        problem(ffull, f"owner: no maker '{r['owner']}' in Maker House")
                    ad_ = r.get("address")
                    if ad_ is not None:
                        if not (isinstance(ad_, dict) and set(ad_) == {"street", "number"} and isinstance(ad_["number"], int)):
                            problem(ffull, "address: street (a street's file name) and number")
                        elif not any(st["slug"] == ad_["street"] for st in streets):
                            problem(ffull, f"address: {x.get('name')} has no street '{ad_['street']}'")
                        elif any(o_.get("address") == ad_ for o_ in plots):
                            problem(ffull, "address: another parcel has it")
                    # (Its zone: the one all its corners lie in.)
                    inn = [zn for zn in zones if all(within(c, zn["outline"]) for c in o)]
                    if len(inn) != 1:
                        problem(ffull, "outline: it must lie inside one zone" if not inn else "outline: it lies in more than one zone")
                    else:
                        r["zone"] = inn[0]["slug"]
                    for other in plots:
                        if any(within(c, other["outline"]) for c in o) or any(within(c, o) for c in other["outline"]):
                            problem(ffull, f"outline: it overlaps parcel {other.get('number')}")
                into.append(r)
        for i, zn in enumerate(zones):
            for other in zones[:i]:
                if any(within(c, other["outline"]) for c in zn["outline"]) or any(within(c, zn["outline"]) for c in other["outline"]):
                    problem(os.path.join(TREE, zn["file"]), f"outline: it overlaps the zone {other.get('name')}")
        plots.sort(key=lambda r: r.get("number", 0))
        # (What is built on its parcels: <settlement>/facilities/<name>.yaml.)
        fac_dir = os.path.join(bfull[:-5], "facilities")
        facs = []
        for fn in sorted(os.listdir(fac_dir)) if os.path.isdir(fac_dir) else []:
            ffull = os.path.join(fac_dir, fn)
            if not re.fullmatch(r"[a-z0-9-]+\.yaml", fn):
                problem(ffull, "a facility's file is named <name>.yaml (lower case, words joined by -)")
                continue
            fc = load(ffull)
            for k in ["name", "kind", "parcel"]:
                if k not in fc:
                    problem(ffull, f"no {k}")
            if fc.get("kind") not in FACILITY_ZONE:
                problem(ffull, f"kind: one of {', '.join(FACILITY_ZONE)}")
            plot = next((r for r in plots if r.get("number") == fc.get("parcel")), None)
            if plot is None:
                problem(ffull, f"parcel {fc.get('parcel')}: no such parcel of {x.get('name')}")
            else:
                zone = next((zn for zn in zones if zn["slug"] == plot.get("zone")), None)
                need = FACILITY_ZONE.get(fc.get("kind"))
                if zone is not None and need and zone.get("use") != need:
                    problem(ffull, f"a {fc.get('kind')} needs a parcel zoned {need}; parcel {plot.get('number')} is zoned {zone.get('use')}")
                fc["owner"] = plot.get("owner")
                # (Its site plan: parts inside the parcel, none overlapping; each process in one part;
                # pipelines between parts, inside the parcel.)
                PART_KINDS = ["module", "workshop", "warehouse", "logistics", "parking"]
                is_pts = lambda o, n: isinstance(o, list) and len(o) >= n and all(isinstance(c, list) and len(c) == 2 and all(isinstance(v, (int, float)) for v in c) for c in o)
                names, housed = [], []
                for pt in fc.get("parts") or []:
                    nm = pt.get("name")
                    if pt.get("kind") not in PART_KINDS:
                        problem(ffull, f"part {nm}: kind one of {', '.join(PART_KINDS)}")
                    if nm in names:
                        problem(ffull, f"part {nm} twice")
                    names.append(nm)
                    if not is_pts(pt.get("outline"), 3):
                        problem(ffull, f"part {nm}: outline of three or more corners, each [east, north] in metres")
                        continue
                    if not all(within(c, plot["outline"]) for c in pt["outline"]):
                        problem(ffull, f"part {nm}: it reaches outside parcel {plot.get('number')}")
                    for other in fc["parts"]:
                        if other is pt:
                            break
                        if is_pts(other.get("outline"), 3) and overlap(pt["outline"], other["outline"]):
                            problem(ffull, f"part {nm}: it overlaps {other.get('name')}")
                    pt["area"] = area(pt["outline"])
                    housed += pt.get("processes") or []
                if fc.get("parts"):
                    for proc_ in fc.get("processes") or []:
                        if housed.count(proc_) != 1:
                            problem(ffull, f"process '{proc_}' is run in {housed.count(proc_)} parts; it needs exactly one")
                    for proc_ in housed:
                        if proc_ not in (fc.get("processes") or []):
                            problem(ffull, f"a part runs '{proc_}', which the facility doesn't list")
                for pl in fc.get("pipelines") or []:
                    nm = pl.get("name")
                    for k in ["name", "carries", "from", "to", "line"]:
                        if k not in pl:
                            problem(ffull, f"pipeline {nm}: no {k}")
                    for k in pl:
                        if k not in {"name", "carries", "from", "to", "line"}:
                            problem(ffull, f"pipeline {nm}: unknown field '{k}'")
                    for end in ("from", "to"):
                        if pl.get(end) not in names:
                            problem(ffull, f"pipeline {nm}: {end} '{pl.get(end)}' is no part of it")
                    ln = pl.get("line")
                    if not is_pts(ln, 2):
                        problem(ffull, f"pipeline {nm}: line of two or more points, each [east, north] in metres")
                        continue
                    if not all(within(c, plot["outline"]) for c in ln):
                        problem(ffull, f"pipeline {nm}: it runs outside parcel {plot.get('number')}")
                    pl["length"] = sum(((ln[i + 1][0] - ln[i][0]) ** 2 + (ln[i + 1][1] - ln[i][1]) ** 2) ** 0.5 for i in range(len(ln) - 1))
            fc["slug"] = fn[:-5]
            fc["file"] = os.path.relpath(ffull, TREE)
            facs.append(fc)
        if facs:
            x["facilities"] = facs
        # (Its power lines: <settlement>/power-lines/<name>.yaml, from one facility's parcel to another's.)
        pw_dir = os.path.join(bfull[:-5], "power-lines")
        wires = []
        for fn in sorted(os.listdir(pw_dir)) if os.path.isdir(pw_dir) else []:
            ffull = os.path.join(pw_dir, fn)
            if not re.fullmatch(r"[a-z0-9-]+\.yaml", fn):
                problem(ffull, "a power line's file is named <name>.yaml (lower case, words joined by -)")
                continue
            w = load(ffull)
            for k in ["name", "from", "to", "capacity", "line"]:
                if k not in w:
                    problem(ffull, f"no {k}")
            for k in w:
                if k not in {"name", "from", "to", "capacity", "line"}:
                    problem(ffull, f"unknown field '{k}'")
            ln = w.get("line")
            if not (isinstance(ln, list) and len(ln) >= 2 and all(isinstance(c, list) and len(c) == 2 and all(isinstance(v, (int, float)) for v in c) for c in ln)):
                problem(ffull, "line: two or more points, each [east, north] in metres")
                continue
            for end, pt in (("from", ln[0]), ("to", ln[-1])):
                fc = next((f for f in facs if f["slug"] == w.get(end)), None)
                plot = fc and next((r for r in plots if r.get("number") == fc.get("parcel")), None)
                if fc is None:
                    problem(ffull, f"{end}: {x.get('name')} has no facility '{w.get(end)}'")
                elif plot is not None and not within(pt, plot["outline"]):
                    problem(ffull, f"line: it does not {'start' if end == 'from' else 'end'} on {fc.get('name')}'s parcel")
            src = next((f for f in facs if f["slug"] == w.get("from")), None)
            if src is not None and src.get("kind") != "power":
                problem(ffull, f"from: {src.get('name')} makes no power")
            w["length"] = sum(((ln[i + 1][0] - ln[i][0]) ** 2 + (ln[i + 1][1] - ln[i][1]) ** 2) ** 0.5 for i in range(len(ln) - 1))
            w["slug"] = fn[:-5]
            w["file"] = os.path.relpath(ffull, TREE)
            wires.append(w)
        if wires:
            x["power_lines"] = wires
        if plots:
            x["parcels"] = plots
        if zones:
            x["zones"] = zones
        ad["bodies"].append(x)
    # (The planets and moons its settlements are at are celestial bodies (standards/Celestial), not
    # records of its own. This build still lists them with its bodies: each rocky planet and moon of
    # the system of the same name, with what the celestial record says of it.)
    cel_bodies = os.path.join(TREE, "Celestial", "metadata", "systems", name[:-5], "bodies")
    for bn in sorted(os.listdir(cel_bodies)) if os.path.isdir(cel_bodies) else []:
        cb = load(os.path.join(cel_bodies, bn))
        ci = cb.get("identity") or {}
        if ci.get("kind") not in ("rocky planet", "moon"):
            continue
        x = {"name": ci.get("name"), "kind": "planet" if ci["kind"] == "rocky planet" else "moon", "slug": bn[:-5], "file": os.path.relpath(os.path.join(cel_bodies, bn), TREE), "celestial_body": ci.get("key")}
        if ci["kind"] == "moon":
            x["at"] = ci.get("parent")
        for k in ("about", "story"):
            if k in ci:
                x[k] = ci[k]
        if "gravity" in (cb.get("physical") or {}):
            x["gravity"] = cb["physical"]["gravity"]
        ad["bodies"].append(x)
    ad["bodies"].sort(key=lambda x: x.get("slug", ""))
    for k in ["name"]:
        if k not in ad:
            problem(full, f"no {k}")
    if not ad.get("administers"):
        problem(full, "administers: an administration names the system whose land it administers, by its key (the game finds its law by it)")
    names = [x.get("name") for x in ad.get("bodies") or []]
    for x in ad.get("bodies") or []:
        if x.get("kind") not in ("planet", "moon", "settlement", "rig"):
            problem(full, f"body {x.get('name')}: kind one of planet, moon, settlement, rig")
        if names.count(x.get("name")) > 1:
            problem(full, f"body {x.get('name')} twice")
    ad["file"] = os.path.relpath(full, TREE)
    ad["slug"] = name[:-5]
    administrations.append(ad)

# An address (SFO 9): at <system>/<body> in Local Administration (the settlement is the unit of
# administration), then tower, deck, section, unit inside it.
taken = {}


def check_address(rec, where):
    a = rec.get("address")
    if a is None:
        return
    if not isinstance(a, dict) or "at" not in a:
        problem(where, "address: at (<system>/<body>), then tower, deck, section, unit")
        return
    sysm, _, at = str(a["at"]).partition("/")
    adm = next((x for x in administrations if x["slug"] == sysm), None)
    body = next((x for x in (adm or {}).get("bodies", []) if x["slug"] == at), None)
    # (A rig in a system with no administration: a record, with no land round it.)
    rig_file = os.path.join(TREE, "LocalAdministration", "metadata", "administrations", sysm, at + ".yaml")
    if body is None and not (os.path.isfile(rig_file) and (yaml.safe_load(open(rig_file, encoding="utf-8")) or {}).get("kind") == "rig"):
        problem(where, f"address: no '{a['at']}' in Local Administration (<system>/<body>)")
        return
    for k in a:
        if k not in {"at", "tower", "deck", "section", "unit"}:
            problem(where, f"address: unknown field '{k}'")
    # (Inside the settlement is its own business: tower, deck, section and unit aren't checked
    # against a layout, only that no two are at the same one.)
    if "unit" not in a:
        return
    spot = (a["at"], a.get("tower"), a.get("deck"), a.get("section"), a.get("unit"))
    if "unit" in a and spot in taken:
        problem(where, f"address: {taken[spot]} is already there")
    taken[spot] = rec.get("name")


for m in makers:
    check_address(m, os.path.join(TREE, m["file"]))
for ad in administrations:
    check_address(ad, os.path.join(TREE, ad["file"]))

bodies, standards = [], []
for name in sorted(os.listdir(TREE)):
    folder = os.path.join(TREE, name)
    if not os.path.isdir(folder) or name in ("schema", "sources", HOUSE, LOCAL, "Celestial", "Dogma", "People", "Engine"):
        continue
    # (The body's own file: named after its folder, SFO/metadata/SFO.yaml.)
    meta_path = os.path.join(folder, "metadata", name + ".yaml")
    if not os.path.exists(meta_path):
        problem(folder, f"a body's folder needs {name}.yaml")
        continue
    body = load(meta_path)
    for k in ["key", "name", "prefix", "seat", "note"]:
        if k not in body:
            problem(meta_path, f"no {k}")
    for k in body:
        if k not in BODY_FIELDS:
            problem(meta_path, f"unknown field '{k}'")
    if body.get("kind", "consortium") not in BODY_KINDS:
        problem(meta_path, f"kind: one of {', '.join(BODY_KINDS)}")
    for m in body.get("founded_by", []) or []:
        if m not in BRANDS:
            problem(meta_path, f"founded_by: no maker '{m}' in Maker House")
    check_address(body, meta_path)
    if isinstance(body.get("about"), str):
        body["about"] = [body["about"]]
    if body.get("prefix") != name:
        problem(meta_path, f"prefix {body.get('prefix')} but the folder is {name}")
    numbers = {}
    # (Its records: the standards, in its metadata folder.)
    records = os.path.join(folder, "metadata")
    for name in sorted(os.listdir(records)) if os.path.isdir(records) else []:
        full = os.path.join(records, name)
        if name == os.path.basename(folder) + ".yaml":
            continue
        if os.path.isdir(full):
            # (A folder of records under one of the standards: see its `records`. Read below.)
            continue
        m = re.fullmatch(r"([0-9]{4})-[a-z0-9-]+\.yaml", name)
        if not m:
            problem(full, "a standard's file is named NNNN-<slug>.yaml (its permanent number)")
            continue
        n = int(m.group(1))
        if n == 0:
            problem(full, "numbers start at 1")
        if n in numbers:
            problem(full, f"number {n} twice (also {numbers[n]})")
        numbers[n] = name
        st = load(full)
        st["number"] = n
        st["id"] = f"{body.get('prefix', name)} {n}"
        st["body"] = body.get("key", "")
        st["file"] = os.path.relpath(full, TREE)
        standards.append(st)
    bodies.append(body)

# Records in folders (a standard's `records`): chemical elements and materials, each kind to
# its schema (schema/element.schema.yaml, schema/material.schema.yaml).
KINDS = {"elements": "element", "materials": "material", "modules": "module", "goods": "good", "hulls": "hull", "mill-stock": "mill-stock", "equipment": "equipment", "gates": "gate"}
# (Parts are filed in folders of their own: read further down.)
NESTED = {"parts", "mounts"}      # (folders whose records the page lists elsewhere, or not yet)
SCHEMAS = {k: read_schema(os.path.join(TREE, "SFO", "schema", f"{v}.schema.yaml")) for k, v in KINDS.items()}
elements, materials, processes, modules, goods, hulls, mill_stock, equipment, gates = [], [], [], [], [], [], [], [], []
for s in standards:
    if "records" not in s:
        continue
    kind = str(s["records"])
    prefix = s["id"].split(" ")[0]
    folder = os.path.join(TREE, prefix, "metadata", kind)
    if kind in NESTED or s.get("status") in ("superseded", "withdrawn"):      # (a superseded standard's records are gone: its text stays)
        continue
    if kind not in KINDS:
        problem(os.path.join(TREE, s["file"]), f"records: one of {', '.join(KINDS)}")
        continue
    if not os.path.isdir(folder):
        problem(os.path.join(TREE, s["file"]), f"records: no folder metadata/{kind}")
        continue
    seen = {}
    for name in sorted(os.listdir(folder)):
        full = os.path.join(folder, name)
        if name.startswith("."):
            continue
        pattern = r"[0-9]{3}-[a-z-]+\.yaml" if kind == "elements" else r"[A-Z0-9-]+\.yaml" if kind == "mill-stock" else r"[a-z0-9-]+\.yaml"
        if not re.fullmatch(pattern, name):
            problem(full, "an element's file is named NNN-<name>.yaml (its atomic number)" if kind == "elements" else "its file is named <name>.yaml (lower case, words joined by -)")
            continue
        e = load(full)
        ident = e.get("identity") or {}
        if kind == "elements":
            for k in ("name", "symbol", "atomic_number"):
                if k not in ident:
                    problem(full, f"identity: no {k}")
            if ident.get("atomic_number") != int(name[:3]):
                problem(full, f"atomic number {ident.get('atomic_number')} in a file numbered {name[:3]}")
            key = ident.get("symbol")
        elif kind == "mill-stock":
            for k in ("code", "name"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
            if name != str(ident.get("code")) + ".yaml":
                problem(full, "an item of mill stock is filed as <its code>.yaml")
            key = ident.get("code")
            e["slug"] = name[:-5]
        elif kind in ("equipment", "gates"):
            for k in ("name",) if kind == "equipment" else ("name", "key"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
            if ident.get("maker") is not None and ident["maker"] not in BRANDS:
                problem(full, f"identity.maker: no maker '{ident['maker']}' in Maker House")
            key = ident.get("name")
            e["slug"] = name[:-5]
        elif kind == "hulls":
            if ident.get("maker") is not None and ident["maker"] not in BRANDS:
                problem(full, f"identity.maker: no maker '{ident['maker']}' in Maker House")
            if not ident.get("name"):
                problem(full, "identity: no name")
            key = ident.get("name")
            e["slug"] = name[:-5]
        elif kind == "goods":
            for k in ("name", "kind"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
            key = ident.get("name")
            e["slug"] = name[:-5]
        elif kind == "modules":
            for k in ("name", "step"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
            key = ident.get("name")
            e["slug"] = name[:-5]
            key = ident.get("name")
            e["slug"] = name[:-5]
        else:
            for k in ("name", "class"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
            key = ident.get("name")
            e["slug"] = name[:-5]
        if key in seen:
            problem(full, f"{key} twice (also {seen[key]})")
        seen[key] = name
        check_basis(e, full)
        for group, props in e.items():
            if group in ("slug", "basis", "revision", "fits", "life") or (kind == "modules" and group == "recipes"):   # (a revision is held to its schema by validate.py)
                continue
            if kind == "hulls" and group == "open_questions":
                continue
            if kind in ("hulls", "gates") and group in ("fit", "slots", "thrusters", "model", "shape"):
                continue
            if kind == "goods" and group == "composition":
                for c in props or []:
                    if c.get("part") not in {(x.get("identity") or {}).get("symbol") for x in elements} and not os.path.exists(os.path.join(folder, str(c.get("part")) + ".yaml")):
                        problem(full, f"composition: '{c.get('part')}' is no element's symbol and no good's file name")
                continue
            known = SCHEMAS[kind]["properties"].get(group)
            if known is None:
                problem(full, f"unknown group '{group}'")
                continue
            for k in (props if isinstance(props, dict) else {}):
                if k not in known.get("properties", {}):
                    problem(full, f"{group}: unknown property '{k}'")
        e["under"] = s["id"]
        e["file"] = os.path.relpath(full, TREE)
        {"elements": elements, "materials": materials, "processes": processes, "modules": modules, "goods": goods, "hulls": hulls, "mill-stock": mill_stock, "equipment": equipment, "gates": gates}[kind].append(e)
elements.sort(key=lambda e: (e.get("identity") or {}).get("atomic_number", 0))
materials.sort(key=lambda e: (e.get("identity") or {}).get("name", ""))
# (A process's inputs and outputs name elements by symbol, materials by file name.)
# (Processes are retired (SFO 7 superseded): a line's route, derived from its modules' recipes, is written out in a process's shape for the page.)
by_process = {}
gate_of = {g["slug"]: g for g in gates}
for ad in administrations:
    for x in ad["bodies"]:
        where = os.path.join(TREE, x["file"]) if "file" in x else ad["file"]
        if x.get("kind") == "rig":
            if x.get("owner") not in BRANDS:
                problem(where, f"owner: no company '{x.get('owner')}' in Maker House")
            x["facilities"] = [{"name": x["name"], "kind": "rig", "rig": True, "owner": x.get("owner"), "slug": x["slug"], "file": x["file"],
                                "processes": x.get("processes") or [], "lines": x.get("lines") or [], "modules": x.get("modules") or []}]
        if "gate" in x and (x["gate"] or {}).get("ring") not in gate_of:
            problem(where, f"gate.ring: no gate ring '{(x['gate'] or {}).get('ring')}' in the SFO")
symbols = {(e.get("identity") or {}).get("symbol") for e in elements}
slugs = {m.get("slug") for m in materials} | {g.get("slug") for g in goods} | {ms.get("slug") for ms in mill_stock}
# (A process's steps: each in an industrial module.)
by_module = {m.get("slug") for m in modules}
# What a facility is to make: from each process's line of modules, how many of each it needs, what
# they take in and give off, their power and the ground they cover.
mod_of = {m.get("slug"): m for m in modules}
el_name = {(e.get("identity") or {}).get("symbol"): (e.get("identity") or {}).get("name") for e in elements}


def set_to(pr, s):
    """The recipe a module is set to in a line: the one the line's route names, or (shop work, by process) its first."""
    return (pr.get("route") or {}).get(s) or (mod_of[s].get("recipes") or [{}])[0]


# A line that says what it makes (not which process it runs): the route to that through its
# modules' recipes, back from the one that makes it. It is written out as a process (its steps, what
# comes out), which is what the rest of this build and the page read; `route` holds the recipe each
# module is set to. Two lines that make the same thing by different modules are two routes.
routes = []
item_name = lambda t: next((x["identity"]["name"] for x in mill_stock + goods + materials if x.get("slug") == t), el_name.get(t, {"parts": "Parts", "hulls": "Hulls"}.get(t, t)))


def route_for(where, fc, ln, target):
    order = [im.get("module") for im in ln.get("modules") or [] if im.get("module") in mod_of]
    need, chosen = {target}, {}
    for s in reversed(order):
        pick = next((r for r in mod_of[s].get("recipes") or [] if r.get("product") in need), None)
        if pick is not None:
            chosen[s] = pick
            need |= {x.get("item") for x in pick.get("inputs") or []}
        elif mod_of[s].get("recipes"):
            problem(where, f"lines: {mod_of[s]['identity']['name']} has no recipe that leads to {target}")
            return None
    if target not in {r.get("product") for r in chosen.values()}:
        problem(where, f"lines: none of the line's modules has a recipe that makes {target}")
        return None
    steps = [{"module": s, "does": (chosen.get(s) or {}).get("does") or mod_of[s]["identity"].get("description", "")} for s in order]
    ms = next((x for x in mill_stock if x.get("slug") == target), None)
    slug = "make-" + target.lower()
    same = next((r for r in routes if r["makes"] == target and [st["module"] for st in r["equipment"]["steps"]] == order), None)
    if same:
        return same["slug"]
    maker = next(s for s in reversed(order) if (chosen.get(s) or {}).get("product") == target)
    for more in ("", "-in-" + maker, "-in-" + maker + "-at-" + fc["slug"], "-in-" + maker + "-at-" + fc["slug"] + f"-{len(routes)}"):
        if slug + more not in by_process:
            slug += more
            break
    pr = {"slug": slug, "derived": True, "makes": target, "route": chosen, "file": fc["file"],
          "identity": {"name": item_name(target), "kind": "mechanical", "description": f"How {item_name(target)} is made at {fc['name']}: the recipes its modules are set to, one after another."},
          "outputs": {"products": [{"item": (ms.get("made_from") or {}).get("material"), "form": (ms.get("made_from") or {}).get("form")}] if ms else []},
          "equipment": {"facility": fc.get("kind"), "steps": steps}}
    routes.append(pr)
    by_process[slug] = pr
    if ms:
        ms.setdefault("routes", []).append(slug)
    return slug


def plan(pr, output):
    steps = []
    for st in (pr.get("equipment") or {}).get("steps") or []:
        if st.get("module") in mod_of and st["module"] not in steps:
            steps.append(st["module"])
    making = [s for s in steps if set_to(pr, s).get("throughput")]
    made = {set_to(pr, s).get("product"): s for s in making}
    demand = {s: 0.0 for s in steps}
    if making:
        demand[making[-1]] = float(output)
    supplies, by = {}, {}
    for s in reversed(steps):
        m, d = mod_of[s], demand[s]
        for x in set_to(pr, s).get("inputs") or []:
            src = made.get(x.get("item"))
            if src and src != s and steps.index(src) < steps.index(s):
                demand[src] += d * x.get("amount", 0)
            else:
                supplies[x.get("item")] = supplies.get(x.get("item"), 0) + d * x.get("amount", 0)
        for x in set_to(pr, s).get("outputs") or []:
            by[x.get("item")] = by.get(x.get("item"), 0) + d * x.get("amount", 0)
    # (What is given off and needed on the same site is used again.)
    reused = {k: min(supplies[k], by[k]) for k in supplies if k in by}
    for k, v in reused.items():
        supplies[k] -= v
        by[k] -= v
    rows = []
    for s in steps:
        m = mod_of[s]
        rate, size = m.get("rate") or {}, m.get("size") or {}
        through = set_to(pr, s).get("throughput")
        count = max(1, -(-demand[s] // through)) if through else 1
        rows.append({
            "module": s, "count": int(count), "demand": demand[s] if through else None, "through": through,
            "use": demand[s] / (count * through) if through else None,
            "area": count * size.get("length", 0) * size.get("width", 0),
            "power": (set_to(pr, s).get("power", 0) if through else (m.get("needs") or {}).get("power", 0)) * (demand[s] / through if through else 1),
        })
    product = set_to(pr, making[-1]).get("product", "") if making else ""
    return {
        "modules": rows, "product": product,
        "supplies": [{"item": k, "rate": v} for k, v in supplies.items() if v > 1e-9],
        "by_products": [{"item": k, "rate": v} for k, v in by.items() if v > 1e-9],
        "reused": [{"item": k, "rate": v} for k, v in reused.items() if v > 1e-9],
        "area": sum(r["area"] for r in rows), "power": sum(r["power"] for r in rows),
    }


# What a facility can do at most: from the modules it is built of. A line can make as much as its
# tightest module lets through; a power station can supply what its modules can. What it does make
# The ground between modules, and between them and the parcel's edges (m). Invented: room to
# walk and drive round each, until the SFO has a standard for it.
LAYOUT_GAP = 20.0


def lay_out(where, plot, street, order):
    """Modules on a parcel, in rows: the first along the side facing its street, each row
    behind the last, each module's length along its row, `LAYOUT_GAP` round each. Only
    rectangles squared to east and north are laid out yet. Each: the module, its centre
    [east, north] (m), its size, and which way its length runs (0: east, 90: north)."""
    o = plot.get("outline") or []
    es, ns = [p[0] for p in o], [p[1] for p in o]
    w, e, sth, nth = min(es), max(es), min(ns), max(ns)
    if len(o) != 4 or any(p[0] not in (w, e) or p[1] not in (sth, nth) for p in o):
        problem(where, f"layout: parcel {plot.get('number')} isn't a rectangle squared to east and north, which is all that's laid out yet")
        return []
    # (Its front: the side nearest its street.)
    sides = {"north": ((w + e) / 2, nth), "south": ((w + e) / 2, sth), "east": (e, (sth + nth) / 2), "west": (w, (sth + nth) / 2)}
    def gap_to(pt):
        line = (street or {}).get("line") or []
        return min((segment_distance(pt, a, b) for a, b in zip(line, line[1:])), default=0.0)
    front = min(sides, key=lambda k: gap_to(sides[k])) if street else "north"
    along = e - w if front in ("north", "south") else nth - sth
    deep = nth - sth if front in ("north", "south") else e - w
    blocks, rows, row, at, depth = [], [], [], LAYOUT_GAP, 0.0
    for slug, n in order:
        size = mod_of[slug].get("size") or {}
        for _ in range(n):
            length, width = size.get("length", 0), size.get("width", 0)
            if row and at + length > along - LAYOUT_GAP:
                rows.append((row, depth))
                row, at, depth = [], LAYOUT_GAP, 0.0
            row.append((slug, at, length, width, size.get("height", 0)))
            at += length + LAYOUT_GAP
            depth = max(depth, width)
    if row:
        rows.append((row, depth))
    back = LAYOUT_GAP
    for row, d in rows:
        for slug, a, length, width, height in row:
            if a + length > along - LAYOUT_GAP + 1e-6:
                problem(where, f"layout: a {slug} ({length:g} m long) is longer than parcel {plot.get('number')} is wide")
            u, v = a + length / 2, back + width / 2
            centre = {"north": (w + u, nth - v), "south": (w + u, sth + v), "east": (e - v, sth + u), "west": (w + v, sth + u)}[front]
            blocks.append({"module": slug, "centre": [round(centre[0], 3), round(centre[1], 3)], "length": length, "width": width, "height": height, "heading": 0 if front in ("north", "south") else 90})
        back += d + LAYOUT_GAP
    if back > deep + 1e-6:
        problem(where, f"layout: its modules need {back:,.0f} m of depth in rows, and parcel {plot.get('number')} has {deep:,.0f} m")
    return blocks


def segment_distance(p, a, b):
    """How far point p is from the segment a-b (m)."""
    ax, ay, bx, by = a[0], a[1], b[0], b[1]
    dx, dy = bx - ax, by - ay
    t = 0.0 if dx == dy == 0 else max(0.0, min(1.0, ((p[0] - ax) * dx + (p[1] - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(p[0] - (ax + t * dx), p[1] - (ay + t * dy))


# is not recorded: that is the economy's.
for ad in administrations:
    for x in ad["bodies"]:
        for fc in x.get("facilities", []):
            where = os.path.join(TREE, fc["file"])
            plot = next((r for r in x.get("parcels", []) if r.get("number") == fc.get("parcel")), None)
            covered = 0
            for ln in fc.get("lines") or []:
                if "makes" not in ln and "process" not in ln:
                    # (A line of shop modules: it makes whatever parts name them. Worked here by weight, as parts or hulls.)
                    last = next((mod_of[im["module"]] for im in reversed(ln.get("modules") or []) if im.get("module") in mod_of and mod_of[im["module"]].get("recipes")), None)
                    if last is not None:
                        ln["makes"] = last["recipes"][0]["product"]
                if "makes" in ln:
                    made_by = [route_for(where, fc, ln, t) for t in [ln.pop("makes")] + (ln.get("also") or [])]
                    if None in made_by:
                        continue
                    ln["process"], ln["also"] = made_by[0], made_by[1:]
                    if not ln["also"]:
                        del ln["also"]
                    fc.setdefault("processes", [])
                    fc["processes"] += [q for q in made_by if q not in fc["processes"]]
                pr = by_process.get(ln.get("process"))
                if set(ln) - {"also"} != {"process", "modules"}:
                    problem(where, "lines: each is a process and the modules it is built of")
                    continue
                if ln["process"] not in (fc.get("processes") or []):
                    problem(where, f"lines: '{ln['process']}' is not one of its processes")
                    continue
                if pr is None or not (pr.get("equipment") or {}).get("steps"):
                    problem(where, f"lines: '{ln['process']}' has no steps to build a line from")
                    continue
                has = {}
                for im in ln["modules"] or []:
                    if im.get("module") not in mod_of or not isinstance(im.get("count"), int) or im["count"] < 1:
                        problem(where, f"lines: no module '{im.get('module')}' in the SFO, or no count")
                    else:
                        has[im["module"]] = im["count"]
                unit = plan(pr, 1.0)
                for r in unit["modules"]:
                    if r["module"] not in has:
                        problem(where, f"lines: {ln['process']} needs a {r['module']}, and it has none")
                for m in has:
                    if m not in [r["module"] for r in unit["modules"]]:
                        problem(where, f"lines: '{m}' is no step of {ln['process']}")
                # (What else the same line can run: each a process of the facility, whose steps are
                # all in modules the line has.)
                for other in ln.get("also") or []:
                    opr = by_process.get(other)
                    if other not in (fc.get("processes") or []):
                        problem(where, f"lines: also '{other}' is not one of its processes")
                    elif opr is None or not (opr.get("equipment") or {}).get("steps"):
                        problem(where, f"lines: also '{other}' has no steps")
                    else:
                        for st in opr["equipment"]["steps"]:
                            if st.get("module") not in has:
                                problem(where, f"lines: also '{other}' needs a {st.get('module')}, and the line has none")
                # (The most the line can make: the least any of its modules lets through.)
                limits = [(has[r["module"]] * r["through"] / r["demand"], r["module"]) for r in unit["modules"] if r["demand"] and r["module"] in has]
                if not limits:
                    continue
                most, tightest = min(limits)
                full = plan(pr, most)
                rows = []
                for r in full["modules"]:
                    m, n = mod_of[r["module"]], has.get(r["module"], 0)
                    rate, size = m.get("rate") or {}, m.get("size") or {}
                    rows.append({
                        "module": r["module"], "count": n,
                        "can": n * r["through"] if r.get("through") else None,
                        "holds": n * rate["holds"] if rate.get("holds") else None,
                        "at_full": r["demand"], "use": r["demand"] / (n * r["through"]) if r.get("through") and n else None,
                        "area": n * size.get("length", 0) * size.get("width", 0), "power": r["power"],
                    })
                ln["most"] = {
                    "output": most, "product": full["product"], "tightest": tightest, "modules": rows,
                    "supplies": full["supplies"], "by_products": full["by_products"], "reused": full["reused"],
                    "area": sum(r["area"] for r in rows), "power": sum(r["power"] for r in rows),
                }
                covered += ln["most"]["area"]
            # (The modules it says it is built of, where it has no line: a power station's.)
            for im in fc.get("modules") or []:
                if im.get("module") not in mod_of or not isinstance(im.get("count"), int):
                    problem(where, f"modules: no module '{im.get('module')}' in the SFO, or no count")
            built = [(mod_of[im["module"]], im["count"]) for im in fc.get("modules") or [] if im.get("module") in mod_of and isinstance(im.get("count"), int)]
            if built:
                fc["capacity"] = sum(((m.get("rate") or {}).get("power") or 0) * n for m, n in built)
                # (What it can hold and handle: a warehouse's.)
                rate = lambda m: m.get("rate") or {}
                fc["store"] = [{"module": m["slug"], "count": n, "holds": rate(m).get("holds", 0) * n or None, "volume": rate(m).get("volume", 0) * n or None, "handling": rate(m).get("handling", 0) * n or None} for m, n in built]
                # (What that is of each kind of goods: the space that stores it, by its weight as stowed.)
                per = {}
                for m, n in built:
                    for kind in rate(m).get("stores") or []:
                        row = per.setdefault(kind, {"kind": kind, "tonnes": 0.0, "in": []})
                        row["tonnes"] += n * (rate(m).get("volume", 0) * GOODS_KINDS.get(kind, {}).get("density", 0) if rate(m).get("volume") else rate(m).get("holds", 0))
                        row["in"].append(m["slug"])
                fc["holds_of"] = list(per.values())
                fc["built_area"] = sum((m.get("size") or {}).get("length", 0) * (m.get("size") or {}).get("width", 0) * n for m, n in built)
                # (What it burns flat out: its modules' fuel for each MWh, at all it can supply.)
                burn = {}
                for m, n in built:
                    for i in (m.get("inputs") or {}).get("materials") or []:
                        burn[i.get("item")] = burn.get(i.get("item"), 0) + i.get("amount", 0) * ((m.get("rate") or {}).get("power") or 0) * n
                fc["burns"] = [{"item": k, "rate": v} for k, v in burn.items()]
                covered += fc["built_area"]
            # (A works is one pool of stock: it needs somewhere to keep it.)
            if any("most" in ln for ln in fc.get("lines") or []):
                every = [im.get("module") for ln in fc.get("lines") or [] for im in ln.get("modules") or []] + [im.get("module") for im in fc.get("modules") or []]
                fc["stock_room"] = [m for m in every if m in mod_of and ((mod_of[m].get("rate") or {}).get("holds") or (mod_of[m].get("rate") or {}).get("volume"))]
                if not fc["stock_room"]:
                    problem(where, "it makes things and has nowhere to keep them: a works needs a module that stores (a yard, a warehouse)")
                # (How long its room lasts: everything it makes and gives off flat out, with nothing taken away. When
                # it is full, what makes the stock has to stop: the game's to run, worked out here as a measure.)
                counts = [(im.get("module"), im.get("count", 0)) for ln in fc.get("lines") or [] for im in ln.get("modules") or []] + [(im.get("module"), im.get("count", 0)) for im in fc.get("modules") or []]
                room = sum(n * ((mod_of[m].get("rate") or {}).get("holds") or 0) for m, n in counts if m in mod_of)
                out = sum(ln["most"]["output"] + sum(i["rate"] for i in ln["most"]["by_products"]) for ln in fc.get("lines") or [] if "most" in ln)
                if room and out:
                    fc["fills"] = {"holds": room, "rate": out, "days": room / out / 24}
            if "claim" in fc:
                # (The deposit is the survey's, or the energy package's, of the body the settlement is at: a permanent id.)
                _dep = str(fc["claim"].get("deposit", ""))
                _body_key = x.get("at_key") or (yaml.safe_load(open(os.path.join(TREE, "LocalAdministration", "metadata", "administrations", ad["slug"], x["slug"] + ".yaml"), encoding="utf-8")) or {}).get("at")
                _brec = next((yaml.safe_load(open(f__, encoding="utf-8")) or {} for f__ in glob.glob(os.path.join(TREE, "Celestial", "metadata", "systems", "*", "bodies", "*.yaml")) if (yaml.safe_load(open(f__, encoding="utf-8")) or {}).get("identity", {}).get("key") == _body_key), {})
                _ids = set()
                if _brec.get("survey"):
                    _sd = os.path.join(TREE, "..", _brec["survey"]["folder"])
                    for _fn, _k in (("deposits.geojson", "features"),):
                        _pth = os.path.join(_sd, _fn)
                        if os.path.isfile(_pth):
                            _ids |= {f_["properties"]["id"] for f_ in json.load(open(_pth, encoding="utf-8"))[_k]}
                    if _brec.get("energy"):
                        _ed = os.path.join(TREE, "..", _brec["energy"]["folder"])
                        if os.path.isfile(os.path.join(_ed, "fields.geojson")):
                            _ids |= {f_["properties"]["id"] for f_ in json.load(open(os.path.join(_ed, "fields.geojson"), encoding="utf-8"))["features"]}
                        if os.path.isfile(os.path.join(_ed, "coalfields.json")):
                            _ids |= {c_["id"] for c_ in json.load(open(os.path.join(_ed, "coalfields.json"), encoding="utf-8"))}
                    if _dep not in _ids:
                        problem(where, f"claim.deposit: no deposit, field or coalfield '{_dep}' in the survey of {_body_key}")
                else:
                    problem(where, f"claim: {_body_key or 'its body'} is not baked: no survey to claim a deposit in")
            if "exchange" in fc and (fc["exchange"] not in BRANDS or next((m for m in makers if m["key"] == fc["exchange"]), {}).get("business") != "exchange"):
                problem(where, f"exchange: no exchange '{fc['exchange']}' in Maker House")
            if plot is not None and covered > plot.get("area", 0):
                problem(where, f"its modules cover {covered:,.0f} m2, more than parcel {plot.get('number')} ({plot.get('area', 0):,.0f} m2)")
            # Where each module stands on the parcel (see `lay_out`): a line's in the order of its steps,
            # a station's or a warehouse's as listed.
            order = [(r["module"], r["count"]) for ln in fc.get("lines") or [] for r in (ln.get("most") or {}).get("modules", []) if r["count"]]
            order += [(m["slug"], n) for m, n in built]
            if plot is not None and order:
                street = next((st for st in x.get("streets", []) if st.get("slug") == (plot.get("address") or {}).get("street")), None)
                fc["layout"] = lay_out(where, plot, street, order)
# Parts: filed by hull (SFO/metadata/parts/<hull>/<code>.yaml); a part made of other parts has them
# in the folder named after its code (parts/<hull>/<code>/<code>-NNN.yaml). All to part.schema.yaml.
hull_of = {hl.get("slug"): hl for hl in hulls}
PART_SCHEMA = read_schema(os.path.join(TREE, "SFO", "schema", "part.schema.yaml"))
parts = []


def read_part(full, hull, parent, under, text=None):
    pt = load(full) if text is None else load_text(text, full, os.path.join(TREE, "SFO", "schema", "part.schema.yaml"))
    if text is not None:
        pt["_text"] = text
    ident = pt.get("identity") or {}
    code = str(ident.get("code", ""))
    if os.path.basename(full) != code + ".yaml":
        problem(full, "a part's file is named <its code>.yaml")
    if parent is None and not re.fullmatch(r"[A-Z0-9]+-[0-9]{2}", code):
        problem(full, "identity.code: the hull's (or the structure's) code, a dash, two digits (MC07-04)")
    if parent is not None and not re.fullmatch(re.escape(parent) + r"-[0-9]{3}", code):
        problem(full, f"identity.code: {parent}, a dash, three digits")
    if not ident.get("name"):
        problem(full, "identity: no name")
    if any(o["slug"] == code for o in parts):
        problem(full, f"code {code} twice")
    check_basis(pt, full)
    for group, props in pt.items():
        if group == "_text":
            continue
        if group == "basis":
            continue
        known = PART_SCHEMA["properties"].get(group)
        if known is None:
            problem(full, f"unknown group '{group}'")
            continue
        for k in props or {}:
            if k not in known["properties"]:
                problem(full, f"{group}: unknown property '{k}'")
    pt.update({"slug": code, "hull": hull, "under": under, "file": os.path.relpath(full, TREE)})
    if parent is not None:
        pt["parent"] = parent
    parts.append(pt)
    return code


for s in standards:
    if str(s.get("records")) != "parts":
        continue
    root = os.path.join(TREE, s["id"].split(" ")[0], "metadata", "parts")
    for hull in sorted(os.listdir(root)):
        hdir = os.path.join(root, hull)
        if not os.path.isdir(hdir):
            problem(hdir, "parts are filed in a folder named after their hull")
            continue
        if hull not in hull_of and hull not in {(g.get("built_of") or {}).get("parts") for g in gates} and hull not in {m.get("slug") for m in modules} and hull not in {(e_.get("built_of") or {}).get("parts") for e_ in equipment}:
            problem(hdir, f"no hull or industrial module '{hull}' in the SFO, and no gate or equipment built of it")
        for fn in sorted(os.listdir(hdir)):
            full = os.path.join(hdir, fn)
            if os.path.isdir(full):
                if not os.path.exists(full + ".yaml"):
                    problem(full, "a folder of parts is named after the part they make up")
                continue
            code = read_part(full, hull, None, s["id"])
            sub = full[:-5]
            for pn in sorted(os.listdir(sub)) if os.path.isdir(sub) else []:
                read_part(os.path.join(sub, pn), hull, code, s["id"])
# Parts as a rule (SFO 12): an equipment record's `built_of.list` derives its part records, through the one template the
# registry reader also writes (lib.part_yaml). They join `parts` as if read from the folder the record names.
from lib import derived_parts as _derived_parts
for _e in equipment:
    _raw = yaml.safe_load(open(os.path.join(TREE, _e["file"]), encoding="utf-8")) or {}
    _bo = _raw.get("built_of") or {}
    if not _bo.get("list"):
        continue
    _folder = _bo.get("parts") or _e["slug"]
    for _code, _text in _derived_parts(_raw):
        read_part(os.path.join(TREE, "SFO", "metadata", "parts", _folder, _code + ".yaml"), _folder, None, next((s_["id"] for s_ in standards if str(s_.get("records")) == "parts"), ""), text=_text)
# Mill stock: its material in that form; what a unit of it weighs, from the material's density and
# its size (kg per m2 of sheet, plate and film; kg per metre of bar, wire and tube).
import math
stock_of = {}
for ms in mill_stock:
    where = os.path.join(TREE, ms["file"])
    mf, size = ms.get("made_from") or {}, ms.get("size") or {}
    mat = next((m for m in materials if m.get("slug") == mf.get("material")), None)
    if mat is None:
        problem(where, f"made_from.material: no material '{mf.get('material')}'")
        continue
    if mf.get("form") not in ((mat.get("identity") or {}).get("form") or []):
        problem(where, f"made_from.form: {mf.get('material')} doesn't come as {mf.get('form')}")
    density = (mat.get("mass") or {}).get("density")
    t, d, w = size.get("thickness"), size.get("diameter"), size.get("wall")
    bw, bh = size.get("width"), size.get("height")
    if density and t and not d:
        ms["unit"], ms["weight"] = "m2", density * t / 1000
    elif density and bw and bh and w:
        # (A box section: four walls. The view keeps a tube's sizes in mm; width and height stay in m: each by its value.)
        _bm = lambda x: x if x < 1 else x / 1000
        bw_, bh_, w_ = _bm(bw), _bm(bh), _bm(w)
        ms["unit"], ms["weight"] = "m", density * (bw_ * bh_ - (bw_ - 2 * w_) * (bh_ - 2 * w_))
    elif density and d and w:
        ms["unit"], ms["weight"] = "m", density * math.pi * (d ** 2 - (d - 2 * w) ** 2) / 4e6
    elif density and d:
        ms["unit"], ms["weight"] = "m", density * math.pi * d ** 2 / 4e6
    else:
        ms["unit"], ms["weight"] = "kg", 1.0
    ph = ms.get("physical") or {}
    if ms["unit"] == "m2" and ph.get("length") and ph.get("width"):
        ms["piece_mass"] = ms["weight"] * ph["length"] * ph["width"]
    elif ms["unit"] == "m" and ph.get("length"):
        ms["piece_mass"] = ms["weight"] * ph["length"]
    stock_of[ms["slug"]] = ms
# (What a part names must be there: its mill stock, its processes, its designer, the standards it is
# built to, the parts it joins or stands in for. What its stock weighs follows from the quantity.)
codes = {pt["slug"] for pt in parts}
# Mass. A part that says its own mass has it. One that doesn't is given its share of what is left of
# its hull's frame mass (the game's figure), by its surface: an estimate, until what it is cut from
# says better. A part made of parts weighs what they do, each as many times as it has them.
kids = lambda c: [pt for pt in parts if pt.get("parent") == c["slug"]]
times = lambda pt: (pt.get("fit") or {}).get("count", 1)
for hl in hulls:
    mine = [pt for pt in parts if pt["hull"] == hl["slug"]]
    leaves = [pt for pt in mine if not kids(pt)]
    each = lambda pt: times(pt) * (times(next(o for o in mine if o["slug"] == pt["parent"])) if pt.get("parent") else 1)
    frame = ((hl.get("mass") or {}).get("frame") or 0) * 1000
    said = sum((pt.get("physical") or {}).get("mass", 0) * each(pt) for pt in leaves)
    rest = [pt for pt in leaves if (pt.get("physical") or {}).get("mass") is None and (pt.get("shape") or {}).get("surface_area")]
    surface = sum(pt["shape"]["surface_area"] * each(pt) for pt in rest)
    per_m2 = (frame - said) / surface if surface and frame > said else 0
    hl["estimate"] = {"per_m2": per_m2, "surface": surface, "said": said}
    for pt in leaves:
        if (pt.get("physical") or {}).get("mass") is not None:
            pt["mass"], pt["mass_from"] = pt["physical"]["mass"], "said"
        elif pt in rest and per_m2:
            pt["mass"], pt["mass_from"] = pt["shape"]["surface_area"] * per_m2, "share"
    for c in mine:
        if kids(c) and all("mass" in k for k in kids(c)):
            c["mass"], c["mass_from"] = sum(k["mass"] * times(k) for k in kids(c)), "parts"
    hl["parts_mass"] = sum(pt["mass"] * times(pt) for pt in mine if not pt.get("parent") and "mass" in pt)
# (A gate's structure: its parts weigh what they say; one made of parts, what they do.)
structures = []
for name in sorted({(g.get("built_of") or {}).get("parts") for g in gates} - {None}):
    mine = [pt for pt in parts if pt["hull"] == name]
    of = [g for g in gates if (g.get("built_of") or {}).get("parts") == name]
    for pt in mine:
        if (pt.get("physical") or {}).get("mass") is not None:
            pt["mass"], pt["mass_from"] = pt["physical"]["mass"], "said"
    for c in mine:
        if kids(c) and all("mass" in k for k in kids(c)):
            c["mass"], c["mass_from"] = sum(k["mass"] * times(k) for k in kids(c)), "parts"
    structures.append({"slug": name, "key": "gate:" + of[0]["slug"], "identity": {"name": name.replace("-", " ").capitalize()}, "making": of[0].get("making") or {},
                       "parts_mass": sum(pt["mass"] * times(pt) for pt in mine if not pt.get("parent") and "mass" in pt), "gates": [g["slug"] for g in of]})
# (An industrial module's components: each weighs what it says; the module, what they do.)
for m in modules:
    mine = [pt for pt in parts if pt["hull"] == m["slug"]]
    for pt in mine:
        if (pt.get("physical") or {}).get("mass") is not None:
            pt["mass"], pt["mass_from"] = pt["physical"]["mass"], "said"
    for c in mine:
        if kids(c) and all("mass" in k for k in kids(c)):
            c["mass"], c["mass_from"] = sum(k["mass"] * times(k) for k in kids(c)), "parts"
    if mine:
        m["parts_mass"] = sum(pt["mass"] * times(pt) for pt in mine if not pt.get("parent") and "mass" in pt)
codes = {pt["slug"] for pt in parts}
for pt in parts:
    where = os.path.join(TREE, pt["file"])
    mf, ident = pt.get("made_from") or {}, pt.get("identity") or {}
    ms = stock_of.get(mf.get("item"))
    bought = next((g for g in goods if g["slug"] == mf.get("item")), None) if ms is None else None
    if bought is not None:
        # (Bought in whole and built in: a good, by weight.)
        pt["bought"] = bought["slug"]
        if "quantity" in mf:
            pt["stock_mass"] = mf["quantity"]
    elif "item" in mf and ms is None:
        problem(where, f"made_from.item: no mill stock and no good '{mf['item']}'")
    elif ms is not None and "quantity" in mf:
        # (The record says kg. The page still shows it as the stock is counted: m2 of sheet, m of bar.)
        pt["stock_mass"] = mf["quantity"]
        mf["quantity"] = float(f"{mf['quantity'] / ms['weight']:.10g}")
        pt["stock_unit"] = ms["unit"]
    made_in = (pt.get("making") or {}).get("module")
    if made_in is not None and made_in not in mod_of:
        problem(where, f"making.module: no module '{made_in}' in the SFO")
    ph = pt.get("physical") or {}
    for lo, hi in (("operating_min_temperature", "operating_max_temperature"), ("storage_min_temperature", "storage_max_temperature")):
        if lo in ph and hi in ph and ph[lo] > ph[hi]:
            problem(where, f"physical: {lo.replace('_', ' ')} is above {hi.replace('_', ' ')}")
    if "designer" in ident and ident["designer"] not in BRANDS:
        problem(where, f"identity.designer: no company '{ident['designer']}' in Maker House")
    for sid in ident.get("standards") or []:
        if sid not in {s["id"] for s in standards}:
            problem(where, f"identity.standards: no standard '{sid}'")
    for other in (ident.get("interchangeable_with") or []) + ((pt.get("fit") or {}).get("joins_to") or []):
        if other not in codes:
            problem(where, f"no part '{other}'")
for m in modules:
    for kind in (m.get("rate") or {}).get("stores") or []:
        if kind not in GOODS_KINDS:
            problem(os.path.join(TREE, m["file"]), f"rate.stores: the game has no kind of goods '{kind}'")
for pr in processes + modules:
    where = os.path.join(TREE, pr["file"])
    for group in ("inputs", "outputs"):
        for listed in ("materials", "consumables", "products", "by_products", "waste"):
            for x in (pr.get(group) or {}).get(listed, []) or []:
                item = x.get("item")
                if item is None and not x.get("name"):
                    problem(where, f"{group}.{listed}: an entry needs item or name")
                elif item is not None and item not in symbols and item not in slugs:
                    problem(where, f"{group}.{listed}: '{item}' is no element's symbol and no material's or good's file name")

ids = {s["id"] for s in standards}
for s in standards:
    check_standard(s, ids)
cited = {}
for s in standards:
    for r in s.get("refs", []) or []:
        cited.setdefault(r, []).append(s["id"])


# ---------------------------------------------------------------- the game's content
def ron_str(t):
    return '"' + str(t).replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ").strip() + '"'


def caps(t):
    # (The game's font is capitals.)
    return " ".join(str(t).split()).upper()


def ron_value(v):
    if isinstance(v, list):
        return f"Range({float(v[0])!r}, {float(v[1])!r})"
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        return f"Num({float(v)!r})"
    return f"Text({ron_str(caps(v))})"


def enum(e):
    return "".join(w.capitalize() for w in str(e).split("_"))


# (The eight RON files this build once wrote for the game are gone (2026-10-07): the game reads the registry's records
# directly through crates/registry, and keeps only its own hand-kept files beside them: aliases, prices, shapes, sheet.)


# ---------------------------------------------------------------- reports
# Where the registry does not yet join up. Nothing here stops the build: these are gaps to close,
# not mistakes. Each report is a table; a row is ok, a gap, or a note.
reports = []


# Each report belongs to a group on the page, and names the docs that say its rules (paths from the repository root).
REPORT_GROUPS = {"ships": ["equipment-parts", "equipment", "mounts", "members", "hulls", "budgets", "volume", "dimensions", "structure-mc-07", "shock", "mass", "chains"],
                 "industry": ["plant", "balance", "materials", "goods", "takers", "power", "gates"],
                 "people": ["needs", "work", "buildings", "census", "traffic"],
                 "sky": ["worlds", "celestial"], "engine": ["clocks"],
                 "quality": ["confidence", "invented", "review", "revisions", "dictionary"]}
REPORT_DOCS = {"mounts": ["docs/ships/registry-note-for-ships.md"], "members": ["docs/ships/registry-note-for-ships.md"], "budgets": ["docs/ships/registry-note-for-ships.md", "docs/ships/equipment-review.md"],
               "needs": ["docs/registry-people.md", "docs/registry-spirit.md"], "work": ["docs/registry-people.md"], "buildings": ["docs/registry-people.md"], "census": ["docs/registry-people.md", "docs/registry-order.md"],
               "traffic": ["docs/registry-people.md", "docs/registry-stocking.md"], "worlds": ["docs/survey-contract.md"], "celestial": ["docs/survey-contract.md"], "clocks": ["docs/tick-tree.md"],
               "balance": ["docs/registry-recipes.md"], "materials": ["docs/registry-recipes.md"], "goods": ["docs/registry-recipes.md"], "takers": ["docs/registry-recipes.md"],
               "confidence": ["docs/registry-audit.md"], "invented": ["docs/registry-audit.md"], "review": ["docs/registry-audit.md"], "revisions": ["docs/registry-integration.md"], "dictionary": ["standards/README.md"],
               "equipment-parts": ["docs/registry-recipes.md"], "chains": ["docs/registry-recipes.md"], "plant": ["docs/registry-recipes.md"], "hulls": ["docs/registry-integration.md"], "gates": ["docs/standards.md"]}


def report(key, title, about, columns, rows):
    group = "chains" if key.startswith("chain-") else next((g for g, ks in REPORT_GROUPS.items() if key in ks), "other")
    reports.append({"key": key, "title": title, "about": about, "columns": columns, "rows": rows, "group": group, "docs": REPORT_DOCS.get("chains" if group == "chains" else key, []),
                    "gaps": sum(1 for r in rows if r["state"] == "gap"), "ok": sum(1 for r in rows if r["state"] == "ok")})


def row(state, *cells):
    return {"state": state, "cells": [c if isinstance(c, dict) else {"t": str(c)} for c in cells]}


link = lambda text, key: {"t": str(text), "k": key}
tonnes = lambda kg_: f"{kg_ / 1000:,.2f} t" if abs(kg_) >= 1000 else f"{kg_:,.0f} kg"
run_at = {}
for ad in administrations:
    for x in ad["bodies"]:
        for fc in x.get("facilities", []):
            for name in fc.get("processes") or []:
                run_at.setdefault(name, []).append(fc)
lined = {name for ad in administrations for x in ad["bodies"] for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln for name in [ln["process"]] + (ln.get("also") or [])}
part_link = lambda pt: link(f"{pt['slug']} {pt['identity'].get('name', '')}", "part:" + pt["slug"])

# (The processes that make a material as ingot: what a mill's stock starts from.)
# (Nothing is made of nothing: a part a module makes has something going in, its stock or its own parts.)
for pt in parts:
    if (pt.get("making") or {}).get("module") and not (pt.get("made_from") or {}).get("item") and not kids(pt):
        problem(os.path.join(TREE, pt["file"]), "making.module: it is made in a module and nothing goes in: it needs what it is made from, or parts of its own")
# (The lines built with a module that makes something: where a part made in that module can be made.)
shop_lines = lambda mod: [ln for ad in administrations for x in ad["bodies"] for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln and any(r["module"] == mod and r["can"] for r in ln["most"]["modules"])]
makers_of = lambda item: [m for m in modules if any(r.get("product") == item for r in m.get("recipes") or [])]
ingot_makers = lambda mat: [q for q in routes if any(o.get("item") == mat and o.get("form") == "ingot" for o in (q.get("outputs") or {}).get("products") or [])]
# 1. The chain from a hull down to rock: how far each part gets.
eq_of = {e["slug"]: e for e in equipment}
# (A hull's fit and nozzles against its slots: each names a slot it has, and what is fitted is of the slot's kind and no bigger.)
for hl in hulls:
    where = os.path.join(TREE, hl["file"])
    slots = {s_["name"]: s_ for s_ in hl.get("slots") or []}
    if not slots:
        continue
    for ft in hl.get("fit") or []:
        s_, e_ = slots.get(ft.get("slot")), eq_of.get(ft.get("item"))
        if s_ is None:
            problem(where, f"fit: it has no slot '{ft.get('slot')}'")
        elif e_ is not None and (e_.get("identity") or {}).get("slot") != s_["kind"]:
            problem(where, f"fit: {ft['item']} is for a {(e_.get('identity') or {}).get('slot')} slot; {ft['slot']} is a {s_['kind']} slot")
        elif e_ is not None and ((e_.get("performance") or {}).get("size_class") or 1) > s_["size"]:
            problem(where, f"fit: {ft['item']} is of size class {e_['performance']['size_class']}, too big for {ft['slot']} (size {s_['size']})")
    for t_ in hl.get("thrusters") or []:
        if t_.get("slot") not in slots:
            problem(where, f"thrusters: nozzle {t_.get('nozzle')} is driven by '{t_.get('slot')}', which is no slot of it")
# (The hulls whose parts are listed: the others are coarse, in the Hulls report.)
built_hulls = [hl for hl in hulls if hl.get("parts_mass")]
# (Equipment whose parts are listed: each part weighs what it says; one made of parts, what they do.)
built_equipment = []
for e in equipment:
    name = (e.get("built_of") or {}).get("parts")
    if not name:
        continue
    mine = [pt for pt in parts if pt["hull"] == name]
    if not mine:
        problem(os.path.join(TREE, e["file"]), f"built_of.parts: no folder of parts '{name}'")
        continue
    for pt in mine:
        if (pt.get("physical") or {}).get("mass") is not None:
            pt["mass"], pt["mass_from"] = pt["physical"]["mass"], "said"
    for c in mine:
        if kids(c) and all("mass" in k for k in kids(c)):
            c["mass"], c["mass_from"] = sum(k["mass"] * times(k) for k in kids(c)), "parts"
    e["parts_mass"] = sum(pt["mass"] * times(pt) for pt in mine if not pt.get("parent") and "mass" in pt)
    built_equipment.append({"slug": name, "key": "eq:" + e["slug"], "identity": {"name": e["identity"]["name"]}, "making": e.get("making") or {}, "parts_mass": e["parts_mass"], "equipment": e})
makes_good = lambda slug: [m for m in modules if any(rc.get("product") == slug for rc in m.get("recipes") or [])]
rows = []
for be in built_equipment:
    e = be["equipment"]; said = (e.get("physical") or {}).get("mass") or 0
    mine = [pt for pt in parts if pt["hull"] == be["slug"] and not pt.get("parent")]
    off = abs(be["parts_mass"] - said) / said if said else 1
    rows.append(row("ok" if off < 0.01 else "gap", link(e["identity"]["name"], "eq:" + e["slug"]), len(mine), tonnes(be["parts_mass"]), tonnes(said), "agree" if off < 0.01 else f"{off:.0%} apart"))
report("equipment-parts", "Equipment: what each is built of", "Each piece of equipment whose parts are listed: how many kinds of part, what they weigh together, and what the product is said to weigh. They should agree.", ["Equipment", "Kinds of part", "Its parts weigh", "It is said to weigh", "State"], rows)
for hl in built_hulls + structures + built_equipment:
    mine = [pt for pt in parts if pt["hull"] == hl["slug"]]
    leaves = [pt for pt in mine if not kids(pt)]
    rows = []
    how = lambda pt: [q for q in [(pt.get("making") or {}).get("module")] if q]
    # (The hull itself, and each part made of parts: is it said how it is put together, and is a yard built to do it?)
    for name, key, procs in [(hl["identity"]["name"] + " (hull)", link(hl["identity"]["name"] + ("" if "key" in hl else " (hull)"), hl.get("key", "hull:" + hl["slug"])), [q for q in [(hl.get("making") or {}).get("module")] if q])] + [(None, part_link(pt), how(pt)) for pt in mine if kids(pt)]:
        steps = [("says how it is put together", bool(procs) and all(q in mod_of for q in procs)), ("somewhere is built to do it", bool(procs) and all(shop_lines(q) for q in procs))]
        if name and hl.get("fit"):
            out = (hl.get("making") or {}).get("fitting_out")
            steps += [("says how it is fitted out", out in mod_of), ("a yard is built to fit it out", bool(shop_lines(out)))]
        reached = next((i for i, (_, good) in enumerate(steps) if not good), len(steps))
        rows.append(row("ok" if reached == len(steps) else "gap", key, f"{reached} of {len(steps)}", "complete" if reached == len(steps) else "stops at: " + steps[reached][0]))
    for pt in leaves:
        ms = stock_of.get((pt.get("made_from") or {}).get("item"))
        if pt.get("bought"):
            steps = [("has a mass", "mass" in pt), ("says what it is made from", True), ("a module has a recipe that makes that good", bool(makes_good(pt["bought"]))),
                     ("says how it is built in", bool(how(pt)) and all(q in mod_of for q in how(pt))), ("a yard is built to build it in", bool(how(pt)) and all(shop_lines(q) for q in how(pt)))]
            reached = next((i for i, (_, good) in enumerate(steps) if not good), len(steps))
            rows.append(row("ok" if reached == len(steps) else "gap", part_link(pt), f"{reached} of {len(steps)}", "complete" if reached == len(steps) else "stops at: " + steps[reached][0]))
            continue
        steps = [
            ("has a mass", "mass" in pt),
            ("says what it is cut from", ms is not None),
            ("a module has a recipe that makes that stock", bool(ms and makers_of(ms["slug"]))),
            ("a facility is built to make it", any(q in lined for q in (ms or {}).get("routes") or [])),
            ("the ingot that stock is made from can be made", any(q["slug"] in lined for q in ingot_makers((ms or {}).get("made_from", {}).get("material"))) or (ms is not None and "ingot" not in ((next((m_ for m_ in materials if m_.get("slug") == ms.get("made_from", {}).get("material")), {}).get("identity") or {}).get("form") or []))),   # (what is not a metal starts from no ingot)
            ("says how it is made from its stock", bool(how(pt)) and all(q in mod_of for q in how(pt))),
        ] + ([("a yard is built to cut and form that stock", any(q in lined for q in stock_of[ms["slug"] + "-PANEL"].get("routes") or []))] if ms and ms["slug"] + "-PANEL" in stock_of and "welding-bay" in how(pt) else []) + [
            ("a yard is built to make it", bool(how(pt)) and all(shop_lines(q) for q in how(pt))),
        ]
        reached = next((i for i, (_, good) in enumerate(steps) if not good), len(steps))
        rows.append(row("ok" if reached == len(steps) else "gap", part_link(pt), f"{reached} of {len(steps)}", "complete" if reached == len(steps) else "stops at: " + steps[reached][0]))
    report(f"chain-{hl['slug']}", f"Chain: {hl['identity']['name']} down to a factory", "The thing itself and each part made of parts: is it said how it is put together, and is somewhere built to do it. Each part that is not made of other parts: does it have a mass, say what it is cut from, does a process make that stock, has the process real steps, is a facility built to run it, can the ingot that stock starts from be made (a process with a line built for it), is it said how the part is made from its stock, and is a yard built to make it.", ["Part", "Links made", "Where it stops"], rows)
    # (Where it can be built, and how fast at most: the yards with a line for its assembly, flat out.)
    built = []
    for ad in administrations:
        for x in ad["bodies"]:
            for fc in x.get("facilities", []):
                for ln in fc.get("lines") or []:
                    if "most" in ln and any(r["module"] == (hl.get("making") or {}).get("module") and r["can"] for r in ln["most"]["modules"]) and ln["most"]["output"] and hl.get("parts_mass"):
                        # (Fitted out on the same line, where it can be: its equipment goes through the dock too.)
                        fits = any(r["module"] == (hl.get("making") or {}).get("fitting_out") for r in ln["most"]["modules"])
                        through = hl["parts_mass"] + (sum((eq_of.get(ft.get("item"), {}).get("physical") or {}).get("mass", 0) for ft in hl.get("fit") or []) if fits else 0)
                        built.append({"at": fc["name"], "settlement": x["name"], "days": through / 1000 / ln["most"]["output"] / 24, "fitted": fits})
    hl["built"] = built

# (A rig's turning part: how fast, for the weight it gives. And what it eats flat out.)
for ad in administrations:
    for x in ad["bodies"]:
        if x.get("kind") != "rig":
            continue
        sp = x.get("spin") or {}
        if sp.get("radius") and sp.get("gravity"):
            w_ = (sp["gravity"] / sp["radius"]) ** 0.5
            x["spin_worked"] = {"rate": w_, "rpm": w_ * 60 / (2 * math.pi), "g": sp["gravity"] / G0}
        cargo_ = max([(e_.get("performance") or {}).get("capacity", 0) for e_ in equipment if (e_.get("identity") or {}).get("slot") == "cargo"] or [0])
        x["feed"] = [{"item": i["item"], "rate": i["rate"], "loads": i["rate"] * 24 * 1000 / cargo_ if cargo_ else None, "hold": cargo_} for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln for i in ln["most"]["supplies"] if next((g_ for g_ in goods if g_["slug"] == i["item"]), {}).get("identity", {}).get("kind") == "rock"]

# 1b. Equipment: what each hull is fitted with, each against the game's module, and whether it is
# said what it is made of.
eq_by = {e["slug"]: e for e in equipment}
# (The game reads equipment, hulls, structures and gate rings from these records now: there is no
# second copy of the game's to hold them to.)
game_key = lambda key: (key or "").partition(".")[2]           # (equipment.drive.torch.s1 is the game's drive.torch.s1)
rows = []
for hl in hulls:
    fitted = 0.0
    for ft in hl.get("fit") or []:
        e = eq_by.get(ft.get("item"))
        if e is None:
            problem(os.path.join(TREE, hl["file"]), f"fit: no equipment '{ft.get('item')}' in the SFO")
            continue
        fitted += (e.get("physical") or {}).get("mass", 0)
    hl["fitted_mass"] = fitted
for e in equipment:
    mass = (e.get("physical") or {}).get("mass")
    on = [hl["identity"]["name"] for hl in hulls + gates if any(ft.get("item") == e["slug"] for ft in hl.get("fit") or [])]
    rows.append(row("note", link(e["identity"]["name"], "eq:" + e["slug"]), ", ".join(on) or "no hull", tonnes(mass) if mass is not None else "", (e.get("performance") or {}).get("kind", ""), "not yet said"))
report("equipment", "Equipment: what hulls are fitted with", "Each piece of ship equipment: the hulls fitted with it, what it weighs and what kind of device it is. The game reads these records; what each is made of is its parts (the Equipment parts report).", ["Equipment", "Fitted to", "Mass", "Kind", "Made of"], rows)

# 1c. Stargates: what opening and holding each ring's tube costs, by the laws (Dogma's Tube; the
# same formulas as crates/physics/src/hyper.rs), and each ring against the game's.
LAW = {l["identity"]["label"]: l["value"] for s_ in dogma for l in s_["laws"]}
tube_time = lambda m, span_ly: LAW["TUBE_T_LY"] * span_ly * LY * m ** LAW["TUBE_GAMMA"]      # (the law is for each metre)
tube_energy = lambda m, span_ly: LAW["TUBE_EPS"] * m * span_ly * LY * math.e      # (at its natural time)
station = next((m for m in modules if (m.get("rate") or {}).get("power")), None)
rows = []
for g in gates:
    d, span = (g.get("size") or {}).get("opening"), (g.get("performance") or {}).get("span")
    if d and span and all(k in LAW for k in ("TUBE_T_LY", "TUBE_GAMMA", "TUBE_EPS", "TUBE_RHO", "TUBE_K", "TUBE_HOLD")):
        mu = LAW["TUBE_RHO"] * d ** LAW["TUBE_K"]
        opening = tube_energy(mu, span)
        ships = [("A ship of 100 t", 1e5)] + [(hl["identity"]["name"] + ", loaded", hl["parts_mass"] + hl.get("fitted_mass", 0) + 1000 * ((hl.get("mass") or {}).get("fuel", 0) + (hl.get("mass") or {}).get("hold", 0))) for hl in hulls if hl.get("parts_mass")] + [("A hauler of 1,000 t", 1e6), ("A capital ship of 100,000 t", 1e8)]
        g["worked"] = {
            "tube_mass": mu, "open_energy": opening, "open_time": tube_time(mu, span), "hold_power": opening / LAW["TUBE_HOLD"], "hold_months": LAW["TUBE_HOLD"] / (30 * 86400),
            "stations": (opening / LAW["TUBE_HOLD"] / 1e6 / station["rate"]["power"]) if station else None, "station": station["slug"] if station else None,
            "crossings": [{"what": w, "mass": m, "time": tube_time(m, span), "energy": tube_energy(m, span)} for w, m in ships],
        }
        hold = f"{g['worked']['hold_power'] / 1e9:,.0f} GW to hold at its full span"
    else:
        hold = "not worked out: its opening, its span or a law is missing"
    rows.append(row("ok" if span else "gap", link(g["identity"]["name"], "gate:" + g["slug"]), f"{span:g} ly" if span else "", "read by the game", hold))
    st = next((x for x in structures if g["slug"] in x["gates"]), None)
    if st and "worked" in g:
        mine = [pt for pt in parts if pt["hull"] == st["slug"]]
        each = lambda pt: times(pt) * (times(next(o for o in mine if o["slug"] == pt["parent"])) if pt.get("parent") else 1)
        can = lambda proc: sum(ln["most"]["output"] for ad in administrations for x in ad["bodies"] for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln and (proc in [ln["process"]] + (ln.get("also") or []) or any(r["module"] == proc and r["can"] for r in ln["most"]["modules"])))
        run_in = lambda proc: ", ".join(sorted({fc["name"] for ad in administrations for x in ad["bodies"] for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln and (proc in [ln["process"]] + (ln.get("also") or []) or any(r["module"] == proc and r["can"] for r in ln["most"]["modules"]))}))
        steps, stock_t, ingot_t, made_t = [], {}, {}, {}
        for pt in [pt for pt in mine if not kids(pt)]:
            ms = stock_of.get((pt.get("made_from") or {}).get("item"))
            if ms is None:
                continue
            stock_t[ms["slug"]] = stock_t.get(ms["slug"], 0) + pt.get("stock_mass", 0) * each(pt) / 1000
            for q in [q for q in [(pt.get("making") or {}).get("module")] if q]:
                made_t[q] = made_t.get(q, 0) + pt.get("mass", 0) * each(pt) / 1000
        def step(what, tonnes_, procs):
            procs = [q for q in ([procs] if isinstance(procs, str) else procs or []) if q]
            rate = sum(can(q) for q in procs)
            steps.append({"what": what, "tonnes": tonnes_, "at": ", ".join(sorted({n_ for q in procs for n_ in run_in(q).split(", ") if n_})), "rate": rate, "days": tonnes_ / rate / 24 if rate else None})
        for q, t_ in made_t.items():
            step(f"{mod_of[q]['identity']['name']}: its parts made from stock", t_, q)
        for code, t_ in stock_t.items():
            ms = stock_of[code]
            step(f"{ms['identity']['name']} rolled or drawn", t_, ms.get("routes") or [])
            mat = (ms.get("made_from") or {}).get("material")
            ingot_t[mat] = ingot_t.get(mat, 0) + t_
        for mat, t_ in ingot_t.items():
            casts = [q["slug"] for q in ingot_makers(mat) if q["slug"] in lined]
            step(f"{next(m_ for m_ in materials if m_.get('slug') == mat)['identity']['name']} cast as ingot (at least: the mills' own losses come on top)", t_, casts)
        cargo = max([(ft_.get("performance") or {}).get("capacity", 0) for ft_ in equipment if (ft_.get("identity") or {}).get("slot") == "cargo"] or [0])
        for c in [c for c in mine if kids(c)]:
            for q_ in [q for q in [(c.get("making") or {}).get("module")] if q]:
                step(f"{mod_of[q_]['identity']['name']}: its {c['identity']['name'].lower()}s put together", c.get("mass", 0) * each(c) / 1000, q_)
        fit_mass = 0.0
        for ft in g.get("fit") or []:
            if ft.get("item") not in eq_by:
                problem(os.path.join(TREE, g["file"]), f"fit: no equipment '{ft.get('item')}' in the SFO")
            else:
                fit_mass += (eq_by[ft["item"]].get("physical") or {}).get("mass", 0) * ft.get("count", 1)
        coils = sum(ft.get("count", 1) for ft in g.get("fit") or [] if ft.get("item") == "throat-coil")
        # (Its heat: a fusion plant turns half its fuel's energy into power, so as much again is
        # heat to shed. Shed from the ring's own skin, which then runs at T: P = e * sigma * A * T^4.)
        skin = sum((pt.get("shape") or {}).get("surface_area", 0) * times(pt) for pt in mine if not pt.get("parent"))
        EMISS = 0.9
        g["worked"]["fit_mass"] = fit_mass
        g["worked"]["coil_power"] = g["worked"]["hold_power"] / coils if coils else None
        g["worked"]["skin"] = {"area": skin, "heat": g["worked"]["hold_power"], "temperature": (g["worked"]["hold_power"] / (EMISS * SIGMA * skin)) ** 0.25, "emissivity": EMISS} if skin else None
        g["worked"]["build"] = {"structure": st["slug"], "mass": st["parts_mass"], "stock": [{"item": k, "tonnes": v} for k, v in stock_t.items()], "steps": steps,
                                "stations": math.ceil(g["worked"]["stations"]) if g["worked"].get("stations") else None,
                                "trips": math.ceil(st["parts_mass"] / cargo) if cargo else None, "hold": cargo}
    rows.append(row("ok" if st and st["parts_mass"] else "gap", link(g["identity"]["name"], "gate:" + g["slug"]), "", "", f"its ring weighs {tonnes(st['parts_mass'])}, of parts that are guesses" if st and st["parts_mass"] else "what the ring weighs and is made of: not yet said"))
    rows.append(row("gap", link(g["identity"]["name"], "gate:" + g["slug"]), "", "", "what in the ring holds the tube, what its power stations are made of, how it is assembled in orbit and what carries its parts there: not yet said"))
for ad in administrations:
    for x in ad["bodies"]:
        g = gate_of.get((x.get("gate") or {}).get("ring"))
        if g and "worked" in g:
            # (How far the gate reaches: from where the two stars are, in the celestial registry.)
            here_, there_ = (yaml.safe_load(open(os.path.join(TREE, "Celestial", "metadata", "systems", s_ + ".yaml"), encoding="utf-8")) for s_ in (ad["slug"], REGISTRY[REGISTRY_SYSTEM[x["gate"]["to"]]][0].split(os.sep)[-1][:-5]))
            x["gate"]["distance"] = float(f"{sum((a - b) ** 2 for a, b in zip(here_['position']['from_home'], there_['position']['from_home'])) ** 0.5 / LY:.4g}")
            share = x["gate"]["distance"] / g["performance"]["span"]
            x["gate_worked"] = {"ring": g["identity"]["name"], "span": g["performance"]["span"], "hold_power": g["worked"]["hold_power"] * share, "open_energy": g["worked"]["open_energy"] * share,
                                "stations": math.ceil(g["worked"]["stations"] * share) if g["worked"].get("stations") else None,
                                "crossings": [{"what": c["what"], "mass": c["mass"], "time": c["time"] * share, "energy": c["energy"] * share} for c in g["worked"]["crossings"]]}
            rows.append(row("ok" if share <= 1 else "gap", link(x["name"], f"bd:{ad['slug']}:{x['slug']}"), f"{x['gate']['distance']:g} ly", f"a {g['identity']['name']}, which spans {g['performance']['span']:g} ly", f"{x['gate_worked']['hold_power'] / 1e9:.1f} GW to hold, {x['gate_worked']['stations']} power stations" if share <= 1 else "further than its ring spans"))
report("gates", "Stargates: each ring, and what it costs to hold", "Each gate ring: its span, and the power its tube takes to hold, worked out from the laws. The game reads these records; a gap is a ring that or something a ring does not yet say.", ["Ring", "Span", "Against the game", "Note"], rows)

# 1d. Plant: what each industrial module is built of.
rows = []
for m in modules:
    mine = [pt for pt in parts if pt["hull"] == m["slug"]]
    tops = [pt for pt in mine if not pt.get("parent")]
    leaves = [pt for pt in mine if not kids(pt)]
    cut = [pt for pt in leaves if (pt.get("made_from") or {}).get("item") in stock_of]
    size = m.get("size") or {}
    floor = size.get("length", 0) * size.get("width", 0)
    need = {}
    for pt in cut:
        need[pt["made_from"]["item"]] = need.get(pt["made_from"]["item"], 0) + pt.get("stock_mass", 0) * times(pt) / 1000
    m["stock_needed"] = [{"item": k, "tonnes": v} for k, v in need.items()]
    rows.append(row("gap" if not tops or len(cut) < len(leaves) else "ok", link(m["identity"]["name"], "mod:" + m["slug"]), len(tops) or "none listed", sum(times(pt) for pt in tops) or "", tonnes(m["parts_mass"]) if m.get("parts_mass") else "", f"{m['parts_mass'] / floor:,.0f} kg/m2" if m.get("parts_mass") and floor else "", f"{len(cut)} of {len(leaves)}" if leaves else "", ", ".join(f"{v:,.0f} t of {stock_of[k]['identity']['name']}" for k, v in need.items())))
report("plant", "Plant: what each industrial module is built of", "Each industrial module: the kinds of component it is built of, how many pieces that is, what they weigh together, that weight over its floor, and how many of its components say what they are made from. A gap is a module with no components listed, or with components that do not yet say what they are made from.", ["Module", "Kinds of component", "Pieces", "Weight", "Over its floor", "Say what they are made from", "Stock they take"], rows)

# 1e. Hulls: each hull's budget (what it weighs, what it carries, what its equipment takes of its
# space, how hard it can push), and the loads the registry has to move as loads of its hold.
rows = []
big_loads = []
for ad in administrations:
    for x in ad["bodies"]:
        for r_ in x.get("feed") or []:
            big_loads.append((f"a day's rock for {x['name']}, flat out", r_["rate"] * 24))
for st in structures:
    big_loads.append((f"one {st['identity']['name'].lower()}", st["parts_mass"] / 1000))
for hl in sorted(hulls, key=lambda h_: h_["identity"].get("revision") == "outdated"):
    ms, ds, sz = hl.get("mass") or {}, hl.get("design") or {}, hl.get("size") or {}
    frame = hl.get("parts_mass", 0) / 1000 or ms.get("frame", 0)
    fitted = hl.get("fitted_mass", 0) / 1000
    fuel, hold = ms.get("fuel", 0), ms.get("hold", 0)
    loaded = frame + fitted + fuel + hold
    fit_vol = sum((eq_by.get(ft.get("item"), {}).get("physical") or {}).get("volume", 0) for ft in hl.get("fit") or [])
    hl["budget"] = {"frame": frame, "fitted": fitted, "fuel": fuel, "hold": hold, "loaded": loaded, "fit_volume": fit_vol,
                    "payload": hold / loaded if loaded else 0, "main_g": ds.get("main_thrust", 0) * 1e6 / (loaded * 1000) / G0 if loaded else 0, "lift_g": ds.get("lift_thrust", 0) * 1e6 / (loaded * 1000) / G0 if loaded else 0,
                    "loads": [{"what": w_, "tonnes": t_, "loads": t_ / hold if hold else None} for w_, t_ in big_loads]}
    old = hl["identity"].get("revision") == "outdated"
    rows.append(row("note" if old else "ok", link(hl["identity"]["name"], "hull:" + hl["slug"]), "outdated: not to be balanced against" if old else "current", hl["identity"].get("class", ""), f"{frame:,.0f} t" + ("" if hl.get("parts_mass") else " (the game's)"), f"{loaded:,.0f} t", f"{hold:g} t ({100 * hold / loaded:.0f}%)" if loaded else "", f"{fuel:g} t",
                    f"{100 * fit_vol / sz['volume']:.1f}% of {sz['volume']:,} m3" if sz.get("volume") else "", f"{hl['budget']['main_g']:.1f} g", f"{hl['budget']['lift_g']:.2f} g",
                    "; ".join(f"{b_['loads']:,.0f} loads for {b_['what']}" for b_ in hl["budget"]["loads"] if b_["loads"])))
report("hulls", "Hulls: each one's budget, and the loads to be moved", "Each hull: what it weighs bare and loaded, what it carries and what share of its loaded weight that is, its fuel, what its equipment takes of its space, how hard its main drive and its lift push it loaded, and how many loads of its hold the registry's big loads are. An outdated hull is an early rough guess the game still has: listed so it is not forgotten, and not to be balanced against. Nothing here is a gap.", ["Hull", "Standing", "Class", "Bare", "Loaded", "Carries", "Fuel", "Equipment takes", "Main drive", "Lift", "Loads"], rows)

# 2. Mass: what a thing weighs against what it is made of.
rows = []
for hl in built_hulls:
    frame = ((hl.get("mass") or {}).get("frame") or 0) * 1000
    got = hl.get("parts_mass", 0)
    if frame:
        d = got - frame
        rows.append(row("ok" if abs(d) <= 0.01 * frame else "gap", link(hl["identity"]["name"] + " (hull)", "hull:" + hl["slug"]), tonnes(frame), "its frame, in the game", tonnes(got), "its parts", f"{d:+,.0f} kg ({100 * d / frame:+.1f}%)"))
    for c in [pt for pt in parts if pt["hull"] == hl["slug"] and kids(pt)]:
        own, sub = (c.get("physical") or {}).get("mass"), sum(k["mass"] * times(k) for k in kids(c) if "mass" in k)
        missing = [k for k in kids(c) if "mass" not in k]
        if missing:
            rows.append(row("gap", part_link(c), "", "", tonnes(sub), "its parts", f"{len(missing)} of its parts have no mass"))
        elif own is not None:
            rows.append(row("ok" if abs(own - sub) <= 0.01 * max(own, 1) else "gap", part_link(c), tonnes(own), "its own record", tonnes(sub), "its parts", f"{sub - own:+,.0f} kg"))
        else:
            rows.append(row("ok", part_link(c), tonnes(sub), "its parts", "", "", "worked out from its parts"))
    for pt in [pt for pt in parts if pt["hull"] == hl["slug"] and "stock_mass" in pt and "mass" in pt]:
        lost = pt["stock_mass"] - pt["mass"]
        rows.append(row("ok" if lost >= 0 else "gap", part_link(pt), tonnes(pt["mass"]), "its own record", tonnes(pt["stock_mass"]), "the stock it takes", f"{lost:,.0f} kg lost as offcut" if lost >= 0 else "it weighs more than the stock it is cut from"))
report("mass", "Mass: a thing against what it is made of", "A hull against its parts, a part made of parts against them, a part against the stock it is cut from. A gap is a difference of more than 1%, a part with no mass, or a part heavier than its stock.", ["What", "Weighs", "By", "Against", "By", "Difference"], rows)

# 2b. Structure: each hull's parts against the loads it is designed to stand, and the hull against
# real vehicles. First-order sizing: a pressurised part as a cylinder across its smaller dimension
# (hoop stress), a landing leg as a column (strength, and Euler buckling with pinned ends).
_src_path = os.path.join(TREE, "sources", "research_rock_structure.json")
BENCH = json.load(open(_src_path, encoding="utf-8")) if os.path.exists(_src_path) else {}
bench = lambda topic, fig: (BENCH.get(topic) or {}).get(fig) or {}
mat_by = {m.get("slug"): m for m in materials}
for hl in built_hulls:
    ds = hl.get("design") or {}
    if not ds:
        continue
    mine = [pt for pt in parts if pt["hull"] == hl["slug"]]
    leaves = [pt for pt in mine if not kids(pt)]
    stock = lambda pt: stock_of.get((pt.get("made_from") or {}).get("item"))
    mech = lambda ms: (mat_by.get((ms.get("made_from") or {}).get("material")) or {}).get("mechanical") or {}
    longest = lambda pt: max((pt.get("physical") or {}).get(k, 0) for k in ("length", "width", "height"))
    case = lambda pt: (pt.get("limits") or {}).get("load_case")
    SF, PF, eta = ds.get("safety_factor", 1), ds.get("pressure_factor", 1), ds.get("strut_efficiency", 1)
    frame = ((hl.get("mass") or {}).get("frame") or 0) * 1000
    # (Its parts, its equipment and its fuel; and with its hold full. It lands as its design says.)
    light = hl.get("parts_mass", 0) + hl.get("fitted_mass", 0) + 1000 * (hl.get("mass") or {}).get("fuel", 0)
    full = light + 1000 * (hl.get("mass") or {}).get("hold", 0)
    ship = light if ds.get("lands") == "empty" else full
    as_lands = "empty of cargo" if ds.get("lands") == "empty" else "loaded"
    rows = []
    # (Holding the cabin's air.)
    for pt in [pt for pt in leaves if case(pt) == "pressure"]:
        ms = stock(pt)
        ph, sy, have = pt.get("physical") or {}, mech(ms or {}).get("yield_strength"), ((ms or {}).get("size") or {}).get("thickness")
        if not (ms and sy and have and ph.get("width")):
            rows.append(row("gap", part_link(pt), "holds the cabin's air", "", "", "", "its stock, its size or its material's strength is not said"))
            continue
        r = min(ph["width"], ph["height"]) / 2
        need = ds.get("cabin_pressure", 0) * 1e3 * r * SF * PF / (sy * 1e6) * 1000
        rows.append(row("ok" if have >= need else "gap", part_link(pt), f"holds {ds.get('cabin_pressure')} kPa across {2 * r:.1f} m", f"{need:.1f} mm of skin", f"{have:g} mm", f"{have / need:.2f} times", "as a cylinder across its smaller dimension; its flat sides are not checked"))
    thin = [pt for pt in leaves if case(pt) is None and ((stock(pt) or {}).get("size") or {}).get("thickness") is not None]
    below = [pt for pt in thin if stock(pt)["size"]["thickness"] < ds.get("minimum_gauge", 0)]
    if thin:
        rows.append(row("gap" if below else "note", f"{len(thin)} skin and plate parts with no load case", "none set", f"{ds.get('minimum_gauge'):g} mm at least", f"{min(stock(pt)['size']['thickness'] for pt in thin):g} mm the thinnest", "", "their gauge was chosen; nothing checks them against thrust, bending or buckling" if not below else f"{len(below)} are below the least gauge"))
    # (Where it can hover and set down: where gravity is no more than its lift holds.)
    hover = ds.get("lift_thrust", 0) * 1e6 / ship if ship else 0
    # (Taking the landing: on the heaviest ground it can hover over.)
    legs = {}
    for pt in [pt for pt in leaves if case(pt) == "landing"]:
        legs.setdefault(pt.get("parent"), []).append(pt)
    count = sum(times(next(o for o in mine if o["slug"] == k)) for k in legs if k)
    v = ds.get("landing_speed", 0)
    energy = 0.5 * ship * v ** 2
    most = None            # (the least, over the legs' parts, of the force a leg takes before it fails)
    strokes = []
    for parent, pts in legs.items():
        stroke = min(longest(pt) for pt in pts)
        strokes.append(stroke)
        force = (energy / (stroke * eta) + ship * hover) / max(count, 1)
        for pt in pts:
            ms = stock(pt)
            size, mc = (ms or {}).get("size") or {}, mech(ms or {})
            d, w, sy, em = size.get("diameter"), size.get("wall"), mc.get("yield_strength"), mc.get("youngs_modulus")
            if not (d and sy and em):
                rows.append(row("gap", part_link(pt), "takes the landing", "", "", "", "its stock's size or its material's strength or stiffness is not said"))
                continue
            di = d - 2 * w if w else 0
            a_, i_ = math.pi * (d ** 2 - di ** 2) / 4e6, math.pi * (d ** 4 - di ** 4) / 64e12
            takes = min(sy * 1e6 * a_, math.pi ** 2 * em * 1e9 * i_ / longest(pt) ** 2)
            most = takes if most is None else min(most, takes)
            strength = sy * 1e6 * a_ / (force * SF)
            buckling = math.pi ** 2 * em * 1e9 * i_ / longest(pt) ** 2 / (force * SF)
            rows.append(row("ok" if min(strength, buckling) >= 1 else "gap", part_link(pt), f"{force / 1e6:.2f} MN on each of {count} legs", f"{force * SF / 1e6:.2f} MN with its factor", ms["identity"]["name"], f"strength {strength:.2f} times, buckling {buckling:.2f} times", f"a column {longest(pt):.2f} m long, stopping {ship / 1000:.0f} t from {v:g} m/s in {stroke:.2f} m, on ground of {hover / G0:.2f} g"))
    # (What follows for everything aboard: the jolt of the designed landing, and the hardest landing
    # the legs take before one fails, with its jolt.)
    landing = {}
    if legs and ship and strokes:
        s_ = min(strokes)
        jolt = v ** 2 / (2 * s_ * eta) / G0
        spare = (most or 0) * count - ship * hover
        vmax = (2 * spare * s_ * eta / ship) ** 0.5 if spare > 0 else 0
        jmax = vmax ** 2 / (2 * s_ * eta) / G0
        landing = {"designed": v, "jolt": jolt, "hardest": vmax, "hardest_jolt": jmax, "stroke": s_}
        rows.append(row("note", "The landing its legs are designed for", f"{v:g} m/s", "", f"a jolt of {jolt:.2f} g aboard", "", (bench("landing_gear", "design_sink_speed_landing_weight").get("source") or "") + ": 14 CFR 25.473"))
        rows.append(row("note", "The hardest landing its legs take", "until one yields or buckles", "", f"{vmax:.1f} m/s", f"a jolt of {jmax:.1f} g aboard", "faster than this a leg fails; the jolt is what everything aboard must take, people and cargo"))
    # (Against real vehicles and the game's own engines.)
    sz = hl.get("size") or {}
    if all(k in sz for k in ("length", "width", "height")) and hl.get("parts_mass"):
        box = 2 * (sz["length"] * sz["width"] + sz["length"] * sz["height"] + sz["width"] * sz["height"])
        ref = bench("fuselage_structure", "areal_mass_formula_AA241")
        rows.append(row("note", "The hull's parts over its outer box", f"{box:,.0f} m2 of box", "about 24 kg/m2 for an airliner's fuselage" if ref else "", f"{hl['parts_mass'] / box:.1f} kg/m2", f"{hl['parts_mass'] / box / 24:.2f} times", (ref.get("source") or "") + ": a weight formula for pressurised fuselages, by wetted area"))
    gear = sum(o.get("mass", 0) * times(o) for o in mine if o["slug"] in legs)
    if gear and ship:
        rows.append(row("note", "The landing legs' share of the ship", f"{ship / 1000:,.0f} t {as_lands}", "3 to 6% in transport aircraft, with wheels and brakes, for 1 g", f"{100 * gear / ship:.1f}% ({gear / 1000:.1f} t)", "", (bench("weight_fractions", "landing_gear_share_MTOW_VT").get("source") or "")))
    can, cannot = [], []
    if hover:
        for ad in administrations:
            for x in ad["bodies"]:
                if x.get("kind") != "settlement":
                    continue
                at = next((y for y in ad["bodies"] if y.get("name") == x.get("at")), None)
                if "position" not in x:
                    can.append(f"{x['name']} (in orbit)")
                elif at is not None and "gravity" in at:
                    (can if at["gravity"] <= hover else cannot).append(f"{x['name']} ({at['gravity'] / G0:.2f} g)")
        rows.append(row("note", "Where it can hover and set down", f"{ship / 1000:,.0f} t {as_lands}", f"{ds['lift_thrust']:g} MN of lift", f"ground of up to {hover / G0:.2f} g", "", f"its lift nozzles hold it where gravity is no more than {hover:.2f} m/s2. Not every ship has to land on a planet."))
        if full > ship:
            rows.append(row("note", "With its hold full", f"{full / 1000:,.0f} t", f"{ds['lift_thrust']:g} MN of lift", f"ground of up to {ds['lift_thrust'] * 1e6 / full / G0:.2f} g", "", "loaded, it unloads in orbit: it is not built to set down full"))
        if can or cannot:
            rows.append(row("note", "Settlements it can set down at", f"{len(can)} of {len(can) + len(cannot)}", "", ", ".join(can), "", ("Too heavy for: " + ", ".join(cannot)) if cannot else ""))
    if ds.get("main_thrust") and ship:
        rows.append(row("note", "Main drive", f"{light / 1000:,.0f} t empty of cargo, {full / 1000:,.0f} t full", "", f"{ds['main_thrust']:g} MN", f"{ds['main_thrust'] * 1e6 / light / G0:.1f} g empty, {ds['main_thrust'] * 1e6 / full / G0:.2f} g full", "its acceleration flat out"))
    # (The main drive's push through the hull: each part on its path as a thin-walled box of its
    # stock's gauge. Strength: the push, with its factor, over the wall's section. Buckling: a flat
    # panel between stiffeners b apart holds 4 pi^2 E / (12 (1 - nu^2)) (t / b)^2 before it buckles,
    # so the stress says how close its stiffeners must be.)
    push = ds.get("main_thrust", 0) * 1e6 * SF
    for code in ds.get("thrust_path") or []:
        pt = next((o for o in mine if o["slug"] == code), None)
        ms = pt and stock(pt)
        t_ = ((ms or {}).get("size") or {}).get("thickness")
        mc = mech(ms or {})
        ph = (pt or {}).get("physical") or {}
        if not (pt and t_ and mc.get("yield_strength") and mc.get("youngs_modulus") and ph.get("width") and ph.get("height")):
            rows.append(row("gap", part_link(pt) if pt else code, "carries the main drive's push", "", "", "", "no such part, or its stock's gauge or its material's strength or stiffness is not said"))
            continue
        wall = 2 * (ph["width"] + ph["height"]) * t_ / 1000
        stress = push / wall
        nu = mc.get("poissons_ratio", 0.33)
        apart = t_ / 1000 * (4 * math.pi ** 2 * mc["youngs_modulus"] * 1e9 / (12 * (1 - nu ** 2) * stress)) ** 0.5
        strong = mc["yield_strength"] * 1e6 / stress
        rows.append(row("ok" if strong >= 1 and apart >= 0.3 else "gap", part_link(pt), f"the main drive's push, {push / 1e6:.1f} MN with its factor", f"{stress / 1e6:.0f} MPa in a wall {2 * (ph['width'] + ph['height']):.0f} m round", f"{t_:g} mm", f"strength {strong:.1f} times", f"it buckles unless stiffened every {apart * 100:.0f} cm or closer" + ("" if apart >= 0.3 else ": closer than can be built (30 cm taken as the least). A thicker skin, or stiffeners as parts, is needed")))
    hl["worked"] = {"ship": ship, "full": full, "light": light, "as_lands": as_lands, "hover": hover, "landing": landing, "can": can, "cannot": cannot}
    report(f"structure-{hl['slug']}", f"Structure: {hl['identity']['name']} against its loads", "Each part that has a load case, against that load, by first-order sizing; and the hull against real vehicles and its own engines. A gap is a part too weak for its load, or a figure outside what real vehicles show.", ["What", "Load", "Needs", "Has", "Margin", "Note or source"], rows)

# 2c. Shock: what breaking will be worked out from, and how much of it is there.
rows = []
has = lambda recs, f: sum(1 for e in recs if f(e))
n = has(materials, lambda m: (m.get("mechanical") or {}).get("fracture_toughness") is not None)
rows.append(row("ok" if n == len(materials) else "gap", "Materials with a fracture toughness", f"{n} of {len(materials)}", "how well each resists a crack running through it", ", ".join(m["identity"]["name"] for m in materials if (m.get("mechanical") or {}).get("fracture_toughness") is None) or ""))
n = has(mill_stock, lambda s_: (s_.get("physical") or {}).get("shock_limit") is not None)
rows.append(row("ok" if mill_stock and n == len(mill_stock) else "gap", "Mill stock with a shock limit", f"{n} of {len(mill_stock)}", "the hardest jolt each takes as cargo", "none says yet" if not n else "guesses, marked to review"))
single = [p_ for p_ in parts if not kids(p_) and p_["hull"] not in {m["slug"] for m in modules}]
n = has(single, lambda p_: (p_.get("physical") or {}).get("shock_limit") is not None)
rows.append(row("ok" if single and n == len(single) else "gap", "Parts with a shock limit", f"{n} of {len(single)}", "the hardest jolt each takes, fitted or carried", "none says yet" if not n else "guesses, marked to review"))
for hl in hulls:
    ld = (hl.get("worked") or {}).get("landing") or {}
    if ld:
        rows.append(row("ok", link(hl["identity"]["name"], "hull:" + hl["slug"]), f"{ld['hardest']:.1f} m/s", "the hardest landing its legs take", f"a jolt of {ld['hardest_jolt']:.1f} g aboard; designed for {ld['designed']:g} m/s, {ld['jolt']:.2f} g"))
report("shock", "Shock: what breaking is worked out from", "What a ship's landing does to what is aboard, and whether each material, item of stock and part says what it can take (SFO 15). A gap is a kind of record that does not yet say.", ["What", "How many", "Meaning", "Note"], rows)

# 2d. Power: in each settlement, what its stations can supply against what its facilities draw
# flat out, and whether a line reaches each that draws.
rows = []
for ad in administrations:
    for x in ad["bodies"]:
        facs = x.get("facilities", [])
        # (What each draws flat out: its lines, and what stands beside them and draws all the time: a yard, a store.)
        idle = lambda fc: sum(((mod_of[im["module"]].get("needs") or {}).get("power") or 0) * im.get("count", 0) for im in fc.get("modules") or [] if im.get("module") in mod_of and not mod_of[im["module"]].get("recipes"))
        draw = {fc["slug"]: sum(ln["most"]["power"] for ln in fc.get("lines") or [] if "most" in ln) + idle(fc) for fc in facs}
        supply = sum(fc.get("capacity") or 0 for fc in facs if fc.get("kind") in ("power", "rig"))
        if not supply and not any(draw.values()):
            continue
        total = sum(draw.values())
        rows.append(row("ok" if supply >= total else "gap", x["name"] + ", all of it", f"{total:,.0f} MW" if total >= 10 else f"{total:,.1f} MW", f"{supply:,.0f} MW", "its own plant" if x.get("kind") == "rig" else "its power stations", "enough for everything flat out at once" if supply >= total else f"{total - supply:,.0f} MW short with everything flat out at once"))
        for fc in facs:
            if not draw[fc["slug"]] or fc.get("rig"):
                continue
            wires = [pw for pw in x.get("power_lines", []) if pw.get("to") == fc["slug"]]
            can = sum(pw["capacity"] for pw in wires)
            rows.append(row("ok" if can >= draw[fc["slug"]] else "gap", fc["name"], f"{draw[fc['slug']]:,.1f} MW", f"{can:,.0f} MW" if wires else "", ", ".join(pw["name"] for pw in wires) or "no line reaches it", "" if can >= draw[fc["slug"]] else ("its lines carry less than it draws" if wires else "")))
report("power", "Power: what is supplied against what is drawn", "In each settlement: what its power stations can supply against what its facilities draw with every line flat out, and for each facility the lines that reach it against what it draws. A gap is a shortfall, or a facility no line reaches.", ["What", "Draws flat out", "Can get", "From", "Note"], rows)

# 3. Volume: a hull's parts' boxes against the space the hull takes.
rows = []
for hl in built_hulls:
    vol = (hl.get("size") or {}).get("volume")
    boxes = sum((pt.get("physical") or {}).get("length", 0) * (pt.get("physical") or {}).get("width", 0) * (pt.get("physical") or {}).get("height", 0) * times(pt) for pt in parts if pt["hull"] == hl["slug"] and not pt.get("parent"))
    unsized = [pt for pt in parts if pt["hull"] == hl["slug"] and not pt.get("parent") and not (pt.get("physical") or {}).get("length")]
    if vol:
        rows.append(row("note", link(hl["identity"]["name"], "hull:" + hl["slug"]), f"{vol:,.0f} m3", f"{boxes:,.0f} m3", f"{boxes / vol:.1f} times", "The boxes are each part's outer extent and overlap where parts do, so they add up to more than the hull."))
    for pt in unsized:
        rows.append(row("gap", part_link(pt), "", "", "", "no size"))
report("volume", "Volume: a hull against its parts' boxes", "The space the hull's shape takes in the game, against the boxes of its parts added up. A gap is a part with no size.", ["What", "Hull's volume", "Parts' boxes", "Ratio", "Note"], rows)

# 3b. People: what a person needs, rung by rung, and whether what meets each need is described.
# (standards/People: needs and professions.)
_pdir = os.path.join(TREE, "People", "metadata")
needs = [dict(load(os.path.join(_pdir, "needs", f)), slug=f[:-5]) for f in sorted(os.listdir(os.path.join(_pdir, "needs")))] if os.path.isdir(_pdir) else []
professions = [dict(load(os.path.join(_pdir, "professions", f)), slug=f[:-5]) for f in sorted(os.listdir(os.path.join(_pdir, "professions")))] if os.path.isdir(_pdir) else []
_made = {rc.get("product") for m in modules for rc in m.get("recipes") or []} | {x.get("item") for m in modules for rc in m.get("recipes") or [] for x in rc.get("outputs") or []}
_markets = {SOLD_AS.get(g["slug"], "").split(".")[-1] for g in goods if any(g["slug"] == pr for pr in _made)}
_bdir = os.path.join(TREE, "SFO", "metadata", "buildings")
buildings = [dict(load(os.path.join(_bdir, f)), slug=f[:-5]) for f in sorted(os.listdir(_bdir))] if os.path.isdir(_bdir) else []
tail = lambda k: str(k).split(".")[-1]
RUNGS = ["alive", "together", "better", "to matter"]
rows = []
for nd in sorted(needs, key=lambda n: (RUNGS.index(n["identity"]["rung"]), n["identity"]["name"])):
    lacks, how = [], []
    for t_ in nd.get("takes") or []:
        it = t_["item"]
        how.append(f"{item_name(it)} {t_['rate'] * 86400:.3g} kg a day")
        if it not in _made and it not in _markets and it not in ("water", "O"):
            lacks.append(f"nothing described makes {item_name(it).lower()}")
    housed = [b_ for b_ in buildings if any(tail(m_["need"]) == nd["slug"] for m_ in b_.get("meets") or [])]
    if "space" in nd:
        how.append(f"{nd['space']:g} m2")
        if not housed:
            lacks.append("no building is described that gives the floor")
    if "power" in nd:
        how.append(f"{nd['power']:g} W")
    for s_ in nd.get("served_by") or []:
        how.append(f"one {tail(s_['profession'])} to {s_.get('serves', 0):,.0f}")
        if not any(tail(x_["profession"]) == tail(s_["profession"]) for b_ in buildings for x_ in b_.get("staff") or []):
            lacks.append(f"nowhere is described for a {tail(s_['profession'])} to work")
    if not how:
        _sights = [yaml.safe_load(open(f__, encoding="utf-8")) or {} for f__ in glob.glob(os.path.join(TREE, "Celestial", "metadata", "sights", "*.yaml"))]
        if (nd.get("identity") or {}).get("key") == "need.wandering" and _sights:
            how.append(f"{len(_sights)} sights worth going to see: " + ", ".join(s__["identity"]["name"] for s__ in _sights))
        elif (nd.get("identity") or {}).get("key") == "need.work":
            how.append("a job: the census counts who has one; the game pays the wage")
        else:
            lacks.append("nothing says yet what meets it")
    rows.append(row("gap" if lacks else "ok", nd["identity"]["name"], nd["identity"]["rung"], "; ".join(how), "; ".join(dict.fromkeys(lacks)) or "met by what is described" + (": " + ", ".join(b_["identity"]["name"].lower() for b_ in housed) if housed else "")))
report("needs", "Needs: what a person needs, and whether it is described", "The ladder: each thing a person needs, what meets it for one person, and whether the registry describes something that makes or gives it. A gap is a need nothing described can meet yet.", ["Need", "Rung", "What meets it, for one person", "State"], rows)
rows = []
for pf in professions:
    yrs = ((pf.get("training") or {}).get("time") or 0) / YEAR_S
    rows.append(row("note", pf["identity"]["name"], (pf.get("training") or {}).get("learned", ""), f"{yrs:.0f} years" if yrs else "", ", ".join(n["identity"]["name"].lower() for n in needs if any(s_["profession"] == pf["slug"] for s_ in n.get("served_by") or [])), "no works says yet how many it takes"))
report("work", "Work: the trades", "Each trade: how it is learned, how long that takes, and which needs its people meet. Each works says in module.staff how many of which trade run it.", ["Trade", "Learned", "Takes", "Meets", "Works"], rows)

rows = []
for b_ in buildings:
    ppl = max((m_["people"] for m_ in b_.get("meets") or []), default=0)
    rows.append(row("note", b_["identity"]["name"], ", ".join(f"{tail(m_['need'])} for {m_['people']:,.0f}" for m_ in b_.get("meets") or []), f"{b_.get('floor', 0):,.0f} m2", f"{b_.get('storeys', 0):g}",
                    tonnes((b_.get("physical") or {}).get("mass", 0)), ", ".join(f"{x_['count']:,.0f} {tail(x_['profession'])}" for x_ in b_.get("staff") or []) or "none", f"{b_.get('life', 0) / YEAR_S:.0f} years"))
report("buildings", "Buildings: where people live and are served", "Each building that is not a works: which needs it meets and of how many people, its floor, its weight (all materials), who works in it, and how long it is built to stand.", ["Building", "Meets", "Floor", "Storeys", "Weight", "Staff", "Built to stand"], rows)

# 3c. Takers: is each thing given off taken by something? (The matter loop: nothing made is nothing's to take.)
_ins = {}
for m in modules:
    for rc in m.get("recipes") or []:
        for x in rc.get("inputs") or []:
            _ins.setdefault(x.get("item"), []).append(m["identity"]["name"])
for nd in needs:
    for t_ in nd.get("takes") or []:
        _ins.setdefault(t_["item"], []).append("people")
_outs = {}
for m in modules:
    for rc in m.get("recipes") or []:
        for x in rc.get("outputs") or []:
            _outs.setdefault(x.get("item"), []).append(m["identity"]["name"])
for nd in needs:
    for g_ in nd.get("gives") or []:
        _outs.setdefault(g_["item"], []).append("people")
rows = []
_elements = {(e_.get("identity") or {}).get("symbol") for e_ in elements}
for it in sorted(_outs, key=lambda i: item_name(i).lower()):
    if it in _elements:       # (a metal won beside the main one is a product, not a waste)
        continue
    takers = list(dict.fromkeys(_ins.get(it) or []))
    rows.append(row("ok" if takers else "gap", item_name(it), ", ".join(list(dict.fromkeys(_outs[it]))[:6]) + (" and others" if len(set(_outs[it])) > 6 else ""), ", ".join(takers[:6]) + (" and others" if len(takers) > 6 else "") or "nothing takes it"))
report("takers", "Takers: is each thing given off taken by something?", "Everything that comes out of a recipe beside its product, and everything people give off: what gives it, and what takes it in. A gap is a thing that piles up for ever, or is thrown away: a loop that is not closed. Elements are left out: a metal won beside the main one is a product.", ["Given off", "By", "Taken by"], rows)

# 3d''. Mounts (SFO 19): does each piece of equipment lie within the mount it says it fits?
_mdir = os.path.join(TREE, "SFO", "metadata", "mounts")
MOUNTS = {}
for _f in sorted(os.listdir(_mdir)) if os.path.isdir(_mdir) else []:
    _m = yaml.safe_load(open(os.path.join(_mdir, _f), encoding="utf-8")) or {}
    MOUNTS[_m["identity"]["key"]] = _m
rows = []
for e_ in equipment:
    raw = yaml.safe_load(open(os.path.join(TREE, e_["file"]), encoding="utf-8")) or {}   # (the record as written: keys, not the old view's names)
    mk = raw.get("fits")
    if not mk:
        continue
    mt = MOUNTS.get(mk)
    if not mt:
        rows.append(row("gap", e_["identity"]["name"], mk, "no such mount")); continue
    ph, env, bears, feeds = raw.get("physical") or {}, mt["envelope"], mt["bears"], mt.get("feeds") or {}
    over = []
    dims = sorted([ph.get("length", 0), ph.get("width", 0), ph.get("height", 0)], reverse=True); room = sorted([env["length"], env["width"], env["height"]], reverse=True)
    if any(d_ > r_ * 1.0001 for d_, r_ in zip(dims, room)): over.append(f"size {dims[0]:g} x {dims[1]:g} x {dims[2]:g} m in {room[0]:g} x {room[1]:g} x {room[2]:g}")
    if ph.get("mass", 0) > bears["mass"]: over.append(f"weight {ph['mass']:,.0f} kg over {bears['mass']:,.0f}")
    draw = (raw.get("needs") or {}).get("power", 0)
    if feeds.get("power") and draw > feeds["power"]: over.append(f"draws {draw / 1e6:g} MW over {feeds['power'] / 1e6:g}")
    th = (raw.get("function") or {}).get("thrust", 0)
    if bears.get("thrust") and th > bears["thrust"]: over.append(f"thrust {th / 1e6:g} MN over {bears['thrust'] / 1e6:g}")
    rows.append(row("gap" if over else "ok", e_["identity"]["name"], mk, "; ".join(over) or "within it"))
report("mounts", "Mounts: does each piece of equipment fit the mount it is built to?", "SFO 19: a mount is what a hull's slot offers; equipment is built to one. Each piece against its mount: its size in the envelope, its weight, its draw and its thrust within what the mount bears and feeds.", ["Equipment", "Mount", "State"], rows)
# 3c. Engine: the tree of clocks (standards/Engine/metadata/scheduling): each period a whole number of the realtime tick, every parent a clock.
_clocks = {}
for f__ in sorted(glob.glob(os.path.join(TREE, "Engine", "metadata", "scheduling", "*.yaml"))):
    c__ = yaml.safe_load(open(f__, encoding="utf-8")) or {}
    _clocks[c__["identity"]["key"]] = c__
_base = min((c__["trigger"]["every"] for c__ in _clocks.values() if c__["trigger"].get("every")), default=None)
def _clock_rows(parent, depth, rows):
    for k__, c__ in _clocks.items():
        if c__.get("parent") != parent:
            continue
        tr = c__["trigger"]; bad = []
        if tr["kind"] == "period" and not tr.get("every"): bad.append("a period with no `every`")
        if tr.get("every") and _base:
            n__ = tr["every"] / _base
            if abs(n__ - round(n__)) > 1e-6: bad.append(f"{tr['every']:g} s is not a whole number of the {_base:g} s tick")
        if c__.get("parent") and c__["parent"] not in _clocks: bad.append(f"parent {c__['parent']} is not a clock")
        for r__ in (c__.get("reads") or []) + (c__.get("posts") or []):
            if r__ not in _clocks: bad.append(f"{r__} is not a clock")
        when = f"every {tr['every']:g} s ({tr['every'] / _base:,.0f} ticks)" if tr.get("every") else ("on event" if tr["kind"] == "event" else "group")
        rows.append(row("gap" if bad else "ok", ("\u2007\u2007" * depth) + ("\u2514 " if depth else "") + c__["identity"]["name"], c__["scope"], c__["coupling"].replace("_", "-"), when, "; ".join(bad) or (c__.get("rule") or "")[:140]))
        _clock_rows(k__, depth + 1, rows)
_crows = []
_clock_rows(None, 0, _crows)
for k__, c__ in _clocks.items():
    if c__.get("parent") and c__["parent"] not in _clocks:
        _crows.append(row("gap", c__["identity"]["name"], c__["scope"], c__["coupling"], "", f"parent {c__['parent']} is not a clock"))
if _crows:
    report("clocks", "Clocks: the engine's tree of time", "standards/Engine: each clock, under the clock whose due time makes it step, with how many of it there are (scope), how tightly it exchanges data (coupling, which decides the thread, process or machine it runs on) and when it steps. Every period must be a whole number of the realtime tick, so every input is stamped by the same tick numbers and replay works per layer.", ["Clock", "One per", "Coupling", "Steps", "Rule, or what is wrong"], _crows)

# 3d'''. Members (SFO 13): the tubes a frame is cut from, read as an engineer sizes: weight a metre, the load
# at which each yields (with the safety factor 1.5), and the pinned length at which it buckles under 50 and 200 kN (Euler).
rows = []
for ms in sorted(mill_stock, key=lambda m_: ((m_.get("made_from") or {}).get("material", ""), (m_.get("size") or {}).get("diameter", 0), (m_.get("size") or {}).get("wall", 0))):
    size_ = ms.get("size") or {}
    form_ = (ms.get("made_from") or {}).get("form")
    if form_ == "tube" and size_.get("diameter") and size_.get("wall"):
        d_, w_ = size_["diameter"], size_["wall"]
        d_m, w_m = (d_, w_) if d_ < 1 else (d_ / 1000, w_ / 1000)      # (the view keeps sizes in mm)
        area = math.pi * (d_m ** 2 - (d_m - 2 * w_m) ** 2) / 4
        inertia = math.pi * (d_m ** 4 - (d_m - 2 * w_m) ** 4) / 64
        section = f"{d_m * 1000:g} x {w_m * 1000:g}"
    elif form_ == "box" and size_.get("width") and size_.get("height") and size_.get("wall"):
        # (A welded box: the same figures, for the engineer's own members at an engine mount.)
        _m = lambda x: x if x < 1 else x / 1000      # (the view keeps a tube's sizes in mm; a box's width and height stay in m)
        bw_m, bh_m, w_m = _m(size_["width"]), _m(size_["height"]), _m(size_["wall"])
        area = bw_m * bh_m - (bw_m - 2 * w_m) * (bh_m - 2 * w_m)
        inertia = (bw_m * bh_m ** 3 - (bw_m - 2 * w_m) * (bh_m - 2 * w_m) ** 3) / 12
        section = f"box {bw_m * 1000:g} x {bh_m * 1000:g} x {w_m * 1000:g}"
    else:
        continue
    mat = next((m for m in materials if m.get("slug") == ms["made_from"].get("material")), None) or {}
    mech = mat.get("mechanical") or {}
    E_, sy = mech.get("youngs_modulus"), mech.get("yield_strength")      # (the view keeps GPa and MPa)
    E_, sy = (E_ * 1e9 if E_ else E_), (sy * 1e6 if sy else sy)
    yields = f"{area * sy / 1.5 / 1e3:,.0f} kN" if sy else "no yield strength on record"
    euler = lambda P: f"{math.pi * math.sqrt(E_ * inertia / P):.1f} m" if E_ else "no modulus on record"
    rows.append(row("ok" if E_ and sy else "note", (ms.get("identity") or {}).get("name", ms["slug"]), section, f"{ms.get('weight', 0):.2f}", yields, euler(50e3), euler(200e3), f"{sy / (mat.get('mass') or {}).get('density', 1) / 1e3:.0f}" if sy else ""))
report("members", "Members: the tubes and box sections a frame is cut from", "SFO 13: every round tube and box section in stock, read as an engineer sizes a frame. Yields: the axial load at which it yields with the safety factor 1.5. Buckles: the pinned length at which Euler buckling takes it under 50 kN and under 200 kN; a longer member needs a bigger tube or a brace. Specific strength: yield strength over density, kN m per kg.", ["Tube", "mm", "kg/m", "Yields at", "Buckles at 50 kN", "Buckles at 200 kN", "kN m/kg"], rows)

# 3e. Traffic and census (docs/registry-people.md): the ships the supply runs take, and who lives where by trade.
try:
    import census as _census
    import glob
    _setts, _rts = _census.derive()
    _routes = [row("ok", "the gate" if r_["from"] == "gate" else _setts[r_["from"]][1]["identity"]["name"], _setts[r_["to"]][1]["identity"]["name"], "works" if r_.get("industrial") else "people", f"{r_['tonnes_day'] / 1000:.1f}", f"{r_['one_way'] / _census.DAY:.1f}", r_["ships"], f"{r_['interval'] / _census.DAY:.1f}") for r_ in sorted(_rts, key=lambda r_: -r_["tonnes_day"])]
    report("traffic", "Traffic: the ships the runs take", f"Every run between settlements: for people, what a port's people take a day of stocked goods (settlement.resupply); for works, what a port's facilities take in a day that the port does not make, from the nearest port that makes it or, made nowhere in the system, from the gate. The flight one way at a torchship's cruise of {_census.CRUISE:g} m/s2 over the two orbits' typical separation; the haulers ({_census.HAULER}, {(_census.hulls[_census.HAULER]['identity'].get('hold') or 0) / 1000:.0f} t, a day's turnaround each end) that run needs; so the delivery interval. The sum is the freight line's fleet (org.treistun-freight.fleet); the rest of seeding.traffic's target are independents.", ["From", "To", "For", "t a day", "Days one way", "Haulers", "Every (days)"], _routes)
    _rows = []
    for _f, _d in sorted(_setts.values(), key=lambda fd: fd[1]["identity"]["name"]):
        if not _d.get("census"):
            continue
        _c = {c_["profession"].split(".")[-1]: c_["count"] for c_ in _d["census"]}
        _dep = _c.pop("dependant", 0); _work = sum(_c.values())
        _rows.append(row("gap" if _work > _d["population"] or _work < 0.25 * _d["population"] else "ok", _d["identity"]["name"], f"{_d['population']:,}", f"{_work:,}", f"{_dep:,}", f"{_work / _d['population']:.0%}", ", ".join(f"{k_} {v_}" for k_, v_ in sorted(_c.items(), key=lambda kv: -kv[1])[:6])))
    report("census", "Census: who lives where, by trade", "Each settlement's people at day 0: at work (the staff of its works, module.staff; the trades its needs are served by, need.served_by; pilots from the fleets based there) and dependants (the rest). In the rich countries about half the people are at work; under a quarter here is a settlement whose works and services, as described, give most of its people nothing to do; over all of them, one over-built. The rows carry each port's share: the service trades are described (shops, yards, offices, depots, taverns, clinics, schools); what is still thin against a real economy is manufacturing for people at every port (works making consumer goods), health and schooling. The operator hires from these: finite.", ["Settlement", "People", "At work", "Dependants", "Share at work", "Largest trades"], _rows)
except Exception as _e:    # (the census tool is beside this build; without it the page lacks two reports, no more)
    print("traffic/census report:", _e)

# 3g. Budgets (the ships session's ask): for each hull as fitted, power, heat, and air and water for its people.
_eq_raw = {}
for _f in glob.glob(os.path.join(TREE, "SFO", "metadata", "equipment", "*.yaml")):
    _r = yaml.safe_load(open(_f, encoding="utf-8")) or {}
    _eq_raw[_r["identity"]["key"]] = _r
_air_rate = next((t_["rate"] for n_ in _census.needs.values() for t_ in n_.get("takes", []) if n_["identity"]["key"] == "need.air" and t_["item"] == "element.o"), 1.036e-5)
_water_rate = sum(t_["rate"] for n_ in _census.needs.values() if n_["identity"]["key"] in ("need.water", "need.washing") for t_ in n_.get("takes", []) if t_["item"] == "good.water")
rows = []
for _h in sorted(glob.glob(os.path.join(TREE, "SFO", "metadata", "hulls", "*.yaml"))):
    _hull = yaml.safe_load(open(_h, encoding="utf-8")) or {}
    _fit = _hull.get("fit") or []
    if not _fit:
        continue
    _out = _draw = _waste = _jet = _reject = _carry = 0.0; _seats = 0; _air = _water = 0.0; _wrec = _arec = 0.0; _missing = []
    for _fi in _fit:
        _e = _eq_raw.get(_fi["item"])
        if not _e:
            _missing.append(_fi["item"]); continue
        _fn = _e.get("function") or {}; _k = _fn.get("kind")
        _draw += (_e.get("needs") or {}).get("power", 0)
        if _k == "power_plant":
            _out += _fn["output"]; _waste += plant_waste(_fn)
        elif _k in JET_KINDS:
            _n = _fi.get("nozzles", 1 if _k != "thrusters" else 4)
            _jet += jet_heat(_fn, _n)   # (one rule: lib.jet_heat, SFO 22; an engine record derives its own heat_to_hull)
        elif _k == "radiator":
            _reject += _fn["rejects"]
        elif _k == "coolant_loop":
            _carry += _fn["carries"]
        elif _k == "cabin":
            _seats += _fn.get("seats", 0)
        elif _k == "store":
            if _fn.get("holds") == "good.water": _water += _fn["capacity"]
            if _fn.get("holds") == "element.o": _air += _fn["capacity"]
        elif _k == "life_support":
            _wrec = max(_wrec, _fn.get("water_recovery", 0.0)); _arec = max(_arec, _fn.get("air_recovery", 0.0))
    _heat_in = _waste + _draw + _jet      # (power used inside ends as heat inside, bar what leaves as a beam: the studio's rule too)
    _power = f"{_out / 1e6:.1f} MW made, {_draw / 1e6:.2f} MW drawn" + ("" if _out >= _draw else ": SHORT")
    _heat = f"{_heat_in / 1e6:.1f} MW aboard ({_waste / 1e6:.1f} plant loss, {_draw / 1e6:.1f} drawn and spent inside, {_jet / 1e6:.1f} from the jets at full burn); radiators {_reject / 1e6:.0f} MW, loops {_carry / 1e6:.0f} MW" + (": NO RADIATORS" if _heat_in > 0 and _reject == 0 else ("" if _reject >= _heat_in else ": SHORT"))
    if _seats:
        _days = lambda cap, rate: f"{cap / (rate * _seats * 86400):.0f} days" if cap else "no store"
        _life = f"{_seats} seats: air {_days(_air, _air_rate * (1 - _arec))}, water {_days(_water, _water_rate * (1 - _wrec))}" + (f" (life support recovers {_wrec:.0%} of water, {_arec:.0%} of air)" if _wrec or _arec else "")
    else:
        _life = "no cabin fitted: no crew figure (air and water days need seats)"
    _state = "gap" if (_out < _draw or _reject < _heat_in or _missing) else "ok"
    rows.append(row(_state, _hull["identity"]["name"], _power, _heat, _life, ", ".join(_missing) or ""))
report("budgets", "Budgets: each hull as fitted", "For every hull with a fit: power (its plants' output against everything's draw), heat (the plants' waste heat, everything drawn, since power spent inside ends as heat inside, and the share of the jets' power that reaches the hull, function.heat_to_hull, at full burn, against the radiators and coolant loops fitted), and air and water (days the fitted stores last the cabins' seats, at the needs' rates less what the life support recovers). The studio's checks should read the same records.", ["Hull", "Power", "Heat", "Air and water", "Unknown equipment"], rows)

# 3f. Worlds (docs/survey-contract.md): every body grown by the planet simulation, its packages, and what the store holds besides.
_store = os.environ.get("UNIVERSE_WORLDS") or os.path.expanduser("~/git/planet-sim/out/worlds")
_index = {}
try:
    _rel = json.load(open(os.path.join(_store, "releases.json"), encoding="utf-8"))
    _index = {w_["world_id"]: w_ for w_ in _rel.get("worlds", [])}
except Exception:
    pass
rows = []
_baked = set()
import glob as _glob
for b_ in sorted((yaml.safe_load(open(f__, encoding="utf-8")) or {} for f__ in _glob.glob(os.path.join(TREE, "Celestial", "metadata", "systems", "*", "bodies", "*.yaml"))), key=lambda b_: (b_.get("identity") or {}).get("key", "")):
    sv = b_.get("survey")
    if not sv:
        continue
    _baked.add(sv["world_id"])
    en, bk = b_.get("energy"), b_.get("bake")
    w_ = _index.get(sv["world_id"], {})
    energy_s = ("empty: no life, so no oil, gas or coal" if en and not (en.get("oil_fields") or en.get("gas_fields") or en.get("coalfields")) else f"{en['oil_fields']:,} oil, {en['gas_fields']:,} gas fields, {en['coalfields']} coalfields" if en else ("none: no life, so no oil, gas or coal" if w_ and (w_.get("packages") or {}).get("energy") is None else "not installed"))
    surface_s = f"v{bk['version']}, {bk.get('bytes', 0) / 1e9:.2f} GB in the store" if bk else "none"
    hist = "history" if (w_.get("packages") or {}).get("history") else ""
    rows.append(row("ok", (b_.get("identity") or {}).get("name", ""), sv["world_id"], f"{sv.get('deposits', 0):,} deposits in {sv.get('districts', 0):,} districts; land {sv.get('land_share', 0):.0%}", energy_s, surface_s + (f"; {hist} in the store" if hist else ""), ", ".join(sv.get("in_store") or []) or "all copied", w_.get("status", "no index")))
for wid, w_ in sorted(_index.items()):
    if wid in _baked:
        continue
    why = "superseded by " + w_["superseded_by"] if w_.get("status") == "superseded" else ("no body: grown from no record of ours" if not w_.get("body") else "not installed: run tools/standards/world_install.py --store")
    rows.append(row("note", w_.get("body") or "(none)", wid, "", "", "", "in the store, not installed", why))
report("worlds", "Worlds: bodies grown by the planet simulation", f"Every body with provenance baked: its world, what its survey found, its energy package (none by design where a world never had life), its surface bake (versioned, kept in the worlds store, not here), and which survey files stay in the store for size. Below, what the store ({_store}) lists that is not installed, and why. Install: tools/standards/world_install.py --store <root>.", ["Body", "World", "Survey", "Energy", "Surface", "In the store", "Index says"], rows)

# 3d'. Dimensions: every physical thing has a length, a width and a height. Which are worked out, and which only stand in?
_stand_in = lambda e_: any(str(b_.get("note", "")).startswith(("Not worked out", "Not measured")) for b_ in e_.get("basis") or [])
_fitted = lambda e_: any(b_.get("rule") == "rule.fitted-within" or str(b_.get("note", "")).startswith("Fitted within") for b_ in e_.get("basis") or [])
rows = []
for kind_, recs_ in (("Hulls", hulls), ("Equipment", equipment), ("Parts", parts)):
    owed = [e_ for e_ in recs_ if _stand_in(e_)]; fitted = [e_ for e_ in recs_ if _fitted(e_)]
    rows.append(row("gap" if owed else "ok", kind_, len(recs_), len(recs_) - len(owed) - len(fitted), len(fitted), len(owed), ", ".join((e_.get("identity") or {}).get("name", e_.get("slug", "")) for e_ in owed[:8]) + (f" and {len(owed) - 8} more" if len(owed) > 8 else "")))
report("dimensions", "Dimensions: sized, fitted, or only standing in", "Every hull, piece of equipment and part must say its length, width and height. Sized: measured, or worked out from what it does or from a real one. Fitted: a part given its share of its product's room, so that the parts fit the whole, its own shape not drawn. Standing in: a shape to its volume, owed a real size.", ["Kind", "Records", "Sized", "Fitted", "Standing in", "Owed"], rows)

# 3d. The dictionary: the registry's shared words, and what each value means (standards/dictionary.schema.yaml).
_dict = load(os.path.join(TREE, "dictionary.schema.yaml")).get("definitions") or {}
rows = [row("note", name if i == 0 else "", v, (d.get("x-values") or {}).get(v, ""), d.get("description", "") if i == 0 else "") for name, d in _dict.items() for i, v in enumerate(d.get("enum") or [])]
report("dictionary", "Dictionary: the registry's shared words", "Each list of values a field may take, and what each value means. A record's revision says how firm it is (status) and what to do when it does not fit your work (on_conflict): read those two first.", ["Word", "Value", "Means", "About"], rows)
# (How firm each kind's records are unless one says otherwise: its schema's default revision.)
rows = []
for kind, sch in SCHEMAS.items():
    dflt = ((sch.get("properties") or {}).get("revision") or {}).get("default")
    if dflt:
        own = [e for e in {"equipment": equipment, "hulls": hulls, "parts": parts, "modules": modules, "goods": goods, "materials": materials, "gates": gates}.get(kind, []) if e.get("revision")]
        rows.append(row("note", KIND_NAME.get(kind, kind) if "KIND_NAME" in globals() else kind, dflt.get("status", ""), dflt.get("on_conflict", ""), dflt.get("because", ""), ", ".join(f"{e['identity'].get('name', e['slug'])}: {e['revision']['status']}" for e in own) or "none"))
report("revisions", "Revisions: how firm each kind of record is", "What a record of each kind is unless it says otherwise, and the records that say otherwise. draft: do not balance against it. review: build against it, propose changes. complete: a hard limit, and its record may not change.", ["Kind", "Status", "On conflict", "Because", "Records that say otherwise"], rows)

# 4. What goes in against what comes out, for each industrial module.
rows = []
for m, rc in [(m, rc) for m in modules for rc in m.get("recipes") or []]:
    ins = sum(x.get("amount", 0) for x in rc.get("inputs") or [])
    outs = sum(x.get("amount", 0) for x in rc.get("outputs") or [])
    if not ins or not rc.get("throughput"):
        continue
    d = ins - (1 + outs)
    rows.append(row("ok" if abs(d) <= 0.02 * ins else "gap", link(m["identity"]["name"] + (f": {item_name(rc['product'])}" if len(m["recipes"]) > 1 else ""), "mod:" + m["slug"]), f"{ins:.4g} t", f"{1 + outs:.4g} t", f"{d:+.3g} t ({100 * d / ins:+.1f}%)", "balanced" if abs(d) <= 0.02 * ins else ("more goes in than comes out" if d > 0 else "more comes out than goes in")))
report("balance", "Balance: what goes into a module against what comes out", "For each tonne of a module's product: everything that goes in, against the product and everything else that comes out. Matter is not made or lost, so they should match. A gap is a difference of more than 2%.", ["Module", "Goes in", "Comes out", "Difference", ""], rows)

# 5. Each material, in each form it is said to come in: does a module's recipe make a stock of it in that form?
rows = []
_made = {}
for md in modules:
    for rc in md.get("recipes") or []:
        for k_ in [rc.get("product")] + [x.get("item") for x in rc.get("outputs") or []]:
            ms_ = stock_of.get(k_)
            if ms_:
                _made.setdefault(((ms_.get("made_from") or {}).get("material"), (ms_.get("made_from") or {}).get("form")), []).append(md["identity"]["name"])
for m in materials:
    for form in (m.get("identity") or {}).get("form") or []:
        by = sorted(set(_made.get((m["slug"], form), [])))
        rows.append(row("ok" if by else "gap", link(m["identity"]["name"], "mat:" + m["slug"]), form, ", ".join(by) or "nothing makes it: no stock of it in this form, or no recipe for that stock"))
report("materials", "Materials: is every form made", "Each material in each form it is said to come in, and the module whose recipe makes a stock of it in that form. A gap is a form named on the material with no stock, or stock with no recipe: either a works to describe, or a form to strike.", ["Material", "Form", "Made by"], rows)

for gd in goods:
    where = os.path.join(TREE, gd["file"])
    ore = (gd.get("game") or {}).get("ore")
    if ore is not None and ore not in ORES:
        problem(where, f"game.ore: the game has no ore '{ore}'")
    rock = (gd.get("source") or {}).get("won_from")
    if rock is not None and next((o for o in goods if o["slug"] == rock), {}).get("identity", {}).get("kind") != "rock":
        problem(where, f"source.won_from: no rock '{rock}' among the goods")
# 7. Each good: does something make it, and does something use it?
rows = []
for gd in goods:
    made = [m for m in modules if any(rc.get("product") == gd["slug"] or any(x.get("item") == gd["slug"] for x in rc.get("outputs") or []) for rc in m.get("recipes") or [])]
    made += [{"identity": {"name": "People"}, "slug": ""}] if any(g_["item"] == gd["slug"] for nd in needs for g_ in nd.get("gives") or []) else []
    uses = lambda slug: [m for m in modules if any(x.get("item") == slug for rc in m.get("recipes") or [] for x in rc.get("inputs") or [])]
    used = uses(gd["slug"])
    kind = (gd.get("identity") or {}).get("kind")
    need_made, need_used = kind not in ("rock", "raw", "consumable", "fuel"), kind not in ("by-product", "product", "rock")
    if kind == "rock":
        won = [o for o in goods if (o.get("source") or {}).get("won_from") == gd["slug"]]
        occurs = (gd.get("source") or {}).get("occurs")
        ore = (gd.get("game") or {}).get("ore")
        rows.append(row("ok" if occurs else "gap", link(gd["identity"]["name"], "good:" + gd["slug"]), kind, (f"dug on {occurs}" if occurs else "nowhere said") + ("" if ore else "; the game has no ore for it yet"), ", ".join(o["identity"]["name"] for o in won) or ", ".join(m["identity"]["name"] for m in used) or "nothing uses it yet"))
        continue
    if kind == "raw":
        rock = next((o for o in goods if o["slug"] == (gd.get("source") or {}).get("won_from")), None)
        rows.append(row("ok" if rock else "gap", link(gd["identity"]["name"], "good:" + gd["slug"]), kind, ("won from " + rock["identity"]["name"] + (f", {100 * gd['source']['yield']:.3g}% of it" if "yield" in gd["source"] else ", how much not said")) if rock else "no rock it is won from", ", ".join(m["identity"]["name"] for m in used) or "nothing uses it"))
        continue
    gap = (need_made and not made) or (need_used and not used)
    rows.append(row("gap" if gap else "ok", link(gd["identity"]["name"], "good:" + gd["slug"]), kind, ", ".join(m["identity"]["name"] for m in made) or ("comes from outside" if not need_made else "nothing makes it"), ", ".join(m["identity"]["name"] for m in used) or ("goes out" if not need_used else "nothing uses it")))
report("goods", "Goods: where each comes from and goes", "Each good: where it comes from and where it goes. A rock is dug; a raw good is won from a rock; consumables and fuel come from outside; the rest come out of one module and go into another, or out.", ["Good", "Kind", "Comes out of", "Goes into"], rows)


# 8. Confidence: where every number comes from.
# (Elements and materials are from published sources, named in their files; a process's amounts are
# its material's composition. Other records say for themselves, in `basis`; one that doesn't is a gap.)
DEFAULT_TIER = {"elements": "sourced", "materials": "sourced"}
KIND_NAME = {"elements": "Elements", "materials": "Materials", "modules": "Industrial modules", "goods": "Goods", "hulls": "Hulls", "mill-stock": "Mill stock", "parts": "Parts", "equipment": "Ship equipment", "gates": "Stargates"}
KEY_OF = {"gates": lambda e: "gate:" + e["slug"], "equipment": lambda e: "eq:" + e["slug"], "elements": lambda e: "el:" + e["identity"]["symbol"], "materials": lambda e: "mat:" + e["slug"], "processes": lambda e: "proc:" + e["slug"], "modules": lambda e: "mod:" + e["slug"], "goods": lambda e: "good:" + e["slug"], "hulls": lambda e: "hull:" + e["slug"], "mill-stock": lambda e: "stock:" + e["slug"], "parts": lambda e: "part:" + e["slug"]}


def numbers(d, path=()):
    """The numbers in a record as it is written: (group, property) for each."""
    if isinstance(d, bool):
        return []
    if isinstance(d, (int, float)):
        return [path[:2]] if path and path[0] not in ("basis",) else []
    if isinstance(d, dict):
        return [n for k, v in d.items() for n in numbers(v, path + (k,))]
    if isinstance(d, list):
        return [n for v in d for n in numbers(v, path)]
    return []


def tier_of(kind, raw, group, prop, review=False):
    # (A module's further recipes go by what its basis says of `recipes` as a whole: its figures for
    # one are sourced, the others are that set used again.)
    for exact in ((False,) if kind == "modules" and group == "recipes" else (True, False)):
        for en in raw.get("basis") or []:
            if (f"{group}.{prop}" in en["of"]) if exact else (group in en["of"]):
                return basis_review(en) if review else basis_tier(en)
    return False if review else DEFAULT_TIER.get(kind, "unsaid")


rows, detail, reviews = [], [], []
for kind, recs in (("elements", elements), ("materials", materials), ("processes", processes), ("modules", modules), ("goods", goods), ("hulls", hulls), ("mill-stock", mill_stock), ("parts", parts), ("equipment", equipment), ("gates", gates)):
    tally = {"sourced": 0, "derived": 0, "invented": 0, "unsaid": 0}
    to_review = 0
    for e in recs:
        raw = load_rec(e)
        if kind == "modules":
            # (Its first recipe is counted as the module's own figures; the others here.)
            raw["recipes"] = [{{"throughput": "rate", "product": "makes"}.get(k, k): v for k, v in rc.items()} for rc in (raw.get("recipes") or [])[1:]]
        mine = {"sourced": [], "derived": [], "invented": [], "unsaid": []}
        for path in numbers(raw):
            group, prop = path[0], path[1] if len(path) > 1 else ""
            if group == "identity" and kind not in ("elements",):
                continue
            mine[tier_of(kind, raw, group, prop)].append(f"{group}.{prop}".replace("_", " "))
            if tier_of(kind, raw, group, prop, review=True):
                to_review += 1
                reviews.append(row("gap", link((e.get("identity") or {}).get("name", e.get("slug")), KEY_OF[kind](e)), KIND_NAME[kind], f"{group}.{prop}".replace("_", " "), raw[group][prop] if isinstance(raw.get(group), dict) and prop in raw[group] else "", next((en.get("note", "") for en in raw.get("basis") or [] if f"{group}.{prop}" in en["of"]), "")))
        for t in tally:
            tally[t] += len(mine[t])
        if mine["invented"] or mine["unsaid"]:
            name = (e.get("identity") or {}).get("name", e.get("slug"))
            detail.append(row("gap" if mine["unsaid"] else "note", link(name, KEY_OF[kind](e)), KIND_NAME[kind], len(mine["invented"]), ", ".join(sorted(set(mine["invented"]))), len(mine["unsaid"]), ", ".join(sorted(set(mine["unsaid"])))))
    total = sum(tally.values())
    if total:
        share = lambda t: f"{tally[t]:,} ({100 * tally[t] / total:.0f}%)" if tally[t] else ""
        rows.append(row("gap" if tally["unsaid"] else "note" if tally["invented"] else "ok", KIND_NAME[kind], len(recs), f"{total:,}", share("sourced"), share("derived"), share("invented"), share("unsaid"), to_review or ""))
report("confidence", "Confidence: where the numbers come from", "Every number in the registry, by where it comes from. Sourced: from a published source or the game. Derived: worked out from other figures. Invented: chosen, to be balanced or replaced. A gap is a number whose record does not say.", ["Kind of record", "Records", "Numbers", "Sourced", "Derived", "Invented", "Not said", "To review"], rows)
report("review", "To review: guesses put in so a figure is there", "Each figure marked for review: a guess entered so the game has something to work with, to be replaced when a source or a way to work it out is found. Every row is a gap until it is reviewed.", ["Record", "Kind", "Figure", "Guess", "Note"], reviews)
report("invented", "Confidence: the records with invented or unexplained numbers", "Each record that has a number that was chosen, or one it does not explain. A gap is a number not explained.", ["Record", "Kind", "Invented", "Which", "Not said", "Which"], detail)


# ---------------------------------------------------------------- the celestial registry
# standards/Celestial: the seeded world written down. metadata/galaxy.yaml (the seed and its laws);
# metadata/systems/<system>.yaml, and in the folder of the same name bodies/<name>.yaml and
# fields/<name>.yaml. Each record has a status: seeded (as the seed makes it, written out by
# tools/standards/celestial_export.py), curated (a person's; the truth) or frozen.
CEL = os.path.join(TREE, "Celestial")
celestial = {"galaxy": {}, "systems": [], "groups": {}, "rock_classes": [], "vocabulary": []}
if os.path.isdir(CEL):
    cschema = {k: read_schema(os.path.join(CEL, "schema", f"{k}.schema.yaml")) for k in ("system", "body", "population", "rock-class", "vocabulary", "seeding")}
    # (The three seeding records share one schema; this build still takes each by its own name, and the galaxy's settings flat.)
    cschema["asteroids"] = cschema["conditions"] = cschema["seeding"]
    cschema["galaxy"] = {"properties": {**cschema["seeding"]["properties"]["galaxy"]["properties"], "note": {}}}
    celestial["groups"] = {k: {g: {q: v.get("description", "") for q, v in d["properties"].items()} for g, d in cschema[k]["properties"].items() if "properties" in d} for k in ("system", "body", "population", "rock-class")}

    def cel_load(full, kind):
        rec = load(full)
        known = cschema[kind]["properties"]
        for g, props in rec.items():
            if g not in known:
                problem(full, f"unknown group '{g}'")
            elif "properties" in known[g]:
                for q in props or {}:
                    if q not in known[g]["properties"]:
                        problem(full, f"{g}: unknown property '{q}'")
        if kind != "rock-class" and rec.get("provenance") not in ("seeded", "curated", "frozen", "baked"):
            problem(full, "provenance: one of seeded, curated, frozen, baked")
        idn_ = rec.get("identity") or {}
        if os.path.basename(full)[:-5] != re.sub(r"[^a-z0-9]+", "-", str(idn_.get("also") or idn_.get("name", "")).lower()).strip("-"):
            problem(full, "a celestial record's file is named after it as the seed first called it (lower case, words joined by -); a renamed body keeps the seed's name in identity.also")
        rec["slug"], rec["file"] = os.path.basename(full)[:-5], os.path.relpath(full, TREE)
        return rec

    cpath = os.path.join(CEL, "metadata", "seeding", "conditions.yaml")
    if os.path.exists(cpath):
        celestial["conditions"] = load(cpath)
        check_basis(celestial["conditions"], cpath)
    # (The vocabulary: each kind of thing a system has.)
    vdir = os.path.join(CEL, "metadata", "vocabulary")
    celestial["vocabulary"] = []
    for fn in sorted(os.listdir(vdir)) if os.path.isdir(vdir) else []:
        if not fn.endswith(".yaml"):
            continue
        vfull = os.path.join(vdir, fn)
        v_ = load(vfull)
        for g_ in v_:
            if g_ not in cschema["vocabulary"]["properties"]:
                problem(vfull, f"unknown field '{g_}'")
        for q in v_.get("identity") or {}:
            if q not in cschema["vocabulary"]["properties"]["identity"]["properties"]:
                problem(vfull, f"identity: unknown property '{q}'")
        if v_.get("in_game") not in ("made", "partly", "not made"):
            problem(vfull, "in_game: one of made, partly, not made")
        check_basis(v_, vfull)
        v_["slug"], v_["file"] = fn[:-5], os.path.relpath(vfull, TREE)
        celestial["vocabulary"].append(v_)
    GROUPS_ = ["star", "world", "moon", "region", "small body", "place", "condition"]
    celestial["vocabulary"].sort(key=lambda v_: (GROUPS_.index(v_["identity"]["group"]) if v_["identity"].get("group") in GROUPS_ else 99, v_["identity"]["name"]))
    # (How asteroids lie: the Sun's belts, the measure for each system's.)
    apath = os.path.join(CEL, "metadata", "seeding", "asteroids.yaml")
    if os.path.exists(apath):
        laws = load(apath)
        for g_, props in laws.items():
            known = cschema["asteroids"]["properties"].get(g_)
            if known is None:
                problem(apath, f"unknown group '{g_}'")
            elif g_ != "basis":
                for q in props or {}:
                    if q not in known["properties"]:
                        problem(apath, f"{g_}: unknown property '{q}'")
        check_basis(laws, apath)
        celestial["asteroids"] = laws
    for rc_path in (sorted(glob_ for glob_ in os.listdir(os.path.join(CEL, "metadata", "rock-classes")) if glob_.endswith(".yaml")) if os.path.isdir(os.path.join(CEL, "metadata", "rock-classes")) else []):
        check_basis(load(os.path.join(CEL, "metadata", "rock-classes", rc_path)), os.path.join(CEL, "metadata", "rock-classes", rc_path))
    # (The kinds of asteroid: each yields a rock of the SFO's goods.)
    rdir = os.path.join(CEL, "metadata", "rock-classes")
    celestial["rock_classes"] = [cel_load(os.path.join(rdir, fn), "rock-class") for fn in sorted(os.listdir(rdir)) if fn.endswith(".yaml")] if os.path.isdir(rdir) else []
    for rc in celestial["rock_classes"]:
        for q in ("yields", "rich_yields"):
            ore_ = (rc.get("mining") or {}).get(q)
            if ore_ is not None and next((g_ for g_ in goods if g_["slug"] == ore_), {}).get("identity", {}).get("kind") != "rock":
                problem(os.path.join(TREE, rc["file"]), f"mining.{q}: no rock '{ore_}' among the goods")
    gpath = os.path.join(CEL, "metadata", "seeding", "galaxy.yaml")
    if os.path.exists(gpath):
        celestial["galaxy"] = load(gpath)
        for q in celestial["galaxy"]:
            if q not in cschema["galaxy"]["properties"]:
                problem(gpath, f"unknown field '{q}'")
    sdir = os.path.join(CEL, "metadata", "systems")
    for fn in sorted(os.listdir(sdir)) if os.path.isdir(sdir) else []:
        if not fn.endswith(".yaml"):
            continue
        sysm = cel_load(os.path.join(sdir, fn), "system")
        sysm["bodies"] = [cel_load(os.path.join(sdir, fn[:-5], "bodies", b), "body") for b in sorted(os.listdir(os.path.join(sdir, fn[:-5], "bodies")))] if os.path.isdir(os.path.join(sdir, fn[:-5], "bodies")) else []
        sysm["fields"] = [cel_load(os.path.join(sdir, fn[:-5], "fields", b), "population") for b in sorted(os.listdir(os.path.join(sdir, fn[:-5], "fields")))] if os.path.isdir(os.path.join(sdir, fn[:-5], "fields")) else []
        sub = lambda d_, k_: [cel_load(os.path.join(sdir, fn[:-5], d_, b), k_) for b in sorted(os.listdir(os.path.join(sdir, fn[:-5], d_)))] if os.path.isdir(os.path.join(sdir, fn[:-5], d_)) else []
        sysm["small_bodies"], sysm["regions"] = sub("small-bodies", "body"), sub("regions", "population")
        # (Its star is a body record. This build still reads it as the system's own: mass and luminosity in the Sun's.)
        sun = next((b for b in sysm["bodies"] if b["identity"]["kind"] == "star"), None)
        if sun is None:
            problem(os.path.join(sdir, fn), "no star among its bodies")
        else:
            sysm["bodies"].remove(sun)
            sysm["star_body"] = sun
            sysm["star"] = {"class": (sun.get("star") or {}).get("class"), "luminosity": (sun.get("star") or {}).get("luminosity"), "mass": float(f"{(sun.get('physical') or {}).get('mass', 0) / SUN_KG:.4g}")}
        names = {b["identity"]["name"] for b in sysm["bodies"]} | {sysm["identity"]["name"]}
        for sb in sysm["small_bodies"]:
            if sb["identity"].get("parent") not in names:
                problem(os.path.join(TREE, sb["file"]), f"identity.parent: no body '{sb['identity'].get('parent')}' in {sysm['identity']['name']}")
            if (sb.get("rock") or {}).get("class") and not os.path.exists(os.path.join(CEL, "metadata", "rock-classes", sb["rock"]["class"] + ".yaml")):
                problem(os.path.join(TREE, sb["file"]), f"rock.class: no rock class '{sb['rock']['class']}'")
        for b in sysm["bodies"]:
            scape = (b.get("surface") or {}).get("landscape")
            if scape is not None and not os.path.exists(os.path.join(ROOT, scape)):
                problem(os.path.join(TREE, b["file"]), f"surface.landscape: no file '{scape}'")
            if b["identity"].get("parent") not in names:
                problem(os.path.join(TREE, b["file"]), f"identity.parent: no body '{b['identity'].get('parent')}' in {sysm['identity']['name']}")
        for f_ in sysm["fields"]:
            if f_["identity"].get("anchor") not in names:
                problem(os.path.join(TREE, f_["file"]), f"identity.anchor: no body '{f_['identity'].get('anchor')}' in {sysm['identity']['name']}")
        # (In order out from what each goes round.)
        sysm["bodies"].sort(key=lambda b: (b.get("orbit") or {}).get("semi_major_axis", 0))
        celestial["systems"].append(sysm)
    # Each system's belts, worked out: the main belt between two resonances of its first gas giant
    # (with no giant, round its frost line); two swarms on each giant's orbit; an icy belt past its
    # outermost giant. How many: the Sun's, by the ground each covers (the same number for each
    # square AU), and by size as the size law says.
    laws = celestial.get("asteroids") or {}
    AU_KM = AU_M / 1000
    res = lambda r_: (lambda a_, b_: (b_ / a_) ** (2 / 3))(*[float(v) for v in r_.split(":")])
    for sysm in celestial["systems"]:
        belts = []
        if not laws:
            continue
        planets = [b for b in sysm["bodies"] if b["identity"].get("parent") == sysm["identity"]["name"] and b["identity"]["kind"] != "asteroid"]
        giants = [b for b in planets if b["identity"]["kind"] in ("gas giant", "ice giant")]
        au = lambda b: b["orbit"]["semi_major_axis"] / AU_KM
        frost = 2.7 * (sysm.get("star") or {}).get("luminosity", 1) ** 0.5
        mb, ob, tj, sz = laws.get("main_belt") or {}, laws.get("outer_belt") or {}, laws.get("trojans") or {}, laws.get("sizes") or {}
        smaller = lambda n_, from_km, to_km: n_ * (from_km / to_km) ** sz.get("exponent", 2)
        ring = lambda lo, hi: math.pi * (hi ** 2 - lo ** 2)
        # (The giant that shapes the belt: the first gas giant out by the frost line, or else the first giant.)
        gas = next((b for b in giants if b["identity"]["kind"] == "gas giant" and au(b) > 0.8 * frost), giants[0] if giants else None)
        if mb:
            lo, hi = (au(gas) * res(mb["inner_resonance"]), au(gas) * res(mb["outer_resonance"])) if gas else (mb.get("no_giant_inner", 0.8) * frost, mb.get("no_giant_outer", 1.3) * frost)
            n1 = mb["count_over_1km"] * ring(lo, hi) / ring(mb["inner_edge"], mb["outer_edge"])
            belts.append({"name": "Main belt", "kind": "main", "inner": lo, "outer": hi, "by": gas["identity"]["name"] if gas else None, "over_1km": n1, "over_100m": smaller(n1, 1, 0.1), "over_smallest": smaller(n1, 1, sz.get("smallest", 15) / 1000),
                          "over_100km": smaller(n1, 1, 100), "spacing": mb.get("spacing"), "families": round(mb.get("families", 0) * ring(lo, hi) / ring(mb["inner_edge"], mb["outer_edge"])), "family_share": mb.get("family_share"), "inside_frost": hi <= frost})
        for g_ in giants if tj else []:
            for lead in ("L4", "L5"):
                n1 = tj["count_over_1km"] / 2 * ((g_.get("physical") or {}).get("mass", 0) / tj["giant_mass"] if tj.get("giant_mass") else 1)
                belts.append({"name": f"{g_['identity']['name']} {lead}", "kind": "trojan", "inner": au(g_), "outer": au(g_), "by": g_["identity"]["name"], "over_1km": n1, "over_100m": smaller(n1, 1, 0.1), "over_smallest": smaller(n1, 1, sz.get("smallest", 15) / 1000),
                              "spread": tj.get("spread"), "lead": lead, "inside_frost": au(g_) <= frost})
        if giants and ob:
            last = giants[-1]
            lo, hi = au(last) / res(ob["inner_resonance"]), au(last) / res(ob["outer_resonance"])
            n100 = ob["count_over_100km"] * ring(lo, hi) / ring(ob["inner_edge"], ob["outer_edge"])
            belts.append({"name": "Outer belt", "kind": "outer", "inner": lo, "outer": hi, "by": last["identity"]["name"], "over_100km": n100, "over_1km": smaller(n100, 100, 1), "inside_frost": False})
        # (The game's fields, each in the belt it lies in.)
        for fl in sysm["fields"]:
            an = next((b for b in sysm["bodies"] if b["identity"]["name"] == fl["identity"].get("anchor")), None)
            a_ = au(an) if an and an.get("orbit") else None
            kind = {"family": "main", "trojan": "trojan", "outer": "outer"}.get(fl["identity"].get("kind"))
            cand = [bl for bl in belts if bl["kind"] == kind and (kind != "trojan" or (bl["name"].split(" ")[-1] in fl["identity"]["name"] and bl["by"] in fl["identity"]["name"]))]
            if cand:
                cand[0].setdefault("fields", []).append(fl["slug"])
                fl["belt"] = cand[0]["name"]
                fl["in_belt"] = a_ is None or kind == "trojan" or cand[0]["inner"] * 0.98 <= a_ <= cand[0]["outer"] * 1.02
        zn = laws.get("zones") or {}
        def mix_of(bl):
            if bl["kind"] == "trojan":
                parts_ = [("trojans", 1.0)]
            elif bl["kind"] == "outer":
                parts_ = [("outer", 1.0)]
            else:
                # (By the ground the belt has in each zone.)
                cuts = [bl["inner"], min(max(zn.get("warm_to", 0.93) * frost, bl["inner"]), bl["outer"]), min(max(zn.get("frost_to", 1.04) * frost, bl["inner"]), bl["outer"]), bl["outer"]]
                areas = [cuts[i + 1] ** 2 - cuts[i] ** 2 for i in range(3)]
                parts_ = [(z_, a_ / sum(areas)) for z_, a_ in zip(("warm", "frost_line", "cold"), areas) if a_ > 1e-12]
            out_ = {}
            for z_, w_ in parts_:
                for rc in celestial["rock_classes"]:
                    v_ = (rc.get("found") or {}).get(z_, 0) * w_
                    if v_:
                        out_[rc["slug"]] = out_.get(rc["slug"], 0) + v_
            return {"zones": [{"zone": z_, "share": w_} for z_, w_ in parts_], "classes": sorted(({"class": k_, "share": v_} for k_, v_ in out_.items()), key=lambda c_: -c_["share"])}
        for bl in belts:
            bl["mix"] = mix_of(bl)
        star_kg = (sysm.get("star") or {}).get("mass", 0) * SUN_KG
        for b in sysm["bodies"]:
            par = next((o for o in sysm["bodies"] if o["identity"]["name"] == b["identity"].get("parent")), None)
            big = (par.get("physical") or {}).get("mass") if par else star_kg
            m_, a_ = (b.get("physical") or {}).get("mass"), (b.get("orbit") or {}).get("semi_major_axis")
            if big and m_ and a_ and b["identity"]["kind"] != "asteroid":
                b["balance"] = {"reach": a_ * (m_ / (3 * big)) ** (1 / 3), "round": par["identity"]["name"] if par else sysm["identity"]["name"], "stable": big / m_ > 24.96, "ratio": big / m_}
        # (What follows from each body's mass, size, spin and orbit: worked out, not written.)
        lum_w = (sysm.get("star") or {}).get("luminosity", 0) * SUN_W
        for b in sysm["bodies"]:
            ph, ob = b.get("physical") or {}, b.get("orbit") or {}
            if not (ph.get("mass") and ph.get("radius")):
                continue
            R_, M_ = ph["radius"] * 1000, ph["mass"]
            par = next((o for o in sysm["bodies"] if o["identity"]["name"] == b["identity"].get("parent")), None)
            a_star = ((par or b).get("orbit") or {}).get("semi_major_axis", 0) * 1000        # (its distance from the star: its planet's, for a moon)
            w = {"density": M_ / (4 / 3 * math.pi * R_ ** 3), "escape": (2 * G_N * M_ / R_) ** 0.5, "orbit_speed": (G_N * M_ / R_) ** 0.5,
                 "to_orbit": G_N * M_ / R_ / 2, "to_escape": G_N * M_ / R_}
            if a_star and lum_w:
                w["sunlight"] = lum_w / (4 * math.pi * a_star ** 2)
                w["bare_temperature"] = (w["sunlight"] * (1 - ph.get("albedo", 0.3)) / (4 * SIGMA)) ** 0.25
            if ph.get("day") and b["identity"]["kind"] != "asteroid":
                sync = (G_N * M_ * (abs(ph["day"]) * 3600 / (2 * math.pi)) ** 2) ** (1 / 3)
                w["stationary_orbit"] = sync / 1000
                w["stationary_holds"] = "balance" in b and sync / 1000 < b["balance"]["reach"] / 3 and sync > R_
                w["spin_speed"] = 2 * math.pi * R_ / (abs(ph["day"]) * 3600)
            if ob.get("semi_major_axis"):
                w["nearest"], w["farthest"] = ob["semi_major_axis"] * (1 - ob.get("eccentricity", 0)), ob["semi_major_axis"] * (1 + ob.get("eccentricity", 0))
            b["worked"] = w
        # (What it has of each kind in the vocabulary, counted from its records.)
        kinds = {}
        for b in sysm["bodies"]:
            k_ = {"rocky planet": "rocky-planet", "gas giant": "gas-giant", "ice giant": "ice-giant", "moon": "moon", "asteroid": "asteroid"}.get(b["identity"]["kind"])
            kinds[k_] = kinds.get(k_, 0) + 1
            if (b.get("physical") or {}).get("rings"):
                kinds["ring-system"] = kinds.get("ring-system", 0) + 1
        kinds["star"] = 1
        for bl in belts:
            k_ = {"main": "main-belt", "trojan": "trojan-swarm", "outer": "outer-belt"}[bl["kind"]]
            kinds[k_] = kinds.get(k_, 0) + 1
        kinds["asteroid-family"] = sum(bl.get("families") or 0 for bl in belts)
        kinds["asteroid-moon"] = len(sysm["fields"])
        kinds["balance-point"] = 5 * sum(1 for b in sysm["bodies"] if "balance" in b)
        # (What the registry has seeded of what the game does not make.)
        for sb in sysm.get("small_bodies") or []:
            k_ = {"comet": "comet", "centaur": "centaur", "crossing asteroid": "crossing-asteroid", "captured moon": "captured-moon", "dwarf planet": "dwarf-planet", "asteroid": "asteroid"}[sb["identity"]["kind"]]
            kinds[k_] = kinds.get(k_, 0) + 1
            o_ = sb.get("orbit") or {}
            if "semi_major_axis" in o_ and sb["identity"].get("parent") == sysm["identity"]["name"]:
                sb["nearest"], sb["farthest"] = o_["semi_major_axis"] * (1 - o_.get("eccentricity", 0)) / AU_KM, o_["semi_major_axis"] * (1 + o_.get("eccentricity", 0)) / AU_KM
        for rg in sysm.get("regions") or []:
            k_ = {"scattered disc": "scattered-disc", "far cloud": "far-cloud", "meteoroid stream": "meteoroid-stream"}[rg["identity"]["kind"]]
            kinds[k_] = kinds.get(k_, 0) + 1
            if rg["identity"]["kind"] == "meteoroid stream":
                ex = rg.get("extent") or {}
                rg["crosses"] = [b["identity"]["name"] for b in planets if ex.get("inner", 0) <= au(b) <= ex.get("outer", 0)]
        # (What each giant does to its moons: the heat its kneading makes in them, and the dose of its belt.)
        cn = celestial.get("conditions") or {}
        th, rb = cn.get("tidal_heating") or {}, cn.get("radiation_belt") or {}
        for b in sysm["bodies"]:
            par = next((o for o in giants if o["identity"]["name"] == b["identity"].get("parent")), None)
            if par is None or b["identity"]["kind"] != "moon":
                continue
            M_, R_, a_m, e_ = par["physical"]["mass"], b["physical"]["radius"] * 1000, b["orbit"]["semi_major_axis"] * 1000, b["orbit"].get("eccentricity", 0)
            cond = {}
            if th:
                n_ = (G_N * M_ / a_m ** 3) ** 0.5
                heat = 10.5 * th.get("love_over_q", 0) * G_N * M_ ** 2 * R_ ** 5 * n_ * e_ ** 2 / a_m ** 6
                flux = heat / (4 * math.pi * R_ ** 2)
                cond["tidal"] = {"heat": heat, "flux": flux, "state": "far more than any moon known: an orbit this close and this stretched would long since have been made round. Its orbit as seeded is not one that lasts" if flux >= 20 * th.get("volcanic_above", 1) else "volcanic" if flux >= th.get("volcanic_above", 1) else "warm inside: an icy one may keep a buried sea" if flux >= th.get("sea_above", 0.03) else "slight"}
                if flux >= th.get("sea_above", 0.03):
                    kinds["tidal-heating"] = kinds.get("tidal-heating", 0) + 1
            if rb and par["identity"]["kind"] == "gas giant":
                radii = a_m / (par["physical"]["radius"] * 1000)
                dose = rb["dose"] * (radii / rb["at"]) ** -rb["falls_as"] * par["physical"]["mass"] / rb["giant_mass"]
                cond["radiation"] = {"radii": radii, "dose": dose, "deadly": dose >= rb.get("deadly_above", 100)}
                if dose >= rb.get("deadly_above", 100):
                    kinds["radiation-belt"] = kinds.get("radiation-belt", 0) + 1
            b["conditions"] = cond
        sysm["has"] = kinds
        sysm["belts"] = belts
        sysm["frost_line"] = frost
    home = celestial["galaxy"].get("home")
    celestial["systems"].sort(key=lambda s: (s["identity"]["name"] != home, (s.get("position") or {}).get("distance", 0)))
    if home and home not in {s["identity"]["name"] for s in celestial["systems"]}:
        problem(gpath, f"home: no system '{home}' written out")
# The celestial report: each system's records by status, and each body Local Administration has
# against the celestial record of the same name.
rows = []
for sysm in celestial["systems"]:
    recs = [sysm] + sysm["bodies"] + sysm["fields"]
    if sysm.get("small_bodies") or sysm.get("regions"):
        rows.append(row("note", link(sysm["identity"]["name"], "cs:" + sysm["slug"]), f"{len(sysm.get('small_bodies') or [])} small bodies, {len(sysm.get('regions') or [])} regions", "seeded in the registry", "the game does not make these yet"))
    for b in sysm["bodies"]:
        t_ = (b.get("conditions") or {}).get("tidal") or {}
        if t_.get("state", "").startswith("far more"):
            rows.append(row("gap", link(b["identity"]["name"], f"cb:{sysm['slug']}:{b['slug']}"), "", "", f"its planet would knead {t_['flux']:,.0f} W into each m2 of it, against about 2 for the most volcanic moon known: its orbit, as the seed makes it, is too close and too stretched to last"))
    count = lambda st: sum(1 for r_ in recs if r_.get("provenance") == st)
    rows.append(row("ok", link(sysm["identity"]["name"], "cs:" + sysm["slug"]), f"{len(sysm['bodies'])} bodies, {len(sysm['fields'])} fields", f"{count('seeded')} seeded, {count('curated')} curated, {count('frozen')} frozen", ""))
# (A body a person has taken over: what follows from its mass and radius must still agree with them.
# The game takes its mass, radius, day, orbit (size, shape, tilt), axis, rings, terrain, relief, air
# and colour; its gravity, period and temperature it works out itself. A system taken over: its star,
# and its planets and moons exactly as written. A field taken over: its count, extent and class.)
for sysm in celestial["systems"]:
    if sysm.get("provenance") != "seeded":
        rows.append(row("ok", link(sysm["identity"]["name"], "cs:" + sysm["slug"]), sysm["provenance"], "", "its star, and its planets and moons exactly as written: one the seed makes that has no record is not there"))
    for fl in sysm["fields"]:
        if fl.get("provenance") != "seeded":
            rows.append(row("ok", link(fl["identity"]["name"], f"cf:{sysm['slug']}:{fl['slug']}"), fl["provenance"], "", "taken by the game as written"))
    for b in sysm["bodies"]:
        if b.get("provenance") == "seeded":
            continue
        ph = b.get("physical") or {}
        g_ = G_N * ph.get("mass", 0) / (ph.get("radius", 1) * 1000) ** 2
        said = ph.get("gravity")
        off = said is not None and abs(said - g_) > 0.01 * g_
        rows.append(row("gap" if off else "ok", link(b["identity"]["name"], f"cb:{sysm['slug']}:{b['slug']}"), b["provenance"], "", f"its gravity is written as {said} and its mass and radius give {g_:.3f}: the game uses its mass and radius" if off else "taken by the game as written"))
for ad in administrations:
    sysm = next((s for s in celestial["systems"] if s["identity"]["name"] == ad.get("name")), None)
    if sysm is None:
        rows.append(row("gap", ad.get("name"), "", "", "Local Administration has it; no celestial record"))
        continue
    for x in ad["bodies"]:
        if x.get("kind") not in ("planet", "moon"):
            continue
        b = next((b for b in sysm["bodies"] if b["identity"]["name"] == x["name"]), None)
        if b is None:
            rows.append(row("gap", link(x["name"], f"bd:{ad['slug']}:{x['slug']}"), "", "", "Local Administration has it; no celestial record"))
        elif "gravity" in x and abs(x["gravity"] - (b.get("physical") or {}).get("gravity", 0)) > 0.02 * x["gravity"]:
            rows.append(row("gap", link(x["name"], f"cb:{sysm['slug']}:{b['slug']}"), "", "", f"its gravity is {x['gravity']} in Local Administration and {(b.get('physical') or {}).get('gravity')} here"))
        else:
            x["celestial"] = f"cb:{sysm['slug']}:{b['slug']}"
for where_ in ("warm", "frost_line", "cold", "trojans", "outer"):
    tot = sum((rc.get("found") or {}).get(where_, 0) for rc in celestial["rock_classes"])
    if celestial["rock_classes"]:
        rows.append(row("ok" if abs(tot - 1) < 1e-6 else "gap", "Rock classes found, " + where_.replace("_", " "), f"{len(celestial['rock_classes'])} classes", "", f"their shares add to {tot:g}" + ("" if abs(tot - 1) < 1e-6 else ", not 1")))
for rc in celestial["rock_classes"]:
    lacks = [w_ for w_, has_ in (("its density", (rc.get("physical") or {}).get("density_rubble")), ("what it is made of", rc.get("composition")), ("what it yields", (rc.get("mining") or {}).get("yields"))) if not has_]
    if lacks or not rc["identity"].get("label"):
        rows.append(row("gap", link(rc["identity"]["name"], "cr:" + rc["slug"]), "rock class", "", "; ".join((["not in the game yet"] if not rc["identity"].get("label") else []) + (["not said: " + ", ".join(lacks)] if lacks else []))))
for sysm in celestial["systems"]:
    for bl in sysm.get("belts") or []:
        if bl["kind"] != "trojan" or bl.get("fields"):
            rows.append(row("ok" if bl.get("fields") else "note", f"{sysm['identity']['name']}: {bl['name']}", f"{bl['inner']:.2f} to {bl['outer']:.2f} AU" if bl["kind"] != "trojan" else f"at {bl['inner']:.2f} AU", f"about {bl['over_1km']:,.0f} over 1 km", f"{len(bl.get('fields') or [])} of the game's fields in it" if bl.get("fields") else "the game has no field in it"))
    for fl in sysm["fields"]:
        if fl.get("in_belt") is False or "belt" not in fl:
            rows.append(row("gap", link(fl["identity"]["name"], f"cf:{sysm['slug']}:{fl['slug']}"), "", "", "it lies in no belt its system has, by the laws written out"))
keys_ = {rc["identity"]["label"].lower() for rc in celestial["rock_classes"] if rc["identity"].get("label")}
for sysm in celestial["systems"]:
    for fl in sysm["fields"]:
        if celestial["rock_classes"] and (fl.get("rocks") or {}).get("class") not in keys_:
            rows.append(row("gap", link(fl["identity"]["name"], f"cf:{sysm['slug']}:{fl['slug']}"), "", "", f"its class '{(fl.get('rocks') or {}).get('class')}' is no rock class written out"))
report("celestial", "Celestial: what is written out, and against Local Administration", "Each system written out: its records by status. Each planet and moon Local Administration has: is there a celestial record of the same name, and do they agree. A gap is a body with no celestial record, or one where the two differ.", ["What", "Records", "By status", "Note"], rows)


# ---------------------------------------------------------------- the page
# ---------------------------------------------------------------- every record against its schema
# Types, enums, required fields, patterns, and no field its schema does not name: validate.py is the one gate for a record's shape
# (additionalProperties: false in every schema); the build adds only what crosses records.
# The game's loader is to be at least this strict.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
_misfits, _unheld = V.check_all()
for _full, _what in _misfits:
    problem(_full, "schema: " + _what)
for _rel in _unheld:
    problem(os.path.join(TREE, _rel), "no schema holds it")
# (The change tracker follows a registry that is whole: standards/changes.yaml.)
if not _misfits and not _unheld:
    import tracker
    tracker.write(V.SEEN)


def write_game_keys():
    """standards/game-keys.yaml: for every record the game knows today by another key or by its name, the
    two side by side. Data for the game to rename by; it goes when the game loads by the registry's keys."""
    rows = []
    for key, (rel, rec) in sorted(REGISTRY.items()):
        kind, _, rest = key.partition(".")
        idn = rec.get("identity") if isinstance(rec.get("identity"), dict) else rec
        was, where = None, None
        if kind == "org" and rec.get("kind") in OLD_KEY:
            was, where = OLD_KEY[rec["kind"]] + "." + rest.replace("-", "_"), "formerly brands.ron" if rec["kind"] == "company" else "formerly bodies.ron"
        elif kind == "module":
            was, where = rest, "formerly industry.ron, settlements.ron"
        elif kind in ("rock-class", "law") and idn.get("label"):
            if kind == "law":
                continue                    # (the engine makes its constant under the law's label: nothing to rename)
            was, where = idn["label"], "belt.rs"
        elif kind in ("system", "body", "population") and rec.get("in_game") != "not made":
            was, where = idn.get("also") or idn.get("name"), "by name: celestial.ron, the seed" + ("; the record's name is its people's, `also` is the seed's" if idn.get("also") else "")
        elif kind == "settlement":
            was, where = idn.get("name"), "formerly by name: settlements.ron, places.ron"
        elif kind == "standard":
            was, where = "SFO " + rest.split(".")[-1], "formerly standards.ron"
        elif kind == "good" and rec.get("game"):
            was, where = ", ".join(str(v) for v in rec["game"].values()), "goods.ron, ores.ron (what it is traded and dug as; not the same thing)"
        if was is not None and was != key:
            rows.append((key, was, where))
    with open(os.path.join(TREE, "game-keys.yaml"), "w", encoding="utf-8") as f:
        f.write("# GENERATED by tools/standards/build.py: do not edit. Every record the game knows today by another key, or by\n"
                "# its name: the registry's key, what the game has, and where. For the game to rename by. A record not\n"
                "# here is either not in the game or has the same key there.\n")
        for key, was, where in rows:
            f.write(f"- {{ key: {key}, game: {json.dumps(was)}, in: {json.dumps(where)} }}\n")
    return len(rows)


def write_html():
    data = {
        "bodies": [{k: b[k] for k in ("key", "name", "prefix", "seat", "address", "note", "kind", "purpose", "details", "founded_by", "about") if k in b} for b in bodies],
        "brands": BRANDS,
        "house": house,
        "local": local,
        "port": PORT,
        "goods_kinds": GOODS_KINDS,
        "administrations": administrations,
        "celestial": celestial,
        "dogma": dogma,
        # (Logos: MakerHouse/logos/<a maker's file name>.svg, drawn inline.)
        "logos": {f[:-4]: open(os.path.join(TREE, HOUSE, "logos", f), encoding="utf-8").read().strip() for f in sorted(os.listdir(os.path.join(TREE, HOUSE, "logos"))) if f.endswith(".svg")} if os.path.isdir(os.path.join(TREE, HOUSE, "logos")) else {},
        "makers": makers,
        "elements": elements,
        "materials": materials,
        "processes": processes,
        "routes": routes,
        "modules": modules,
        "goods": goods,
        "hulls": hulls,
        "mill_stock": mill_stock,
        "equipment": equipment,
        "gates": gates,
        "structures": structures,
        "gate_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["gates"]["properties"].items() if "properties" in d},
        "equipment_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["equipment"]["properties"].items() if "properties" in d},
        "mill_stock_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["mill-stock"]["properties"].items() if "properties" in d},
        "parts": parts,
        "part_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in PART_SCHEMA["properties"].items() if "properties" in d},
        "hull_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["hulls"]["properties"].items() if "properties" in d},
        "good_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["goods"]["properties"].items() if "properties" in d},
        "module_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["modules"]["properties"].items() if "properties" in d},
        # (Icons: SFO/icons/<a record's file name>.svg, drawn inline so they take the page's colour.)
        "icons": {f[:-4]: open(os.path.join(TREE, "SFO", "icons", f), encoding="utf-8").read().strip() for f in sorted(os.listdir(os.path.join(TREE, "SFO", "icons"))) if f.endswith(".svg")} if os.path.isdir(os.path.join(TREE, "SFO", "icons")) else {},
        "process_groups": {},   # (processes are retired; a line's route is drawn in their shape without a schema)
        # (Each property's unit or note, from the schemas.)
        "element_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["elements"]["properties"].items() if "properties" in d},
        "material_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["materials"]["properties"].items() if "properties" in d},
        "standards": sorted(standards, key=lambda s: (s["body"], s["number"])),
        "cited": cited,
        "problems": problems,
        "reports": reports,
    }
    template = open(os.path.join(os.path.dirname(__file__), "page.html"), encoding="utf-8").read()
    page = template.replace("/*DATA*/null", json.dumps(data, ensure_ascii=False).replace("</", "<\\/"))
    with open(os.path.join(TREE, "index.html"), "w", encoding="utf-8") as f:
        f.write(page)


write_html()
if problems:
    print(f"{len(problems)} problem(s):", file=sys.stderr)
    for p in problems:
        print("  " + p, file=sys.stderr)
    print("standards/index.html written (it lists them too); the game's content NOT updated", file=sys.stderr)
    sys.exit(1)
_renames = write_game_keys()
print(f"{len(makers)} makers, {len(bodies)} bodies, {len(standards)} standards, {len(elements)} elements, {len(materials)} materials, {len(processes)} processes, {len(modules)} modules, {len(goods)} goods, {len(hulls)} hulls")
print(f"  reports: " + ", ".join(f"{r['key']} {r['gaps']} gaps" for r in reports))
print("  standards/index.html, standards/game-keys.yaml, standards/changes.yaml")
