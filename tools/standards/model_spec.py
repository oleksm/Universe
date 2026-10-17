"""What a model is built to, and what has changed since: the spec a record sets for its model, the spec it set when its
installed version was made, and the gap between them, each change with what it asks of the modeller.

    python3 tools/standards/model_spec.py <key>            # the gap brief of one record (JSON)
    python3 tools/standards/model_spec.py --all            # every installed model's verdict, one line each

next_asset.py --rework and --gap use it. The verdicts (docs/asset-contract.md, "Rework"):
  rebuild   its body changed: the record's size left the model's tolerance, or the registry asks it rebuilt
  review    the record's parts changed since the model was made (added, removed, renamed): look whether the model shows them
  rig       its moving parts want pivots, named nodes or a motion file (a motion the model does not carry, a node it lacks)
  measure   its envelope boxes are to be measured from the installed model (model_measure.py)
  optimise  over its triangle budget
  none      nothing to do: a port moved (the game draws the joint at every port: rule 1 of 2026-10-16), a mass changed
The registry may also ask by name: a record's `model_requests` (do, because, asked, of_version), open until a version
above of_version is installed.
"""
import functools, hashlib, json, os, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import LOW, HIGH, ROOT, TREE, records, required_nodes, size_of, store, tail  # noqa: E402

RANK = {"rebuild": 5, "rig": 4, "review": 3, "measure": 2, "optimise": 1, "none": 0}
NO_MODEL = ("layout", "module")     # (no monolith works models: the owner, 2026-10-15; their old models are reference pictures only)
FROM_MODEL = ("measured on blender", "measured on the model", "measured on model", "on blender's model")


def _parts(r):
    b = r.get("built_of")
    if isinstance(b, list):
        return {p.get("what") or p.get("item"): (p.get("note") or "") for p in b}
    if isinstance(b, dict) and b.get("list"):
        return {p["name"]: (p.get("description") or "") for p in b["list"]}
    return {}


def spec(kind, r):
    """The figures of a record its model is built to; masses, prose and duties left out (a model does not show them)."""
    s = {"size": size_of(kind, r), "parts": sorted(_parts(r)), "nodes": sorted(required_nodes(kind, r)),
         "budget": r.get("triangle_budget"), "envelope": r.get("envelope") or [], "shells": r.get("shells") or []}
    s["ports"] = sorted(({"id": p["id"], "at": [round(v, 3) for v in p["at"]], "facing": [round(v, 3) for v in p["facing"]], "connector": p.get("connector") or p.get("interface")}
                         for p in r.get("ports") or [] if p.get("medium") != "load"), key=lambda p: p["id"])
    s["motion"] = sorted(({k: m.get(k) for k in ("id", "kind", "point", "direction", "angle", "distance", "moves", "boxes") if m.get(k) is not None}
                          for m in r.get("motion") or []), key=lambda m: m["id"])
    if kind == "run":
        s["outside"] = r.get("outside")
    return s


def sha(s):
    return hashlib.sha256(json.dumps(s, sort_keys=True).encode()).hexdigest()


def manifest(r):
    v = r.get("visual") or {}
    f = os.path.join(store(), v.get("path", ""), "manifest.json")
    return json.load(open(f)) if v and os.path.exists(f) else None


def _git(*a):
    return subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True).stdout


@functools.lru_cache(maxsize=None)
def _installed_record(path, needle):
    """The record as it stood when its installed version was written: the oldest commit carrying that version's path."""
    rel = os.path.relpath(path, ROOT)
    commits = _git("log", "--format=%H", "--reverse", "-S", needle, "--", rel).split()
    if not commits:
        return None
    return yaml.safe_load(_git("show", f"{commits[0]}:{rel}") or "null")


def installed_spec(kind, path, r):
    """The spec the installed version was made against: the manifest's own (installs since 2026-10-17), else the record
    from git at the commit that installed it. None if neither is found."""
    m = manifest(r)
    if m and m.get("spec"):
        return m["spec"], "manifest"
    v = r.get("visual") or {}
    if not v:
        return None, None
    old = _installed_record(path, v["path"])
    return (spec(kind, old), "git") if old else (None, None)


def open_requests(r):
    v = (r.get("visual") or {}).get("version", 0)
    return [q for q in r.get("model_requests") or [] if q.get("of_version", 0) >= v]


