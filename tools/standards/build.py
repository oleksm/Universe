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
import math
import os
import re
import sys

import yaml

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
        if k not in {"key", "name", "ticker", "business", "address", "note", "who", "what", "story", "slug", "file"}:
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
ZONE_USES = ["port", "industrial", "commercial", "civic", "residential"]
# What zone each kind of facility needs.
FACILITY_ZONE = {"foundry": "industrial", "mill": "industrial", "power": "industrial", "warehouse": "port"}
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
GOODS_KINDS = {k: {"name": n.title(), "density": float(d)} for k, n, d in re.findall(r'key: "(goods\.[a-z_]+)",\s*name: "([^"]*)",.*?bulk_density: ([0-9.]+)', open(os.path.join(ROOT, "content", "base", "goods.ron"), encoding="utf-8").read(), re.S)}


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
            if k not in {"name", "kind", "at", "position", "about", "story", "zones", "parcels", "facilities", "streets", "power_lines"}:
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
            for k in fc:
                if k not in {"name", "kind", "parcel", "processes", "parts", "pipelines", "lines", "modules", "exchange"}:
                    problem(ffull, f"unknown field '{k}'")
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
                    for k in pt:
                        if k not in {"name", "kind", "does", "processes", "outline"}:
                            problem(ffull, f"part {nm}: unknown field '{k}'")
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
    for k in ["name"]:
        if k not in ad:
            problem(full, f"no {k}")
    for k in ad:
        if k not in {"name", "bodies", "address", "about", "story", "zoning"}:
            problem(full, f"unknown field '{k}'")
    names = [x.get("name") for x in ad.get("bodies") or []]
    for x in ad.get("bodies") or []:
        if x.get("kind") not in ("planet", "moon", "settlement"):
            problem(full, f"body {x.get('name')}: kind one of planet, moon, settlement")
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
    if body is None:
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
    if not os.path.isdir(folder) or name in ("schema", HOUSE, LOCAL):
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
KINDS = {"elements": "element", "materials": "material", "processes": "process", "modules": "module", "goods": "good", "hulls": "hull", "mill-stock": "mill-stock"}
# (Parts are filed in folders of their own: read further down.)
NESTED = {"parts"}
SCHEMAS = {k: yaml.safe_load(open(os.path.join(TREE, "SFO", "schema", f"{v}.schema.yaml"), encoding="utf-8")) for k, v in KINDS.items()}
elements, materials, processes, modules, goods, hulls, mill_stock = [], [], [], [], [], [], []
for s in standards:
    if "records" not in s:
        continue
    kind = str(s["records"])
    prefix = s["id"].split(" ")[0]
    folder = os.path.join(TREE, prefix, "metadata", kind)
    if kind in NESTED:
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
        elif kind == "hulls":
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
        {"elements": elements, "materials": materials, "processes": processes, "modules": modules, "goods": goods, "hulls": hulls, "mill-stock": mill_stock}[kind].append(e)
elements.sort(key=lambda e: (e.get("identity") or {}).get("atomic_number", 0))
materials.sort(key=lambda e: (e.get("identity") or {}).get("name", ""))
# (A process's inputs and outputs name elements by symbol, materials by file name.)
# (A facility's processes: SFO processes, each one for its kind of facility.)
by_process = {pr.get("slug"): pr for pr in processes}
for ad in administrations:
    for x in ad["bodies"]:
        for fc in x.get("facilities", []):
            for name in fc.get("processes") or []:
                pr = by_process.get(name)
                if pr is None:
                    problem(os.path.join(TREE, fc["file"]), f"processes: no process '{name}' in the SFO")
                elif (pr.get("equipment") or {}).get("facility") != fc.get("kind"):
                    problem(os.path.join(TREE, fc["file"]), f"processes: '{name}' is run in a {(pr.get('equipment') or {}).get('facility')}, not a {fc.get('kind')}")
symbols = {(e.get("identity") or {}).get("symbol") for e in elements}
slugs = {m.get("slug") for m in materials} | {g.get("slug") for g in goods}
# (A process's steps: each in an industrial module.)
by_module = {m.get("slug") for m in modules}
for pr in processes:
    for st in (pr.get("equipment") or {}).get("steps") or []:
        if st.get("module") not in by_module:
            problem(os.path.join(TREE, pr["file"]), f"equipment.steps: no module '{st.get('module')}' in the SFO")
