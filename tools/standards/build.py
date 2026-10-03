#!/usr/bin/env python3
"""The standards, from their YAML, checked and built.

    python3 tools/standards/build.py

Reads `standards/`: a folder per body (named after its prefix) with `<PREFIX>.yaml`, and its
standards beside it, flat, one file each: `NNNN-<slug>.yaml`, the number its permanent id
(`SFO 12`: never reused, never changed, whatever it's later filed under). A standard's `topics`
are free tags describing it; how they'll be classified is left until patterns show.

Writes:
- `standards/index.html`: browse by topic or by number (rerun and refresh after an edit);
- `content/base/bodies.ron` and `content/base/standards.ron`: what the game loads (generated:
  edit the YAML, not these; the game's tree is the topics for now).

Exits non-zero with every problem listed if anything's wrong (the page still shows them).
See docs/standards.md.
"""
import html
import json
import os
import re
import sys

import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TREE = os.path.join(ROOT, "standards")
CONTENT = os.path.join(ROOT, "content", "base")

STATUSES = ["draft", "published", "superseded", "withdrawn"]
CHECKS = ["at_most", "at_least", "equals", "fits_within", "provides"]
BODY_FIELDS = {"key", "name", "prefix", "seat", "note", "kind", "purpose", "details", "founded_by", "about"}
BODY_KINDS = ["consortium", "independent", "authority", "corporation", "players"]
STANDARD_FIELDS = {"version", "title", "parent", "records", "purpose", "details", "status", "topics", "scope", "sections", "refs", "params", "requires", "text", "licence", "published"}

problems = []


def problem(where, what):
    problems.append(f"{os.path.relpath(where, ROOT)}: {what}")


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return yaml.safe_load(f) or {}
    except yaml.YAMLError as e:
        problem(path, f"not valid YAML: {e}")
        return {}


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
HOUSE = "MakerHouse"
house = load(os.path.join(TREE, HOUSE, "metadata", HOUSE + ".yaml"))
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
        if k not in {"key", "name", "ticker", "note", "who", "what", "story", "slug", "file"}:
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
local = load(os.path.join(TREE, LOCAL, "metadata", LOCAL + ".yaml"))
administrations = []
adm_dir = os.path.join(TREE, LOCAL, "metadata", "administrations")
for name in sorted(os.listdir(adm_dir)) if os.path.isdir(adm_dir) else []:
    full = os.path.join(adm_dir, name)
    if not re.fullmatch(r"[a-z0-9-]+\.yaml", name):
        problem(full, "an administration's file is named <name>.yaml (lower case, words joined by -)")
        continue
    ad = load(full)
    for k in ["name", "system"]:
        if k not in ad:
            problem(full, f"no {k}")
    for k in ad:
        if k not in {"name", "system", "bodies"}:
            problem(full, f"unknown field '{k}'")
    names = [x.get("name") for x in ad.get("bodies") or []]
    for x in ad.get("bodies") or []:
        if x.get("kind") not in ("planet", "moon", "belt", "settlement"):
            problem(full, f"body {x.get('name')}: kind one of planet, moon, belt, settlement")
        if "at" in x and x["at"] not in names:
            problem(full, f"body {x.get('name')}: at '{x['at']}', no such body")
        if names.count(x.get("name")) > 1:
            problem(full, f"body {x.get('name')} twice")
    ad["file"] = os.path.relpath(full, TREE)
    ad["slug"] = name[:-5]
    administrations.append(ad)

