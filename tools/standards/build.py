#!/usr/bin/env python3
"""The standards, from their YAML, checked and built.

    python3 tools/standards/build.py

Reads `standards/`: a folder per body (named after its prefix) with `_body.yaml`, and its
standards beside it, flat, one file each: `NNNN-<slug>.yaml`, the number its permanent id
(`FSO 12`: never reused, never changed, whatever it's later filed under). A standard's `topics`
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
BODY_FIELDS = {"key", "name", "prefix", "seat", "note", "kind", "founded_by", "about"}
BODY_KINDS = ["consortium", "independent", "authority", "corporation", "players"]
# The game's brands (members and makers are named by them).
BRANDS = dict(re.findall(r'key: "(brand\.[a-z0-9_]+)", name: "([^"]*)"', open(os.path.join(ROOT, "content", "base", "brands.ron"), encoding="utf-8").read()))
STANDARD_FIELDS = {"version", "title", "purpose", "details", "status", "topics", "scope", "sections", "refs", "params", "requires", "text", "licence", "published"}

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
    for r in s.get("refs", []) or []:
        if r not in ids:
            problem(where, f"refers to {r}: no such standard (ids are like 'FSO 12')")
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
bodies, standards = [], []
for name in sorted(os.listdir(TREE)):
    folder = os.path.join(TREE, name)
    if not os.path.isdir(folder) or name == "schema":
        continue
    meta_path = os.path.join(folder, "_body.yaml")
    if not os.path.exists(meta_path):
        problem(folder, "a body's folder needs _body.yaml")
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
            problem(meta_path, f"founded_by: no brand '{m}' in content/base/brands.ron")
    if isinstance(body.get("about"), str):
        body["about"] = [body["about"]]
    if body.get("prefix") != name:
        problem(meta_path, f"prefix {body.get('prefix')} but the folder is {name}")
    numbers = {}
    for name in sorted(os.listdir(folder)):
        full = os.path.join(folder, name)
        if name == "_body.yaml":
            continue
        if os.path.isdir(full):
            problem(full, "no folders in a body: its standards sit flat beside _body.yaml")
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
        "bodies": [{k: b[k] for k in ("key", "name", "prefix", "seat", "note", "kind", "founded_by", "about") if k in b} for b in bodies],
        "brands": BRANDS,
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
print(f"{len(bodies)} bodies, {len(standards)} standards, {len({t for s in standards for t in s.get('topics', [])})} topics")
print(f"  standards/index.html\n  content/base/bodies.ron, content/base/standards.ron")
