"""A slot is a spec: every parameter a product fitted there must meet, and the ship as fitted against the hull's spec.

A slot's spec is its mount's limits (kind, size class, envelope, bears, feeds) and the mount's and slot's own `spec`
limits (common.schema.yaml spec_limit). `limits` lists them with where each comes from; `judge` checks a product
against them; `ship_values` and `shipyard` judge the ship from its fitted products only. Used by ship.py spec,
modules --socket and shipyard.
"""
import json, math, os, struct
import numpy as np

from ship_qualify import GAME, ROOT

ASSETS = os.environ.get("UNIVERSE_ASSETS") or os.path.expanduser("~/git/freefall-assets")


def limits(slot, mounts, hull=None):
    """Every limit of one slot, each {of, ..., from}."""
    out = [{"of": "identity.slot", "one_of": [slot["kind"]], "from": "slot kind"},
           {"of": "size_class", "max": slot["size"], "from": "slot size"}]
    m = mounts.get(slot.get("mount")) if slot.get("mount") else None
    if m:
        mk = m["identity"]["key"]
        out.append({"of": "fits", "one_of": sorted(k for k, x in mounts.items() if x["identity"]["slot"] == m["identity"]["slot"]
                                                     and x["identity"]["size_class"] <= m["identity"]["size_class"]),
                    "from": f"{mk}: a mount of its kind, no bigger class"})
        e = m.get("envelope") or {}
        for a in ("length", "width", "height"):
            if e.get(a):
                out.append({"of": f"physical.{a}", "max": e[a], "from": f"{mk} envelope"})
        b, f = m.get("bears") or {}, m.get("feeds") or {}
        if b.get("mass"):
            out.append({"of": "physical.mass", "max": b["mass"], "from": f"{mk} bears"})
        if b.get("thrust") and slot["kind"] == "engine":
            out.append({"of": "function.thrust", "max": b["thrust"], "from": f"{mk} bears"})
        if b.get("recoil"):
            out.append({"of": "function.recoil", "max": b["recoil"], "from": f"{mk} bears"})
        if f.get("power") and slot["kind"] != "power":
            out.append({"of": "needs.power", "max": f["power"], "from": f"{mk} feeds"})
        out += [dict(x, **{"from": f"{mk} spec"}) for x in m.get("spec") or []]
    out += [dict(x, **{"from": f"slot {slot['name']} spec"}) for x in slot.get("spec") or []]
    return out


def _glb(path):
    b = open(path, "rb").read()
    n = struct.unpack("<I", b[12:16])[0]
    return json.loads(b[20:20 + n])["nodes"]


def _local(x):
    if "matrix" in x:
        return np.array(x["matrix"], float).reshape(4, 4).T
    m = np.eye(4)
    if "rotation" in x:
        qx, qy, qz, qw = x["rotation"]
        m[:3, :3] = [[1 - 2 * (qy * qy + qz * qz), 2 * (qx * qy - qz * qw), 2 * (qx * qz + qy * qw)],
                     [2 * (qx * qy + qz * qw), 1 - 2 * (qx * qx + qz * qz), 2 * (qy * qz - qx * qw)],
                     [2 * (qx * qz - qy * qw), 2 * (qy * qz + qx * qw), 1 - 2 * (qx * qx + qy * qy)]]
    if "scale" in x:
        m[:3, :3] = m[:3, :3] @ np.diag(x["scale"])
    if "translation" in x:
        m[:3, 3] = x["translation"]
    return m


def frames(path):
    """Every named node's world matrix in a GLB."""
    nodes = _glb(path)
    parent = {c: i for i, x in enumerate(nodes) for c in x.get("children", [])}
    def world(i):
        return world(parent[i]) @ _local(nodes[i]) if i in parent else _local(nodes[i])
    return {x.get("name", ""): world(i) for i, x in enumerate(nodes)}


def hull_frames(hull):
    from ship_qualify import glb_nodes  # noqa: F401  (same model choice: the copy model_sha256 pins)
    import hashlib
    rel, sha = hull.get("model"), hull.get("model_sha256")
    for base in (ROOT, GAME):
        p = os.path.join(base, rel or "")
        if rel and os.path.isfile(p) and (not sha or hashlib.sha256(open(p, "rb").read()).hexdigest() == sha):
            return frames(p)
    return None


def product_frames(q):
    path = (q.get("visual") or {}).get("path")
    p = os.path.join(ASSETS, path or "", "model.glb")
    return frames(p) if path and os.path.isfile(p) else None


def nozzles(q, slot_frame):
    """Each nozzle of a product fitted at slot_frame: (node, share, at, push) in the hull's frame, or None (no model)."""
    pf = product_frames(q)
    if pf is None:
        return None
    out = []
    for n in q.get("nozzles") or []:
        m = pf.get(n["node"])
        if m is None:
            out.append((n["node"], n["share"], None, None))
            continue
        w = slot_frame @ m
        ex = w[:3, :3] @ np.array([0, 0, -1.0])
        out.append((n["node"], n["share"], w[:3, 3], -ex / np.linalg.norm(ex)))
    return out


def _get(rec, path):
    v = rec
    for p in path.split("."):
        v = v.get(p) if isinstance(v, dict) else None
    return v


