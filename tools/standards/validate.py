#!/usr/bin/env python3
"""Checks every record of the registry against its schema: types, enums, required fields, patterns,
and no field its schema does not name. A small reader of JSON Schema (draft 7, the part the
registry's schemas use), so the build needs nothing but PyYAML.

    python3 tools/standards/validate.py        # lists what does not fit; exits 1 if anything
"""
import os, re, sys
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TREE = os.path.join(ROOT, "standards")
_schemas = {}
KEYS = {}            # every record's key, and the file that has it (filled by check_all)


def schema(path):
    path = os.path.normpath(path)
    if path not in _schemas:
        with open(path, encoding="utf-8") as f:
            _schemas[path] = yaml.safe_load(f)
    return _schemas[path]


def resolve(ref, here):
    """A `$ref`: `#/definitions/x` in the same file, or `<file>#/definitions/x` beside it."""
    file, _, pointer = ref.partition("#")
    path = os.path.normpath(os.path.join(os.path.dirname(here), file)) if file else here
    node = schema(path)
    for step in [s for s in pointer.split("/") if s]:
        node = node[step]
    return node, path


TYPES = {"string": str, "number": (int, float), "integer": int, "boolean": bool, "array": list, "object": dict, "null": type(None)}


def is_type(v, t):
    if t in ("number", "integer") and isinstance(v, bool):
        return False
    return isinstance(v, TYPES[t])


def validate(v, sch, here, at=""):
    """What of `v` does not fit `sch` (from the schema file `here`): a list of plain sentences."""
    if not isinstance(sch, dict):
        return []
    if "$ref" in sch:
        target, path = resolve(sch["$ref"], here)
        return validate(v, target, path, at)
    out, where = [], at or "the record"
    t = sch.get("type")
    if t is not None and not any(is_type(v, x) for x in (t if isinstance(t, list) else [t])):
        return [f"{where}: should be {' or '.join(t) if isinstance(t, list) else ('an ' if t[0] in 'aeiou' else 'a ') + t}, is {v!r}"[:200]]
    if "enum" in sch and v not in sch["enum"]:
        out.append(f"{where}: {v!r} is not one of {', '.join(map(str, sch['enum']))}")
    if "const" in sch and v != sch["const"]:
        out.append(f"{where}: should be {sch['const']!r}")
    if isinstance(v, str):
        if "pattern" in sch and not re.search(sch["pattern"], v):
            out.append(f"{where}: {v!r} does not match {sch['pattern']}")
        if "minLength" in sch and len(v) < sch["minLength"]:
            out.append(f"{where}: too short")
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        if "minimum" in sch and v < sch["minimum"]:
            out.append(f"{where}: {v} is below {sch['minimum']}")
        if "maximum" in sch and v > sch["maximum"]:
            out.append(f"{where}: {v} is above {sch['maximum']}")
    if isinstance(v, list):
        if "minItems" in sch and len(v) < sch["minItems"]:
            out.append(f"{where}: needs at least {sch['minItems']}")
        if "maxItems" in sch and len(v) > sch["maxItems"]:
            out.append(f"{where}: takes at most {sch['maxItems']}")
        if "items" in sch:
            for i, x in enumerate(v):
                out += validate(x, sch["items"], here, f"{at}[{i}]")
    if isinstance(v, dict):
        props = sch.get("properties") or {}
        for k in sch.get("required") or []:
            if k not in v:
                out.append(f"{where}: no {k}")
        extra = sch.get("additionalProperties", True)
        for k, x in v.items():
            if k in props:
                out += validate(x, props[k], here, f"{at}.{k}" if at else k)
            elif extra is False:
                out.append(f"{where}: unknown field '{k}'")
            elif isinstance(extra, dict):
                out += validate(x, extra, here, f"{at}.{k}" if at else k)
    for key in ("anyOf", "oneOf"):
        if key in sch and not any(not validate(v, s, here, at) for s in sch[key]):
            out.append(f"{where}: fits none of its alternatives")
    return out


def schema_of(rel):
    """The schema a record is held to, by where it is filed (its path under standards/)."""
    p = rel.split(os.sep)
    root, name = p[0], p[-1]
    S = lambda r, n: os.path.join(TREE, r, "schema", n + ".schema.yaml")
    ORG = os.path.join(TREE, "organisation.schema.yaml")              # (companies, the standards body, administrations: one schema)
    if root == "Celestial":
        if len(p) == 3:
            return S(root, {"galaxy.yaml": "galaxy", "asteroids.yaml": "asteroids", "conditions.yaml": "conditions"}.get(name, ""))
        if p[2] == "rock-classes":
            return S(root, "rock-class")
        if p[2] == "vocabulary":
            return S(root, "vocabulary")
        if p[2] == "systems":
            return S(root, "system" if len(p) == 4 else {"bodies": "body", "fields": "population", "small-bodies": "body", "regions": "population"}.get(p[4], ""))
    if root == "MakerHouse":
        return ORG if p[2] == "makers" else None
    if root == "LocalAdministration":
        if p[2] != "administrations":
            return None
        if len(p) == 4:
            return ORG
        if len(p) == 5:
            return S(root, "body")
        return S(root, {"zones": "zone", "parcels": "parcel", "streets": "street", "power-lines": "power-line", "facilities": "facility"}.get(p[5], ""))
    if root == "SFO":
        if len(p) == 3:
            return ORG if name == "SFO.yaml" else S(root, "standard")
        kind = p[2]
        return S(root, {"elements": "element", "materials": "material", "processes": "process", "modules": "module", "goods": "good", "hulls": "hull", "mill-stock": "mill-stock", "equipment": "equipment", "gates": "gate", "parts": "part"}.get(kind, ""))
    return None


