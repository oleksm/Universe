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
FACILITY_ZONE = {"foundry": "industrial", "mill": "industrial", "yard": "industrial", "power": "industrial", "warehouse": "port"}
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
ORES = set(re.findall(r'key: "(ore\.[a-z_]+)"', open(os.path.join(ROOT, "content", "base", "ores.ron"), encoding="utf-8").read()))
GOODS_KINDS = {k: {"name": n.title(), "density": float(d)} for k, n, d in re.findall(r'key: "(goods\.[a-z_]+)",\s*name: "([^"]*)",.*?bulk_density: ([0-9.]+)', open(os.path.join(ROOT, "content", "base", "goods.ron"), encoding="utf-8").read(), re.S)}


TIERS = ["sourced", "derived", "invented"]


def check_basis(rec, where):
    """A record's `basis`: entries of what they cover, a tier, and for a sourced one its source."""
    bs = rec.get("basis")
    if bs is None:
        return
    if not isinstance(bs, list):
        problem(where, "basis: a list of entries (of, tier, source, note)")
        return
    for en in bs:
        if not isinstance(en, dict) or not isinstance(en.get("of"), list) or en.get("tier") not in TIERS:
            problem(where, f"basis: each entry names what it is of and a tier ({', '.join(TIERS)})")
        elif set(en) - {"of", "tier", "source", "note", "review"}:
            problem(where, "basis: unknown field")
        elif en["tier"] == "sourced" and not en.get("source"):
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
            if k not in {"name", "kind", "at", "gravity", "position", "about", "story", "zones", "parcels", "facilities", "streets", "power_lines", "gate"} | ({"owner", "processes", "lines", "modules", "spin"} if x.get("kind") == "rig" else set()):
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
    if not os.path.isdir(folder) or name in ("schema", "sources", HOUSE, LOCAL, "Celestial"):
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
KINDS = {"elements": "element", "materials": "material", "processes": "process", "modules": "module", "goods": "good", "hulls": "hull", "mill-stock": "mill-stock", "equipment": "equipment", "gates": "gate", "layouts": "layout"}
# (Parts are filed in folders of their own: read further down.)
NESTED = {"parts"}
SCHEMAS = {k: yaml.safe_load(open(os.path.join(TREE, "SFO", "schema", f"{v}.schema.yaml"), encoding="utf-8")) for k, v in KINDS.items()}
elements, materials, processes, modules, goods, hulls, mill_stock, equipment, gates, layouts = [], [], [], [], [], [], [], [], [], []
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
        elif kind in ("equipment", "gates"):
            for k in ("name",) if kind == "equipment" else ("name", "key"):
                if not ident.get(k):
                    problem(full, f"identity: no {k}")
            if ident.get("maker") is not None and ident["maker"] not in BRANDS:
                problem(full, f"identity.maker: no maker '{ident['maker']}' in Maker House")
            key = ident.get("name")
            e["slug"] = name[:-5]
        elif kind == "layouts":
            if not ident.get("hull"):
                problem(full, "identity: no hull")
            key = ident.get("hull")
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
        check_basis(e, full)
        for group, props in e.items():
            if group in ("slug", "basis"):
                continue
            if kind in ("hulls", "gates") and group == "fit":
                continue
            # (A layout's lists are checked further down, with its hull.)
            if kind == "layouts" and group in ("decks", "compartments", "openings"):
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
            for k in props or {}:
                if k not in known["properties"]:
                    problem(full, f"{group}: unknown property '{k}'")
        e["under"] = s["id"]
        e["file"] = os.path.relpath(full, TREE)
        {"elements": elements, "materials": materials, "processes": processes, "modules": modules, "goods": goods, "hulls": hulls, "mill-stock": mill_stock, "equipment": equipment, "gates": gates, "layouts": layouts}[kind].append(e)
