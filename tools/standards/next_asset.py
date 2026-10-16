"""The asset lookup: which registry record needs a model next, and everything a modeller needs to make it.

    python3 tools/standards/next_asset.py                 # the next one, as a brief
    python3 tools/standards/next_asset.py --json          # the same, as JSON (for an agent)
    python3 tools/standards/next_asset.py --list 20       # the queue, the first 20
    python3 tools/standards/next_asset.py --summary       # how many records have a model, by kind
    python3 tools/standards/next_asset.py --report        # done and left as a page (out/model-report.html), opened; --no-open to only write it
    python3 tools/standards/next_asset.py --key equipment.power.battery.s1   # the brief of one record
    python3 tools/standards/next_asset.py --hulls         # include hulls (the ships session's designs)

The queue is every record of a kind that takes a model (equipment, modules, buildings, structures, gates; hulls on
asking) with no `visual` yet and not retired, in the order the player sees them most: equipment the experimental ship
designs use first (docs/asset-usage.yaml, the user's order of 2026-10-11: most designs, then most fitted), then equipment
fitted on hulls and structures (ship equipment first, the user's order of 2026-10-10), then the rest of the equipment, then structures
and gates, then works modules and buildings (their vision still being settled), held records (revision priority low:
the MC-07) last; within each, the most used first, current designs before outdated ones. Docs:
docs/asset-contract.md.
"""
import datetime, glob, json, os, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import KINDS, TREE, LOW, HIGH, records, required_nodes, size_of, store, tail  # noqa: E402


def usage():
    """How often each key stands in the world: fitted, built, laid out."""
    used = {}
    def add(k, n=1):
        used[k] = used.get(k, 0) + n
    for f in glob.glob(os.path.join(TREE, "SFO/metadata/hulls/*.yaml")) + glob.glob(os.path.join(TREE, "SFO/metadata/structures/*.yaml")):
        d = yaml.safe_load(open(f)) or {}
        if (d.get("revision") or {}).get("priority") == "low":     # (a held hull's fits lend no priority)
            continue
        for x in d.get("fit") or []:
            add(x.get("item"), x.get("count", 1))
    for f in glob.glob(os.path.join(TREE, "SFO/metadata/stock/*.yaml")) + glob.glob(os.path.join(TREE, "SFO/metadata/mill-stock/*.yaml")):
        add((yaml.safe_load(open(f)) or {}).get("package"))      # (a package is used by every stock it carries)
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


def designs():
    """Equipment the experimental ship designs use (docs/asset-usage.yaml): key -> (designs using it, instances)."""
    f = os.path.join(os.path.dirname(TREE), "docs", "asset-usage.yaml")
    d = (yaml.safe_load(open(f)) or {}) if os.path.exists(f) else {}
    return {k: tuple(v) for k, v in (d.get("equipment") or {}).items()}


def tier(kind, key, used):
    """Ship equipment first (the user, 2026-10-10): what is in use, then the rest; then structures and gates; works
    modules and buildings after (their vision is still being settled); hulls last (they are the ships session's)."""
    if kind == "equipment" and used.get(key):
        return 0
    if kind == "package":      # (stock is drawn as its packages: the user, 2026-10-10)
        return 0
    if kind == "equipment":
        return 1
    if kind in ("structure", "gate", "unit", "run", "connector"):      # (works units and their kit: the owner, 2026-10-15)
        return 2
    if kind == "module":
        return 3
    if kind == "building":
        return 4
    return 5


def stage(r):
    return (r.get("identity") or {}).get("revision") or ""


