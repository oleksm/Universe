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


def hull_model(hull):
    """The hull's model file: the copy model_sha256 pins (fso or the game's), else None."""
    import hashlib
    rel, sha = hull.get("model"), hull.get("model_sha256")
    for base in (ROOT, GAME):
        p = os.path.join(base, rel or "")
        if rel and os.path.isfile(p) and (not sha or hashlib.sha256(open(p, "rb").read()).hexdigest() == sha):
            return p
    return None


_SOLID = {}


def solid(path):
    """A model as a uniform solid (ship-rule.mass-properties): (volume m3, centroid, inertia about the centroid per kg of
    mass, box sides), from every triangle mesh at its node (signed tetrahedra from the origin). None: no mesh."""
    if path in _SOLID:
        return _SOLID[path]
    b = open(path, "rb").read()
    n = struct.unpack("<I", b[12:16])[0]
    j = json.loads(b[20:20 + n])
    binoff = 20 + n + 8
    COMP = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2), 5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}

    def acc(i):
        a = j["accessors"][i]
        bv = j["bufferViews"][a["bufferView"]]
        code, size = COMP[a["componentType"]]
        k = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[a["type"]]
        start = binoff + bv.get("byteOffset", 0) + a.get("byteOffset", 0)
        stride = bv.get("byteStride") or size * k
        arr = np.frombuffer(b, dtype=np.dtype(code).newbyteorder("<"), count=(a["count"] - 1) * stride // size + k, offset=start)
        return arr.reshape(-1, stride // size)[:, :k] if stride != size * k else arr.reshape(-1, k)
    vol, mom, cov = 0.0, np.zeros(3), np.zeros((3, 3))
    lo, hi = np.full(3, np.inf), np.full(3, -np.inf)
    canon = np.array([[2, 1, 1], [1, 2, 1], [1, 1, 2]]) / 120.0
    for ni, node in enumerate(j.get("nodes", [])):
        if "mesh" not in node:
            continue
        M = frames_from(j, ni)
        for prim in j["meshes"][node["mesh"]]["primitives"]:
            if prim.get("mode", 4) != 4 or "POSITION" not in prim["attributes"]:
                continue
            v = acc(prim["attributes"]["POSITION"]).astype(float) @ M[:3, :3].T + M[:3, 3]
            idx = acc(prim["indices"]).reshape(-1).astype(np.int64) if "indices" in prim else np.arange(len(v))
            t = v[idx.reshape(-1, 3)]                                   # (m, 3 corners, xyz)
            det = np.einsum("ij,ij->i", t[:, 0], np.cross(t[:, 1], t[:, 2]))
            vol += det.sum() / 6.0
            mom += (det[:, None] * t.sum(axis=1)).sum(axis=0) / 24.0
            cov += np.einsum("m,mai,ab,mbj->ij", det, t, canon, t)         # (the tetrahedron's second moment, origin corner)
            lo, hi = np.minimum(lo, v.min(axis=0)), np.maximum(hi, v.max(axis=0))
    if not np.isfinite(lo).all() or vol <= 1e-9:
        _SOLID[path] = None if not np.isfinite(lo).all() else (0.0, (lo + hi) / 2, None, hi - lo)
        return _SOLID[path]
    c = mom / vol
    cov_c = cov - vol * np.outer(c, c)                                  # about the centroid
    inertia = (np.eye(3) * np.trace(cov_c) - cov_c) / vol               # per kg
    _SOLID[path] = (vol, c, inertia, hi - lo)
    return _SOLID[path]


def frames_from(j, i, _cache={}):
    nodes = j["nodes"]
    parent = {c: k for k, x in enumerate(nodes) for c in x.get("children", [])}
    m = _local(nodes[i])
    while i in parent:
        i = parent[i]
        m = _local(nodes[i]) @ m
    return m


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


RULE = os.path.join(ROOT, "standards/SpaceshipBuilding/metadata/rules/mass-properties.yaml")
ALLOC = os.path.join(ROOT, "standards/SpaceshipBuilding/metadata/rules/flight-allocation.yaml")


def figure(name):
    import yaml
    for f in (yaml.safe_load(open(RULE)).get("figures") or []) + (yaml.safe_load(open(ALLOC)).get("figures") or []):
        if f["name"] == name:
            return f["value"]
    raise KeyError(f"ship-rule.mass-properties has no figure {name}")


def product_centroid(q):
    """Where a product's own mass sits in its frame: its model's solid centroid, else its frame's origin."""
    path = (q.get("visual") or {}).get("path")
    p = os.path.join(ASSETS, path or "", "model.glb")
    sol = solid(p) if path and os.path.isfile(p) else None
    return sol[1] if sol else np.zeros(3)


def _point(m, r):
    return m * (np.eye(3) * (r @ r) - np.outer(r, r))


def ship_values(hull, slots, fit, eq, hf):
    """The ship as fitted, tanks full (the loaded state): mass, centre of mass, thrust by role and what of it the attitude
    nozzles can keep balanced, turning authority, power margin, unfed engines."""
    hmass = float((hull.get("physical") or {}).get("mass") or 0)
    sol = solid(hull_model(hull)) if hull_model(hull) else None
    hat = sol[1] if sol else np.zeros(3)
    pts = [(hmass, hat)]
    supply = draw = 0.0
    held, engines, jets = set(), [], []
    for f in fit:
        q, s = eq.get(f["item"]), slots.get(f["slot"])
        if not q or not s:
            continue
        fn, m = q.get("function") or {}, float((q.get("physical") or {}).get("mass") or 0)
        frame = hf.get(f"mount_{s['name']}") if hf else None
        at = (frame @ np.r_[product_centroid(q), 1.0])[:3] if frame is not None else hat
        pts.append((m, at))
        draw += float((q.get("needs") or {}).get("power") or 0)
        if fn.get("kind") == "power_plant":
            supply += float(fn.get("output") or 0)
        if fn.get("kind") == "battery":
            supply += float(fn.get("rate") or 0)
        if fn.get("kind") == "tank" and fn.get("holds"):
            held.add(fn["holds"])
            pts.append((float(fn.get("capacity") or 0), at))           # (full)
        if fn.get("kind") == "engine":
            engines.append(fn)
            for node, share, nat, push in (nozzles(q, frame) or []) if frame is not None else []:
                if push is not None:
                    jets.append((s.get("role"), float(fn.get("thrust") or 0) * share, nat, push))
    mass = sum(m for m, _ in pts)
    com = sum(m * p for m, p in pts) / mass if mass else np.zeros(3)
    I = np.zeros((3, 3))
    if sol and sol[2] is not None:
        I += hmass * sol[2] + _point(hmass, hat - com)
    for m, p in pts[1:]:
        I += _point(m, p - com)
    Iinv = np.linalg.inv(I) if np.linalg.det(I) > 1e-9 else None
    thrust = lambda role, ax: sum(t * max(0.0, float(p @ np.array(ax))) for r, t, _, p in jets if r == role)
    att = [(t, np.cross(at - com, p * t)) for r, t, at, p in jets if r == "attitude"]
    senses = {(i, tq[i] > 0) for _, tq in att for i in range(3) if abs(tq[i]) > 1e-6 * max(_, 1)}

    def authority():
        """The least turn acceleration (rad/s2) its attitude nozzles give about any axis, either way, holding the other two."""
        if Iinv is None or not att:
            return 0.0
        from scipy.optimize import linprog
        A = np.array([Iinv @ tq for _, tq in att]).T          # 3 x n: each nozzle at full, its turn acceleration
        best = np.inf
        for i in range(3):
            for sgn in (1, -1):
                others = [k for k in range(3) if k != i]
                r = linprog(-sgn * A[i], A_eq=A[others], b_eq=np.zeros(2), bounds=[(0, 1)] * len(att), method="highs")
                best = min(best, -r.fun if r.status == 0 else 0.0)
        return max(best, 0.0)

    turn = authority()

    def balanced(role, ax):
        """What of a role's thrust along ax it can give while the attitude nozzles keep the turn it leaves under STRAIGHT x
        the weakest turn rate (ship-rule.mass-properties)."""
        own = [(t, np.cross(at - com, p * t), t * max(0.0, float(p @ np.array(ax)))) for r, t, at, p in jets if r == role]
        full = sum(x[2] for x in own)
        if not full or Iinv is None:
            return 0.0
        from scipy.optimize import linprog
        A_role = Iinv @ sum(x[1] for x in own)                 # its turn acceleration at full
        A_att = np.array([Iinv @ tq for _, tq in att]).T if att else np.zeros((3, 0))
        tol = figure("straight") * turn
        n = A_att.shape[1]
        # variables: k (share of full), u (attitude nozzles); |A_role k + A_att u| <= tol per axis; maximise k
        c = np.r_[-1.0, np.zeros(n)]
        M = np.c_[A_role.reshape(3, 1), A_att]
        r = linprog(c, A_ub=np.r_[M, -M], b_ub=np.full(6, tol), bounds=[(0, 1)] + [(0, 1)] * n, method="highs")
        return full * (r.x[0] if r.status == 0 else 0.0)

    main_t, lift_t = thrust("main", [0, 0, -1]), thrust("lift", [0, 1, 0])
    main_b, lift_b = balanced("main", [0, 0, -1]), balanced("lift", [0, 1, 0])
    unfed = sum(1 for fn in engines if (fn.get("burns") and fn["burns"] not in held) or (fn.get("propellant") and fn["propellant"] not in held))
    tw = lambda f: (lambda g: f / (mass * g) if mass else 0.0)
    return {"ship.mass": mass, "ship.main_thrust": main_t, "ship.lift_thrust": lift_t,
            "ship.main_balanced": main_b, "ship.lift_balanced": lift_b,
            "ship.main_to_weight": tw(main_t), "ship.lift_to_weight": tw(lift_t),
            "ship.main_balanced_to_weight": tw(main_b), "ship.lift_balanced_to_weight": tw(lift_b),
            "ship.attitude_authority": turn, "ship.centre_of_mass": [round(float(x), 4) for x in com],
            "ship.inertia": [round(float(I[i, i]), 1) for i in range(3)],
            "ship.power_margin": supply - draw, "ship.attitude_axes": len(senses), "ship.unfed_engines": unfed}


HULL_REPORT = os.environ.get("HULL_REPORT") or os.path.join(GAME, "target/release/examples/hull_report")
PHYSICS = ("ship.mass", "ship.main_thrust", "ship.lift_thrust", "ship.main_balanced", "ship.lift_balanced",
           "ship.attitude_authority", "ship.attitude_axes", "ship.power_margin")


def game_values(key, fit=None):
    """The ship.* values from the one calculator, the game's hull_report (ship-rule.mass-properties), run on this
    registry's records: (values, None) or (None, why not)."""
    import subprocess
    if not os.path.isfile(HULL_REPORT):
        return None, f"no hull_report at {HULL_REPORT} (cargo build --release -p universe-world --example hull_report)"
    cmd = [HULL_REPORT, key, "--registry", os.path.join(ROOT, "standards")] + (["--fit", fit] if fit else [])
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
        d = json.loads(r.stdout)
    except (ValueError, subprocess.TimeoutExpired) as e:
        return None, f"hull_report {key} --registry ... {'--fit ' if fit else ''}did not answer ({(r.stderr.strip().splitlines() or [str(e)])[-1][:160] if 'r' in dir() else e})"
    if d.get("registry") != os.path.join(ROOT, "standards"):
        return None, "hull_report judged its build's records, not these (no --registry support yet)"
    v = dict(d.get("ship") or {})
    if d.get("loaded_com"):
        v["ship.centre_of_mass"] = d["loaded_com"]
    if d.get("loaded_inertia"):
        m = d["loaded_inertia"]
        v["ship.inertia"] = [m[0][0], m[1][1], m[2][2]] if isinstance(m[0], list) else [m[0], m[4], m[8]]
    return v, None


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
