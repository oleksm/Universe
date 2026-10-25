"""Every object against its schema: what has no schema, what breaks its schema, and schemas the game can't build.

    python3 tools/standards/schema_audit.py              # the report; exit 1 on any violation or missing schema
    python3 tools/standards/schema_audit.py --json
    python3 tools/standards/schema_audit.py --codegen    # also build the game's registry types from these schemas
    python3 tools/standards/schema_audit.py --list       # every file without a schema, not only the counts

What it covers:
  records    every record under standards/<area>/ (validate.py: fields, types, enums, keys, references)
  documents  every other tracked YAML of the repo (standards/*.yaml, docs/, tools/, content/, crates/): held to the
             schema its first line names (# yaml-language-server: $schema=...), or reported as having none
  packages   every model package (freefall-assets models/*/v*/): manifest.json held to the schema of its "format" in
             docs/formats/ (or reported as having none); motion.json to docs/formats/freefall-motion-1.schema.yaml
  schemas    every *.schema.yaml against the rules of the game's type generator (crates/core/registry/build.rs): a
             type list only [T] or [T, null], an array has items, every $ref resolves, a property has a type
"""
import glob, json, os, re, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import validate  # noqa: E402

ROOT = validate.ROOT
GAME = os.path.expanduser("~/git/universe")
ASSETS = os.environ.get("UNIVERSE_ASSETS") or os.path.expanduser("~/git/freefall-assets")
NAMED = re.compile(r"#\s*yaml-language-server:\s*\$schema=(\S+)")


def tracked_yaml():
    out = subprocess.run(["git", "-C", ROOT, "ls-files", "*.yaml", "*.yml"], capture_output=True, text=True).stdout.split()
    return [f for f in out if not f.endswith(".schema.yaml")]


def is_record(rel):
    """validate.py's own: a file in a folder of standards/, outside schema/sources/logos/icons."""
    parts = rel.split("/")
    return parts[0] == "standards" and len(parts) > 2 and not set(parts[1:-1]) & {"schema", "sources", "logos", "icons"}


def documents():
    missing, faults = [], []
    for rel in tracked_yaml():
        if is_record(rel):
            continue
        full = os.path.join(ROOT, rel)
        try:
            first = open(full, encoding="utf-8").readline()
            data = yaml.safe_load(open(full, encoding="utf-8"))
        except yaml.YAMLError as e:
            faults.append((rel, f"not YAML: {str(e).splitlines()[0]}"))
            continue
        except OSError:
            continue
        m = NAMED.search(first)
        if not m:
            missing.append(rel)
            continue
        sp = os.path.normpath(os.path.join(os.path.dirname(full), m.group(1)))
        if not os.path.isfile(sp):
            faults.append((rel, f"names schema {m.group(1)}, which does not exist"))
            continue
        faults += [(rel, e) for e in validate.validate(data, validate.schema(sp), sp)]
    return missing, faults


def packages():
    missing, faults, formats = [], [], {}
    for man in sorted(glob.glob(os.path.join(ASSETS, "models", "*", "v*", "manifest.json"))):
        rel = os.path.relpath(man, ASSETS)
        try:
            d = json.load(open(man))
        except ValueError as e:
            faults.append((rel, f"not JSON: {e}"))
            continue
        fmt = str(d.get("format") or "")
        name = fmt.split("/")[0]
        hits = glob.glob(os.path.join(ROOT, "docs", "formats", f"{name}*.schema.yaml")) if name else []
        if not hits:
            missing.append(rel)
            formats[fmt or "(no format)"] = formats.get(fmt or "(no format)", 0) + 1
            continue
        faults += [(rel, e) for e in validate.validate(d, validate.schema(hits[0]), hits[0])]
    motion = os.path.join(ROOT, "docs", "formats", "freefall-motion-1.schema.yaml")
    for mj in sorted(glob.glob(os.path.join(ASSETS, "models", "*", "v*", "motion.json"))):
        try:
            d = json.load(open(mj))
        except ValueError as e:
            faults.append((os.path.relpath(mj, ASSETS), f"not JSON: {e}"))
            continue
        faults += [(os.path.relpath(mj, ASSETS), e) for e in validate.validate(d, validate.schema(motion), motion)]
    return missing, faults, formats