def gap(kind, key, path, r):
    """What the installed model lacks against the record now: a list of {verdict, what, detail} and the worst verdict."""
    out = []
    add = lambda verdict, what, detail: out.append({"verdict": verdict, "what": what, "detail": detail})
    now = spec(kind, r)
    m = manifest(r) or {}
    old, source = installed_spec(kind, path, r)
    # Size: the model as measured at install, against the record now (the installer's own rule).
    if now["size"] and m.get("size_m"):
        a, b = sorted(m["size_m"]), sorted(now["size"])
        ok = lambda x, y: LOW * y <= x <= HIGH * y or abs(x - y) <= 0.03          # (a thin piece: 3 cm either way)
        off = [f"{x:.2f} m against {y:.2f}" for x, y in zip(a, b) if y and not ok(x, y)]
        if off and m.get("bounds_m"):                                              # (a record's box centred on the model's origin, round a lopsided body)
            lo, hi = m["bounds_m"]
            c = sorted([2 * max(abs(lo[0]), abs(hi[0])), hi[1] - lo[1], 2 * max(abs(lo[2]), abs(hi[2]))])     # (glTF y is up: from the ground, not centred)
            if all(ok(x, y) for x, y in zip(c, b)):
                off = []
        if off:
            add("rebuild", "size", f"the model's sides (sorted) are out of {LOW:.0%} to {HIGH:.0%} of the record's: {'; '.join(off)}")
    if old:
        parts_now = _parts(r)
        new = [p for p in now["parts"] if p not in old["parts"] and not any(t in parts_now[p] for t in FROM_MODEL)]
        gone = [p for p in old["parts"] if p not in now["parts"]]
        if new:
            add("review", "parts added", "the record has parts the model was not made with: " + "; ".join(f"{p} ({parts_now[p][:160]})" if parts_now[p] else p for p in new))
        if gone:
            add("review", "parts removed", "the model was made with parts the record no longer has: " + "; ".join(gone))
        po = {p["id"]: p for p in old["ports"]}
        moved = [p["id"] for p in now["ports"] if p["id"] in po and (p["at"] != po[p["id"]]["at"] or p["facing"] != po[p["id"]]["facing"])]
        added = [p["id"] for p in now["ports"] if p["id"] not in po]
        if moved or added:
            add("none", "ports", f"ports {'moved: ' + ', '.join(moved) if moved else ''}{'; ' if moved and added else ''}{'added: ' + ', '.join(added) if added else ''}. No model work: the game draws the joint at each port (rule 1, 2026-10-16); a port standing off the body is the registry's to fix")
    # Motion: a moving part wants its pivot and a motion file in the package.
    pkg = os.path.join(store(), (r.get("visual") or {}).get("path", ""))
    parts_move = [x for x in now["motion"] if not str((x.get("moves") or [""])[0]).startswith("the whole")]     # (a unit travelling whole needs no pivot)
    if parts_move and not os.path.exists(os.path.join(pkg, "motion.json")):
        add("rig", "motion", "parts of it move (" + "; ".join(f"{x['id']}: {x['kind']} of {', '.join(x.get('moves') or [])}" for x in parts_move) + ") and the package has no motion.json: separate those parts, pivot them on the record's axes, install with --motion")
    have = set(m.get("nodes") or [])
    lacking = [n for n in now["nodes"] if not any(h.startswith(n) for h in have)]
    if m and lacking:
        add("rig", "nodes", "the record wants nodes the model lacks: " + ", ".join(n + "*" for n in lacking))
    if kind == "unit" and not now["envelope"] and not now["shells"]:
        add("measure", "envelope", "no envelope boxes: measure them from the installed model (model_measure.py)")
    if now["budget"] and m.get("triangles", 0) > now["budget"]:
        add("optimise", "triangles", f"{m['triangles']:,} triangles against a budget of {now['budget']:,}")
    for q in open_requests(r):
        add(q["do"], "asked", f"{q['because']} (asked {q['asked']}{' by ' + q['by'] if q.get('by') else ''}, of v{q['of_version']})")
    worst = max((x["verdict"] for x in out), key=RANK.get, default="none")
    return {"key": key, "kind": kind, "installed": (r.get("visual") or {}).get("version"), "spec_from": source, "verdict": worst, "gaps": out}


def rework():
    """Every installed model with work to do, worst first."""
    rows = [gap(kind, key, path, r) for kind, key, path, r in records() if r.get("visual") and kind not in NO_MODEL]
    rows = [g for g in rows if g["verdict"] != "none"]
    return sorted(rows, key=lambda g: (-RANK[g["verdict"]], g["kind"], g["key"]))


def main(argv):
    if "--all" in argv:
        for kind, key, path, r in records():
            if r.get("visual") and kind not in NO_MODEL:
                g = gap(kind, key, path, r)
                print(f"{g['verdict']:<9} {key}  " + "; ".join(x["what"] for x in g["gaps"]))
        return
    key = next((a for a in argv[1:] if not a.startswith("--")), None)
    hit = [(k, kk, p, r) for k, kk, p, r in records() if kk == key]
    if not hit:
        sys.exit(f"no record {key} of a kind that takes a model")
    kind, key, path, r = hit[0]
    g = gap(kind, key, path, r) if r.get("visual") else {"key": key, "kind": kind, "installed": None, "verdict": "new", "gaps": []}
    g["spec"] = spec(kind, r)
    print(json.dumps(g, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main(sys.argv)
