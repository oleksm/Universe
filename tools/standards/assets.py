"""The asset contract's shared part (docs/asset-contract.md): which records want a model, what each must satisfy, the
assets store, and a reader for glTF binary files (.glb) that needs nothing but the standard library.

Used by next_asset.py (the lookup) and install_model.py (the installer). Run nothing here.
"""
import glob, hashlib, json, math, os, re, struct
import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TREE = os.path.join(ROOT, "standards")
FORMAT = "freefall-model/1"

# Record kinds that take a model, their folders, and the most triangles one may draw.
KINDS = {
    "equipment": ("SFO/metadata/equipment", 60_000),
    "module": ("SFO/metadata/modules", 150_000),
    "building": ("SFO/metadata/buildings", 150_000),
    "structure": ("SFO/metadata/structures", 400_000),
    "gate": ("SFO/metadata/gates", 400_000),
    "hull": ("SFO/metadata/hulls", 1_000_000),
}
# A model's size against its record's, each of the three sorted dimensions: at least LOW and at most HIGH of the record's.
LOW, HIGH = 0.85, 1.10
JET_KINDS = ("engine", "drive", "lift", "thrusters")
DOOR_KINDS = ("airlock", "ramp", "bay_door", "pressure_door", "cargo_lift")


def store():
    """The assets store: UNIVERSE_ASSETS, else ~/git/freefall-assets."""
    return os.environ.get("UNIVERSE_ASSETS") or os.path.expanduser("~/git/freefall-assets")


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def tail(key):
    """A key's tail, for its package folder: equipment.power.battery.s1 -> power-battery-s1."""
    return key.split(".", 1)[1].replace(".", "-")


def records():
    """Every record that may take a model: (kind, key, path, record)."""
    out = []
    for kind, (folder, _) in KINDS.items():
        for f in sorted(glob.glob(os.path.join(TREE, folder, "*.yaml"))):
            r = yaml.safe_load(open(f, encoding="utf-8")) or {}
            k = (r.get("identity") or {}).get("key")
            if k:
                out.append((kind, k, f, r))
    return out


def size_of(kind, r):
    """The record's size, length by width by height (m), or None where it says none."""
    ph = r.get("physical") or {}
    if all(ph.get(k) for k in ("length", "width", "height")):
        return [float(ph["length"]), float(ph["width"]), float(ph["height"])]
    sz = r.get("size") or {}
    if kind == "gate" and sz.get("opening"):
        d = float(sz["opening"]) + 2 * float(sz.get("thickness", 0))
        return [d, d, float(sz.get("thickness") or d * 0.1)]
    return None


def required_nodes(kind, r):
    """Named nodes the model must have, as name prefixes: what the game attaches things to."""
    if kind != "equipment":
        return []
    fk = (r.get("function") or {}).get("kind")
    need = ["mount"]
    if fk in JET_KINDS and (r.get("function") or {}).get("cycle") != "ducted_fan":
        need.append("nozzle")
    if fk == "engine" and (r.get("function") or {}).get("cycle") == "ducted_fan":
        need.append("duct")
    if fk == "landing_gear":
        need.append("contact")
    if fk in DOOR_KINDS:
        need.append("door")
    if fk == "docking":
        need.append("dock")
    return need


# The modeller's account of a version (--about=<file>): required sections, then free to say what it needs.
ABOUT_REQUIRED = ("model", "work", "considerations")
ABOUT_KNOWN = ABOUT_REQUIRED + ("stats", "evidence")


def read_about(path):
    """The modeller's account (YAML or JSON): model, work, considerations required; stats and evidence optional. A list of
    plain sentences on what is wrong (empty: it is fine) and the account."""
    try:
        a = yaml.safe_load(open(path, encoding="utf-8"))
    except (OSError, yaml.YAMLError) as e:
        return [f"--about {path}: {e}"], None
    if not isinstance(a, dict):
        return [f"--about {path}: not a mapping of sections"], None
    bad = [f"--about: no `{k}` section" for k in ABOUT_REQUIRED if not a.get(k)]
    bad += [f"--about: unknown section `{k}` (sections: {', '.join(ABOUT_KNOWN)})" for k in a if k not in ABOUT_KNOWN]
    return bad, a


# ---- freefall-motion/1 (docs/formats/freefall-motion-1.schema.yaml)

MOTION_FORMAT = "freefall-motion/1"
MOTION_SCHEMA = os.path.join(ROOT, "docs/formats/freefall-motion-1.schema.yaml")


def _m4(t):
    """A column-major 16 into rows."""
    return [[t[c * 4 + r] for c in range(4)] for r in range(4)]


def _apply(m, p):
    return [m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3] for r in range(3)]