KEY = re.compile(r"^[a-z][a-z-]*(\.[a-z0-9][a-z0-9-]*)+$")


def key_of(rel, rec):
    """The key a record must have, from where it is filed: `<kind>.<name>`, the kind being its
    schema's. Where the name cannot be told from the path alone, the kind's prefix (ending in a dot)."""
    p = rel.split(os.sep)
    root, stem = p[0], p[-1][:-5]
    low = stem.lower()
    if root == "SFO":
        if len(p) == 3:
            return "org." + low if stem == "SFO" else "standard.sfo." + str(int(stem.split("-")[0]))
        kind = p[2]
        if kind == "elements":
            return "element." + str((rec.get("identity") or {}).get("symbol", "")).lower()
        if kind in ("equipment", "gates"):
            return {"equipment": "equipment.", "gates": "gate."}[kind]
        return {"materials": "material.", "processes": "process.", "modules": "module.", "goods": "good.", "hulls": "hull.", "mill-stock": "stock.", "parts": "part."}[kind] + low
    if root == "MakerHouse":
        return "org."
    if root == "Celestial":
        if len(p) == 3:
            return "seeding." + low
        if p[2] in ("rock-classes", "vocabulary"):
            return {"rock-classes": "rock-class.", "vocabulary": "vocabulary."}[p[2]] + low
        if len(p) == 4:
            return "system." + low
        return {"bodies": "body.", "fields": "population.", "small-bodies": "body.", "regions": "population."}[p[4]] + p[3] + "." + low
    if root == "LocalAdministration":
        if len(p) == 4:
            return "org." + low
        if len(p) == 5:
            return {"settlement": "settlement.", "rig": "rig."}.get(rec.get("kind"), "la-body.") + p[3] + "." + low
        kind = {"zones": "zone", "parcels": "parcel", "streets": "street", "power-lines": "power-line", "facilities": "facility"}[p[5]]
        return f"{kind}.{p[3]}.{p[4]}." + (low[len("parcel-"):] if kind == "parcel" else low)
    return None


def key_in(rec):
    """Where a record's key is written: in its identity, or (one with no identity group) at its top."""
    return (rec.get("identity") or {}).get("key") if isinstance(rec.get("identity"), dict) else rec.get("key")


def check_all():
    """Every record against its schema: (file, what does not fit)."""
    found, unheld = [], []
    for dp, dns, fns in os.walk(TREE):
        dns[:] = [d for d in dns if d not in ("schema", "sources", "logos", "icons")]
        for fn in sorted(fns):
            if not fn.endswith(".yaml"):
                continue
            full = os.path.join(dp, fn)
            rel = os.path.relpath(full, TREE)
            if os.sep not in rel or fn.endswith(".schema.yaml"):
                continue
            sp = schema_of(rel)
            if sp is None:
                continue
            if not os.path.exists(sp):
                unheld.append(rel)
                continue
            with open(full, encoding="utf-8") as f:
                rec = yaml.safe_load(f) or {}
            for e in validate(rec, schema(sp), sp):
                found.append((full, e))
            # (Its key: there, of its kind, as its file says, and no other record's.)
            key, want = key_in(rec), key_of(rel, rec)
            if not isinstance(key, str) or not KEY.match(key):
                found.append((full, f"key: {key!r} is not <kind>.<name> in lower case, words joined by -"))
            elif want and (key != want if not want.endswith(".") else not key.startswith(want)):
                found.append((full, f"key: should be {want}{'<name>' if want.endswith('.') else ''}, is {key}"))
            elif key in KEYS:
                found.append((full, f"key: {key} is also {KEYS[key]}'s"))
            else:
                KEYS[key] = rel
    return found, unheld


if __name__ == "__main__":
    found, unheld = check_all()
    for full, e in found:
        print(f"{os.path.relpath(full, ROOT)}: {e}")
    for rel in unheld:
        print(f"standards/{rel}: no schema holds it")
    print(f"{len(found)} thing(s) that do not fit their schema, {len(unheld)} record(s) with no schema")
    sys.exit(1 if found or unheld else 0)
