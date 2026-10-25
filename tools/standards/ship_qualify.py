"""Socket and fit qualification for any hull record: the criteria of ship-check.socket-qualification, run.

    python3 tools/standards/ship_qualify.py                     # every hull record
    python3 tools/standards/ship_qualify.py hull.explorer-study # one hull
    python3 tools/standards/ship_qualify.py hull.x --candidate=fit.json   # slots/fit/thrusters to replace, before adoption
    python3 tools/standards/ship_qualify.py --check             # exit 1 if a hull not marked outdated has a fault

The fit rules (F3) are the game's (crates/core/world/src/ship.rs mount_fit) so the registry and the game judge alike;
the hull rules (S3, S4, S5, F4, T2) are what the game does not check yet. A criterion that cannot be judged (no model
to read) is reported as unchecked, never as a pass.
"""
import glob, hashlib, json, os, struct, sys
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
M = os.path.join(ROOT, "standards/SFO/metadata")
GAME = os.path.expanduser("~/git/universe")


def load(folder):
    out = {}
    for p in glob.glob(os.path.join(M, folder, "*.yaml")):
        r = yaml.safe_load(open(p))
        out[r["identity"]["key"]] = r
    return out


def _nodes(p):
    b = open(p, "rb").read()
    n = struct.unpack("<I", b[12:16])[0]
    return {x.get("name", "") for x in json.loads(b[20:20 + n]).get("nodes", [])}


def glb_nodes(rel, sha=None):
    """Node names of a model: an absolute path, or the copy under fso/main whose sha256 is the record's pin."""
    if os.path.isabs(rel):
        return _nodes(rel) if os.path.isfile(rel) else None
    found = [p for p in (os.path.join(b, rel) for b in (ROOT, GAME)) if os.path.isfile(p)]
    if sha:
        found = [p for p in found if hashlib.sha256(open(p, "rb").read()).hexdigest() == sha]
    return _nodes(found[0]) if found else None


def dims(phys):
    return [phys.get("length"), phys.get("width"), phys.get("height")]