elements.sort(key=lambda e: (e.get("identity") or {}).get("atomic_number", 0))
materials.sort(key=lambda e: (e.get("identity") or {}).get("name", ""))
# (A process's inputs and outputs name elements by symbol, materials by file name.)
# (A facility's processes: SFO processes, each one for its kind of facility.)
by_process = {pr.get("slug"): pr for pr in processes}
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
for ad in administrations:
    for x in ad["bodies"]:
        for fc in x.get("facilities", []):
            for name in fc.get("processes") or []:
                pr = by_process.get(name)
                if pr is None:
                    problem(os.path.join(TREE, fc["file"]), f"processes: no process '{name}' in the SFO")
                elif (pr.get("equipment") or {}).get("facility") != fc.get("kind") and not fc.get("rig"):
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
        problem(full, "identity.code: the hull's (or the structure's) code, a dash, two digits (MC07-04)")
    if parent is not None and not re.fullmatch(re.escape(parent) + r"-[0-9]{3}", code):
        problem(full, f"identity.code: {parent}, a dash, three digits")
    if not ident.get("name"):
        problem(full, "identity: no name")
    if any(o["slug"] == code for o in parts):
        problem(full, f"code {code} twice")
    check_basis(pt, full)
    for group, props in pt.items():
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
        if hull not in hull_of and hull not in {(g.get("built_of") or {}).get("parts") for g in gates} and hull not in {m.get("slug") for m in modules}:
            problem(hdir, f"no hull or industrial module '{hull}' in the SFO, and no gate built of it")
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
                name_of = lambda slug: next((r["identity"]["name"] for r in materials + goods + elements if r.get("slug") == slug or (r.get("identity") or {}).get("symbol") == slug), slug)
                out.append(f"            (name: {ron_str(fc['name'])}, kind: {ron_str(fc['kind'])}, parcel: {fc['parcel']},")
                out.append("                makes: [" + ", ".join(f"({ron_str(name_of(p))}, {float(o)!r})" for p, o in makes) + f"], draws: {float(draws)!r}, supplies: {float(fc.get('capacity') or 0)!r}, holds: {float(holds)!r},")
                listed = [(r["module"], r["count"]) for ln in fc.get("lines") or [] for r in (ln.get("most") or {}).get("modules", []) if r["count"]] + [(im["module"], im["count"]) for im in fc.get("modules") or []]
                # (Flat out, an hour: what it takes in, gives off and burns, each with the game's kind of
                # goods it is, if it has one yet: none, and the game doesn't trade it.)
                def flow(slug, rate):
                    rec = next((r for r in goods + materials if r.get("slug") == slug), None)
                    kind = ((rec or {}).get("game") or {}).get("goods", "")
                    return f"({ron_str(name_of(slug))}, {ron_str(kind)}, {float(rate)!r})"
                takes = [flow(i["item"], i["rate"]) for ln in fc.get("lines") or [] if ln.get("most") for i in ln["most"]["supplies"]]
                gives = [flow(ln["most"]["product"], ln["most"]["output"]) for ln in fc.get("lines") or [] if ln.get("most")]
                gives += [flow(i["item"], i["rate"]) for ln in fc.get("lines") or [] if ln.get("most") for i in ln["most"]["by_products"]]
                burns = [flow(i["item"], i["rate"]) for i in fc.get("burns") or []]
                out.append("                takes: [" + ", ".join(takes) + "], gives: [" + ", ".join(gives) + "], burns: [" + ", ".join(burns) + "],")
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
    # Ship layouts (SFO 18), what the game builds hulls' insides from: compartments (boxes in
    # metres back from the nose, above the keel, from the centre line) and the openings between them.
    out = [head + "// Ship layouts from the SFO (SFO 18): each hull's compartments and openings. Boxes: (aft from, aft to, up from, up to, side from, side to), m.\n["]
    for lay in sorted(layouts, key=lambda x: x["slug"]):
        out.append(f"    (hull: {ron_str(lay['identity']['hull'])}, compartments: [")
        for c in lay.get("compartments") or []:
            ad = c["address"]
            bxs = ", ".join("(" + ", ".join(repr(float(v)) for v in bx) + ")" for bx in c["boxes"])
            out.append(f"        (name: {ron_str(c['name'])}, function: {ron_str(c['function'])}, deck: {int(ad['deck'])}, section: {ron_str(ad['section'])}, unit: {int(ad['unit'])}, pressurised: {'true' if c.get('pressurised') else 'false'}, boxes: [{bxs}]),")
        out.append("    ], openings: [")
        for o in lay.get("openings") or []:
            a, b = o["between"]
            at = ", ".join(repr(float(v)) for v in o["at"])
            out.append(f"        (kind: {ron_str(o['kind'])}, a: {ron_str(a)}, b: {ron_str(b)}, at: ({at}), width: {float(o['width'])!r}, height: {float(o['height'])!r}, seals: {'true' if o.get('seals') else 'false'}),")
        out.append("    ]),")
    out.append("]\n")
    with open(os.path.join(CONTENT, "layouts.ron"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))


# ---------------------------------------------------------------- reports
# Where the registry does not yet join up. Nothing here stops the build: these are gaps to close,
# not mistakes. Each report is a table; a row is ok, a gap, or a note.
reports = []


