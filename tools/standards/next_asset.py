"""The asset lookup: which registry record needs a model next, and everything a modeller needs to make it.

    python3 tools/standards/next_asset.py                 # the next one, as a brief
    python3 tools/standards/next_asset.py --json          # the same, as JSON (for an agent)
    python3 tools/standards/next_asset.py --list 20       # the queue, the first 20
    python3 tools/standards/next_asset.py --summary       # how many records have a model, by kind
    python3 tools/standards/next_asset.py --key equipment.power.battery.s1   # the brief of one record
    python3 tools/standards/next_asset.py --hulls         # include hulls (the ships session's designs)

The queue is every record of a kind that takes a model (equipment, modules, buildings, structures, gates; hulls on
asking) with no `visual` yet and not retired, in the order the player sees them most: equipment fitted on hulls and
structures, then structures and gates, then works modules standing at facilities, then buildings standing at
settlements, then the rest; within each, the most used first, current designs before outdated ones. Docs:
docs/asset-contract.md.
"""
import glob, json, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import KINDS, TREE, LOW, HIGH, records, required_nodes, size_of, store, tail  # noqa: E402


def usage():
    """How often each key stands in the world: fitted, built, laid out."""
    used = {}
    def add(k, n=1):
        used[k] = used.get(k, 0) + n
    for f in glob.glob(os.path.join(TREE, "SFO/metadata/hulls/*.yaml")) + glob.glob(os.path.join(TREE, "SFO/metadata/structures/*.yaml")):
        for x in (yaml.safe_load(open(f)) or {}).get("fit") or []:
            add(x.get("item"), x.get("count", 1))
    for f in glob.glob(os.path.join(TREE, "LocalAdministration/**/*.yaml"), recursive=True):
        d = yaml.safe_load(open(f)) or {}
        for x in d.get("modules") or []:
            add(x.get("module"), x.get("count", 1))
        for ln in d.get("lines") or []:
            for x in ln.get("modules") or []:
                add(x.get("module"), x.get("count", 1))
        for x in d.get("buildings") or []:
            add(x.get("building"), x.get("count", 1))
    return used


def tier(kind, key, used):
    if kind == "equipment" and used.get(key):
        return 0
    if kind in ("structure", "gate"):
        return 1
    if kind == "module" and used.get(key):
        return 2
    if kind == "building" and used.get(key):
        return 3
    return {"hull": 0, "equipment": 4, "module": 5, "building": 6}.get(kind, 7)


def stage(r):
    return (r.get("identity") or {}).get("revision") or ""


def brief(kind, key, path, r, used):
    i = r.get("identity") or {}
    size = size_of(kind, r)
    b = {"key": key, "kind": kind, "name": i.get("name"), "description": i.get("description") or i.get("about"),
         "maker": i.get("maker"), "design_stage": stage(r) or "draft", "used": used.get(key, 0),
         "size_m": {"length": size[0], "width": size[1], "height": size[2]} if size else None,
         "size_rule": f"the model's three sides, sorted, each {LOW:.0%} to {HIGH:.0%} of the record's, sorted (glTF metres; axes free)",
         "mass_kg": (r.get("physical") or {}).get("mass"),
         "required_nodes": required_nodes(kind, r), "triangle_budget": KINDS[kind][1],
         "record": os.path.relpath(path, os.path.dirname(TREE)),
         "install": f"python3 tools/standards/install_model.py <model.glb> --as={key} --source=<file.blend> --push",
         "package": f"{store()}/models/{tail(key)}/v<N>/"}
    fn = r.get("function") or {}
    if fn:
        b["function"] = {k: v for k, v in fn.items() if not isinstance(v, list)}   # (a rocket's chamber design comes with it)
    if r.get("fits"):
        m = yaml.safe_load(open(os.path.join(TREE, "SFO/metadata/mounts", r["fits"].split(".", 1)[1] + ".yaml"))) if os.path.exists(os.path.join(TREE, "SFO/metadata/mounts", r["fits"].split(".", 1)[1] + ".yaml")) else {}
        b["mount"] = {"key": r["fits"], "envelope_m": m.get("envelope"), "attachment": m.get("attachment")}
    lst = (r.get("built_of") or {}).get("list") or []
    if lst:
        b["parts"] = [{"code": p["code"], "name": p["name"], "description": p.get("description"), "mass_kg": p["mass"], "box_m": [p["length"], p["width"], p["height"]], "of": p.get("item")} for p in lst]
    elif (r.get("built_of") or {}).get("parts"):
        folder = os.path.join(TREE, "SFO/metadata/parts", r["built_of"]["parts"])
        b["parts"] = [{"code": d["identity"]["code"], "name": d["identity"]["name"], "mass_kg": (d.get("physical") or {}).get("mass"),
                       "box_m": [(d.get("physical") or {}).get(k) for k in ("length", "width", "height")]}
                      for d in (yaml.safe_load(open(f)) for f in sorted(glob.glob(folder + "/*.yaml")))]
    if r.get("basis"):
        b["basis"] = [{"of": x.get("of"), "tier": x.get("tier"), "rule": x.get("rule"), "note": x.get("note")} for x in r["basis"]]
    if kind == "hull":
        b["conventions"] = "docs/ship-import.md (nose +Y, up +Z, COL_ meshes, nozzle_/gear/dock empties); hulls are the ships session's designs"
    return b


