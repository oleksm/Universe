"""Ship sockets and validation from the registry, for Blender, Ships and anyone building a hull.

    python3 tools/standards/ship.py requirements [--json]
        Which blocks a ship must carry to fly and which are optional (ship-rule.base-blocks).
    python3 tools/standards/ship.py sockets [--class=1..4] [--kind=engine] [--hull=<key> | --dims=L,W,H] [--json]
        The mounts a socket may offer up to that size class (compact ones included), each with its envelope, what it
        bears, its holder and how many products fit it; with a hull or dims, whether its envelope fits the hull.
    python3 tools/standards/ship.py validate <hull-key> [--candidate=fit.json] [--model=export.glb] [--json]
        Runs ship-check.socket-qualification (S1-S5, F1-F4, T1-T3, H1) and the requirements: R1 every required block
        has a socket; R2 (only when the fit is not empty) every required block is filled. Exit 1 on any fault.
        --candidate replaces the record's slots/fit/thrusters; --model reads markers and nozzles from an export
        before it is installed.

Everything is read from the registry records; nothing here is a second copy of a rule.
"""
import glob, json, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ship_qualify import ROOT, load, qualify  # noqa: E402

RULE = os.path.join(ROOT, "standards/SpaceshipBuilding/metadata/rules/base-blocks.yaml")


def blocks():
    return yaml.safe_load(open(RULE))["blocks"]


def need_of(kind):
    for b in blocks():
        if kind in b["kinds"]:
            return b
    return None


def requirements(opt):
    bl = blocks()
    if opt.get("--json"):
        return print(json.dumps(bl, indent=1))
    for req in (True, False):
        print("REQUIRED (a ship must carry each to fly)" if req else "OPTIONAL")
        for b in bl:
            if b["required"] == req:
                alt = {"main_engine": " or an engine pushing along the nose", "cold_gas_engine": " or a cold-gas engine"}.get(b.get("also_by"), "")
                print(f"  {b['need']:16} socket kind {', '.join(b['kinds'])}{alt}" + (f"  - {b['note']}" if b.get("note") else ""))


def sockets(opt):
    mounts, eq, hulls = load("mounts"), load("equipment"), load("hulls")
    cls = int(opt.get("--class", 4))
    dims = None
    if opt.get("--hull"):
        ph = hulls[opt["--hull"]]["physical"]
        dims = sorted([ph["length"], ph["width"], ph["height"]])
    elif opt.get("--dims"):
        dims = sorted(float(x) for x in opt["--dims"].split(","))
    rows = []
    for key, m in sorted(mounts.items()):
        i = m["identity"]
        if i.get("size_class", 1) > cls or (opt.get("--kind") and i.get("slot") != opt["--kind"]):
            continue
        e = m.get("envelope") or {}
        ed = [e.get("length"), e.get("width"), e.get("height")]
        b = need_of(i.get("slot"))
        fits = sorted(k for k, q in eq.items() if q.get("fits") == key)
        rows.append({"mount": key, "kind": i.get("slot"), "size_class": i.get("size_class"), "compact": key.endswith("c"),
                     "required": bool(b and b["required"]), "need": (b["need"] if b else "main propulsion or attitude (an engine stands in)" if i.get("slot") == "engine" else None), "envelope_m": ed,
                     "bears": m.get("bears") or {}, "holder": m.get("holder"), "products": len(fits), "examples": fits[:4],
                     "fits_hull": None if dims is None or not all(ed) else all(a <= c * 1.0001 for a, c in zip(sorted(ed), dims))})
    if opt.get("--json"):
        return print(json.dumps(rows, indent=1))
    print(f"{'mount':26}{'kind':13}{'req':5}{'envelope m':>20}  {'bears kg':>9}  {'holder':8}{'products':>9}" + ("  fits hull" if dims else ""))
    for r in rows:
        ed = "x".join(f"{x:g}" for x in r["envelope_m"] if x)
        print(f"{r['mount']:26}{r['kind']:13}{('yes' if r['required'] else 'alt' if r['kind'] == 'engine' else '-'):5}{ed:>20}  {r['bears'].get('mass', 0):>9g}  {str(r['holder'] or '-'):8}{r['products']:>9}"
              + (f"  {'yes' if r['fits_hull'] else 'NO'}" if dims else ""))


def validate(key, opt):
    hulls, mounts, eq = load("hulls"), load("mounts"), load("equipment")
    if key not in hulls:
        sys.exit(f"ship.py: no hull {key}")
    h = dict(hulls[key])
    if opt.get("--candidate"):
        h.update(json.load(open(opt["--candidate"])))
    model = os.path.abspath(opt["--model"]) if opt.get("--model") else None
    faults, unchecked = qualify(h, mounts, eq, model=model)
    slots = {s["name"]: s for s in h.get("slots") or []}
    kinds = {s["kind"] for s in slots.values()}
    fitted = {}
    for f in h.get("fit") or []:
        q, s = eq.get(f["item"]), slots.get(f["slot"])
        if q and s and (q.get("function") or {}).get("kind") != "inert":
            fitted.setdefault(s["kind"], []).append(q)
    for b in blocks():
        if not b["required"]:
            continue
        offered = any(k in kinds for k in b["kinds"]) or (b.get("also_by") and "engine" in kinds)
        if not offered:
            faults.append(("R1", f"no socket for {b['need']} ({', '.join(b['kinds'])})"))
        if h.get("fit"):
            have = any(k in fitted for k in b["kinds"])
            if not have and b.get("also_by"):
                fed = b["also_by"] == "cold_gas_engine"
                have = any((q.get("function") or {}).get("cycle") == "cold_gas" if fed else (q.get("function") or {}).get("cycle") != "cold_gas"
                           for q in fitted.get("engine", []))
            if not have:
                faults.append(("R2", f"{b['need']} not fitted: the ship cannot be commissioned"))
    if opt.get("--json"):
        return print(json.dumps({"hull": key, "pass": not faults, "faults": [{"criterion": c, "message": m} for c, m in faults],
                                 "unchecked": [{"criterion": c, "message": m} for c, m in unchecked]}, indent=1)) or faults
    print(f"{key}: {len(slots)} sockets, {len(h.get('fit') or [])} fitted - {'PASS' if not faults else f'{len(faults)} fault(s)'}")
    by = {}
    for c, m in faults:
        by.setdefault(c, []).append(m)
    for c, ms in sorted(by.items()):
        print(f"  {c} x{len(ms)}: " + "; ".join(ms[:4]) + (" ..." if len(ms) > 4 else ""))
    for c, m in unchecked:
        print(f"  unchecked {c}: {m}")
    return faults


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        sys.exit(__doc__)
    cmd, rest = argv[0], argv[1:]
    opt = {a.split("=", 1)[0]: (a.split("=", 1)[1] if "=" in a else True) for a in rest if a.startswith("--")}
    args = [a for a in rest if not a.startswith("--")]
    if cmd == "requirements":
        requirements(opt)
    elif cmd == "sockets":
        sockets(opt)
        print("req: yes = a required block; alt = an engine socket, which stands in for main propulsion or attitude") if not opt.get("--json") else None
    elif cmd == "validate" and args:
        sys.exit(1 if validate(args[0], opt) else 0)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
