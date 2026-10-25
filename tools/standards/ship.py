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

    python3 tools/standards/ship.py spec <hull>:<slot> [--json]
        The slot's spec: every parameter a product fitted there must meet (min, max, one of, direction), each with
        where it comes from (slot kind and size, its mount's envelope/bears/feeds/spec, the slot's own spec).
    python3 tools/standards/ship.py modules (--mount=<key> | --socket=<hull>:<slot> | --kind=<kind> [--class=N]) [--all] [--json]
        The products that fit: key, size, mass, power, thrust, mount, revision, whether an installed model exists.
        --socket judges each against the slot's whole spec; --all also lists the rejected ones and why.
        --installed: only products with an installed model; --models: each product's model.glb path (to open
        and review its look); --json rows carry it as glb.
    python3 tools/standards/ship.py shipyard <hull> [--candidate=fit.json] [--json]
        The ship as fitted, from its fitted products only: each fitted product against its slot's spec, every
        required block filled, and the hull's spec (ship.lift_to_weight, power_margin, attitude_axes, ...). Exit 1
        on any fault.
    python3 tools/standards/ship.py product <equipment-key> [--json]
        One product's sheet: physical, needs, function, fits mount (envelope, holder), ports (connectors and pins),
        model package path and manifest sha, revision and confidence.
    python3 tools/standards/ship.py get <any-key> [--json]       any record: a material, cable/wire run, connector,
        interface, mount, rule, stock... (prints the record and its file)
    python3 tools/standards/ship.py find <words...>             keys and names containing all the words
    python3 tools/standards/ship.py classify <hull-key> [--candidate=json] [--json]
        Role class, socket size classes and mounts, each required block offered/filled, commissioned or not, and why.
    python3 tools/standards/ship.py decisions [<words...>]      the registry's design decisions (docs/ships/*.yaml):
        file, status line; read the file for the choice
    python3 tools/standards/ship.py missing "<what>" [--for=<key>]
        Something is not in the registry: prints the one message to send the registry to have it made.

Everything is read from the registry records; nothing here is a second copy of a rule. When a lookup finds nothing it
says so and prints the missing command: the registry is asked only for what does not exist.
"""
import glob, json, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ship_qualify import ROOT, load, qualify  # noqa: E402
import slot_spec  # noqa: E402

RULE = os.path.join(ROOT, "standards/SpaceshipBuilding/metadata/rules/base-blocks.yaml")


def blocks():
    return yaml.safe_load(open(RULE))["blocks"]


ROLE = {"main_engine": "main", "cold_gas_engine": "attitude", "lift_engine": "lift"}


def stands_in(slot, b, q=None):
    """An engine slot standing in for block b: by its role; a slot with no role (old hulls) by the fitted engine's cycle."""
    if slot["kind"] != "engine" or not b.get("also_by"):
        return False
    if slot.get("role"):
        return slot["role"] == ROLE[b["also_by"]]
    if b["also_by"] == "lift_engine":
        return False
    if q is None:
        return True
    cold = (q.get("function") or {}).get("cycle") == "cold_gas"
    return cold == (b["also_by"] == "cold_gas_engine")


def offered(slots, b):
    return any(s["kind"] in b["kinds"] or stands_in(s, b) for s in slots)


def filled(pairs, b):
    return any(s["kind"] in b["kinds"] or stands_in(s, b, q) for s, q in pairs)


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
                alt = {"main_engine": " or an engine slot of role main", "cold_gas_engine": " or an engine slot of role attitude (cold gas)", "lift_engine": " or an engine slot of role lift"}.get(b.get("also_by"), "")
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
    pairs = []
    for f in h.get("fit") or []:
        q, s = eq.get(f["item"]), slots.get(f["slot"])
        if q and s and (q.get("function") or {}).get("kind") != "inert":
            pairs.append((s, q))
    for b in blocks():
        if not b["required"]:
            continue
        if not offered(slots.values(), b):
            faults.append(("R1", f"no socket for {b['need']} ({', '.join(b['kinds'])})"))
        if h.get("fit") and not filled(pairs, b):
            faults.append(("R2", f"{b['need']} not fitted: the ship cannot be commissioned"))
    if opt.get("--quiet"):
        return faults
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



def _index():
    """Every record by key; cached, rebuilt when any record file is newer than the cache."""
    import pickle
    tree = os.path.join(ROOT, "standards")
    cache = os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "freefall-registry-index.pkl")
    newest = max(os.path.getmtime(p) for p in glob.glob(os.path.join(tree, "**", "*.yaml"), recursive=True))
    try:
        stamp, idx = pickle.load(open(cache, "rb"))
        if stamp >= newest:
            return idx
    except Exception:
        pass
    from reg import records
    idx = {k: (f, d) for k, _, f, d in records()}
    os.makedirs(os.path.dirname(cache), exist_ok=True)
    pickle.dump((newest, idx), open(cache, "wb"))
    return idx