def queue(hulls=False):
    used = usage()
    q = [(kind, key, path, r) for kind, key, path, r in records()
         if not r.get("visual") and stage(r) != "retired" and (hulls or kind != "hull")]
    q.sort(key=lambda x: (tier(x[0], x[1], used), stage(x[3]) == "outdated", -used.get(x[1], 0), x[1]))
    return q, used


def main(argv):
    opt = {a.split("=", 1)[0].lstrip("-"): (a.split("=", 1)[1] if "=" in a else True) for a in argv[1:] if a.startswith("--")}
    if "list" in opt and opt["list"] is True and len(argv) > argv.index("--list") + 1:
        opt["list"] = argv[argv.index("--list") + 1]
    if "key" in opt and opt["key"] is True:
        opt["key"] = argv[argv.index("--key") + 1]
    if opt.get("summary"):
        rows = {}
        for kind, key, path, r in records():
            t = rows.setdefault(kind, [0, 0, 0]); t[0] += 1; t[1] += bool(r.get("visual")); t[2] += stage(r) == "retired"
        print(f"{'kind':<10} {'records':>7} {'modelled':>8} {'retired':>7}")
        for kind, (n, m, x) in rows.items():
            print(f"{kind:<10} {n:>7} {m:>8} {x:>7}")
        return
    q, used = queue(bool(opt.get("hulls")))
    if opt.get("key"):
        hit = [(k, key, p, r) for k, key, p, r in records() if key == opt["key"]]
        if not hit:
            sys.exit(f"no record {opt['key']} of a kind that takes a model")
        print(json.dumps(brief(*hit[0], used), indent=1, ensure_ascii=False))
        return
    if opt.get("list"):
        print(f"{'#':>3} {'kind':<10} {'used':>4} {'stage':<9} key")
        for n, (kind, key, path, r) in enumerate(q[:int(opt["list"])], 1):
            print(f"{n:>3} {kind:<10} {used.get(key, 0):>4} {stage(r) or 'draft':<9} {key}")
        print(f"{len(q)} records want a model")
        return
    if not q:
        print("nothing wants a model")
        return
    b = brief(*q[0], used)
    if opt.get("json"):
        print(json.dumps(b, indent=1, ensure_ascii=False))
        return
    print(f"next: {b['key']} ({b['kind']}, {b['name']}), used {b['used']}x, {b['design_stage']}")
    print(f"  {b['description'] or ''}")
    if b["size_m"]:
        print(f"  size {b['size_m']['length']} x {b['size_m']['width']} x {b['size_m']['height']} m ({b['size_rule']}); mass {b['mass_kg']} kg")
    if b.get("mount"):
        print(f"  mount {b['mount']['key']}: envelope {b['mount']['envelope_m']}")
    print(f"  nodes required: {', '.join(n + '*' for n in b['required_nodes']) or 'none'}; budget {b['triangle_budget']:,} triangles")
    for p in b.get("parts", []):
        print(f"  part {p['code']} {p['name']}: {p['mass_kg']} kg, box {p['box_m']}")
    print(f"  install: {b['install']}")
    print(f"  {len(q) - 1} more after it")


if __name__ == "__main__":
    main(sys.argv)