def report(key, title, about, columns, rows):
    reports.append({"key": key, "title": title, "about": about, "columns": columns, "rows": rows,
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
ingot_makers = lambda mat: [q for q in processes if any(o.get("item") == mat and o.get("form") == "ingot" for o in (q.get("outputs") or {}).get("products") or [])]
# 1. The chain from a hull down to rock: how far each part gets.
eq_of = {e["slug"]: e for e in equipment}
for hl in hulls + structures:
    mine = [pt for pt in parts if pt["hull"] == hl["slug"]]
    leaves = [pt for pt in mine if not kids(pt)]
    rows = []
    how = lambda pt: (pt.get("making") or {}).get("processes") or []
    # (The hull itself, and each part made of parts: is it said how it is put together, and is a yard built to do it?)
    for name, key, procs in [(hl["identity"]["name"] + " (hull)", link(hl["identity"]["name"] + ("" if "key" in hl else " (hull)"), hl.get("key", "hull:" + hl["slug"])), [q for q in [(hl.get("making") or {}).get("process")] if q])] + [(None, part_link(pt), how(pt)) for pt in mine if kids(pt)]:
        steps = [("says how it is put together", bool(procs) and all(q in by_process for q in procs)), ("somewhere is built to do it", bool(procs) and all(q in lined for q in procs))]
        if name and hl.get("fit"):
            out = (hl.get("making") or {}).get("fitting_out")
            steps += [("says how it is fitted out", out in by_process), ("a yard is built to fit it out", out in lined)]
        reached = next((i for i, (_, good) in enumerate(steps) if not good), len(steps))
        rows.append(row("ok" if reached == len(steps) else "gap", key, f"{reached} of {len(steps)}", "complete" if reached == len(steps) else "stops at: " + steps[reached][0]))
    for pt in leaves:
        ms = stock_of.get((pt.get("made_from") or {}).get("item"))
        proc = ((ms or {}).get("making") or {}).get("process")
        pr = by_process.get(proc)
        steps = [
            ("has a mass", "mass" in pt),
            ("says what it is cut from", ms is not None),
            ("that stock has a process", pr is not None),
            ("the process has steps", bool(pr and (pr.get("equipment") or {}).get("steps"))),
            ("a facility is built to run it", proc in lined),
            ("the ingot that stock is made from can be made", any(q["slug"] in lined for q in ingot_makers((ms or {}).get("made_from", {}).get("material")))),
            ("says how it is made from its stock", bool(how(pt)) and all(q in by_process for q in how(pt))),
            ("a yard is built to make it", bool(how(pt)) and all(q in lined for q in how(pt))),
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
                    if "most" in ln and ln["process"] == (hl.get("making") or {}).get("process") and ln["most"]["output"] and hl.get("parts_mass"):
                        # (Fitted out on the same line, where it can be: its equipment goes through the dock too.)
                        fits = (hl.get("making") or {}).get("fitting_out") in (ln.get("also") or [])
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
            x["spin_worked"] = {"rate": w_, "rpm": w_ * 60 / (2 * math.pi), "g": sp["gravity"] / 9.81}
        cargo_ = max([(e_.get("performance") or {}).get("capacity", 0) for e_ in equipment if (e_.get("identity") or {}).get("slot") == "cargo"] or [0])
        x["feed"] = [{"item": i["item"], "rate": i["rate"], "loads": i["rate"] * 24 * 1000 / cargo_ if cargo_ else None, "hold": cargo_} for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln for i in ln["most"]["supplies"] if next((g_ for g_ in goods if g_["slug"] == i["item"]), {}).get("identity", {}).get("kind") == "rock"]

# 1b. Equipment: what each hull is fitted with, each against the game's module, and whether it is
# said what it is made of.
eq_by = {e["slug"]: e for e in equipment}
_mods = open(os.path.join(ROOT, "content", "base", "modules.ron"), encoding="utf-8").read()
GAME_MODULES = {k: float(m) for k, m in re.findall(r'\(key: "([^"]+)",.*?mass: ([0-9.e+]+)', _mods)}
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
    mass, game = (e.get("physical") or {}).get("mass"), GAME_MODULES.get(e["identity"].get("key"))
    on = [hl["identity"]["name"] for hl in hulls + gates if any(ft.get("item") == e["slug"] for ft in hl.get("fit") or [])]
    same = game is not None and mass is not None and abs(game - mass) < 0.5
    rows.append(row("gap", link(e["identity"]["name"], "eq:" + e["slug"]), ", ".join(on) or "no hull", tonnes(mass) if mass is not None else "", ("the same in the game" if same else f"the game says {tonnes(game)}") if game is not None else "not in the game", "not yet said"))
report("equipment", "Equipment: what hulls are fitted with", "Each piece of ship equipment: the hulls fitted with it, its mass against the game's module of the same key, and whether it is said what it is made of. A gap is one that differs from the game, or does not yet say what it is made of.", ["Equipment", "Fitted to", "Mass", "Against the game", "Made of"], rows)

# 1b. Ship layouts (SFO 18): a hull's inside, divided. Checked: its hull exists; names, kinds and
# functions known; boxes the right way round; no two compartments overlapping; each opening on a
# face its two compartments share (to the outside: on one of its compartment's faces); a door a
# person fits through; every compartment reachable from outside. Worked out: each compartment's
# volume and floor area, and what's sealed. (Whether the boxes fit the hull as modelled is checked
# against the model: tools/standards/hulls/check_layout.py.)
LAYOUT_SCHEMA = SCHEMAS["layouts"]["properties"]
FUNCTIONS = set(LAYOUT_SCHEMA["compartments"]["items"]["properties"]["function"]["enum"])
OPENINGS = set(LAYOUT_SCHEMA["openings"]["items"]["properties"]["kind"]["enum"])
hull_slugs = {h["slug"] for h in hulls}
lay_rows = []
for lay in layouts:
    where = os.path.join(TREE, lay["file"])
    if lay["identity"].get("hull") not in hull_slugs:
        problem(where, f"identity.hull: no hull '{lay['identity'].get('hull')}' in the SFO")
    decks_n = {d.get("number") for d in lay.get("decks") or []}
    boxes_of = {}
    for c in lay.get("compartments") or []:
        nm = c.get("name")
        if nm in boxes_of or nm == "outside":
            problem(where, f"compartments: '{nm}' twice (or named outside)")
        if c.get("function") not in FUNCTIONS:
            problem(where, f"compartments: {nm}: no function '{c.get('function')}' (one of {', '.join(sorted(FUNCTIONS))})")
        if (c.get("address") or {}).get("deck") not in decks_n:
            problem(where, f"compartments: {nm}: no deck {(c.get('address') or {}).get('deck')}")
        bxs = []
        for bx in c.get("boxes") or []:
            if len(bx) != 6 or any(bx[2 * k] >= bx[2 * k + 1] for k in range(3)):
                problem(where, f"compartments: {nm}: a box is [aft from, aft to, up from, up to, side from, side to], each from below to: {bx}")
                continue
            bxs.append([(float(bx[2 * k]), float(bx[2 * k + 1])) for k in range(3)])
        boxes_of[nm] = bxs
        c["volume"] = round(sum((b[0][1] - b[0][0]) * (b[1][1] - b[1][0]) * (b[2][1] - b[2][0]) for b in bxs), 1)
        c["floor_area"] = round(sum((b[0][1] - b[0][0]) * (b[2][1] - b[2][0]) for b in bxs), 1)
    names_ = list(boxes_of)
    for ia, na in enumerate(names_):
        for nb in names_[ia + 1:]:
            if any(all(min(p[k][1], q[k][1]) - max(p[k][0], q[k][0]) > 0.01 for k in range(3)) for p in boxes_of[na] for q in boxes_of[nb]):
                problem(where, f"compartments: {na} and {nb} overlap")

    def faces(bx, at):
        return [(k, sd) for k in range(3) for sd in (0, 1) if abs(at[k] - bx[k][sd]) < 0.01 and all(bx[m][0] - 0.01 <= at[m] <= bx[m][1] + 0.01 for m in range(3) if m != k)]

    links = {n: set() for n in names_ + ["outside"]}
    for o in lay.get("openings") or []:
        pair = o.get("between") or []
        label = " - ".join(map(str, pair))
        if o.get("kind") not in OPENINGS:
            problem(where, f"openings: {label}: no kind '{o.get('kind')}' (one of {', '.join(sorted(OPENINGS))})")
        if len(pair) != 2 or any(n not in links for n in pair):
            problem(where, f"openings: {label}: joins no compartment of that name")
            continue
        at = [float(v) for v in o.get("at") or []]
        a, b = pair
        if a != "outside" and b != "outside":
            fa = [f for bx in boxes_of[a] for f in faces(bx, at)]
            fb = [f for bx in boxes_of[b] for f in faces(bx, at)]
            if not any(x[0] == y[0] and x[1] != y[1] for x in fa for y in fb):
                problem(where, f"openings: {label} at {o.get('at')}: not on a face they share")
        elif not any(faces(bx, at) for bx in boxes_of[b if a == "outside" else a]):
            problem(where, f"openings: {label} at {o.get('at')}: not on a face of its compartment")
        if o.get("kind") in ("door", "pressure-door") and (o.get("width", 0) < 0.8 or o.get("height", 0) < 2.0):
            problem(where, f"openings: {label}: {o.get('width')} x {o.get('height')} m is smaller than a person's door (0.8 x 2.0)")
        links[a].add(b)
        links[b].add(a)
    seen_, todo = {"outside"}, ["outside"]
    while todo:
        for n in links[todo.pop()]:
            if n not in seen_:
                seen_.add(n)
                todo.append(n)
    cs = lay.get("compartments") or []
    unreached = [c["name"] for c in cs if c["name"] not in seen_]
    lay["worked"] = {"volume": round(sum(c["volume"] for c in cs), 1), "sealed": round(sum(c["volume"] for c in cs if c.get("pressurised")), 1), "floor_area": round(sum(c["floor_area"] for c in cs), 1), "unreached": unreached}
    lay_rows.append(row("gap" if unreached else "ok", link(lay["identity"]["hull"].upper(), "lay:" + lay["slug"]), len(cs), len(lay.get("openings") or []), f"{lay['worked']['volume']:,.0f} m³", f"{lay['worked']['sealed']:,.0f} m³", f"{lay['worked']['floor_area']:,.0f} m²", ", ".join(unreached)))
report("layouts", "Layouts: hulls' insides", "Each hull's layout: its compartments and openings, its space, what of it is sealed, and its floor. A gap is a compartment there's no way into from outside.", ["Hull", "Compartments", "Openings", "Space", "Sealed", "Floor", "No way in"], lay_rows)

# 1c. Stargates: what opening and holding each ring's tube costs, by the laws (config/dogma.ron,
# Tube; the same formulas as crates/physics/src/hyper.rs), and each ring against the game's.
_dogma = open(os.path.join(ROOT, "config", "dogma.ron"), encoding="utf-8").read()
LAW = {k: float(v) for k, v in re.findall(r'name: "(TUBE_[A-Z_]+)", value: ([0-9.e+-]+)', _dogma)}
LY = 9.4607304725808e15
_structs = open(os.path.join(ROOT, "content", "base", "structures.ron"), encoding="utf-8").read()
GAME_RINGS = {k: float(v) for k, v in re.findall(r'key: "([^"]+)",[^\n]*?span_ly: ([0-9.]+)', _structs)}
tube_time = lambda m, span_ly: LAW["TUBE_T_LY"] * span_ly * m ** LAW["TUBE_GAMMA"]
tube_energy = lambda m, span_ly: LAW["TUBE_EPS"] * m * span_ly * LY * math.e      # (at its natural time)
station = next((m for m in modules if (m.get("rate") or {}).get("power")), None)
rows = []
for g in gates:
    d, span = (g.get("size") or {}).get("opening"), (g.get("performance") or {}).get("span")
    game = GAME_RINGS.get(g["identity"]["key"])
    if d and span and all(k in LAW for k in ("TUBE_T_LY", "TUBE_GAMMA", "TUBE_EPS", "TUBE_RHO", "TUBE_K", "TUBE_HOLD")):
        mu = LAW["TUBE_RHO"] * d ** LAW["TUBE_K"]
        opening = tube_energy(mu, span)
        ships = [("A ship of 100 t", 1e5)] + [(hl["identity"]["name"] + ", loaded", hl.get("parts_mass", 0) + max(0.0, (hl.get("design") or {}).get("loaded_mass", 0) * 1000 - ((hl.get("mass") or {}).get("frame") or 0) * 1000)) for hl in hulls if hl.get("parts_mass")] + [("A hauler of 1,000 t", 1e6), ("A capital ship of 100,000 t", 1e8)]
        g["worked"] = {
            "tube_mass": mu, "open_energy": opening, "open_time": tube_time(mu, span), "hold_power": opening / LAW["TUBE_HOLD"], "hold_months": LAW["TUBE_HOLD"] / (30 * 86400),
            "stations": (opening / LAW["TUBE_HOLD"] / 1e6 / station["rate"]["power"]) if station else None, "station": station["slug"] if station else None,
            "crossings": [{"what": w, "mass": m, "time": tube_time(m, span), "energy": tube_energy(m, span)} for w, m in ships],
        }
        hold = f"{g['worked']['hold_power'] / 1e9:,.0f} GW to hold at its full span"
    else:
        hold = "not worked out: its opening, its span or a law is missing"
    rows.append(row("ok" if game == span else "gap", link(g["identity"]["name"], "gate:" + g["slug"]), f"{span:g} ly" if span else "", (f"the same in the game" if game == span else f"the game says {game:g} ly") if game is not None else "not in the game", hold))
    st = next((x for x in structures if g["slug"] in x["gates"]), None)
    if st and "worked" in g:
        mine = [pt for pt in parts if pt["hull"] == st["slug"]]
        each = lambda pt: times(pt) * (times(next(o for o in mine if o["slug"] == pt["parent"])) if pt.get("parent") else 1)
        can = lambda proc: sum(ln["most"]["output"] for ad in administrations for x in ad["bodies"] for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln and proc in [ln["process"]] + (ln.get("also") or []))
        run_in = lambda proc: ", ".join(sorted({fc["name"] for ad in administrations for x in ad["bodies"] for fc in x.get("facilities", []) for ln in fc.get("lines") or [] if "most" in ln and proc in [ln["process"]] + (ln.get("also") or [])}))
        steps, stock_t, ingot_t, made_t = [], {}, {}, {}
        for pt in [pt for pt in mine if not kids(pt)]:
            ms = stock_of.get((pt.get("made_from") or {}).get("item"))
            if ms is None:
                continue
            stock_t[ms["slug"]] = stock_t.get(ms["slug"], 0) + pt.get("stock_mass", 0) * each(pt) / 1000
            for q in (pt.get("making") or {}).get("processes") or []:
                made_t[q] = made_t.get(q, 0) + pt.get("mass", 0) * each(pt) / 1000
        def step(what, tonnes_, procs):
            procs = [q for q in ([procs] if isinstance(procs, str) else procs or []) if q]
            rate = sum(can(q) for q in procs)
            steps.append({"what": what, "tonnes": tonnes_, "at": ", ".join(sorted({n_ for q in procs for n_ in run_in(q).split(", ") if n_})), "rate": rate, "days": tonnes_ / rate / 24 if rate else None})
        for q, t_ in made_t.items():
            step(f"{by_process[q]['identity']['name']}: its parts made from stock", t_, q)
        for code, t_ in stock_t.items():
            ms = stock_of[code]
            step(f"{ms['identity']['name']} rolled or drawn", t_, (ms.get("making") or {}).get("process"))
            mat = (ms.get("made_from") or {}).get("material")
            ingot_t[mat] = ingot_t.get(mat, 0) + t_
        for mat, t_ in ingot_t.items():
            casts = [q["slug"] for q in ingot_makers(mat) if q["slug"] in lined]
            step(f"{next(m_ for m_ in materials if m_.get('slug') == mat)['identity']['name']} cast as ingot (at least: the mills' own losses come on top)", t_, casts)
        cargo = max([(ft_.get("performance") or {}).get("capacity", 0) for ft_ in equipment if (ft_.get("identity") or {}).get("slot") == "cargo"] or [0])
        for c in [c for c in mine if kids(c)]:
            for q_ in (c.get("making") or {}).get("processes") or []:
                step(f"{by_process[q_]['identity']['name']}: its {c['identity']['name'].lower()}s put together", c.get("mass", 0) * each(c) / 1000, q_)
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
        SIGMA, EMISS = 5.670374419e-8, 0.9
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
            share = x["gate"]["distance"] / g["performance"]["span"]
            x["gate_worked"] = {"ring": g["identity"]["name"], "span": g["performance"]["span"], "hold_power": g["worked"]["hold_power"] * share, "open_energy": g["worked"]["open_energy"] * share,
                                "stations": math.ceil(g["worked"]["stations"] * share) if g["worked"].get("stations") else None,
                                "crossings": [{"what": c["what"], "mass": c["mass"], "time": c["time"] * share, "energy": c["energy"] * share} for c in g["worked"]["crossings"]]}
            rows.append(row("ok" if share <= 1 else "gap", link(x["name"], f"bd:{ad['slug']}:{x['slug']}"), f"{x['gate']['distance']:g} ly", f"a {g['identity']['name']}, which spans {g['performance']['span']:g} ly", f"{x['gate_worked']['hold_power'] / 1e9:.1f} GW to hold, {x['gate_worked']['stations']} power stations" if share <= 1 else "further than its ring spans"))
report("gates", "Stargates: each ring against the game, and what it costs to hold", "Each gate ring: its span against the game's ring of the same key, and the power its tube takes to hold, worked out from the laws. A gap is a ring that differs from the game, or something a ring does not yet say.", ["Ring", "Span", "Against the game", "Note"], rows)

# 1d. Plant: what each industrial module is built of.
rows = []
for m in modules:
    mine = [pt for pt in parts if pt["hull"] == m["slug"]]
    tops = [pt for pt in mine if not pt.get("parent")]
    leaves = [pt for pt in mine if not kids(pt)]
    cut = [pt for pt in leaves if (pt.get("made_from") or {}).get("item") in stock_of]
    size = m.get("size") or {}
    floor = size.get("length", 0) * size.get("width", 0)
    rows.append(row("gap" if not tops or len(cut) < len(leaves) else "ok", link(m["identity"]["name"], "mod:" + m["slug"]), len(tops) or "none listed", sum(times(pt) for pt in tops) or "", tonnes(m["parts_mass"]) if m.get("parts_mass") else "", f"{m['parts_mass'] / floor:,.0f} kg/m2" if m.get("parts_mass") and floor else "", f"{len(cut)} of {len(leaves)}" if leaves else ""))
report("plant", "Plant: what each industrial module is built of", "Each industrial module: the kinds of component it is built of, how many pieces that is, what they weigh together, that weight over its floor, and how many of its components say what they are made from. A gap is a module with no components listed, or with components that do not yet say what they are made from.", ["Module", "Kinds of component", "Pieces", "Weight", "Over its floor", "Say what they are made from"], rows)

# 2. Mass: what a thing weighs against what it is made of.
rows = []
for hl in hulls:
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
for hl in hulls:
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
    ship = hl.get("parts_mass", 0) + max(0.0, ds.get("loaded_mass", 0) * 1000 - frame)   # (its parts, and what the game fits and loads)
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
            rows.append(row("ok" if min(strength, buckling) >= 1 else "gap", part_link(pt), f"{force / 1e6:.2f} MN on each of {count} legs", f"{force * SF / 1e6:.2f} MN with its factor", ms["identity"]["name"], f"strength {strength:.2f} times, buckling {buckling:.2f} times", f"a column {longest(pt):.2f} m long, stopping {ship / 1000:.0f} t from {v:g} m/s in {stroke:.2f} m, on ground of {hover / 9.81:.2f} g"))
    # (What follows for everything aboard: the jolt of the designed landing, and the hardest landing
    # the legs take before one fails, with its jolt.)
    landing = {}
    if legs and ship and strokes:
        s_ = min(strokes)
        jolt = v ** 2 / (2 * s_ * eta) / 9.81
        spare = (most or 0) * count - ship * hover
        vmax = (2 * spare * s_ * eta / ship) ** 0.5 if spare > 0 else 0
        jmax = vmax ** 2 / (2 * s_ * eta) / 9.81
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
        rows.append(row("note", "The landing legs' share of the ship", f"{ship / 1000:,.0f} t loaded", "3 to 6% in transport aircraft, with wheels and brakes, for 1 g", f"{100 * gear / ship:.1f}% ({gear / 1000:.1f} t)", "", (bench("weight_fractions", "landing_gear_share_MTOW_VT").get("source") or "")))
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
                    (can if at["gravity"] <= hover else cannot).append(f"{x['name']} ({at['gravity'] / 9.81:.2f} g)")
        rows.append(row("note", "Where it can hover and set down", f"{ship / 1000:,.0f} t loaded", f"{ds['lift_thrust']:g} MN of lift", f"ground of up to {hover / 9.81:.2f} g", "", f"its lift nozzles hold it where gravity is no more than {hover:.2f} m/s2. Not every ship has to land on a planet."))
        if can or cannot:
            rows.append(row("note", "Settlements it can set down at", f"{len(can)} of {len(can) + len(cannot)}", "", ", ".join(can), "", ("Too heavy for: " + ", ".join(cannot)) if cannot else ""))
    if ds.get("main_thrust") and ship:
        rows.append(row("note", "Main drive", f"{ship / 1000:,.0f} t loaded", "", f"{ds['main_thrust']:g} MN", f"{ds['main_thrust'] * 1e6 / ship / 9.81:.1f} g", "its acceleration flat out; nothing checks the hull against it"))
    hl["worked"] = {"ship": ship, "hover": hover, "landing": landing, "can": can, "cannot": cannot}
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
        draw = {fc["slug"]: sum(ln["most"]["power"] for ln in fc.get("lines") or [] if "most" in ln) for fc in facs}
        supply = sum(fc.get("capacity") or 0 for fc in facs if fc.get("kind") in ("power", "rig"))
        if not supply and not any(draw.values()):
            continue
        total = sum(draw.values())
        rows.append(row("ok" if supply >= total else "gap", x["name"] + ", all of it", f"{total:,.0f} MW", f"{supply:,.0f} MW", "its own plant" if x.get("kind") == "rig" else "its power stations", "enough for everything flat out at once" if supply >= total else f"{total - supply:,.0f} MW short with everything flat out at once"))
        for fc in facs:
            if not draw[fc["slug"]] or fc.get("rig"):
                continue
            wires = [pw for pw in x.get("power_lines", []) if pw.get("to") == fc["slug"]]
            can = sum(pw["capacity"] for pw in wires)
            rows.append(row("ok" if can >= draw[fc["slug"]] else "gap", fc["name"], f"{draw[fc['slug']]:,.1f} MW", f"{can:,.0f} MW" if wires else "", ", ".join(pw["name"] for pw in wires) or "no line reaches it", "" if can >= draw[fc["slug"]] else ("its lines carry less than it draws" if wires else "")))
report("power", "Power: what is supplied against what is drawn", "In each settlement: what its power stations can supply against what its facilities draw with every line flat out, and for each facility the lines that reach it against what it draws. A gap is a shortfall, or a facility no line reaches.", ["What", "Draws flat out", "Can get", "From", "Note"], rows)

# 3. Volume: a hull's parts' boxes against the space the hull takes.
rows = []
for hl in hulls:
    vol = (hl.get("size") or {}).get("volume")
    boxes = sum((pt.get("physical") or {}).get("length", 0) * (pt.get("physical") or {}).get("width", 0) * (pt.get("physical") or {}).get("height", 0) * times(pt) for pt in parts if pt["hull"] == hl["slug"] and not pt.get("parent"))
    unsized = [pt for pt in parts if pt["hull"] == hl["slug"] and not pt.get("parent") and not (pt.get("physical") or {}).get("length")]
    if vol:
        rows.append(row("note", link(hl["identity"]["name"], "hull:" + hl["slug"]), f"{vol:,.0f} m3", f"{boxes:,.0f} m3", f"{boxes / vol:.1f} times", "The boxes are each part's outer extent and overlap where parts do, so they add up to more than the hull."))
    for pt in unsized:
        rows.append(row("gap", part_link(pt), "", "", "", "no size"))
report("volume", "Volume: a hull against its parts' boxes", "The space the hull's shape takes in the game, against the boxes of its parts added up. A gap is a part with no size.", ["What", "Hull's volume", "Parts' boxes", "Ratio", "Note"], rows)

# 4. What goes in against what comes out, for each industrial module.
rows = []
for m in modules:
    ins = sum(x.get("amount", 0) for x in (m.get("inputs") or {}).get("materials") or [])
    outs = sum(x.get("amount", 0) for x in (m.get("outputs") or {}).get("by_products") or [])
    if not ins or not (m.get("rate") or {}).get("throughput"):
        continue
    d = ins - (1 + outs)
    rows.append(row("ok" if abs(d) <= 0.02 * ins else "gap", link(m["identity"]["name"], "mod:" + m["slug"]), f"{ins:.4g} t", f"{1 + outs:.4g} t", f"{d:+.3g} t ({100 * d / ins:+.1f}%)", "balanced" if abs(d) <= 0.02 * ins else ("more goes in than comes out" if d > 0 else "more comes out than goes in")))
report("modules", "Balance: what goes into a module against what comes out", "For each tonne of a module's product: everything that goes in, against the product and everything else that comes out. Matter is not made or lost, so they should match. A gap is a difference of more than 2%.", ["Module", "Goes in", "Comes out", "Difference", ""], rows)

# 5. Each material, in each form it comes in: does a process make it?
rows = []
makes = {}
for pr in processes:
    for o in (pr.get("outputs") or {}).get("products") or []:
        if o.get("item") and o.get("form"):
            makes.setdefault((o["item"], o["form"]), []).append(pr.get("slug"))
for m in materials:
    for form in (m.get("identity") or {}).get("form") or []:
        by = makes.get((m["slug"], form), [])
        rows.append(row("ok" if by else "gap", link(m["identity"]["name"], "mat:" + m["slug"]), form, link(by_process[by[0]]["identity"]["name"], "proc:" + by[0]) if by else "no process makes it"))
report("stock", "Materials: is every form made by a process", "Each material in each form it is said to come in, and the process that makes it in that form.", ["Material", "Form", "Made by"], rows)

# 6. Each process: has it real steps, and is it run anywhere?
rows = []
for pr in processes:
    steps = bool((pr.get("equipment") or {}).get("steps"))
    at = run_at.get(pr["slug"], [])
    state = "ok" if steps and pr["slug"] in lined else "gap"
    rows.append(row(state, link(pr["identity"]["name"], "proc:" + pr["slug"]), "yes" if steps else "no steps", ", ".join(f["name"] for f in at) or "nowhere", "yes" if pr["slug"] in lined else "no line built for it"))
report("processes", "Processes: steps, and somewhere they are run", "Each process: whether it is broken into steps with their modules, which facilities list it, and whether any of them has a line built for it.", ["Process", "Steps", "Listed by", "A line built"], rows)

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
    made = [m for m in modules if (m.get("rate") or {}).get("product") == gd["slug"] or any(x.get("item") == gd["slug"] for x in (m.get("outputs") or {}).get("by_products") or [])]
    used = [m for m in modules if any(x.get("item") == gd["slug"] for x in (m.get("inputs") or {}).get("materials") or [])]
    kind = (gd.get("identity") or {}).get("kind")
    need_made, need_used = kind not in ("rock", "raw", "consumable", "fuel"), kind not in ("by-product", "product", "rock")
    if kind == "rock":
        won = [o for o in goods if (o.get("source") or {}).get("won_from") == gd["slug"]]
        used = [m for m in modules if any(x.get("item") == gd["slug"] for x in (m.get("inputs") or {}).get("materials") or [])]
        occurs = (gd.get("source") or {}).get("occurs")
        ore = (gd.get("game") or {}).get("ore")
        rows.append(row("ok" if occurs else "gap", link(gd["identity"]["name"], "good:" + gd["slug"]), kind, (f"dug on {occurs}" if occurs else "nowhere said") + ("" if ore else "; the game has no ore for it yet"), ", ".join(o["identity"]["name"] for o in won) or ", ".join(m["identity"]["name"] for m in used) or "nothing uses it yet"))
        continue
    if kind == "raw":
        rock = next((o for o in goods if o["slug"] == (gd.get("source") or {}).get("won_from")), None)
        used = [m for m in modules if any(x.get("item") == gd["slug"] for x in (m.get("inputs") or {}).get("materials") or [])]
        rows.append(row("ok" if rock else "gap", link(gd["identity"]["name"], "good:" + gd["slug"]), kind, ("won from " + rock["identity"]["name"] + (f", {100 * gd['source']['yield']:.3g}% of it" if "yield" in gd["source"] else ", how much not said")) if rock else "no rock it is won from", ", ".join(m["identity"]["name"] for m in used) or "nothing uses it"))
        continue
    gap = (need_made and not made) or (need_used and not used)
    rows.append(row("gap" if gap else "ok", link(gd["identity"]["name"], "good:" + gd["slug"]), kind, ", ".join(m["identity"]["name"] for m in made) or ("comes from outside" if not need_made else "nothing makes it"), ", ".join(m["identity"]["name"] for m in used) or ("goes out" if not need_used else "nothing uses it")))
report("goods", "Goods: where each comes from and goes", "Each good: where it comes from and where it goes. A rock is dug; a raw good is won from a rock; consumables and fuel come from outside; the rest come out of one module and go into another, or out.", ["Good", "Kind", "Comes out of", "Goes into"], rows)


# 8. Confidence: where every number comes from.
# (Elements and materials are from published sources, named in their files; a process's amounts are
# its material's composition. Other records say for themselves, in `basis`; one that doesn't is a gap.)
DEFAULT_TIER = {"elements": "sourced", "materials": "sourced", "processes": "derived"}
KIND_NAME = {"elements": "Elements", "materials": "Materials", "processes": "Processes", "modules": "Industrial modules", "goods": "Goods", "hulls": "Hulls", "mill-stock": "Mill stock", "parts": "Parts", "equipment": "Ship equipment", "gates": "Stargates", "layouts": "Ship layouts"}
KEY_OF = {"layouts": lambda e: "lay:" + e["slug"], "gates": lambda e: "gate:" + e["slug"], "equipment": lambda e: "eq:" + e["slug"], "elements": lambda e: "el:" + e["identity"]["symbol"], "materials": lambda e: "mat:" + e["slug"], "processes": lambda e: "proc:" + e["slug"], "modules": lambda e: "mod:" + e["slug"], "goods": lambda e: "good:" + e["slug"], "hulls": lambda e: "hull:" + e["slug"], "mill-stock": lambda e: "stock:" + e["slug"], "parts": lambda e: "part:" + e["slug"]}


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
    for exact in (True, False):
        for en in raw.get("basis") or []:
            if (f"{group}.{prop}" in en["of"]) if exact else (group in en["of"]):
                return bool(en.get("review")) if review else en["tier"]
    return False if review else DEFAULT_TIER.get(kind, "unsaid")


rows, detail, reviews = [], [], []
for kind, recs in (("elements", elements), ("materials", materials), ("processes", processes), ("modules", modules), ("goods", goods), ("hulls", hulls), ("mill-stock", mill_stock), ("parts", parts), ("equipment", equipment), ("gates", gates), ("layouts", layouts)):
    tally = {"sourced": 0, "derived": 0, "invented": 0, "unsaid": 0}
    to_review = 0
    for e in recs:
        raw = load(os.path.join(TREE, e["file"]))
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
celestial = {"galaxy": {}, "systems": [], "groups": {}}
if os.path.isdir(CEL):
    cschema = {k: yaml.safe_load(open(os.path.join(CEL, "schema", f"{k}.schema.yaml"), encoding="utf-8")) for k in ("galaxy", "system", "body", "field")}
    celestial["groups"] = {k: {g: {q: v.get("description", "") for q, v in d["properties"].items()} for g, d in cschema[k]["properties"].items() if "properties" in d} for k in ("system", "body", "field")}

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
        if rec.get("status") not in ("seeded", "curated", "frozen"):
            problem(full, "status: one of seeded, curated, frozen")
        if os.path.basename(full)[:-5] != re.sub(r"[^a-z0-9]+", "-", str((rec.get("identity") or {}).get("name", "")).lower()).strip("-"):
            problem(full, "a celestial record's file is named after it (lower case, words joined by -)")
        rec["slug"], rec["file"] = os.path.basename(full)[:-5], os.path.relpath(full, TREE)
        return rec

    gpath = os.path.join(CEL, "metadata", "galaxy.yaml")
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
        sysm["fields"] = [cel_load(os.path.join(sdir, fn[:-5], "fields", b), "field") for b in sorted(os.listdir(os.path.join(sdir, fn[:-5], "fields")))] if os.path.isdir(os.path.join(sdir, fn[:-5], "fields")) else []
        names = {b["identity"]["name"] for b in sysm["bodies"]} | {sysm["identity"]["name"]}
        for b in sysm["bodies"]:
            if b["identity"].get("parent") not in names:
                problem(os.path.join(TREE, b["file"]), f"identity.parent: no body '{b['identity'].get('parent')}' in {sysm['identity']['name']}")
        for f_ in sysm["fields"]:
            if f_["identity"].get("anchor") not in names:
                problem(os.path.join(TREE, f_["file"]), f"identity.anchor: no body '{f_['identity'].get('anchor')}' in {sysm['identity']['name']}")
        # (In order out from what each goes round.)
        sysm["bodies"].sort(key=lambda b: (b.get("orbit") or {}).get("semi_major_axis", 0))
        celestial["systems"].append(sysm)
    home = celestial["galaxy"].get("home")
    celestial["systems"].sort(key=lambda s: (s["identity"]["name"] != home, (s.get("position") or {}).get("distance", 0)))
    if home and home not in {s["identity"]["name"] for s in celestial["systems"]}:
        problem(gpath, f"home: no system '{home}' written out")
# The celestial report: each system's records by status, and each body Local Administration has
# against the celestial record of the same name.
rows = []
for sysm in celestial["systems"]:
    recs = [sysm] + sysm["bodies"] + sysm["fields"]
    count = lambda st: sum(1 for r_ in recs if r_.get("status") == st)
    rows.append(row("ok", link(sysm["identity"]["name"], "cs:" + sysm["slug"]), f"{len(sysm['bodies'])} bodies, {len(sysm['fields'])} fields", f"{count('seeded')} seeded, {count('curated')} curated, {count('frozen')} frozen", ""))
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
report("celestial", "Celestial: what is written out, and against Local Administration", "Each system written out: its records by status. Each planet and moon Local Administration has: is there a celestial record of the same name, and do they agree. A gap is a body with no celestial record, or one where the two differ.", ["What", "Records", "By status", "Note"], rows)


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
        "celestial": celestial,
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
        "equipment": equipment,
        "gates": gates,
        "layouts": layouts,
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
        "process_groups": {g: {k: v.get("description", "") for k, v in d["properties"].items() if "properties" in d} for g, d in SCHEMAS["processes"]["properties"].items() if "properties" in d},
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
write_ron()
print(f"{len(makers)} makers, {len(bodies)} bodies, {len(standards)} standards, {len(elements)} elements, {len(materials)} materials, {len(processes)} processes, {len(modules)} modules, {len(goods)} goods, {len(hulls)} hulls")
print(f"  reports: " + ", ".join(f"{r['key']} {r['gaps']} gaps" for r in reports))
print(f"  standards/index.html\n  content/base/bodies.ron, content/base/standards.ron, content/base/brands.ron, content/base/settlements.ron, content/base/industry.ron, content/base/layouts.ron")
