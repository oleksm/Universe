"""The Planet Simulator extractor: what the planet sim says, against what the registry's Planet Simulator root holds.

    python3 tools/standards/planet_lod_extract.py <sim tree> <run dir> [--tools=<dir>] [--package=<dir>] [--json]

Reads the sim's code (its module constants with their comments, its landmark types, its aging tables, its event
kinds), the planet tools' class tables (--tools, default the run's repository's tools/), one run's files (the npz keys
of a checkpoint, its ctx, store and lines, the package's features.json) and reports:
  - words the sim uses that the registry lacks (landmark types, events, ctx keys no field claims, layer classes);
  - figures that disagree (aging, thresholds, constants a record names);
  - ids that moved (layer classes against their frozen ids) and vocabularies short of their source;
  - registry words the sim no longer uses.
It writes nothing: a missing word is proposed to the registry (a draft record), never added silently.
"""
import ast, glob, json, os, re, sys, zipfile
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
TREE = os.path.join(os.path.dirname(os.path.dirname(HERE)), "standards")
MYR = 3.15576e13


def registry():
    out = {}
    for f in glob.glob(os.path.join(TREE, "PlanetSim", "metadata", "**", "*.yaml"), recursive=True):
        d = yaml.safe_load(open(f)) or {}
        k = (d.get("identity") or {}).get("key", "")
        if "." in k and not k.startswith("root."):
            out.setdefault(k.split(".")[0], {})[k] = d
    return out


def constants(paths):
    """Module and class constants (UPPER names) with literal values, and the comment on their line."""
    out = {}
    for f in paths:
        src = open(f).read()
        lines = src.split("\n")
        for node in ast.walk(ast.parse(src)):
            if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name) and node.targets[0].id.isupper():
                try:
                    v = ast.literal_eval(node.value)
                except Exception:
                    continue
                c = lines[node.lineno - 1].split("#", 1)
                out[node.targets[0].id] = {"value": v, "comment": c[1].strip() if len(c) > 1 else "", "file": os.path.relpath(f, os.path.dirname(os.path.dirname(f)))}
    return out


def calls_with_string(paths, names, pos):
    """String literals passed as argument `pos` to calls named in `names` (add_mark, _record, event)."""
    out = set()
    for f in paths:
        for node in ast.walk(ast.parse(open(f).read())):
            if isinstance(node, ast.Call):
                fn = node.func.attr if isinstance(node.func, ast.Attribute) else getattr(node.func, "id", None)
                if fn in names and len(node.args) > pos[fn] and isinstance(node.args[pos[fn]], ast.Constant) and isinstance(node.args[pos[fn]].value, str):
                    out.add(node.args[pos[fn]].value)
    return out


def npz_keys(path):
    with zipfile.ZipFile(path) as z:
        return sorted(n[:-4] for n in z.namelist() if n.endswith(".npy"))