def brief(kind, key, path, r, used):
    i = r.get("identity") or {}
    size = size_of(kind, r)
    b = {"key": key, "kind": kind, "name": i.get("name"), "description": i.get("description") or i.get("about"),
         "maker": i.get("maker"), "design_stage": stage(r) or "draft", "used": used.get(key, 0),
         "priority": (r.get("revision") or {}).get("priority", "normal"), "priority_because": (r.get("revision") or {}).get("because") if (r.get("revision") or {}).get("priority") else None,
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
    if kind == "unit" and r.get("fits"):
        m = yaml.safe_load(open(os.path.join(TREE, "SFO/metadata/plant-mounts", r["fits"].split(".", 1)[1] + ".yaml")))
        b["mount"] = {"key": r["fits"], "footprint_m": m.get("footprint")}
    elif r.get("fits"):
        m = yaml.safe_load(open(os.path.join(TREE, "SFO/metadata/mounts", r["fits"].split(".", 1)[1] + ".yaml"))) if os.path.exists(os.path.join(TREE, "SFO/metadata/mounts", r["fits"].split(".", 1)[1] + ".yaml")) else {}
        b["mount"] = {"key": r["fits"], "envelope_m": m.get("envelope"), "attachment": m.get("attachment")}
    lst = (r.get("built_of") or {}).get("list") or [] if not isinstance(r.get("built_of"), list) else []
    if isinstance(r.get("built_of"), list):
        b["made_of"] = [{"what": p.get("what", p["item"]), "of": p["item"], "mass_kg": p.get("mass", p.get("quantity")), "note": p.get("note")} for p in r["built_of"]]   # (older records list {item, quantity})
    if kind == "layout":
        b["layout"] = {k: r.get(k) for k in ("site", "bays", "mobile", "links", "frame", "ground")}
        b["model_notes"] = "a layout is assembled from its units' models placed at their bays (x along, y across, z up, metres from its middle at ground level) on the frame and ground works it names; model the frame and the links, place the units"
    if lst:
        b["parts"] = [{"code": p["code"], "name": p["name"], "description": p.get("description"), "mass_kg": p["mass"], "box_m": [p["length"], p["width"], p["height"]], "of": p.get("item")} for p in lst]
    elif isinstance(r.get("built_of"), dict) and r["built_of"].get("parts"):
        folder = os.path.join(TREE, "SFO/metadata/parts", r["built_of"]["parts"])
        b["parts"] = [{"code": d["identity"]["code"], "name": d["identity"]["name"], "mass_kg": (d.get("physical") or {}).get("mass"),
                       "box_m": [(d.get("physical") or {}).get(k) for k in ("length", "width", "height")]}
                      for d in (yaml.safe_load(open(f)) for f in sorted(glob.glob(folder + "/*.yaml")))]
    if r.get("basis"):
        b["basis"] = [{"of": x.get("of"), "tier": x.get("tier"), "rule": x.get("rule"), "note": x.get("note")} for x in r["basis"]]
    if kind == "package":
        carried = sorted((yaml.safe_load(open(f)) or {})["identity"]["key"] for f in glob.glob(os.path.join(TREE, "SFO/metadata/stock/*.yaml")) + glob.glob(os.path.join(TREE, "SFO/metadata/mill-stock/*.yaml"))
                         if (yaml.safe_load(open(f)) or {}).get("package") == key)
        b["carries"] = {"count": len(carried), "examples": carried[:12]}
        b["holds"] = r.get("holds"); b["handling"] = r.get("handling"); b["keeps"] = r.get("keeps")
        b["model_notes"] = "one model for every stock it carries: draw it full, with neutral contents the game can tint by the stock's material; no labels"
    if kind == "hull":
        b["conventions"] = "docs/ship-import.md (nose +Y, up +Z, COL_ meshes, nozzle_/gear/dock empties); hulls are the ships session's designs"
    return b


def queue(hulls=False):
    used = usage()
    q = [(kind, key, path, r) for kind, key, path, r in records()
         if not r.get("visual") and stage(r) != "retired" and (hulls or kind != "hull")
         and kind not in ("module", "layout")]      # (no monolith works models: the owner, 2026-10-15; a works is units and runs snapped together)
    low = lambda r: (r.get("revision") or {}).get("priority") == "low"     # (a hold the user set: last of all)
    ds = designs()      # (the experimental designs' equipment ahead of the rest: the user, 2026-10-11)
    q.sort(key=lambda x: (low(x[3]), x[1] not in ds, tuple(-n for n in ds.get(x[1], (0, 0))), tier(x[0], x[1], used),
                          stage(x[3]) == "outdated", -used.get(x[1], 0), x[1]))
    return q, used


REPORT_CSS = """:root{--bg:#fbfaf7;--fg:#1d1d1b;--mute:#6b6a66;--line:#e2e0da;--bar:#3f7d5a;--track:#e8e6e0}
@media (prefers-color-scheme:dark){:root{--bg:#161615;--fg:#e9e7e2;--mute:#9a9892;--line:#2d2c2a;--bar:#5fae82;--track:#2a2928}}
body{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif;margin:0 auto;max-width:1100px;padding:24px 16px}
h1{font-size:22px;margin:0 0 4px}h2{font-size:17px;margin:28px 0 8px}p{color:var(--mute);margin:0 0 16px}
table{border-collapse:collapse;width:100%;margin-bottom:8px}th,td{text-align:left;padding:4px 8px;border-bottom:1px solid var(--line)}
th{color:var(--mute);font-weight:600}td.n,th.n{text-align:right;font-variant-numeric:tabular-nums}code{font-size:12.5px}
.bar{background:var(--track);border-radius:3px;height:8px;min-width:120px}.bar i{display:block;background:var(--bar);height:8px;border-radius:3px}
.wrap{overflow-x:auto}td.img{width:64px;padding:2px 8px}td.img img{width:56px;height:56px;object-fit:contain;display:block;border-radius:4px;background:var(--track)}"""


def report(q, used):
    """The models done and left, as an HTML page in out/ (git-ignored): totals by kind, the queue in order, what is in."""
    from html import escape as e
    rows, done = {}, []
    for kind, key, path, r in records():
        t = rows.setdefault(kind, [0, 0, 0]); t[0] += 1; t[1] += bool(r.get("visual")); t[2] += stage(r) == "retired"
        if r.get("visual"):
            done.append((kind, key, r))
    name = lambda r: e((r.get("identity") or {}).get("name") or "")
    ph = lambda r: r.get("physical") or {}
    dims = lambda r: " x ".join(f"{ph(r)[k]:g}" for k in ("length", "width", "height") if isinstance(ph(r).get(k), (int, float)))
    mass = lambda r: (lambda m: "" if not isinstance(m, (int, float)) else f"{m / 1000:,.1f} t" if m >= 1000 else f"{m:,.0f} kg")(ph(r).get("mass"))
    h = [f"<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>Model report</title><style>{REPORT_CSS}</style>",
         f"<h1>Model report</h1><p>{datetime.datetime.now():%Y-%m-%d %H:%M}, from <code>next_asset.py --report</code>. "
         f"{sum(t[1] for t in rows.values())} modelled, {len(q)} in the queue.</p>",
         "<h2>By kind</h2><div class=wrap><table><tr><th>Kind</th><th class=n>Records</th><th class=n>Done</th><th class=n>Retired</th><th class=n>Left</th><th>Progress</th></tr>"]
    for kind, (n, m, x) in rows.items():
        live = n - x
        h.append(f"<tr><td>{kind}</td><td class=n>{n}</td><td class=n>{m}</td><td class=n>{x}</td><td class=n>{live - m}</td>"
                 f"<td><div class=bar><i style='width:{100 * m / live if live else 100:.0f}%'></i></div></td></tr>")
    h.append("</table></div><h2>Left, in queue order</h2><div class=wrap><table><tr><th class=n>#</th><th>Kind</th><th>Key</th><th>Name</th><th>Stage</th><th class=n>Used</th></tr>")
    for n, (kind, key, path, r) in enumerate(q, 1):
        h.append(f"<tr><td class=n>{n}</td><td>{kind}</td><td><code>{key}</code></td><td>{name(r)}</td><td>{stage(r) or 'draft'}</td><td class=n>{used.get(key, 0)}</td></tr>")
    h.append("</table></div><h2>Done</h2><div class=wrap><table><tr><th></th><th>Kind</th><th>Key</th><th>Name</th><th class=n>Size, m</th><th class=n>Mass</th><th class=n>Version</th><th class=n>Triangles</th></tr>")
    for kind, key, r in sorted(done, key=lambda x: (x[0], x[1])):
        v = r["visual"]
        d = os.path.join(store(), v.get("path", ""))
        icon, thumb = os.path.join(d, "icon.png"), os.path.join(d, "thumb.png")
        img = (f"<a href='file://{e(thumb)}'><img loading=lazy src='file://{e(icon if os.path.exists(icon) else thumb)}' alt=''></a>" if os.path.exists(thumb) or os.path.exists(icon) else "")
        h.append(f"<tr><td class=img>{img}</td><td>{kind}</td><td><code>{key}</code></td><td>{name(r)}</td><td class=n>{dims(r)}</td><td class=n>{mass(r)}</td><td class=n>{v.get('version', '')}</td><td class=n>{v.get('triangles', 0):,}</td></tr>")
    h.append("</table></div>")
    out = os.path.join(os.path.dirname(TREE), "out", "model-report.html")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, "w").write("\n".join(h))
    return out


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
    if opt.get("report"):
        out = report(q, used)
        print(out)
        if not opt.get("no-open"):
            subprocess.Popen(["xdg-open", out], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return
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
