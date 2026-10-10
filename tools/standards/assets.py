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
    bad += [f"motion: no node {n!r} in the model" for n in sorted(set(refs) - names)]
    moving = [n["node"] for n in mo["nodes"]]
    frames = set(moving) | {mo["root"]}       # (an attachment's frame must be known: the root's, or a listed node's bind)
    atts = [(f"actuator {a['name']}", x["parent"]) for a in mo.get("actuators", []) for x in (a["fixed"], a["moving"])]
    atts += [(f"hose {h['name']}", x["parent"]) for h in mo.get("hoses", []) for x in h["ends"] + h.get("guides", [])]
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