def judge(q, lims, slot_frame=None):
    """Each limit against a product: [(limit, value, ok, why)]; ok None where it cannot be judged."""
    res = []
    for L in lims:
        of = L["of"]
        if of == "push":
            ns = nozzles(q, slot_frame) if slot_frame is not None else None
            if ns is None:
                res.append((L, None, None, "no slot frame or product model to place nozzles"))
                continue
            ax = np.array(L["axis"], float) / np.linalg.norm(L["axis"])
            worst = None
            for node, _, _, push in ns:
                if push is None:
                    res.append((L, node, False, f"nozzle node {node} not in the product's model"))
                    continue
                ang = math.degrees(math.acos(max(-1.0, min(1.0, float(push @ ax)))))
                worst = ang if worst is None else max(worst, ang)
            if worst is not None:
                ok = worst <= L.get("within", 0) + 1e-6
                res.append((L, round(worst, 1), ok, f"pushes {worst:.0f} deg from {L['axis']}" + ("" if ok else f", more than {L.get('within')}")))
            elif not ns:
                res.append((L, None, False, "no nozzles"))
            continue
        v = _get(q, of)
        n = len(v) if isinstance(v, list) else v
        ok, why = True, []
        if "present" in L and (v is not None) != L["present"]:
            ok, why = False, why + ["must " + ("" if L["present"] else "not ") + "be stated"]
        if "one_of" in L and v not in L["one_of"]:
            ok, why = False, why + [f"{v!r} not one of {L['one_of']}"]
        for k, cmp, word in (("min", lambda a, b: a >= b * (1 - 1e-4), "under"), ("max", lambda a, b: a <= b * (1 + 1e-4), "over")):
            if k in L:
                if n is None:
                    if k == "min":
                        ok, why = False, why + [f"not stated (min {L[k]})"]
                elif not cmp(n, L[k]):
                    ok, why = False, why + [f"{n:g} {word} {L[k]:g}"]
        res.append((L, v if not isinstance(v, list) else len(v), ok, "; ".join(why)))
    return res


def ship_values(hull, slots, fit, eq, hf):
    """The ship as fitted: mass, thrust by role, to_weight, power margin, attitude senses, unfed engines."""
    mass = float((hull.get("physical") or {}).get("mass") or 0)
    pts = [(mass, np.zeros(3))]
    supply = draw = 0.0
    held, engines, jets = set(), [], []
    for f in fit:
        q, s = eq.get(f["item"]), slots.get(f["slot"])
        if not q or not s:
            continue
        fn, m = q.get("function") or {}, float((q.get("physical") or {}).get("mass") or 0)
        frame = hf.get(f"mount_{s['name']}") if hf else None
        pts.append((m, frame[:3, 3] if frame is not None else np.zeros(3)))
        mass += m
        draw += float((q.get("needs") or {}).get("power") or 0)
        if fn.get("kind") == "power_plant":
            supply += float(fn.get("output") or 0)
        if fn.get("kind") == "battery":
            supply += float(fn.get("rate") or 0)
        if fn.get("kind") == "tank" and fn.get("holds"):
            held.add(fn["holds"])
        if fn.get("kind") == "engine":
            engines.append(fn)
            for node, share, at, push in (nozzles(q, frame) or []) if frame is not None else []:
                if push is not None:
                    jets.append((s.get("role"), float(fn.get("thrust") or 0) * share, at, push))
    com = sum(m * p for m, p in pts) / mass if mass else np.zeros(3)
    thrust = lambda role, ax: sum(t * max(0.0, float(p @ np.array(ax))) for r, t, _, p in jets if r == role)
    senses = set()
    for r, t, at, p in jets:
        if r == "attitude":
            tq = np.cross(at - com, p * t)
            for i in range(3):
                if abs(tq[i]) > 1e-6 * max(t, 1):
                    senses.add((i, tq[i] > 0))
    unfed = sum(1 for fn in engines if (fn.get("burns") and fn["burns"] not in held) or (fn.get("propellant") and fn["propellant"] not in held))
    main_t, lift_t = thrust("main", [0, 0, -1]), thrust("lift", [0, 1, 0])
    return {"ship.mass": mass, "ship.main_thrust": main_t, "ship.lift_thrust": lift_t,
            "ship.main_to_weight": lambda g: main_t / (mass * g) if mass else 0.0,
            "ship.lift_to_weight": lambda g: lift_t / (mass * g) if mass else 0.0,
            "ship.power_margin": supply - draw, "ship.attitude_axes": len(senses), "ship.unfed_engines": unfed}


def judge_ship(hull, vals):
    res = []
    for L in hull.get("spec") or []:
        v = vals.get(L["of"])
        if callable(v):
            v = v(L.get("gravity", 9.80665))
        if v is None:
            res.append((L, None, None, "not a ship parameter"))
            continue
        ok, why = True, []
        if "min" in L and v < L["min"] - 1e-9:
            ok, why = False, why + [f"{v:.3g} under {L['min']:g}"]
        if "max" in L and v > L["max"] + 1e-9:
            ok, why = False, why + [f"{v:.3g} over {L['max']:g}"]
        res.append((L, round(v, 4) if isinstance(v, float) else v, ok, "; ".join(why)))
    return res