def _rot(axis, a):
    n = math.sqrt(sum(x * x for x in axis)); x, y, z = (v / n for v in axis); c, s = math.cos(a), math.sin(a); C = 1 - c
    return [[c + x * x * C, x * y * C - z * s, x * z * C + y * s, 0], [y * x * C + z * s, c + y * y * C, y * z * C - x * s, 0],
            [z * x * C - y * s, z * y * C + x * s, c + z * z * C, 0], [0, 0, 0, 1]]


def _about(pivot, r):
    """Rotation r (4x4) about a point."""
    t = [[1, 0, 0, pivot[0]], [0, 1, 0, pivot[1]], [0, 0, 1, pivot[2]], [0, 0, 0, 1]]
    ti = [[1, 0, 0, -pivot[0]], [0, 1, 0, -pivot[1]], [0, 0, 1, -pivot[2]], [0, 0, 0, 1]]
    return _mul(_mul(t, r), ti)


def _rigid(t):
    """What is wrong with a transform as a rigid one (None: rigid)."""
    if not all(math.isfinite(v) for v in t):
        return "not finite"
    m = _m4(t)
    if any(abs(m[3][k] - (0, 0, 0, 1)[k]) > 1e-6 for k in range(4)):
        return "bottom row is not 0 0 0 1"
    for i in range(3):
        for j in range(3):
            d = sum(m[k][i] * m[k][j] for k in range(3))
            if abs(d - (1 if i == j else 0)) > 1e-4:
                return "rotation not orthonormal (scaled or sheared)"
    det = (m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
           + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]))
    return None if det > 0 else "mirrored (determinant below 0)"


def _shape(root, sc, v, at):
    """The schema keywords a format file uses (type, const, enum, required, properties, additionalProperties, items,
    minItems, maxItems, minLength, $ref): what does not fit, as `path: why`. No dependency, so the installer runs anywhere."""
    if "$ref" in sc:
        node = root
        for part in sc["$ref"].lstrip("#/").split("/"):
            node = node[part]
        return _shape(root, node, v, at)
    where = at or "(top)"
    t = sc.get("type")
    ok = {"object": isinstance(v, dict), "array": isinstance(v, list), "string": isinstance(v, str),
          "number": isinstance(v, (int, float)) and not isinstance(v, bool), "integer": isinstance(v, int) and not isinstance(v, bool),
          "boolean": isinstance(v, bool)}
    if t and not ok[t]:
        return [f"{where}: not a {t}"]
    if "const" in sc and v != sc["const"]:
        return [f"{where}: must be {sc['const']!r}"]
    if "enum" in sc and v not in sc["enum"]:
        return [f"{where}: {v!r} is not one of {sc['enum']}"]
    out = []
    if isinstance(v, str) and len(v) < sc.get("minLength", 0):
        out.append(f"{where}: empty")
    if isinstance(v, dict):
        out += [f"{where}: no `{k}`" for k in sc.get("required", []) if k not in v]
        props = sc.get("properties", {})
        for k, x in v.items():
            if k in props:
                out += _shape(root, props[k], x, f"{at}/{k}" if at else k)
            elif sc.get("additionalProperties") is False:
                out.append(f"{where}: unknown `{k}`")
            elif isinstance(sc.get("additionalProperties"), dict):
                out += _shape(root, sc["additionalProperties"], x, f"{at}/{k}" if at else k)
    if isinstance(v, list):
        if len(v) < sc.get("minItems", 0) or len(v) > sc.get("maxItems", len(v)):
            out.append(f"{where}: {len(v)} items")
        if "items" in sc:
            for i, x in enumerate(v):
                out += _shape(root, sc["items"], x, f"{at}/{i}")
    return out


def _inv(m):
    """A rigid transform's inverse."""
    r = [[m[j][i] for j in range(3)] for i in range(3)]
    t = [-sum(r[i][k] * m[k][3] for k in range(3)) for i in range(3)]
    return [[*r[0], t[0]], [*r[1], t[1]], [*r[2], t[2]], [0, 0, 0, 1]]


def _unit(v):
    n = math.sqrt(sum(c * c for c in v)); return [c / n for c in v]


def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def _cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def _r3(m, v):
    return [sum(m[i][k] * v[k] for k in range(3)) for i in range(3)]


def _angle(m):
    """A rotation's angle (rad)."""
    return math.acos(max(-1.0, min(1.0, (m[0][0] + m[1][1] + m[2][2] - 1) / 2)))


def _turn(m, h):
    """The signed angle a rotation turns about the unit axis h (exact when it is a turn about h)."""
    u = _unit(_cross(h, [1, 0, 0] if abs(h[0]) < 0.9 else [0, 1, 0]))
    w = _r3(m, u); w = [w[k] - h[k] * _dot(w, h) for k in range(3)]
    return math.atan2(_dot(_cross(u, w), h), _dot(u, w))