def qualify(hull, mounts, equipment, model=None):
    """Faults and unchecked notes for one hull record: (criterion, message)."""
    faults, unchecked = [], []
    f = lambda c, m: faults.append((c, m))
    phys = hull.get("physical") or {}
    hd = dims(phys)
    hull_ok = all(isinstance(x, (int, float)) and x > 0 for x in hd)
    has_parts = bool((hull.get("built_of") or {}).get("parts")) and os.path.isdir(os.path.join(M, "parts", hull["built_of"]["parts"]))
    if not (isinstance(phys.get("mass"), (int, float)) and phys["mass"] > 0) and not has_parts:
        f("H1", "no whole mass: neither physical.mass nor a parts folder (the game takes Recorded or Parts)")
    slots = {}
    for s in hull.get("slots") or []:
        if s["name"] in slots:
            f("S1", f"slot {s['name']} declared twice")
        slots[s["name"]] = s
    nodes = glb_nodes(model) if model else (glb_nodes(hull["model"], hull.get("model_sha256")) if hull.get("model") else None)
    if nodes is None and (slots or hull.get("thrusters")):
        why = "no copy of the model matches model_sha256" if hull.get("model_sha256") and not model else "no model to read"
        unchecked.append(("S5/T2", f"{why}: markers and nozzles not checked"))
    room = 0.0
    for s in slots.values():
        mk = s.get("mount")
        if not mk:
            continue
        m = mounts.get(mk)
        if not m:
            f("S2", f"slot {s['name']}: no mount {mk}")
            continue
        mi = m["identity"]
        if mi.get("slot") != s["kind"] or mi.get("size_class", 1) > s["size"]:
            f("S2", f"slot {s['name']} ({s['kind']} {s['size']}) offers {mk} ({mi.get('slot')} {mi.get('size_class')})")
        e = m.get("envelope") or {}
        ed = [e.get("length"), e.get("width"), e.get("height")]
        if hull_ok and all(ed) and any(a > b * 1.0001 for a, b in zip(sorted(ed), sorted(hd))):
            f("S3", f"slot {s['name']}: {mk} envelope {ed} m does not fit the hull {[round(x, 2) for x in hd]} m")
        if all(ed):
            room += ed[0] * ed[1] * ed[2]
        if nodes is not None and f"mount_{s['name']}" not in nodes:
            f("S5", f"slot {s['name']}: model has no node mount_{s['name']}")
    if hull_ok and room > hd[0] * hd[1] * hd[2]:
        f("S4", f"slot mount envelopes {room:.1f} m3 exceed the hull's box {hd[0] * hd[1] * hd[2]:.1f} m3")
    used = set()
    for fit in hull.get("fit") or []:
        sn, key = fit["slot"], fit["item"]
        if sn not in slots:
            f("F1", f"fit {key} names no slot {sn}")
            continue
        if sn in used:
            f("F1", f"slot {sn} fitted twice")
        used.add(sn)
        q = equipment.get(key)
        if not q:
            f("F2", f"slot {sn}: no equipment {key}")
            continue
        s = slots[sn]
        qd = dims(q.get("physical") or {})
        if hull_ok and all(qd) and any(a > b * 1.0001 for a, b in zip(sorted(qd), sorted(hd))):
            f("F4", f"slot {sn}: {key} {qd} m does not fit the hull")
        mk = s.get("mount")
        if not mk:
            if q.get("size_class", 1) > s["size"]:
                f("F3", f"slot {sn}: {key} (size {q.get('size_class')}) too big for size {s['size']}")
            continue
        m = mounts.get(mk)
        if not m:
            continue
        fm = q.get("fits")
        if not fm:
            f("F3", f"slot {sn}: {key} names no mount (slot offers {mk})")
            continue
        fr = mounts.get(fm)
        if not fr:
            f("F3", f"slot {sn}: {key} fits unknown {fm}")
            continue
        if fr["identity"].get("slot") != m["identity"].get("slot") or fr["identity"].get("size_class", 1) > m["identity"].get("size_class", 1):
            f("F3", f"slot {sn}: {key} is built to {fm}; slot offers {mk}")
            continue
        e = m.get("envelope") or {}
        for name, v, most in zip(("length", "width", "height"), qd, (e.get("length"), e.get("width"), e.get("height"))):
            if v and most and v > most * 1.0001:
                f("F3", f"slot {sn}: {key} {name} {v} past its mount's {most}")
        bears, feeds, fn = m.get("bears") or {}, m.get("feeds") or {}, q.get("function") or {}
        if bears.get("mass") and (q.get("physical") or {}).get("mass", 0) > bears["mass"] * 1.0001:
            f("F3", f"slot {sn}: {key} mass past what {mk} bears")
        if feeds.get("power") and fn.get("kind") != "power_plant" and (q.get("needs") or {}).get("power", 0) > feeds["power"] * 1.0001:
            f("F3", f"slot {sn}: {key} draws more power than {mk} feeds")
        if bears.get("thrust") and fn.get("kind") == "engine" and fn.get("thrust", 0) > bears["thrust"] * 1.0001:
            f("F3", f"slot {sn}: {key} thrust past what {mk} bears")
    share, seen = {}, set()
    for t in hull.get("thrusters") or []:      # as the game's import.rs bind_engines
        if t["slot"] not in slots:
            f("T1", f"thruster {t['nozzle']} names no slot {t['slot']}")
            continue
        if nodes is not None and t["nozzle"] not in nodes:
            f("T2", f"model has no nozzle node {t['nozzle']}")
        if slots[t["slot"]]["kind"] != "engine":
            continue                         # other kinds bind by the game's name rule
        if not 0 < t["share"] <= 1:
            f("T3", f"thruster {t['nozzle']}: share {t['share']} not over 0 and up to 1")
        if t["nozzle"] in seen:
            f("T3", f"thruster {t['nozzle']} bound twice")
        seen.add(t["nozzle"])
        share[t["slot"]] = share.get(t["slot"], 0) + t["share"]
    for sn, total in share.items():
        if abs(total - 1) > 1e-9:
            f("T3", f"engine slot {sn}: nozzle shares sum to {total:.3f}, not 1")
    return faults, unchecked


def main(argv):
    opt = {a.split("=", 1)[0]: (a.split("=", 1)[1] if "=" in a else True) for a in argv if a.startswith("--")}
    keys = [a for a in argv if not a.startswith("--")]
    hulls, mounts, equipment = load("hulls"), load("mounts"), load("equipment")
    bad = 0
    for key in keys or sorted(hulls):
        if key not in hulls:
            sys.exit(f"ship_qualify: no hull {key}")
        h = dict(hulls[key])
        if opt.get("--candidate"):
            h.update(json.load(open(opt["--candidate"])))
        faults, unchecked = qualify(h, mounts, equipment)
        outdated = h["identity"].get("revision") == "outdated"
        counted = bool(faults) and not outdated
        bad += counted
        tag = "outdated, not gated" if outdated else ("FAIL" if faults else "ok")
        print(f"{key}: {len(h.get('slots') or [])} slots, {len(h.get('fit') or [])} fitted, {len(faults)} fault(s) [{tag}]")
        by = {}
        for c, msg in faults:
            by.setdefault(c, []).append(msg)
        for c, msgs in sorted(by.items()):
            print(f"  {c} x{len(msgs)}: " + "; ".join(msgs[:3]) + (" ..." if len(msgs) > 3 else ""))
        for c, msg in unchecked:
            print(f"  unchecked {c}: {msg}")
    print(f"ship qualification: {len(keys or hulls)} hull(s); {bad} hull(s) with fault(s)")
    if opt.get("--check") and bad:
        sys.exit(1)


if __name__ == "__main__":
    main(sys.argv[1:])