def main(argv):
    opt = {a.split("=", 1)[0].lstrip("-"): (a.split("=", 1)[1] if "=" in a else True) for a in argv[1:] if a.startswith("--")}
    pos = [a for a in argv[1:] if not a.startswith("--")]
    if len(pos) < 2:
        sys.exit(__doc__)
    sim, run = pos[0], pos[1]
    tools = opt.get("tools") if isinstance(opt.get("tools"), str) else None
    if not tools:          # (the run's repository: <repo>/out/planet/<run>)
        repo = os.path.abspath(os.path.join(run, "..", "..", ".."))
        tools = os.path.join(repo, "tools")
    package = opt.get("package") if isinstance(opt.get("package"), str) else os.path.expanduser(f"~/planet-data/packages/{os.path.basename(os.path.normpath(run))}")
    R = registry()
    stages = sorted(glob.glob(os.path.join(sim, "src", "planet_sim", "stages", "*.py")))
    tool_files = sorted(glob.glob(os.path.join(tools, "*.py")))
    found = {"lacks": [], "disagrees": [], "moved": [], "unused": [], "seen": {}}
    lack = lambda what, word, where: found["lacks"].append({"what": what, "word": word, "where": where})
    differ = lambda what, detail: found["disagrees"].append({"what": what, "detail": detail})

    # Landmark types: the strings the sim writes records with, and its aging tables.
    types = calls_with_string(stages, {"add_mark", "_record", "_volcanoes"}, {"add_mark": 2, "_record": 3, "_volcanoes": 3})
    K = constants(stages)
    aging = {}
    for f in stages:
        for node in ast.walk(ast.parse(open(f).read())):
            if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name) and node.targets[0].id in ("ACTIVE_MYR", "WORN_MYR"):
                aging[node.targets[0].id] = ast.literal_eval(node.value)
    types |= set(aging.get("WORN_MYR", {}))
    marks = {d.get("source"): (k, d) for k, d in R.get("planet-landmark", {}).items()}
    found["seen"]["landmark types"] = sorted(types)
    for t in sorted(types):
        if t not in marks:
            lack("landmark type", t, "stages/zones.py, lines.py")
            continue
        k, d = marks[t]
        for tab, fld, default in (("ACTIVE_MYR", "active", 5.0), ("WORN_MYR", "worn", None)):
            sim_v = aging.get(tab, {}).get(t, default)
            if sim_v is not None and d.get(fld) is not None and abs(d[fld] / MYR - sim_v) > 1e-6:
                differ(k, f"{fld}: registry {d[fld] / MYR:g} Myr, sim {tab} {sim_v:g}")
    for s, (k, d) in marks.items():
        if s not in types:
            found["unused"].append({"key": k, "why": "no landmark of this type is written by the sim's stages"})

    # Events the sim logs, against the dictionary's geological events.
    dic = yaml.safe_load(open(os.path.join(TREE, "dictionary.schema.yaml")))["definitions"]["geological_event"]["enum"]
    evs = calls_with_string(stages, {"event"}, {"event": 0})
    found["seen"]["events"] = sorted(evs)
    words = {d.get("source"): d.get("is") for d in R.get("planet-event", {}).values()}
    for e in sorted(evs):
        if e not in dic and e not in words:
            lack("event", e, "the sim's event log (S.event): a planet-event record maps it")

    # Constants a record names: present in the sim, and the figure where the record carries it.
    for kind in ("planet-line", "planet-handler"):
        for k, d in R.get(kind, {}).items():
            for c in d.get("constants") or []:
                if c not in K:
                    differ(k, f"names the sim's constant {c}, which the sim's stages no longer have")
    if "CLOSE_MYR" in K:
        for k, d in R.get("planet-line", {}).items():
            if "CLOSE_MYR" in (d.get("constants") or []) and d.get("ends_idle") and abs(d["ends_idle"] / MYR - K["CLOSE_MYR"]["value"]) > 1e-6:
                differ(k, f"ends_idle {d['ends_idle'] / MYR:g} Myr, sim CLOSE_MYR {K['CLOSE_MYR']['value']}")
    if "ARC_FRONT_M" in K and "planet-line.arc-front" in R.get("planet-line", {}):
        o = R["planet-line"]["planet-line.arc-front"].get("offset")
        if o and abs(o - K["ARC_FRONT_M"]["value"]) > 1e-6:
            differ("planet-line.arc-front", f"offset {o:g} m, sim ARC_FRONT_M {K['ARC_FRONT_M']['value']:g}")
    for c, (sim_c, rec, q) in {"BIG_Q": ("BIG_Q", "planet-landmark.river-mouth", 1 / 3.15576e7), "DELTA_Q": ("DELTA_Q", "planet-landmark.delta", 1 / 3.15576e7)}.items():
        if c in K and rec in R.get("planet-landmark", {}):
            thr = [x for x in R["planet-landmark"][rec].get("threshold") or [] if x["field"] == "planet-field.discharge"]
            if thr and abs(thr[0]["value"] - K[c]["value"] * q) / thr[0]["value"] > 0.01:
                differ(rec, f"threshold {thr[0]['value']:g} m3/s, sim {c} {K[c]['value']:g} m3/yr = {K[c]['value'] * q:g} m3/s")
    found["seen"]["constants"] = {k: v["value"] for k, v in K.items() if isinstance(v["value"], (int, float))}

    # The run's files: every array key, against the fields that claim them.
    cks = sorted(glob.glob(os.path.join(run, "ck_*_ctx.npz")))
    claimed = {}
    for k, d in R.get("planet-field", {}).items():
        for pr in [d.get("producer") or {}] + (d.get("also_from") or []):
            if pr.get("key"):
                claimed[(pr.get("file"), pr["key"])] = k
    for d in R.get("planet-file", {}).values():          # (a file's bookkeeping arrays: known, not words)
        for b in d.get("bookkeeping") or []:
            claimed.setdefault((d.get("source"), b["key"]), d["identity"]["key"])
    if cks:
        last = cks[-1]
        base = last[:-len("_ctx.npz")]
        files = {"checkpoint": base + ".npz", "ctx": last, "store": base + "_store.npz", "lines": base + "_lines.npz"}
        for name, path in files.items():
            if not os.path.exists(path):
                continue
            keys = npz_keys(path)
            found["seen"][f"{name} keys"] = keys
            if name in ("checkpoint", "ctx"):
                for key in keys:
                    if (name, key) not in claimed:
                        lack(f"{name} array", key, os.path.basename(path))
        for (f, key), k in claimed.items():
            if f in ("checkpoint", "ctx") and key not in found["seen"].get(f"{f} keys", []):
                differ(k, f"producer {f}:{key} is not in {os.path.basename(files[f])}")
    else:
        found["disagrees"].append({"what": "run", "detail": f"no ck_*_ctx.npz in {run}"})
    fj = os.path.join(package, "features.json")
    if os.path.exists(fj):
        d = json.load(open(fj))
        found["seen"]["features.json"] = sorted(d) if isinstance(d, dict) else f"a list of {len(d)}"

    # Layer classes: the tools' tables against the frozen ids.
    T = constants(tool_files)
    lay = R.get("planet-layer", {})
    for tab, key in (("CLIMATE", "planet-layer.climate"), ("COVER", "planet-layer.cover")):
        if tab in T and key in lay:
            sim_names = [x if isinstance(x, str) else x[0] for x in T[tab]["value"]]
            reg = {c["id"]: c for c in lay[key].get("classes") or []}
            for i, n in enumerate(sim_names, 1):
                c = reg.get(i)
                if not c:
                    lack(f"{key} class", n, T[tab]["file"])
                elif not (n.startswith(c.get("source") or c["name"]) or (c.get("source") or c["name"]).startswith(n.split(" (")[0])):
                    found["moved"].append({"layer": key, "id": i, "registry": c.get("source") or c["name"], "sim": n, "table": T[tab]["file"]})
    # Vocabularies against their sources.
    df = os.path.join(os.path.dirname(tools), "docs", "landforms", "deciding_factors.json")
    voc = os.path.join(TREE, "Celestial", "metadata", "ground-vocabularies", "pattern.yaml")
    if os.path.exists(df) and os.path.exists(voc):
        pats = [p["id"] for p in json.load(open(df))]
        have = {e["source"] for e in yaml.safe_load(open(voc))["entries"]}
        missing = [p for p in pats if p not in have]
        if missing:
            lack("pattern (ground-vocabulary.pattern)", f"{len(missing)} of {len(pats)} library entries", "docs/landforms/deciding_factors.json: " + ", ".join(missing[:8]) + (" ..." if len(missing) > 8 else ""))
    if opt.get("json"):
        print(json.dumps(found, indent=1, ensure_ascii=False, default=str))
        return
    print(f"Planet Simulator extract: sim {sim}, run {os.path.basename(os.path.normpath(run))}")
    print(f"  registry: " + ", ".join(f"{len(v)} {k}" for k, v in sorted(R.items())))
    for title, rows, fmt in (("words the registry lacks", found["lacks"], lambda x: f"{x['what']}: {x['word']}  ({x['where']})"),
                             ("figures that disagree", found["disagrees"], lambda x: f"{x['what']}: {x['detail']}"),
                             ("ids that moved", found["moved"], lambda x: f"{x['layer']} id {x['id']}: registry '{x['registry']}', sim '{x['sim']}' ({x['table']})"),
                             ("registry words the sim does not use", found["unused"], lambda x: f"{x['key']}: {x['why']}")):
        print(f"\n{title} ({len(rows)}):")
        for x in rows:
            print("  - " + fmt(x))


if __name__ == "__main__":
    main(sys.argv)