def _swing(a, b):
    """The shortest rotation taking unit a to unit b (3x3)."""
    v = _cross(a, b); c = _dot(a, b); s2 = _dot(v, v)
    if s2 < 1e-24:
        return [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
    k = (1 - c) / s2
    vx = [[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]]
    return [[(1 if i == j else 0) + vx[i][j] + k * sum(vx[i][m] * vx[m][j] for m in range(3)) for j in range(3)] for i in range(3)]


def _diff(a, b):
    return max(abs(a[i][j] - b[i][j]) for i in range(4) for j in range(4))


def check_links(mo, g, pose, bind, parent, eye):
    """Links, joints, ties and bellows over the gimbal envelope, as the schema orders the pose: plain sentences."""
    bad = []
    links = mo.get("links", [])
    by = {ln["name"]: ln for ln in links}


    def current(q):
        """Every node's transform in model root at input q (motion times bind): listed nodes, actuators, solved links."""
        moved = pose(*q)
        A = {mo["root"]: eye}
        for n, b in bind.items():
            m, x = moved.get(n), n
            while m is None and x in parent:
                x = parent[x]; m = moved.get(x)
            A[n] = _mul(m or eye, b)
        for ac in mo.get("actuators", []):
            # (each part: the shortest rotation from its local axis to the line toward the other anchor, at its own anchor;
            # absolute, not a delta from the bind; antiparallel refused: no roll is stated for it)
            f = _apply(A.get(ac["fixed"]["parent"], eye), ac["fixed"]["point"])
            m_ = _apply(A.get(ac["moving"]["parent"], eye), ac["moving"]["point"])
            ax = _unit(ac["axis"])
            for node, at, to in ((ac["body_node"], f, m_), (ac["rod_node"], m_, f)):
                d = _unit([to[k] - at[k] for k in range(3)])
                if _dot(ax, d) < -1 + 1e-9:
                    A[node] = None; continue
                r = _swing(ax, d)
                A[node] = [[*r[0], at[0]], [*r[1], at[1]], [*r[2], at[2]], [0, 0, 0, 1]]
        for ln in links:
            if ln["pose"] != "solved":
                continue
            ja, jb = ln["joints"]
            def fixed_centre(j):
                sd = j["a"] if j["a"]["parent"] != ln["node"] else j["b"]
                return _apply(_mul(A.get(sd["parent"], eye), _m4(sd["frame"])), [0, 0, 0])
            ca, cb = fixed_centre(ja), fixed_centre(jb)
            d = _unit([cb[k] - ca[k] for k in range(3)])
            sa = ja["a"] if ja["a"]["parent"] != ln["node"] else ja["b"]
            h = _unit(_r3(_mul(A.get(sa["parent"], eye), _m4(sa["frame"])), ja["hinges"][0]))
            rr = [h[k] - d[k] * _dot(h, d) for k in range(3)]
            if math.sqrt(_dot(rr, rr)) < math.sin(math.radians(10)):
                A[ln["node"]] = None; continue
            rr = _unit(rr); third = _cross(d, rr)
            ax, ro = _unit(ln["axis"]), _unit(ln["roll_axis"]); lt = _cross(ax, ro)
            R = [[d[i] * ax[j] + rr[i] * ro[j] + third[i] * lt[j] for j in range(3)] for i in range(3)]
            o = _r3(R, ln["origin"])
            A[ln["node"]] = [[*R[0], ca[0] - o[0]], [*R[1], ca[1] - o[1]], [*R[2], ca[2] - o[2]], [0, 0, 0, 1]]
        return A

    def side(A, sd):
        return _mul(A.get(sd["parent"], eye), _m4(sd["frame"]))

    def decompose(j, rel):
        """A joint's relative rotation as its ordered hinge angles, its twist about the line, and what is left."""
        hs = [_unit(h) for h in j["hinges"]]
        rot = [r[:3] for r in rel[:3]]
        if len(hs) == 1:
            t = [_turn(rot, hs[0])]
            rest = _mul4(_rot(hs[0], -t[0]), rel)
        else:
            v = _r3(rot, hs[1])
            t1 = math.atan2(_dot(_cross(hs[1], v), hs[0]), _dot(hs[1], v) - _dot(hs[1], hs[0]) * _dot(v, hs[0]))
            r2 = _mul4(_rot(hs[0], -t1), rel)
            t2 = _turn([r[:3] for r in r2[:3]], hs[1])
            t = [t1, t2]
            rest = _mul4(_rot(hs[1], -t2), r2)
        line = _unit(j["line_axis"])
        tw = _turn([r[:3] for r in rest[:3]], line)
        left = _angle(_mul4(_rot(line, -tw), rest))
        return t, tw, left

    dense = [(0, 0)] + [(r * math.cos(k * math.pi / 16), r * math.sin(k * math.pi / 16)) for r in (0.5, 1.0) for k in range(32)]
    A0 = current((0, 0))
    for ln in links:
        if ln["pose"] == "solved" and A0.get(ln["node"]) is not None and _diff(A0[ln["node"]], _m4(ln["bind"])) > 1e-5:
            bad.append(f"motion: link {ln['name']}: solved at neutral it does not land on its bind")
        for j in ln["joints"]:
            if _diff(side(A0, j["a"]), side(A0, j["b"])) > 1e-5:
                bad.append(f"motion: link {ln['name']} joint {j['name']}: its two sides' frames do not coincide at neutral")
            if len(j["limits"]) != len(j["hinges"]):
                bad.append(f"motion: link {ln['name']} joint {j['name']}: {len(j['hinges'])} hinge(s) but {len(j['limits'])} limit(s)")
    if bad:
        return bad
    worst = {}
    def note(k, v):
        worst[k] = max(worst.get(k, 0.0), v)
    acts = [(ac["name"], n) for ac in mo.get("actuators", []) for n in (ac["body_node"], ac["rod_node"])]
    for nm, n in acts:
        if n in parent and parent[n] != mo["root"]:
            bad.append(f"motion: actuator {nm}: {n!r} is listed under {parent[n]!r}; an actuator part is listed (for its bind) under the root only: the actuator is its pose producer")
    for q in dense:
        A = current(q)
        for nm, n in acts:
            if A.get(n, 0) is None:
                bad.append(f"motion: actuator {nm}: at input {q[0]:.2f}, {q[1]:.2f} {n!r} would have to turn its axis end over end: no roll is stated for that")
        for ln in links:
            if A.get(ln["node"], 0) is None:
                bad.append(f"motion: link {ln['name']}: at input {q[0]:.2f}, {q[1]:.2f} joint a's hinge runs within 10 degrees of the link: roll undetermined")
                continue
            if "rest_length" in ln:
                ca = [side(A, ln["joints"][0]["a"])[k][3] for k in range(3)]; cb = [side(A, ln["joints"][1]["a"])[k][3] for k in range(3)]
                note((ln["name"], "closure"), abs(math.dist(ca, cb) - ln["rest_length"]))
            for j in ln["joints"]:
                rel = _mul(_inv(side(A, j["a"])), side(A, j["b"]))
                t = [rel[k][3] for k in range(3)]; line = _unit(j["line_axis"])
                ax = _dot(t, line); lat = math.sqrt(max(0.0, _dot(t, t) - ax * ax))
                angles, tw, left = decompose(j, rel)
                n = (ln["name"], j["name"])
                note(n + ("axial",), abs(ax)); note(n + ("lateral",), lat); note(n + ("torsion",), abs(tw)); note(n + ("other",), left)
                for i, a in enumerate(angles):
                    note(n + (f"hinge{i}",), abs(a))
            for tie in ln.get("ties", []):
                pa = _apply(A.get(tie["a"]["parent"], eye), tie["a"]["point"]); pb = _apply(A.get(tie["b"]["parent"], eye), tie["b"]["point"])
                note((ln["name"], tie["name"], "tie"), abs(math.dist(pa, pb) - tie["length"]))
    for ln in links:
        c = worst.get((ln["name"], "closure"))
        if c is not None and c > ln["tolerance"]:
            bad.append(f"motion: link {ln['name']}: joint centres move {c:.4f} m off its rest length, over {ln['tolerance']} m (a rigid link cannot follow)")
        for j in ln["joints"]:
            n = (ln["name"], j["name"]); who = f"motion: link {ln['name']} joint {j['name']}"
            if worst.get(n + ("axial",), 0) > (j.get("axial") or 0) + 1e-6:
                bad.append(f"{who}: slides {worst[n + ('axial',)]:.5f} m along its line, allowed {j.get('axial') or 0}")
            if worst.get(n + ("lateral",), 0) > (j.get("lateral") or 0) + 1e-6:
                bad.append(f"{who}: offsets {worst[n + ('lateral',)]:.5f} m sideways, allowed {j.get('lateral') or 0}")
            if worst.get(n + ("torsion",), 0) > (j.get("torsion") or 0) + 1e-6:
                bad.append(f"{who}: twists {worst[n + ('torsion',)]:.5f} rad about its line, allowed {j.get('torsion') or 0}")
            if worst.get(n + ("other",), 0) > 1e-4:
                bad.append(f"{who}: turns {worst[n + ('other',)]:.5f} rad about an axis it does not have (not its hinges in order, not twist)")
            for i, lim in enumerate(j["limits"]):
                if worst.get(n + (f"hinge{i}",), 0) > lim + 1e-9:
                    bad.append(f"{who}: hinge {i + 1} turns {worst[n + (f'hinge{i}',)]:.4f} rad, over its {lim} rad")
        for tie in ln.get("ties", []):
            v = worst.get((ln["name"], tie["name"], "tie"), 0)
            if v > tie["tolerance"]:
                bad.append(f"motion: link {ln['name']} tie {tie['name']}: its ends move {v:.4f} m off its length, over {tie['tolerance']} m")
    if bad:
        return bad

    def ring_frames(b, A):
        """The rings' frames F(t) at a pose (the Hermite evaluator): {node: F}, or None where roll is undetermined."""
        up, dn, L = b["upstream"], b["downstream"], b["tangent_magnitude"]
        fu, fd = A.get(up["parent"], eye), A.get(dn["parent"], eye)
        p0, p1 = _apply(fu, up["point"]), _apply(fd, dn["point"])
        m0 = [L * c for c in _unit(_r3(fu, up["tangent"]))]; m1 = [L * c for c in _unit(_r3(fd, dn["tangent"]))]
        hinge = _unit(_r3(fu, up["hinge_axis"]))
        out = {}
        for r in b["rings"]:
            t = r["t"]; t2, t3 = t * t, t * t * t
            h = [(2 * t3 - 3 * t2 + 1) * p0[k] + (t3 - 2 * t2 + t) * m0[k] + (-2 * t3 + 3 * t2) * p1[k] + (t3 - t2) * m1[k] for k in range(3)]
            dh = [(6 * t2 - 6 * t) * p0[k] + (3 * t2 - 4 * t + 1) * m0[k] + (-6 * t2 + 6 * t) * p1[k] + (3 * t2 - 2 * t) * m1[k] for k in range(3)]
            z = _unit(dh)
            if abs(_dot(hinge, z)) > 1e-3:      # (engine's rule: a planar single hinge, the axis square to the curve; no other roll is chosen)
                return None
            x = [hinge[k] - z[k] * _dot(hinge, z) for k in range(3)]
            x = _unit(x); y = _cross(z, x)
            out[r["node"]] = [[x[0], y[0], z[0], h[0]], [x[1], y[1], z[1], h[1]], [x[2], y[2], z[2], h[2]], [0, 0, 0, 1]]
        return out

    def rings(q, A):
        """The rings' transforms at input q: F(t, pose) F(t, neutral)^-1 bind."""
        out = {}
        for b in mo.get("bellows", []):
            f, f0 = ring_frames(b, A), ring_frames(b, A0)
            if f and f0:
                for r in b["rings"]:
                    out[r["node"]] = _mul(_mul(f[r["node"]], _inv(f0[r["node"]])), _m4(r["bind"]))
        return out
    listed = set(bind)
    for b in mo.get("bellows", []):
        ts = [r["t"] for r in b["rings"]]
        if any(not 0 <= t <= 1 for t in ts) or any(y <= x for x, y in zip(ts, ts[1:])):
            bad.append(f"motion: bellows {b['name']}: ring t must rise within 0 to 1 ({ts})"); continue
        bad += [f"motion: bellows {b['name']}: ring {r['node']!r} is also a listed node: one pose producer only" for r in b["rings"] if r["node"] in listed]
        f0 = ring_frames(b, A0)
        if f0 is None:
            bad.append(f"motion: bellows {b['name']}: at neutral the hinge axis is not square to the curve (a planar single hinge is required)"); continue
        gap, roll_lost = -1.0, False
        for q in dense:
            f = ring_frames(b, current(q))
            if f is None:
                roll_lost = True; continue
            fs = [f[r["node"]] for r in b["rings"]]
            for fa, fb in zip(fs, fs[1:]):     # (the rims facing each other, round the circle: the widest opening between them)
                for k in range(24):
                    ph = 2 * math.pi * k / 24
                    def rim(F, sign):
                        local = [b["radius"] * math.cos(ph), b["radius"] * math.sin(ph), sign * b["ring_width"] / 2]
                        return _apply(F, local)
                    zm = _unit([fa[i][2] + fb[i][2] for i in range(3)])
                    gap = max(gap, _dot([rim(fb, -1)[i] - rim(fa, 1)[i] for i in range(3)], zm))
        if roll_lost:
            bad.append(f"motion: bellows {b['name']}: somewhere in the envelope the hinge axis leaves square to the curve (a planar single hinge is required)")
        if gap > (b.get("skirt_length") or 0) + 1e-6:
            bad.append(f"motion: bellows {b['name']}: neighbouring rims open {gap:.4f} m, more than the {b.get('skirt_length') or 0} m skirt covers")
    for ref in mo.get("references", []):
        A = current(tuple(ref["input"]))
        A.update(rings(tuple(ref["input"]), A))
        for n, t in ref["transforms"].items():
            if A.get(n) is not None and _diff(A[n], _m4(t)) > 2e-6:      # (engine's acceptance threshold for matrix components)
                bad.append(f"motion: reference {ref['pose']}: {n} is {_diff(A[n], _m4(t)):.2e} off what the motion gives")
    return bad


def _mul4(a, b):
    return _mul(a, b)


def check_motion(key, r, info, mo):
    """What is wrong with a motion account against its record and model: plain sentences (empty: it fits)."""
    schema = yaml.safe_load(open(MOTION_SCHEMA, encoding="utf-8"))
    errs = _shape(schema, schema, mo, "")
    if errs:
        return [f"motion: {e}" for e in errs[:12]]
    bad = []
    if mo["key"] != key:
        bad.append(f"motion: key {mo['key']} is not {key}")
    names = set(info["nodes"])
    refs = [mo["root"]] + [n["node"] for n in mo["nodes"]] + [n["parent"] for n in mo["nodes"]]
    g = mo.get("gimbal")
    if g:
        refs += [g["pitch_node"], g["yaw_node"]]
    for a in mo.get("actuators", []):
        refs += [a["body_node"], a["rod_node"], a["fixed"]["parent"], a["moving"]["parent"]]
    for h in mo.get("hoses", []):
        refs += [h["rest_node"]] + [e["parent"] for e in h["ends"]] + [x["parent"] for x in h.get("guides", [])]
    for ln in mo.get("links", []):
        refs += [ln["node"]] + [j[sd]["parent"] for j in ln["joints"] for sd in "ab"] + [t["node"] for t in ln.get("ties", [])]
        refs += [x["parent"] for t in ln.get("ties", []) for x in (t["a"], t["b"])]
    refs += [r["node"] for b in mo.get("bellows", []) for r in b["rings"]]
    bad += [f"motion: no node {n!r} in the model" for n in sorted(set(refs) - names)]
    moving = [n["node"] for n in mo["nodes"]]
    solved_nodes = {ln["node"] for ln in mo.get("links", []) if ln["pose"] == "solved"}
    frames = set(moving) | {mo["root"]} | solved_nodes       # (an attachment's frame must be known: the root, a listed node, a solved link)
    for ln in mo.get("links", []):
        if ln["pose"] == "inherited" and ln["node"] not in moving:
            bad.append(f"motion: link {ln['name']}: inherited, but {ln['node']!r} is not a listed node (it needs a parent and bind there)")
        if ln["pose"] == "solved":
            if ln["node"] in moving:
                bad.append(f"motion: link {ln['name']}: solved, but {ln['node']!r} is also a listed node: one pose producer only")
            bad += [f"motion: link {ln['name']}: solved needs `{k}`" for k in ("bind", "origin", "axis", "roll_axis", "roll_rule", "rest_length", "tolerance") if k not in ln]
    atts = [(f"actuator {a['name']}", x["parent"]) for a in mo.get("actuators", []) for x in (a["fixed"], a["moving"])]
    atts += [(f"hose {h['name']}", x["parent"]) for h in mo.get("hoses", []) for x in h["ends"] + h.get("guides", [])]
    atts += [(f"link {ln['name']} joint {j['name']}", j[sd]["parent"]) for ln in mo.get("links", []) for j in ln["joints"] for sd in "ab"]
    atts += [(f"link {ln['name']} tie {t['name']}", x["parent"]) for ln in mo.get("links", []) for t in ln.get("ties", []) for x in (t["a"], t["b"])]
    for ln in mo.get("links", []):
        for j in ln["joints"]:
            if len(j["limits"]) != len(j["hinges"]):
                bad.append(f"motion: link {ln['name']} joint {j['name']}: {len(j['hinges'])} hinge(s) but {len(j['limits'])} limit(s)")
    bad += [f"motion: {who}: parent {par!r} is neither the root nor a listed node (its frame is unknown)" for who, par in atts if par not in frames]
    bad += [f"motion: node {n!r} listed twice" for n in sorted({n for n in moving if moving.count(n) > 1})]
    parent = {n["node"]: n["parent"] for n in mo["nodes"]}
    for n in parent:
        seen, x = set(), n
        while x in parent:
            if x in seen:
                bad.append(f"motion: {n!r} is its own ancestor"); break
            seen.add(x); x = parent[x]
    bind = {}
    for n in mo["nodes"]:
        why = _rigid(n["bind"])
        if why:
            bad.append(f"motion: {n['node']}'s bind is {why}")
        bind[n["node"]] = _m4(n["bind"])
    for h in mo.get("hoses", []):
        for e in h["ends"]:
            why = _rigid(e["frame"])
            if why:
                bad.append(f"motion: hose {h['name']}: an end frame is {why}")
    if bad:
        return bad
    limit = (r.get("function") or {}).get("gimbal")
    if g:
        if limit is None:
            bad.append("motion: a gimbal, but the record says none (function.gimbal)")
        elif abs(g["limit"] - float(limit)) > 1e-9:
            bad.append(f"motion: gimbal limit {g['limit']} rad, the record's is {limit} (the registry is the authority)")
    elif limit:
        bad.append(f"motion: the record gimbals ({limit} rad) but the motion has no gimbal")
    if bad or not g:
        return bad
    eye = [[1 if i == j else 0 for j in range(4)] for i in range(4)]

    def pose(p, y):
        """Each node's motion (model root) at input (p, y): the pitch stage turns, the yaw stage turns within it."""
        d = max(1.0, math.hypot(p, y))
        mp = _about(g["pivot"], _rot(g["pitch_axis"], g["limit"] * p / d))
        my = _mul(mp, _about(g["pivot"], _rot(g["yaw_axis"], g["limit"] * y / d)))
        return {g["pitch_node"]: mp, g["yaw_node"]: my}

    def at(att, moved):
        """An attachment's point in model root at a pose."""
        par = att["parent"]
        base = bind.get(par, eye)
        m = moved.get(par)
        if m is None:      # (a child of a stage moves with it)
            x = par
            while x in parent and m is None:
                x = parent[x]; m = moved.get(x)
        return _apply(_mul(m or eye, base), att["point"])

    poses = [(0, 0)] + [(math.cos(k * math.pi / 8), math.sin(k * math.pi / 8)) for k in range(16)]
    for a in mo.get("actuators", []):
        lens = [math.dist(at(a["fixed"], pose(*q)), at(a["moving"], pose(*q))) for q in poses]
        least = a["body_length"] + a["rod_length"] - max(lens)
        if least < a["min_overlap"] - 1e-6:
            bad.append(f"motion: actuator {a['name']}: overlap falls to {least:.4f} m at full stroke, under its {a['min_overlap']} m")
        if min(lens) < max(a["body_length"], a["rod_length"]):
            bad.append(f"motion: actuator {a['name']}: closes to {min(lens):.4f} m, shorter than its longer part")
    links = mo.get("links", [])
    if not (links or mo.get("bellows") or mo.get("references") or mo.get("actuators")):
        pass
    elif bad:
        return bad
    else:
        bad += check_links(mo, g, pose, bind, parent, eye)
    for h in mo.get("hoses", []):
        def end_point(e, moved):
            par = e["parent"]; m = moved.get(par)
            x = par
            while m is None and x in parent:
                x = parent[x]; m = moved.get(x)
            return _apply(_mul(m or eye, bind.get(par, eye)), [e["frame"][12], e["frame"][13], e["frame"][14]])
        far = max(math.dist(end_point(h["ends"][0], pose(*q)), end_point(h["ends"][1], pose(*q))) for q in poses)
        if far > h["free_length"] + h["length_tolerance"]:
            bad.append(f"motion: hose {h['name']}: its ends reach {far:.4f} m apart, more than its {h['free_length']} m")
        for e in h["ends"]:
            n = math.sqrt(sum(v * v for v in e["tangent"]))
            if abs(n - 1) > 1e-3:
                bad.append(f"motion: hose {h['name']}: an end tangent is not unit ({n:.4f})")
        if h["min_bend_radius"] <= 0 or h["diameter"] <= 0:
            bad.append(f"motion: hose {h['name']}: diameter and bend radius must be above 0")
    return bad


# ---- glTF binary

def _mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def _trs(n):
    if "matrix" in n:
        m = n["matrix"]
        return [[m[c * 4 + r] for c in range(4)] for r in range(4)]
    tx, ty, tz = n.get("translation", [0, 0, 0])
    qx, qy, qz, qw = n.get("rotation", [0, 0, 0, 1])
    sx, sy, sz = n.get("scale", [1, 1, 1])
    r = [[1 - 2 * (qy * qy + qz * qz), 2 * (qx * qy - qz * qw), 2 * (qx * qz + qy * qw)],
         [2 * (qx * qy + qz * qw), 1 - 2 * (qx * qx + qz * qz), 2 * (qy * qz - qx * qw)],
         [2 * (qx * qz - qy * qw), 2 * (qy * qz + qx * qw), 1 - 2 * (qx * qx + qy * qy)]]
    return [[r[0][0] * sx, r[0][1] * sy, r[0][2] * sz, tx], [r[1][0] * sx, r[1][1] * sy, r[1][2] * sz, ty],
            [r[2][0] * sx, r[2][1] * sy, r[2][2] * sz, tz], [0, 0, 0, 1]]


def inspect_glb(path):
    """A .glb's world bounds (m, glTF axes), its node names, triangles drawn (meshes named COL_* left out), mesh count."""
    b = open(path, "rb").read()
    magic, version, length = struct.unpack_from("<4sII", b, 0)
    if magic != b"glTF" or version != 2:
        raise ValueError("not a glTF 2 binary file (.glb)")
    clen, ctype = struct.unpack_from("<II", b, 12)
    if ctype != 0x4E4F534A:
        raise ValueError("its first chunk is not JSON")
    g = json.loads(b[20:20 + clen])
    nodes, meshes, acc = g.get("nodes", []), g.get("meshes", []), g.get("accessors", [])
    lo, hi = [math.inf] * 3, [-math.inf] * 3
    names, tris = [], 0

    def walk(i, m):
        nonlocal tris
        n = nodes[i]
        w = _mul(m, _trs(n))
        if n.get("name"):
            names.append(n["name"])
        if "mesh" in n:
            mesh = meshes[n["mesh"]]
            drawn = not str(n.get("name", "") or mesh.get("name", "")).startswith("COL_")
            for p in mesh.get("primitives", []):
                a = acc[p["attributes"]["POSITION"]]
                if p.get("mode", 4) == 4 and drawn:
                    tris += (acc[p["indices"]]["count"] if "indices" in p else a["count"]) // 3
                mn, mx = a.get("min"), a.get("max")
                if mn and mx:
                    for cx in (mn[0], mx[0]):
                        for cy in (mn[1], mx[1]):
                            for cz in (mn[2], mx[2]):
                                v = [w[r][0] * cx + w[r][1] * cy + w[r][2] * cz + w[r][3] for r in range(3)]
                                for k in range(3):
                                    lo[k] = min(lo[k], v[k]); hi[k] = max(hi[k], v[k])
        for c in n.get("children", []):
            walk(c, w)

    eye = [[1 if r == c else 0 for c in range(4)] for r in range(4)]
    scene = g.get("scenes", [{}])[g.get("scene", 0)] if g.get("scenes") else {"nodes": list(range(len(nodes)))}
    for i in scene.get("nodes", []):
        walk(i, eye)
    if lo[0] == math.inf:
        raise ValueError("no mesh with position bounds in it")
    prims = [p for m in meshes for p in m.get("primitives", [])]
    images = g.get("images", [])
    return {"bounds": [round(hi[k] - lo[k], 4) for k in range(3)], "lo": [round(v, 4) for v in lo], "hi": [round(v, 4) for v in hi],
            "nodes": names, "triangles": tris, "meshes": len(meshes),
            "stats": {"triangles": tris, "vertices": sum(acc[p["attributes"]["POSITION"]]["count"] for p in prims),
                      "meshes": len(meshes), "primitives": len(prims), "materials": len(g.get("materials", [])),
                      "textures": len(g.get("textures", [])), "images": len(images), "nodes": len(nodes),
                      "animations": len(g.get("animations", [])), "skins": len(g.get("skins", [])),
                      "morph_targets": sum(len(p.get("targets", [])) for p in prims), "bytes": len(b)}}


def check_model(kind, r, info):
    """What is wrong with a model against its record: a list of plain sentences (empty: it fits)."""
    bad = []
    size = size_of(kind, r)
    if size is None:
        bad.append("the record says no size (physical.length, width, height): ask the registry for one first")
    else:
        m, s = sorted(info["bounds"], reverse=True), sorted(size, reverse=True)
        for k, (a, e) in enumerate(zip(m, s)):
            if not (LOW * e - 0.05 <= a <= HIGH * e + 0.05):      # (and 5 cm either way, for thin things: a panel, a door)
                bad.append(f"its {('longest', 'middle', 'shortest')[k]} side is {a:.3g} m against the record's {e:.3g} m "
                           f"(allowed {LOW * e - 0.05:.3g} to {HIGH * e + 0.05:.3g}); model metres, or the record's size is wrong: then a request to the registry")
    for p in required_nodes(kind, r):
        if not any(n.startswith(p) for n in info["nodes"]):
            bad.append(f"no node named {p}* (the game attaches to it)")
    st = info.get("stats") or {}
    if st.get("animations") or st.get("skins") or st.get("morph_targets"):
        bad.append(f"{st.get('animations')} animation(s), {st.get('skins')} skin(s), {st.get('morph_targets')} morph target(s): the game draws "
                   "models static (docs/asset-contract.md, Static models); export without them, moving parts as rigid named nodes")
    budget = KINDS[kind][1]
    if info["triangles"] > budget:
        bad.append(f"{info['triangles']:,} triangles drawn, over the {budget:,} a {kind} may have")
    return bad