def _hint(what, key=None):
    return ("not in the registry. Ask for it once:\n  env AGENTMSG_NAME=blender agentmsg send registry task --summary "
            f"\"missing: {what}\"" + (f" --ref {key}" if key else ""))


def _model(rec):
    v = rec.get("visual") or {}
    path = v.get("path")
    store = os.environ.get("UNIVERSE_ASSETS") or os.path.expanduser("~/git/freefall-assets")
    glb = os.path.join(store, path, "model.glb") if path else None
    ok = bool(glb and os.path.isfile(glb))
    return {"path": path, "manifest_sha256": v.get("manifest_sha256"), "installed": ok, "glb": glb if ok else None}


def modules(opt):
    mounts, eq, hulls = load("mounts"), load("equipment"), load("hulls")
    target = None
    if opt.get("--socket"):
        hk, sn = opt["--socket"].split(":", 1)
        s = next((x for x in hulls[hk].get("slots") or [] if x["name"] == sn), None)
        if not s:
            sys.exit(f"no socket {sn} on {hk}")
        target = s.get("mount")
        kind, cls = s["kind"], s["size"]
    elif opt.get("--mount"):
        target = opt["--mount"]
        if target not in mounts:
            sys.exit(_hint(f"mount {target}", target))
        kind, cls = mounts[target]["identity"]["slot"], mounts[target]["identity"]["size_class"]
    else:
        kind, cls = opt.get("--kind"), int(opt.get("--class", 4))
    rows, rejected, spec_lims, frame = [], [], None, None
    if opt.get("--socket"):
        spec_lims = slot_spec.limits(s, mounts)
        hf = slot_spec.hull_frames(hulls[hk])
        frame = hf.get(f"mount_{sn}") if hf else None
    for k, q in sorted(eq.items()):
        if q["identity"].get("slot") != kind or (q.get("size_class") or 1) > cls:
            continue
        if spec_lims is not None:
            res = slot_spec.judge(q, spec_lims, frame)
            bad = [f"{L['of']}: {why}" for L, _, ok, why in res if ok is False]
            if bad:
                rejected.append((k, bad))
                continue
        elif target:   # the game's own rule, through the qualification code
            probe = {"identity": {"key": "probe"}, "physical": {"mass": 1, "length": 1e3, "width": 1e3, "height": 1e3},
                     "slots": [{"name": "s", "kind": kind, "size": cls, "mount": target}], "fit": [{"slot": "s", "item": k}]}
            if any(c == "F3" for c, _ in qualify(probe, mounts, eq)[0]):
                continue
        f, ph, n = q.get("function") or {}, q.get("physical") or {}, q.get("needs") or {}
        rows.append({"key": k, "size_class": q.get("size_class"), "mass_kg": ph.get("mass"),
                     "dims_m": [ph.get("length"), ph.get("width"), ph.get("height")], "power_W": n.get("power"),
                     "function": f.get("kind"), "thrust_N": f.get("thrust"), "fits": q.get("fits"),
                     "revision": q["identity"].get("revision"), "model": _model(q)["installed"], "glb": _model(q)["glb"]})
    if opt.get("--installed"):
        rows = [r for r in rows if r["model"]]
    if opt.get("--json"):
        return print(json.dumps({"fit": rows, "rejected": [{"key": k, "why": bad} for k, bad in rejected]} if opt.get("--all") else rows, indent=1))
    if opt.get("--models"):
        for r in rows:
            print(f"{r['key']:44}{r['glb'] or 'no model installed'}")
        print(f"(models from {os.environ.get('UNIVERSE_ASSETS') or '~/git/freefall-assets'}; dims, mass and ports: product <key>)") if rows else None
    else:
        print(f"{'product':44}{'mass kg':>9}{'power W':>9}  {'function':16}{'mount':26}{'rev':10}model")
        for r in rows:
            print(f"{r['key']:44}{r['mass_kg'] or 0:>9g}{r['power_W'] or 0:>9g}  {str(r['function']):16}{str(r['fits']):26}{str(r['revision']):10}{'yes' if r['model'] else 'NO'}")
    if opt.get("--all"):
        for k, bad in rejected:
            print(f"  no  {k:40} " + "; ".join(bad))
    if not rows:
        print(_hint(f"a {kind} product of class {cls}" + (f" for {target}" if target else "")))


