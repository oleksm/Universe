#!/usr/bin/env python3
"""Check a hull's layout (SFO 18) against itself and its model.

    python3 tools/standards/hulls/check_layout.py assets/models/mc07.glb mc-07

- Each opening on a face its two compartments share (the outside: any face of the hull's).
- No two compartments overlap.
- Every compartment reachable from outside through openings.
- Nothing of the model passes through a compartment: its triangles' points (corners, edge
  middles, centres) inside a box, more than a hair in, are the hull cutting into the space.

Frame as the parts': metres back from the nose, above the keel, from the centre line (starboard
positive); nose and keel from the model's `Hull_*` meshes. The ramp (a moving part) is left out.
"""
import json, os, struct, sys

import numpy as np
import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
EPS = 0.01


def triangles(path):
    """The model's triangles (glTF frame), and those of its Hull_* meshes."""
    d = open(path, "rb").read()
    n = struct.unpack_from("<I", d, 12)[0]
    j = json.loads(d[20:20 + n])
    b = 20 + n + 8

    def acc(i, dtype, width):
        a = j["accessors"][i]
        bv = j["bufferViews"][a["bufferView"]]
        o = b + bv.get("byteOffset", 0) + a.get("byteOffset", 0)
        x = np.frombuffer(d[o:o + a["count"] * width * np.dtype(dtype).itemsize], dtype=dtype)
        return x.reshape(-1, width) if width > 1 else x

    def local(nd):
        if "matrix" in nd:
            return np.array(nd["matrix"]).reshape(4, 4).T
        x, y, z, w = nd.get("rotation", [0, 0, 0, 1])
        r = np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                      [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                      [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])
        m = np.eye(4)
        m[:3, :3] = r * np.array(nd.get("scale", [1, 1, 1]))
        m[:3, 3] = nd.get("translation", [0, 0, 0])
        return m

    every, hull, owner, names = [], [], [], []

    def walk(i, parent):
        nd = j["nodes"][i]
        m = parent @ local(nd)
        name = nd.get("name", "")
        if "mesh" in nd and not name.startswith("COL_") and "Ramp" not in name:
            for p in j["meshes"][nd["mesh"]]["primitives"]:
                pos = acc(p["attributes"]["POSITION"], np.float32, 3).astype(float)
                pos = pos @ m[:3, :3].T + m[:3, 3]
                ia = j["accessors"][p["indices"]]
                idx = acc(p["indices"], {5121: np.uint8, 5123: np.uint16, 5125: np.uint32}[ia["componentType"]], 1).astype(int).reshape(-1, 3)
                every.append(pos[idx])
                if name not in names:
                    names.append(name)
                owner.append(np.full(len(idx), names.index(name)))
                if name.startswith("Hull_"):
                    hull.append(pos[idx])
        for c in nd.get("children", []):
            walk(c, m)

    for i in j["scenes"][j.get("scene", 0)]["nodes"]:
        walk(i, np.eye(4))
    return np.concatenate(every), np.concatenate(hull), np.concatenate(owner), names


def main():
    glb, hull = sys.argv[1], sys.argv[2]
    layout = yaml.safe_load(open(os.path.join(ROOT, "standards", "SFO", "metadata", "layouts", f"{hull}.yaml")))
    tris, hull_tris, owner, mesh_names = triangles(glb)
    nose, keel = hull_tris[:, :, 2].min(), hull_tris[:, :, 1].min()
    # The model in the layout's frame: aft, up, side.
    t = np.stack([tris[:, :, 2] - nose, tris[:, :, 1] - keel, tris[:, :, 0]], axis=2)
    probes = np.concatenate([t, (t + np.roll(t, 1, axis=1)) / 2, t.mean(axis=1, keepdims=True)], axis=1).reshape(-1, 3)
    probe_owner = np.repeat(owner, 7)
    comps = {c["name"]: [np.array(bx, float).reshape(3, 2) for bx in c["boxes"]] for c in layout["compartments"]}
    problems = []

    # Overlaps.
    names = list(comps)
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            for p in comps[a]:
                for q in comps[b]:
                    if all(min(p[k, 1], q[k, 1]) - max(p[k, 0], q[k, 0]) > EPS for k in range(3)):
                        problems.append(f"{a} and {b} overlap")

    # Openings on a shared face.
    def on_faces(box, at):
        # The faces of `box` the point is on (axis, side), within the face.
        return [(k, s) for k in range(3) for s in (0, 1) if abs(at[k] - box[k, s]) < EPS and all(box[m, 0] - EPS <= at[m] <= box[m, 1] + EPS for m in range(3) if m != k)]

    links = {n: set() for n in names}
    links["outside"] = set()
    for o in layout["openings"]:
        a, b = o["between"]
        at = np.array(o["at"], float)
        for n in (a, b):
            if n != "outside" and n not in comps:
                problems.append(f"opening {a} - {b}: no compartment {n}")
        if a in comps and b in comps:
            fa = [f for bx in comps[a] for f in on_faces(bx, at)]
            fb = [f for bx in comps[b] for f in on_faces(bx, at)]
            if not any(x[0] == y[0] and x[1] != y[1] for x in fa for y in fb):
                problems.append(f"opening {a} - {b} at {o['at']}: not on a face they share")
        elif (a in comps) != (b in comps):
            inside = a if a in comps else b
            if not any(on_faces(bx, at) for bx in comps.get(inside, [])):
                problems.append(f"opening to outside from {inside} at {o['at']}: not on its face")
        if o["kind"] in ("door", "pressure-door") and (o["width"] < 0.8 or o["height"] < 2.0):
            problems.append(f"opening {a} - {b}: {o['width']} x {o['height']} m is smaller than a person's door (0.8 x 2.0)")
        links.setdefault(a, set()).add(b)
        links.setdefault(b, set()).add(a)

    # Reachable from outside.
    seen, todo = {"outside"}, ["outside"]
    while todo:
        for n in links.get(todo.pop(), ()):
            if n not in seen:
                seen.add(n)
                todo.append(n)
    for n in names:
        if n not in seen:
            problems.append(f"{n}: no way in from outside")

    # The hull through a compartment.
    print(f"{'compartment':24} {'volume m3':>9} {'floor m2':>8}  hull inside")
    total = 0.0
    for c in layout["compartments"]:
        vol = floor = 0.0
        cut = 0
        worst = 0.0
        by = {}
        for bx in comps[c["name"]]:
            size = bx[:, 1] - bx[:, 0]
            vol += size.prod()
            floor += size[0] * size[2]
            inside = np.all((probes > bx[:, 0] + 0.05) & (probes < bx[:, 1] - 0.05), axis=1)
            cut += int(inside.sum())
            if inside.any():
                p = probes[inside]
                depth = np.minimum(p - bx[:, 0], bx[:, 1] - p).min(axis=1)
                worst = max(worst, float(depth.max()))
                for k, dp in zip(probe_owner[inside], depth):
                    by[mesh_names[k]] = max(by.get(mesh_names[k], 0.0), float(dp))
        total += vol
        top = ", ".join(f"{k} {v:.2f}" for k, v in sorted(by.items(), key=lambda kv: -kv[1])[:4])
        print(f"{c['name']:24} {vol:9.1f} {floor:8.1f}  " + (f"{cut} points, up to {worst:.2f} m in: {top}" if cut else "clear"))
        if cut:
            problems.append(f"{c['name']}: the hull passes through it ({cut} points, up to {worst:.2f} m in)")
    print(f"{'all':24} {total:9.1f}")
    print()
    print("\n".join(problems) if problems else "no problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