# The Land Register: parcels of land, one file each (LandRegister/metadata/parcels/<name>.yaml),
# to LandRegister/schema/parcel.schema.yaml. Each is owned by a company in Maker House.
LAND = "LandRegister"
land = load(os.path.join(TREE, LAND, "metadata", LAND + ".yaml"))
parcels = []
parcels_dir = os.path.join(TREE, LAND, "metadata", "parcels")
for name in sorted(os.listdir(parcels_dir)) if os.path.isdir(parcels_dir) else []:
    full = os.path.join(parcels_dir, name)
    if name.startswith("."):
        continue
    if not re.fullmatch(r"[a-z0-9-]+\.yaml", name):
        problem(full, "a parcel's file is named <name>.yaml (lower case, words joined by -)")
        continue
    pc = load(full)
    for k in ["name", "administration", "body", "owner"]:
        if k not in pc:
            problem(full, f"no {k}")
    adm = next((a for a in administrations if a["slug"] == pc.get("administration")), None)
    if "administration" in pc and adm is None:
        problem(full, f"administration: no '{pc['administration']}' in Local Administration")
    of = next((x for x in (adm or {}).get("bodies") or [] if x.get("name") == pc.get("body")), None)
    if adm is not None and "body" in pc and of is None:
        problem(full, f"body: no '{pc['body']}' among {adm.get('name')}'s bodies")
    for k in pc:
        if k not in {"name", "administration", "body", "place", "owner"}:
            problem(full, f"unknown field '{k}'")
    pc["body_kind"] = (of or {}).get("kind", "")
    if "owner" in pc and pc["owner"] not in BRANDS and not str(pc["owner"]).startswith("body."):
        problem(full, f"owner: no maker '{pc['owner']}' in Maker House")
    pc["file"] = os.path.relpath(full, TREE)
    pc["slug"] = name[:-5]
    parcels.append(pc)