def spec(ref, opt):
    hulls, mounts = load("hulls"), load("mounts")
    hk, sn = ref.split(":", 1)
    s = next((x for x in (hulls.get(hk) or {}).get("slots") or [] if x["name"] == sn), None)
    if not s:
        sys.exit(f"no socket {sn} on {hk}")
    lims = slot_spec.limits(s, mounts)
    if opt.get("--json"):
        return print(json.dumps({"hull": hk, "slot": sn, "kind": s["kind"], "role": s.get("role"), "limits": lims}, indent=1))
    print(f"{hk}:{sn}  {s['kind']} size {s['size']}" + (f", role {s['role']}" if s.get("role") else "") + f", mount {s.get('mount')}")
    for L in lims:
        cond = ", ".join(f"{k} {L[k]}" for k in ("min", "max", "one_of", "present", "axis", "within", "gravity") if k in L)
        print(f"  {L['of']:18} {cond[:90]:92} {L['from']}" + (f"  ({L['note']})" if L.get("note") else ""))


def shipyard(key, opt):
    hulls, mounts, eq = load("hulls"), load("mounts"), load("equipment")
    if key not in hulls:
        sys.exit(f"ship.py: no hull {key}")
    h = dict(hulls[key])
    if opt.get("--candidate"):
        h.update(json.load(open(opt["--candidate"])))
    slots = {x["name"]: x for x in h.get("slots") or []}
    hf = slot_spec.hull_frames(h)
    faults, notes, fit = [], [], h.get("fit") or []
    for f in fit:
        s, q = slots.get(f["slot"]), eq.get(f["item"])
        if not s or not q:
            faults.append(("Y1", f"fit {f['item']} in {f['slot']}: no such " + ("slot" if not s else "product")))
            continue
        for L, v, ok, why in slot_spec.judge(q, slot_spec.limits(s, mounts), hf.get(f"mount_{s['name']}") if hf else None):
            if ok is False:
                faults.append(("Y1", f"{f['slot']} <- {f['item']}: {L['of']} {why} [{L['from']}]"))
            elif ok is None:
                notes.append(f"{f['slot']}: {L['of']} unchecked ({why})")
    pairs = []
    for f in fit:
        q, s = eq.get(f["item"]), slots.get(f["slot"])
        if q and s and (q.get("function") or {}).get("kind") != "inert":
            pairs.append((s, q))
    for b in blocks():
        if b["required"] and not filled(pairs, b):
            faults.append(("Y2", f"{b['need']} not fitted"))
    vals = slot_spec.ship_values(h, slots, fit, eq, hf)
    for L, v, ok, why in slot_spec.judge_ship(h, vals):
        if ok is False:
            faults.append(("Y3", f"{L['of']} {why}" + (f" at {L['gravity']} m/s2" if L.get("gravity") else "")))
    if hf is None and fit:
        notes.append("no hull model matching model_sha256: nozzle placement unchecked")
    shown = {k: (v(9.80665) if callable(v) else v) for k, v in vals.items()}
    if opt.get("--quiet"):
        return faults
    if opt.get("--json"):
        print(json.dumps({"hull": key, "pass": not faults, "faults": [{"criterion": c, "message": m} for c, m in faults],
                          "ship": {k: (v if isinstance(v, list) else round(float(v), 4)) for k, v in shown.items()}, "unchecked": notes}, indent=1))
        return faults
    print(f"{key}: {len(fit)} fitted of {len(slots)} slots - {'PASS' if not faults else f'{len(faults)} fault(s)'}")
    print("  ship (tanks full): " + ", ".join(f"{k[5:]} {v}" if isinstance(v, list) else f"{k[5:]} {float(v):.4g}" for k, v in shown.items()) + " (to_weight at 1 g here; the spec's own gravity in its check)")
    for c, m in faults:
        print(f"  {c} {m}")
    for n in notes[:10]:
        print(f"  unchecked {n}")
    return faults


def product(key, opt):
    eq, mounts = load("equipment"), load("mounts")
    q = eq.get(key)
    if not q:
        sys.exit(_hint(f"equipment {key}", key))
    m = mounts.get(q.get("fits") or "", {})
    sheet = {"key": key, "name": q["identity"].get("name"), "revision": q["identity"].get("revision"),
             "slot_kind": q["identity"].get("slot"), "size_class": q.get("size_class"), "physical": q.get("physical"),
             "needs": q.get("needs"), "function": q.get("function"),
             "fits": {"mount": q.get("fits"), "envelope": m.get("envelope"), "bears": m.get("bears"), "holder": m.get("holder")},
             "ports": q.get("ports") or [], "internal": q.get("internal") or [], "model": _model(q),
             "confidence": [{"of": b.get("of"), "tier": b.get("tier") or b.get("rule"), "review": b.get("review", False)} for b in q.get("basis") or []]}
    if opt.get("--json"):
        return print(json.dumps(sheet, indent=1, default=str))
    print(f"{key}  {sheet['name']}  [{sheet['revision']}]  {sheet['slot_kind']} class {sheet['size_class']}")
    ph = sheet["physical"] or {}
    print(f"  physical: {ph.get('mass')} kg, {ph.get('length')} x {ph.get('width')} x {ph.get('height')} m;  needs: {sheet['needs']}")
    print(f"  function: {sheet['function']}")
    fm = sheet["fits"]
    print(f"  fits {fm['mount']}: envelope {fm['envelope']}, bears {fm['bears']}, holder {fm['holder']}")
    print(f"  model: {sheet['model']}")
    for pt in sheet["ports"]:
        pins = ", ".join(f"{x['contact']}={x['role']}" for x in pt.get("pins") or [])
        print(f"  port {pt['id']}: {pt.get('medium')} {pt.get('flow')} {pt.get('interface')} {pt.get('half')}"
              + (f" {pt.get('voltage')} V {pt.get('current')} A" if pt.get("voltage") else "") + (f" pins [{pins}]" if pins else "")
              + f" at {pt.get('at')} facing {pt.get('facing')}")
    for c in sheet["confidence"]:
        print(f"  basis {c['of']}: {c['tier']}" + ("  (to review)" if c["review"] else ""))


