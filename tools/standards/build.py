#!/usr/bin/env python3
"""The standards, from their YAML tree, checked and built.

    python3 tools/standards/build.py

Reads `standards/` (a folder per body with `_body.yaml`; a folder per branch, named
`<path>-<slug>`, with `_branch.yaml`; a file per standard, `NNN-<slug>.yaml`, its id
<prefix>/<branch path>/<NNN>), checks it, and writes:

- `standards/index.html`: the tree to browse (open it; rerun and refresh after an edit);
- `content/base/bodies.ron` and `content/base/standards.ron`: what the game loads
  (generated: edit the YAML, not these).

Exits non-zero with every problem listed if anything's wrong (the page still shows them).
The schemas in `standards/schema/` are for the editor (yaml-language-server); the checks
here don't need them. See docs/standards.md.
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
STANDARD_FIELDS = {"version", "title", "status", "scope", "refs", "params", "requires", "text", "licence", "published"}

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


def walk_branch(body, folder, parent_path, branches, standards):
    """A branch folder: its _branch.yaml, its standards, its sub-branches."""
    meta = load(os.path.join(folder, "_branch.yaml"))
    path = str(meta.get("path", ""))
    title = meta.get("title", "")
    if not re.fullmatch(r"[0-9]+(\.[0-9]+)*", path):
        problem(folder, f"_branch.yaml: path '{path}' should be like 2 or 2.1")
        return
    parent = path.rsplit(".", 1)[0] if "." in path else None
    if parent != parent_path:
        problem(folder, f"branch {path} is in the folder of {parent_path or 'its body'} (its parent should be {parent or 'the body'})")
    if not os.path.basename(folder).startswith(path + "-"):
        problem(folder, f"folder should be named '{path}-<slug>'")
    if not title:
        problem(folder, "_branch.yaml: no title")
    branches.append({"path": path, "title": title, "note": meta.get("note", ""), "folder": os.path.relpath(folder, TREE)})
    numbers = set()
    for name in sorted(os.listdir(folder)):
        full = os.path.join(folder, name)
        if os.path.isdir(full):
            if os.path.exists(os.path.join(full, "_branch.yaml")):
                walk_branch(body, full, path, branches, standards)
            else:
                problem(full, "a folder without _branch.yaml (not a branch)")
        elif name.endswith(".yaml") and name != "_branch.yaml":
            m = re.fullmatch(r"([0-9]{3})-[a-z0-9-]+\.yaml", name)
            if not m:
                problem(full, "a standard's file is named NNN-<slug>.yaml")
                continue
            if m.group(1) in numbers:
                problem(full, f"number {m.group(1)} twice in branch {path}")
            numbers.add(m.group(1))
            s = load(full)
            s["id"] = f"{body['prefix']}/{path}/{m.group(1)}"
            s["body"] = body["key"]
            s["branch"] = path
            s["file"] = os.path.relpath(full, TREE)
            standards.append(s)


def check_standard(s, ids):
    where = os.path.join(TREE, s["file"])
    for k in s:
        if k not in STANDARD_FIELDS | {"id", "body", "branch", "file"}:
            problem(where, f"unknown field '{k}'")
    for k in ["version", "title", "status", "scope", "text", "licence"]:
        if k not in s:
            problem(where, f"no {k}")
    if not isinstance(s.get("version", 1), int) or s.get("version", 1) < 1:
        problem(where, "version: a whole number from 1")
    if s.get("status", "published") not in STATUSES:
        problem(where, f"status: one of {', '.join(STATUSES)}")
    lic = s.get("licence", "open")
    if not (lic == "open" or (isinstance(lic, dict) and set(lic) == {"fee"} and isinstance(lic["fee"], (int, float)) and lic["fee"] >= 0)):
        problem(where, "licence: open, or {fee: credits}")
    for r in s.get("refs", []) or []:
        if r not in ids:
            problem(where, f"refers to {r}: no such standard")
        if r == s["id"]:
            problem(where, "refers to itself")
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
    body["branches"] = []
    body_standards = []
    for sub in sorted(os.listdir(folder)):
        full = os.path.join(folder, sub)
        if os.path.isdir(full):
            if os.path.exists(os.path.join(full, "_branch.yaml")):
                walk_branch(body, full, None, body["branches"], body_standards)
            else:
                problem(full, "a folder without _branch.yaml (not a branch)")
    paths = [b["path"] for b in body["branches"]]
    for p in set(paths):
        if paths.count(p) > 1:
            problem(folder, f"branch {p} twice")
    bodies.append(body)
    standards += body_standards

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
        out.append("        branches: [")
        for br in sorted(b["branches"], key=lambda x: [int(n) for n in x["path"].split(".")]):
            out.append(f"            ({ron_str(br['path'])}, {ron_str(caps(br['title']))}),")
        out.append("        ],\n    ),")
    out.append("]\n")
    with open(os.path.join(CONTENT, "bodies.ron"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    out = [head, "["]
    for s in sorted(standards, key=lambda s: s["id"]):
        out.append("    (")
        out.append(f"        key: {ron_str(s['id'])},\n        body: {ron_str(s['body'])},\n        branch: {ron_str(s['branch'])},")
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
        "bodies": [{k: b[k] for k in ("key", "name", "prefix", "seat", "note", "kind", "founded_by", "about", "branches") if k in b} for b in bodies],
        "brands": BRANDS,
        "standards": sorted(standards, key=lambda s: s["id"]),
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
print(f"{len(bodies)} bodies, {sum(len(b['branches']) for b in bodies)} branches, {len(standards)} standards")
print(f"  standards/index.html\n  content/base/bodies.ron, content/base/standards.ron")