def schema_rules():
    """The game's type generator's rules, on every schema node it types."""
    faults = []
    for sp in sorted(glob.glob(os.path.join(ROOT, "standards", "**", "*.schema.yaml"), recursive=True)):
        rel = os.path.relpath(sp, ROOT)
        sch = validate.schema(sp)

        def walk(n, at, typed, overlay=False):
            if not isinstance(n, dict):
                return
            if "$ref" in n:
                file, _, ptr = n["$ref"].partition("#")
                target = os.path.normpath(os.path.join(os.path.dirname(sp), file)) if file else sp
                if not os.path.isfile(target):
                    faults.append((rel, f"{at}: $ref {n['$ref']}: no such schema"))
                else:
                    node = validate.schema(target)
                    for step in [s for s in ptr.split("/") if s]:
                        node = node.get(step) if isinstance(node, dict) else None
                    if node is None:
                        faults.append((rel, f"{at}: $ref {n['$ref']}: no such definition"))
            t = n.get("type")
            if isinstance(t, list):
                rest = [x for x in t if x != "null"]
                if len(rest) != 1:
                    faults.append((rel, f"{at}: type list {t}: the game's types take only [T] or [T, null]"))
            if (t == "array" or (isinstance(t, list) and "array" in t)) and "items" not in n:
                faults.append((rel, f"{at}: an array with no items"))
            if typed and t is None and not any(k in n for k in ("$ref", "properties", "enum", "const", "allOf", "oneOf", "anyOf")):
                faults.append((rel, f"{at}: no type"))
            for k, v in (n.get("properties") or {}).items():
                walk(v, f"{at}.{k}", not overlay, overlay)   # (an allOf overlay only adds to what it derives from)
            if isinstance(n.get("items"), dict):
                walk(n["items"], f"{at}[]", True)
            for k, v in (n.get("definitions") or {}).items():
                walk(v, f"#{k}", False)
            for k in ("allOf", "oneOf", "anyOf"):
                for i, v in enumerate(n.get(k) or []):
                    walk(v, f"{at}/{k}{i}", False, k == "allOf")
        walk(sch, "", False)
    return faults


def codegen():
    """The game's own generator, run on these schemas: (ok, the error lines)."""
    if not os.path.isdir(os.path.join(GAME, "crates/core/registry")):
        return None, ["no game checkout at ~/git/universe"]
    target = os.path.join(os.environ.get("TMPDIR", "/tmp"), "schema-audit-target")
    env = dict(os.environ, UNIVERSE_REGISTRY_ROOT=os.path.join(ROOT, "standards"), CARGO_TARGET_DIR=target)
    r = subprocess.run(["cargo", "check", "-q", "-p", "universe-registry"], cwd=GAME, env=env, capture_output=True, text=True)
    lines = [x.strip() for x in (r.stdout + r.stderr).splitlines() if "panicked" in x or "error" in x.lower() or "assert" in x]
    return r.returncode == 0, lines[:10]


def main(argv):
    opt = set(argv)
    rec_found, rec_unheld = validate.check_all()
    doc_missing, doc_faults = documents()
    pkg_missing, pkg_faults, formats = packages()
    sch_faults = schema_rules()
    cg = codegen() if "--codegen" in opt else (None, [])
    report = {
        "records": {"violations": [{"file": os.path.relpath(f, ROOT), "what": e} for f, e in rec_found],
                    "no_schema": [f"standards/{r}" for r in rec_unheld]},
        "documents": {"violations": [{"file": f, "what": e} for f, e in doc_faults], "no_schema": doc_missing},
        "packages": {"violations": [{"file": f, "what": e} for f, e in pkg_faults], "no_schema": pkg_missing,
                     "formats_without_schema": formats},
        "schemas": {"violations": [{"file": f, "what": e} for f, e in sch_faults],
                    "codegen": {"ran": cg[0] is not None, "ok": cg[0], "errors": cg[1]}},
    }
    bad = any(report[k]["violations"] for k in report) or any(report[k].get("no_schema") for k in ("records", "documents", "packages")) or cg[0] is False
    if "--json" in opt:
        print(json.dumps(report, indent=1))
        sys.exit(1 if bad else 0)
    for part in ("records", "documents", "packages", "schemas"):
        r = report[part]
        print(f"{part}: {len(r['violations'])} violation(s)" + (f", {len(r['no_schema'])} with no schema" if "no_schema" in r else ""))
        for v in r["violations"][:40]:
            print(f"  VIOLATION {v['file']}: {v['what']}")
        if len(r["violations"]) > 40:
            print(f"  ... {len(r['violations']) - 40} more (--json for all)")
        ns = r.get("no_schema") or []
        if ns:
            if "--list" in opt:
                for f in ns:
                    print(f"  NO SCHEMA {f}")
            else:
                by = {}
                for f in ns:
                    d = os.path.dirname(f) if part != "packages" else "models"
                    by[d] = by.get(d, 0) + 1
                for d, n in sorted(by.items(), key=lambda x: -x[1]):
                    print(f"  NO SCHEMA {n:4} in {d}/")
        if part == "packages" and r["formats_without_schema"]:
            print("  formats with no schema in docs/formats/: " + ", ".join(f"{k} ({v})" for k, v in r["formats_without_schema"].items()))
    if cg[0] is not None:
        print(f"codegen: {'ok' if cg[0] else 'FAILS'}" + "".join(f"\n  {x}" for x in cg[1] if not cg[0]))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main(sys.argv[1:])
