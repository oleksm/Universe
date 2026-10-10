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
    return {"bounds": [round(hi[k] - lo[k], 4) for k in range(3)], "nodes": names, "triangles": tris, "meshes": len(meshes)}


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
    budget = KINDS[kind][1]
    if info["triangles"] > budget:
        bad.append(f"{info['triangles']:,} triangles drawn, over the {budget:,} a {kind} may have")
    return bad