def get(key, opt):
    idx = _index()
    if key not in idx:
        sys.exit(_hint(key, key))
    f, d = idx[key]
    if opt.get("--json"):
        return print(json.dumps(d, indent=1, default=str))
    print(f"# {os.path.relpath(f, ROOT)}")
    print(open(f, encoding="utf-8").read())


def find(words):
    ws = [w.lower() for w in words]
    hits = [(k, (d.get("identity") or {}).get("name", "")) for k, (f, d) in sorted(_index().items())
            if all(w in f"{k} {(d.get('identity') or {}).get('name', '')}".lower() for w in ws)]
    for k, n in hits:
        print(f"{k:<55} {n}")
    if not hits:
        print(_hint(" ".join(words)))


def classify(key, opt):
    hulls, mounts, eq = load("hulls"), load("mounts"), load("equipment")
    h = dict(hulls[key])
    if opt.get("--candidate"):
        h.update(json.load(open(opt["--candidate"])))
    slots = h.get("slots") or []
    faults = validate(key, dict(opt, **{"--quiet": True}))
    by = {s["name"]: s for s in slots}
    pairs = [(by[f["slot"]], eq[f["item"]]) for f in h.get("fit") or [] if f["slot"] in by and f["item"] in eq]
    out = {"hull": key, "role_class": h["identity"].get("class"), "revision": h["identity"].get("revision"),
           "socket_size_classes": sorted({s["size"] for s in slots}), "mounts": sorted({s.get("mount") for s in slots if s.get("mount")}),
           "blocks": [{"need": b["need"], "required": b["required"],
                       "offered": offered(slots, b), "filled": filled(pairs, b)} for b in blocks()],
           "faults": [{"criterion": c, "message": m} for c, m in faults]}
    req = [b for b in out["blocks"] if b["required"]]
    out["deliverable"] = ("hull and sockets ready" if all(b["offered"] for b in req) and not faults and not h.get("fit")
                          else "commissionable" if all(b["filled"] for b in req) and not faults else "not ready")
    if opt.get("--json"):
        return print(json.dumps(out, indent=1))
    print(f"{key}: {out['role_class']} [{out['revision']}]  sockets {len(slots)}, size classes {out['socket_size_classes']} -> {out['deliverable']}")
    for b in out["blocks"]:
        if b["required"] or b["offered"]:
            print(f"  {'REQ' if b['required'] else 'opt'} {b['need']:16} socket {'yes' if b['offered'] else 'NO '}  filled {'yes' if b['filled'] else 'no'}")
    for f in out["faults"][:8]:
        print(f"  fault {f['criterion']}: {f['message']}")


def decisions(words):
    ws = [w.lower() for w in words]
    for f in sorted(glob.glob(os.path.join(ROOT, "docs/ships/*.yaml"))):
        name = os.path.basename(f)
        try:
            d = yaml.safe_load(open(f)) or {}
        except Exception:
            continue
        status = str(d.get("withdrawn") and "WITHDRAWN - " or "") + str(d.get("status", ""))
        if all(w in (name + " " + status).lower() for w in ws):
            print(f"{os.path.relpath(f, ROOT):70} {status[:110]}")


def missing(what, opt):
    print(_hint(what, opt.get("--for")))

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
    elif cmd == "spec" and args:
        spec(args[0], opt)
    elif cmd == "shipyard" and args:
        sys.exit(1 if shipyard(args[0], opt) else 0)
    elif cmd == "modules":
        modules(opt)
    elif cmd == "product" and args:
        product(args[0], opt)
    elif cmd == "get" and args:
        get(args[0], opt)
    elif cmd == "find" and args:
        find(args)
    elif cmd == "classify" and args:
        classify(args[0], opt)
    elif cmd == "decisions":
        decisions(args)
    elif cmd == "missing" and args:
        missing(" ".join(args), opt)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