# What a facility is to make: from each process's line of modules, how many of each it needs, what
# they take in and give off, their power and the ground they cover.
mod_of = {m.get("slug"): m for m in modules}
el_name = {(e.get("identity") or {}).get("symbol"): (e.get("identity") or {}).get("name") for e in elements}


def plan(pr, output):
    steps = []
    for st in (pr.get("equipment") or {}).get("steps") or []:
        if st.get("module") in mod_of and st["module"] not in steps:
            steps.append(st["module"])
    making = [s for s in steps if (mod_of[s].get("rate") or {}).get("throughput")]
    made = {(mod_of[s].get("rate") or {}).get("product"): s for s in making}
    demand = {s: 0.0 for s in steps}
    if making:
        demand[making[-1]] = float(output)
    supplies, by = {}, {}
    for s in reversed(steps):
        m, d = mod_of[s], demand[s]
        for x in (m.get("inputs") or {}).get("materials") or []:
            src = made.get(x.get("item"))
            if src and src != s and steps.index(src) < steps.index(s):
                demand[src] += d * x.get("amount", 0)
            else:
                supplies[x.get("item")] = supplies.get(x.get("item"), 0) + d * x.get("amount", 0)
        for x in (m.get("outputs") or {}).get("by_products") or []:
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
        through = rate.get("throughput")
        count = max(1, -(-demand[s] // through)) if through else 1
        rows.append({
            "module": s, "count": int(count), "demand": demand[s] if through else None,
            "use": demand[s] / (count * through) if through else None,
            "area": count * size.get("length", 0) * size.get("width", 0),
            "power": ((m.get("needs") or {}).get("power", 0)) * (demand[s] / through if through else 1),
        })
    product = (mod_of[making[-1]].get("rate") or {}).get("product", "") if making else ""
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
                pr = by_process.get(ln.get("process"))
                if set(ln) != {"process", "modules"}:
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
                # (The most the line can make: the least any of its modules lets through.)
                limits = [(has[r["module"]] * mod_of[r["module"]]["rate"]["throughput"] / r["demand"], r["module"]) for r in unit["modules"] if r["demand"] and r["module"] in has]
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
                        "can": n * rate["throughput"] if rate.get("throughput") else None,
                        "holds": n * rate["holds"] if rate.get("holds") else None,
                        "at_full": r["demand"], "use": r["demand"] / (n * rate["throughput"]) if rate.get("throughput") and n else None,
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
PART_SCHEMA = yaml.safe_load(open(os.path.join(TREE, "SFO", "schema", "part.schema.yaml"), encoding="utf-8"))
parts = []


def read_part(full, hull, parent, under):
    pt = load(full)
    ident = pt.get("identity") or {}
    code = str(ident.get("code", ""))
    if os.path.basename(full) != code + ".yaml":
        problem(full, "a part's file is named <its code>.yaml")
    if parent is None and not re.fullmatch(r"[A-Z0-9]+-[0-9]{2}", code):
        problem(full, "identity.code: the hull's code, a dash, two digits (MC07-04)")
    if parent is not None and not re.fullmatch(re.escape(parent) + r"-[0-9]{3}", code):
        problem(full, f"identity.code: {parent}, a dash, three digits")
    if not ident.get("name"):
        problem(full, "identity: no name")
    if any(o["slug"] == code for o in parts):
        problem(full, f"code {code} twice")
    for group, props in pt.items():
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
        if hull not in hull_of:
            problem(hdir, f"no hull '{hull}' in the SFO")
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
    proc = (ms.get("making") or {}).get("process")
    if proc is not None and proc not in by_process:
        problem(where, f"making.process: no process '{proc}' in the SFO")
    density = (mat.get("mass") or {}).get("density")
    t, d, w = size.get("thickness"), size.get("diameter"), size.get("wall")
    if density and t and not d:
        ms["unit"], ms["weight"] = "m2", density * t / 1000
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
# (A part made of parts: its mass is theirs, each as many times as it has them.)
for c in parts:
    mine = [pt for pt in parts if pt.get("parent") == c["slug"]]
    known = [pt for pt in mine if (pt.get("physical") or {}).get("mass") is not None]
    if mine:
        c["parts_mass"] = sum(pt["physical"]["mass"] * (pt.get("fit") or {}).get("count", 1) for pt in known)
        c["parts_weighed"] = [len(known), len(mine)]
for pt in parts:
    where = os.path.join(TREE, pt["file"])
    mf, ident = pt.get("made_from") or {}, pt.get("identity") or {}
    ms = stock_of.get(mf.get("item"))
    if "item" in mf and ms is None:
        problem(where, f"made_from.item: no mill stock '{mf['item']}'")
    elif ms is not None and "quantity" in mf:
        pt["stock_mass"] = mf["quantity"] * ms["weight"]
        pt["stock_unit"] = ms["unit"]
    for name in (pt.get("making") or {}).get("processes") or []:
        if name not in by_process:
            problem(where, f"making.processes: no process '{name}' in the SFO")
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


def write_ron():
    head = "// GENERATED by tools/standards/build.py from standards/ — edit the YAML there, then rerun it.\n"
    with open(os.path.join(CONTENT, "brands.ron"), "w", encoding="utf-8") as f:
        f.write(head + "// Makers of modules. Each has a home (a settled system, picked from the\n// galaxy's seed) where all its range is sold; farther off, less of it is\n// carried and it costs more (shipping). See docs/content.md.\n[\n")
        for m in makers:
            if m.get("business", "maker") != "maker":
                continue
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
    # Settlements' ground, for the game to stand on it: each settlement with any, by its system,
    # body and name (the game's own spaceport, whose pad grid centre is its position).
    pts = lambda ps: "[" + ", ".join(f"({float(p[0])!r}, {float(p[1])!r})" for p in ps or []) + "]"
    out = [head + "// Settlements' ground from Local Administration: zones, parcels, streets, power lines and\n"
           "// facilities (their modules laid out on their parcels). All in metres [east, north] of the\n"
           "// settlement's position: its spaceport's pad grid centre.\n["]
    for ad in administrations:
        for x in ad["bodies"]:
            if x.get("kind") != "settlement" or not any(x.get(k) for k in ("zones", "parcels", "streets", "power_lines", "facilities")):
                continue
            out.append("    (")
            out.append(f"        system: {ron_str(ad['name'])},\n        body: {ron_str(x.get('at', ''))},\n        name: {ron_str(x['name'])},")
            out.append("        zones: [")
            for zn in x.get("zones", []):
                out.append(f"            (name: {ron_str(zn['name'])}, use: {ron_str(zn['use'])}, outline: {pts(zn['outline'])}),")
            out.append("        ],\n        parcels: [")
            for pc in x.get("parcels", []):
                # (No owner: vacant, the land office's to sell.)
                owner = next((m["name"] for m in makers if m["key"] == pc.get("owner")), pc.get("owner", ""))
                out.append(f"            (number: {pc['number']}, owner: {ron_str(pc.get('owner', ''))}, owner_name: {ron_str(owner)}, outline: {pts(pc['outline'])}),")
            out.append("        ],\n        streets: [")
            for st in x.get("streets", []):
                out.append(f"            (name: {ron_str(st['name'])}, line: {pts(st['line'])}),")
            out.append("        ],\n        power_lines: [")
            for pw in x.get("power_lines", []):
                out.append(f"            (name: {ron_str(pw['name'])}, capacity: {float(pw['capacity'])!r}, line: {pts(pw['line'])}),")
            out.append("        ],\n        facilities: [")
            for fc in x.get("facilities", []):
                # (The most it can do, as worked out above: what each line makes an hour, the power
                # it draws flat out, the power it can supply, what it can hold.)
                makes = [(ln["most"]["product"], ln["most"]["output"]) for ln in fc.get("lines") or [] if ln.get("most")]
                draws = sum(ln["most"]["power"] for ln in fc.get("lines") or [] if ln.get("most"))
                holds = sum(st.get("holds") or 0 for st in fc.get("store") or [])
                name_of = lambda slug: next((r["identity"]["name"] for r in materials + goods if r.get("slug") == slug), slug)
                out.append(f"            (name: {ron_str(fc['name'])}, kind: {ron_str(fc['kind'])}, parcel: {fc['parcel']},")
                out.append("                makes: [" + ", ".join(f"({ron_str(name_of(p))}, {float(o)!r})" for p, o in makes) + f"], draws: {float(draws)!r}, supplies: {float(fc.get('capacity') or 0)!r}, holds: {float(holds)!r},")
                listed = [(r["module"], r["count"]) for ln in fc.get("lines") or [] for r in (ln.get("most") or {}).get("modules", []) if r["count"]] + [(im["module"], im["count"]) for im in fc.get("modules") or []]
                out.append("                modules: [" + ", ".join(f"({ron_str(m)}, {n})" for m, n in listed) + "],")
                out.append("                blocks: [")
                for bl in fc.get("layout", []):
                    out.append(f"                (module: {ron_str(bl['module'])}, centre: ({float(bl['centre'][0])!r}, {float(bl['centre'][1])!r}), length: {float(bl['length'])!r}, width: {float(bl['width'])!r}, height: {float(bl['height'])!r}, heading: {float(bl['heading'])!r}),")
                out.append("            ]),")
            out.append("        ],\n    ),")
    out.append("]\n")
    with open(os.path.join(CONTENT, "settlements.ron"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    # The industrial modules (SFO 10), what the game builds facilities of: each one's size, the
    # power it needs and supplies (MW), and what it holds (t).
    out = [head + "// Industrial modules from the SFO (SFO 10): name, size (m), power needed and supplied (MW), holds (t).\n["]
    for m in sorted(modules, key=lambda m: m["slug"]):
        size, rate = m.get("size") or {}, m.get("rate") or {}
        out.append(f"    (key: {ron_str(m['slug'])}, name: {ron_str(m['identity']['name'])}, length: {float(size.get('length', 0))!r}, width: {float(size.get('width', 0))!r}, height: {float(size.get('height', 0))!r}, needs: {float((m.get('needs') or {}).get('power') or 0)!r}, supplies: {float(rate.get('power') or 0)!r}, holds: {float(rate.get('holds') or 0)!r}),")
    out.append("]\n")
    with open(os.path.join(CONTENT, "industry.ron"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))


# ---------------------------------------------------------------- the page
def write_html():
    data = {
        "bodies": [{k: b[k] for k in ("key", "name", "prefix", "seat", "address", "note", "kind", "purpose", "details", "founded_by", "about") if k in b} for b in bodies],
        "brands": BRANDS,
        "house": house,
        "local": local,
        "port": PORT,
        "goods_kinds": GOODS_KINDS,
        "administrations": administrations,
        # (Logos: MakerHouse/logos/<a maker's file name>.svg, drawn inline.)
        "logos": {f[:-4]: open(os.path.join(TREE, HOUSE, "logos", f), encoding="utf-8").read().strip() for f in sorted(os.listdir(os.path.join(TREE, HOUSE, "logos"))) if f.endswith(".svg")} if os.path.isdir(os.path.join(TREE, HOUSE, "logos")) else {},
        "makers": makers,
        "elements": elements,
        "materials": materials,
        "processes": processes,
        "modules": modules,
        "goods": goods,
        "hulls": hulls,
        "mill_stock": mill_stock,
        "mill_stock_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["mill-stock"]["properties"].items()},
        "parts": parts,
        "part_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in PART_SCHEMA["properties"].items()},
        "hull_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["hulls"]["properties"].items()},
        "good_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["goods"]["properties"].items()},
        "module_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items()} for g, d in SCHEMAS["modules"]["properties"].items()},
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
print(f"{len(makers)} makers, {len(bodies)} bodies, {len(standards)} standards, {len(elements)} elements, {len(materials)} materials, {len(processes)} processes, {len(modules)} modules, {len(goods)} goods, {len(hulls)} hulls")
print(f"  standards/index.html\n  content/base/bodies.ron, content/base/standards.ron, content/base/brands.ron, content/base/settlements.ron, content/base/industry.ron")
