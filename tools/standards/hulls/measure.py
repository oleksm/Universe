"""Measure every named mesh object of a hull's .glb: its box and its surface, in the ship's frame
(x right, y up, z aft; metres)."""
import json, struct, sys
import numpy as np

def measure(path):
    b = open(path, "rb").read()
    n = struct.unpack("<I", b[12:16])[0]
    g = json.loads(b[20:20 + n])
    blob = b[20 + n + 8:]
    def acc(i):
        a = g["accessors"][i]; bv = g["bufferViews"][a["bufferView"]]
        comp = {5126: ("<f4", 4), 5123: ("<u2", 2), 5125: ("<u4", 4), 5121: ("u1", 1)}[a["componentType"]]
        cols = {"SCALAR": 1, "VEC3": 3, "VEC2": 2, "VEC4": 4}[a["type"]]
        off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
        stride = bv.get("byteStride") or comp[1] * cols
        if stride == comp[1] * cols:
            arr = np.frombuffer(blob, dtype=comp[0], count=a["count"] * cols, offset=off).reshape(a["count"], cols)
        else:
            arr = np.array([np.frombuffer(blob, dtype=comp[0], count=cols, offset=off + k * stride) for k in range(a["count"])])
        return arr.astype(np.float64) if comp[0] == "<f4" else arr.astype(np.int64)
    def local(nd):
        if "matrix" in nd:
            return np.array(nd["matrix"], dtype=float).reshape(4, 4).T
        t = np.eye(4); t[:3, 3] = nd.get("translation", [0, 0, 0])
        x, y, z, w = nd.get("rotation", [0, 0, 0, 1])
        r = np.eye(4); r[:3, :3] = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)], [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)], [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
        s = np.diag(list(nd.get("scale", [1, 1, 1])) + [1])
        return t @ r @ s
    out = {}
    def walk(i, parent):
        nd = g["nodes"][i]; m = parent @ local(nd)
        if "mesh" in nd:
            pts, area = [], 0.0
            for prim in g["meshes"][nd["mesh"]]["primitives"]:
                p = acc(prim["attributes"]["POSITION"])
                p = (np.c_[p, np.ones(len(p))] @ m.T)[:, :3]
                idx = acc(prim["indices"]).reshape(-1, 3) if "indices" in prim else np.arange(len(p)).reshape(-1, 3)
                a, b2, c = p[idx[:, 0]], p[idx[:, 1]], p[idx[:, 2]]
                area += np.linalg.norm(np.cross(b2 - a, c - a), axis=1).sum() / 2
                pts.append(p)
            p = np.vstack(pts)
            out[nd.get("name", str(i))] = {"lo": p.min(0).tolist(), "hi": p.max(0).tolist(), "area": float(area)}
        for c in nd.get("children", []):
            walk(c, m)
    for i in g["scenes"][g.get("scene", 0)]["nodes"]:
        walk(i, np.eye(4))
    return out

if __name__ == "__main__":
    r = measure(sys.argv[1])
    json.dump(r, open(sys.argv[2], "w"), indent=1)
    allp = np.array([v["lo"] for v in r.values() if True] + [v["hi"] for v in r.values()])