bodies, standards = [], []
for name in sorted(os.listdir(TREE)):
    folder = os.path.join(TREE, name)
    if not os.path.isdir(folder) or name in ("schema", HOUSE, LAND, LOCAL):
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
KINDS = {"elements": "element", "materials": "material", "processes": "process"}
SCHEMAS = {k: yaml.safe_load(open(os.path.join(TREE, "SFO", "schema", f"{v}.schema.yaml"), encoding="utf-8")) for k, v in KINDS.items()}
elements, materials, processes = [], [], []
for s in standards:
    if "records" not in s:
        continue
    kind = str(s["records"])
    prefix = s["id"].split(" ")[0]
    folder = os.path.join(TREE, prefix, "metadata", kind)
    if kind not in KINDS:
        problem(os.path.join(TREE, s["file"]), f"records: one of {', '.join(KINDS)}")
        continue
    if not os.path.isdir(folder):
        problem(os.path.join(TREE, s["file"]), f"records: no folder metadata/{kind}")
        continue
    seen = {}
    for name in sorted(os.listdir(folder)):
        full = os.path.join(folder, name)
        pattern = r"[0-9]{3}-[a-z-]+\.yaml" if kind == "elements" else r"[a-z0-9-]+\.yaml"
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
        elif kind == "processes":
            for k in ("name", "kind"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
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
        for group, props in e.items():
            if group == "slug":
                continue
            known = SCHEMAS[kind]["properties"].get(group)
            if known is None:
                problem(full, f"unknown group '{group}'")
                continue
            for k in props or {}:
                if k not in known["properties"]:
                    problem(full, f"{group}: unknown property '{k}'")
        e["under"] = s["id"]
        e["file"] = os.path.relpath(full, TREE)
        {"elements": elements, "materials": materials, "processes": processes}[kind].append(e)
elements.sort(key=lambda e: (e.get("identity") or {}).get("atomic_number", 0))
materials.sort(key=lambda e: (e.get("identity") or {}).get("name", ""))
# (A process's inputs and outputs name elements by symbol, materials by file name.)
symbols = {(e.get("identity") or {}).get("symbol") for e in elements}
slugs = {m.get("slug") for m in materials}
for pr in processes:
    where = os.path.join(TREE, pr["file"])
    for group in ("inputs", "outputs"):
        for listed in ("materials", "consumables", "products", "by_products", "waste"):
            for x in (pr.get(group) or {}).get(listed, []) or []:
                item = x.get("item")
                if item is None and not x.get("name"):
                    problem(where, f"{group}.{listed}: an entry needs item or name")
                elif item is not None and item not in symbols and item not in slugs:
                    problem(where, f"{group}.{listed}: '{item}' is no element's symbol and no material's file name")

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


def write_ron():
    head = "// GENERATED by tools/standards/build.py from standards/ — edit the YAML there, then rerun it.\n"
    with open(os.path.join(CONTENT, "brands.ron"), "w", encoding="utf-8") as f:
        f.write(head + "// Makers of modules. Each has a home (a settled system, picked from the\n// galaxy's seed) where all its range is sold; farther off, less of it is\n// carried and it costs more (shipping). See docs/content.md.\n[\n")
        for m in makers:
            f.write(f"    (key: {ron_str(m['key'])}, name: {ron_str(caps(m['name']))}, note: {ron_str(caps(m['note']))}),\n")
        f.write("]\n")
    out = [head, "["]
    for b in bodies:
        out.append("    (")
        out.append(f"        key: {ron_str(b['key'])},\n        name: {ron_str(caps(b['name']))},\n        prefix: {ron_str(b['prefix'])},")
        out.append(f"        seat: {ron_str(b['seat'])},\n        note: {ron_str(caps(b['note']))},")
        # (The game's tree, for now: the topics, flat.)
        out.append("        branches: [")
        for t in sorted({t for s in standards if s["body"] == b["key"] for t in (s.get("topics") or ["all"])}):
            out.append(f"            ({ron_str(t)}, {ron_str(caps(t.replace('-', ' ')))}),")
        out.append("        ],\n    ),")
    out.append("]\n")
    with open(os.path.join(CONTENT, "bodies.ron"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    out = [head, "["]
    for s in sorted(standards, key=lambda s: (s["body"], s["number"])):
        out.append("    (")
        out.append(f"        key: {ron_str(s['id'])},\n        body: {ron_str(s['body'])},\n        branch: {ron_str((s.get('topics') or ['all'])[0])},")
        out.append(f"        version: {s.get('version', 1)},\n        title: {ron_str(caps(s.get('title', '')))},")
        out.append(f"        scope: {ron_str(caps(s.get('scope', '')))},\n        status: {enum(s.get('status', 'draft'))},")
        out.append("        refs: [" + ", ".join(ron_str(r) for r in s.get("refs", []) or []) + "],")
        out.append("        params: [")
        for p in s.get("params", []) or []:
            out.append(f"            (key: {ron_str(p['key'])}, value: {ron_value(p['value'])}, unit: {ron_str(p.get('unit', ''))}, note: {ron_str(caps(p.get('note', '')))}),")
        out.append("        ],\n        requires: [")
        for r in s.get("requires", []) or []:
            out.append(f"            (subject: {ron_str(r['subject'])}, check: {enum(r['check'])}, param: {ron_str(r['param'])}, per: {ron_str(r.get('per', ''))}),")
        out.append("        ],")
        lic = s.get("licence", "open")
        out.append(f"        text: {ron_str(caps(s.get('text', '')))},\n        licence: {'Open' if lic == 'open' else f'Fee({float(lic["fee"])!r})'},")
        out.append(f"        published: {float(s.get('published', 0))!r},\n    ),")
    out.append("]\n")
    with open(os.path.join(CONTENT, "standards.ron"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))


# ---------------------------------------------------------------- the page
def write_html():
    data = {
        "bodies": [{k: b[k] for k in ("key", "name", "prefix", "seat", "note", "kind", "purpose", "details", "founded_by", "about") if k in b} for b in bodies],
        "brands": BRANDS,
        "house": house,
        "land": land,
        "local": local,
        "administrations": administrations,
        "parcels": parcels,
        # (Logos: MakerHouse/logos/<a maker's file name>.svg, drawn inline.)
        "logos": {f[:-4]: open(os.path.join(TREE, HOUSE, "logos", f), encoding="utf-8").read().strip() for f in sorted(os.listdir(os.path.join(TREE, HOUSE, "logos"))) if f.endswith(".svg")} if os.path.isdir(os.path.join(TREE, HOUSE, "logos")) else {},
        "makers": makers,
        "elements": elements,
        "materials": materials,
        "processes": processes,
        # (Icons: SFO/icons/<a record's file name>.svg, drawn inline so they take the page's colour.)
        "icons": {f[:-4]: open(os.path.join(TREE, "SFO", "icons", f), encoding="utf-8").read().strip() for f in sorted(os.listdir(os.path.join(TREE, "SFO", "icons"))) if f.endswith(".svg")} if os.path.isdir(os.path.join(TREE, "SFO", "icons")) else {},
        "process_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["processes"]["properties"].items()},
        # (Each property's unit or note, from the schemas.)
        "element_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["elements"]["properties"].items()},
        "material_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["materials"]["properties"].items()},
        "standards": sorted(standards, key=lambda s: (s["body"], s["number"])),
        "cited": cited,
        "problems": problems,
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
write_ron()
print(f"{len(makers)} makers, {len(bodies)} bodies, {len(standards)} standards, {len(elements)} elements, {len(materials)} materials, {len(processes)} processes")
print(f"  standards/index.html\n  content/base/bodies.ron, content/base/standards.ron, content/base/brands.ron")
