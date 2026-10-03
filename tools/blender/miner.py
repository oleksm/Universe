"""A detailed mining ship, built headless (hard-surface, about 62 m):

    blender -b -P tools/blender/miner.py -- assets/models/miner.glb

The language of a working ship: a faceted bridge forward with raked glazing,
a long chamfered spine in segments with recessed seams, big ore pods on
struts along the sides, a dorsal superstructure with a mast and a dish, a
heavy engine block aft. Large faces broken into armour plates (raised or
recessed, chamfered), seeded greebles along decks and seams (vents, pipes,
hatches, boxes), painted with orange accents and dark metal in the recesses,
markings as text set into the plating, navigation lights and floods.
Follows the import conventions (docs/ship-import.md): nose +Y, up +Z,
COL_* collision, named empties, scene properties.
"""
import sys, math, random
import bpy, bmesh
import numpy as np
from mathutils import Vector, Matrix

out = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "miner.glb"
rnd = random.Random(1984)
bpy.ops.wm.read_factory_settings(use_empty=True)

# ---------------------------------------------------------------- materials
N = 1024
yy, xx = np.mgrid[0:N, 0:N].astype(np.float32) / N
rng = np.random.default_rng(7)

def noise(cells, octaves=4):
    out = np.zeros((N, N), np.float32)
    amp, total = 1.0, 0.0
    for o in range(octaves):
        c = cells * 2 ** o
        g = rng.random((c + 1, c + 1)).astype(np.float32)
        g[:, -1] = g[:, 0]
        g[-1, :] = g[0, :]
        gx, gy = xx * c, yy * c
        ix, iy = gx.astype(int), gy.astype(int)
        fx, fy = gx - ix, gy - iy
        fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
        a = g[iy, ix] * (1 - fx) + g[iy, ix + 1] * fx
        b = g[iy + 1, ix] * (1 - fx) + g[iy + 1, ix + 1] * fx
        out += amp * (a * (1 - fy) + b * fy)
        total += amp
        amp *= 0.5
    return out / total

def image(name, arr, colorspace):
    img = bpy.data.images.new(name, N, N, alpha=True)
    img.colorspace_settings.name = colorspace
    img.pixels = np.flip(np.transpose(arr, (1, 2, 0)), 0).ravel().tolist()
    img.file_format = "PNG"
    img.pack()
    return img

def rgba(*ch):
    return np.stack([c if isinstance(c, np.ndarray) else np.full((N, N), c, np.float32) for c in ch])

# A tiling plate texture (a tile is about 4 m: see the cube projection below):
# fine seams every quarter, grime, wear at the seams, scratches; its normal map.
grime = noise(4, 5)
fine = noise(64, 2)
q = 2.0
sx, sy = xx * q, yy * q
seam = np.minimum(np.abs(sx - np.round(sx)), np.abs(sy - np.round(sy)))
groove = np.clip(1.0 - seam / 0.012, 0, 1) * 0.5
cell = (np.floor(sx) + np.floor(sy) * 13).astype(np.int64)
tone = (np.sin(cell * 12.9898) * 43758.5453) % 1.0
rivet = ((np.abs((sx * 2) % 1 - 0.5) < 0.03) & (np.abs(sy % 1 - 0.06) < 0.02)).astype(np.float32)
scratch = np.clip((noise(128, 1) - 0.86) * 8, 0, 1) * (noise(8, 2) > 0.55)
height = -groove * 0.8 + rivet * 0.5 + fine * 0.06 - scratch * 0.15
dx = (np.roll(height, -1, 1) - np.roll(height, 1, 1)) * 5.0
dy = (np.roll(height, -1, 0) - np.roll(height, 1, 0)) * 5.0
nrm = np.stack([-dx, -dy, np.ones_like(dx)])
nrm /= np.linalg.norm(nrm, axis=0, keepdims=True)
plate_normal = image("plate_n", rgba(*(nrm * 0.5 + 0.5), 1.0), "Non-Color")
wear = groove * 0.5 + scratch

def paint(name, color, rough=0.5, metal=0.0, dirt=0.25):
    shade = (0.95 + 0.05 * tone) * (1 - dirt * (grime - 0.5)) * (1 - 0.25 * groove)
    base = np.stack([np.full((N, N), c, np.float32) for c in color]) * shade
    # Worn to bare metal at the seams and scratches.
    bare = np.clip(wear * 0.8, 0, 1)
    base = base * (1 - bare) + 0.45 * bare
    mr = rgba(1.0, np.clip(rough + 0.15 * grime - 0.25 * bare, 0.05, 1), np.clip(metal + 0.8 * bare, 0, 1), 1.0)
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tb = nt.nodes.new("ShaderNodeTexImage")
    tb.image = image(name + "_c", rgba(*np.clip(base, 0, 1), 1.0), "sRGB")
    nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
    tm = nt.nodes.new("ShaderNodeTexImage")
    tm.image = image(name + "_mr", mr, "Non-Color")
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(tm.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
    tn = nt.nodes.new("ShaderNodeTexImage")
    tn.image = plate_normal
    nm = nt.nodes.new("ShaderNodeNormalMap")
    nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
    nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    return mat

def plain(name, color, rough, metal=0.0, emission=None, strength=0.0):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1)
    bsdf.inputs["Roughness"].default_value = rough
    bsdf.inputs["Metallic"].default_value = metal
    if emission:
        bsdf.inputs["Emission Color"].default_value = (*emission, 1)
        bsdf.inputs["Emission Strength"].default_value = strength
    return mat

M_HULL = paint("hull", (0.80, 0.80, 0.77), rough=0.45, dirt=0.3)
M_ACCENT = paint("accent", (0.86, 0.40, 0.09), rough=0.4, dirt=0.25)
M_DARK = paint("dark", (0.14, 0.15, 0.16), rough=0.55, metal=0.6, dirt=0.15)
M_GLASS = plain("glass", (0.02, 0.03, 0.04), 0.04, emission=(0.9, 0.75, 0.5), strength=0.06)
M_MARK = plain("marking", (0.05, 0.05, 0.06), 0.5)
M_WHITE = plain("lamp_white", (1, 1, 1), 0.3, emission=(1.0, 0.95, 0.85), strength=30.0)
M_RED = plain("lamp_red", (1, 0.1, 0.05), 0.3, emission=(1.0, 0.05, 0.02), strength=25.0)
M_GREEN = plain("lamp_green", (0.1, 1, 0.2), 0.3, emission=(0.05, 1.0, 0.1), strength=25.0)
M_NOZZLE = plain("nozzle", (0.08, 0.07, 0.07), 0.35, metal=1.0, emission=(0.6, 0.75, 1.0), strength=0.0)
M_CHROME = plain("chrome", (0.72, 0.72, 0.70), 0.22, metal=1.0)

# ---------------------------------------------------------------- shapes
parts = []

def obj_from_bm(name, bm, mat):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(mat)
    for m in (M_HULL, M_ACCENT, M_DARK, M_GLASS):
        if m is not mat:
            o.data.materials.append(m)
    return o

def slab(name, size, loc, chamfer=0.6, taper=None, mat=M_HULL):
    """A chamfered box (hard 45° chamfers); `taper`: (front width scale, front height scale, drop) for raked noses."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((v.co.x * size[0], v.co.y * size[1], v.co.z * size[2]))
        if taper and v.co.y > 0:
            v.co.x *= taper[0]
            v.co.z = v.co.z * taper[1] - taper[2]
    bmesh.ops.bevel(bm, geom=list(bm.edges), offset=chamfer, segments=1, affect="EDGES")
    for v in bm.verts:
        v.co += Vector(loc)
    o = obj_from_bm(name, bm, mat)
    parts.append(o)
    return o

def prism(name, sides, radius, length, loc, mat=M_HULL, chamfer=0.25):
    """A prism along Y, a flat on top."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=sides, radius1=radius, radius2=radius, depth=length)
    rot = Matrix.Rotation(math.pi / 2, 4, "X") @ Matrix.Rotation(math.pi / sides, 4, "Z")
    bmesh.ops.transform(bm, matrix=rot, verts=bm.verts)
    bmesh.ops.bevel(bm, geom=[e for e in bm.edges if e.calc_face_angle(0) > 0.5], offset=chamfer, segments=1, affect="EDGES")
    for v in bm.verts:
        v.co += Vector(loc)
    o = obj_from_bm(name, bm, mat)
    parts.append(o)
    return o

def cylinder(name, r1, r2, depth, loc, axis="Y", mat=M_DARK, segs=24):
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=segs, radius1=r1, radius2=r2, depth=depth)
    if axis == "Y":
        bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=Matrix.Rotation(-math.pi / 2, 3, "X"))
    elif axis == "X":
        bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "Y"))
    for v in bm.verts:
        v.co += Vector(loc)
    o = obj_from_bm(name, bm, mat)
    parts.append(o)
    return o

MAINS = (-3.4, 3.4)

# Engine block aft.
slab("engine_block", (15.0, 11.0, 10.0), (0, -25.5, 0.0), chamfer=1.4)
slab("engine_collar", (11.0, 2.0, 8.0), (0, -19.0, 0.3), chamfer=0.8, mat=M_DARK)
# The spine: three segments, seams between them over a narrower core.
slab("spine_core", (8.2, 42.0, 6.6), (0, -1.0, 0.2), chamfer=1.0, mat=M_DARK)
for k, (y, w, h) in enumerate(((-12.5, 10.5, 8.4), (-1.5, 11.0, 8.8), (9.5, 10.5, 8.4))):
    slab("spine_%d" % k, (w, 10.2, h), (0, y, 0.3), chamfer=1.1)
# Ventral keel (gear bays) and a dorsal superstructure with its mast and dish.
slab("keel", (5.5, 30.0, 2.2), (0, -3.0, -4.8), chamfer=0.6, mat=M_DARK)
slab("dorsal", (6.4, 15.0, 3.2), (0, 3.5, 5.4), chamfer=0.9)
slab("dorsal_cap", (4.4, 8.0, 1.4), (0, 4.5, 7.5), chamfer=0.5, mat=M_ACCENT)
cylinder("mast", 0.18, 0.12, 6.0, (0, 1.0, 10.5), axis="Z", mat=M_DARK, segs=8)
cylinder("dish_stem", 0.35, 0.35, 1.2, (0, 7.5, 8.6), axis="Z", mat=M_DARK, segs=12)
dish = cylinder("dish", 2.2, 0.4, 0.6, (0, 7.5, 9.6), axis="Z", mat=M_HULL, segs=32)
# The bridge forward: raked, its glazing on the faces looking forward and up.
bridge = slab("bridge", (9.0, 11.0, 7.0), (0, 20.0, 1.2), chamfer=0.9, taper=(0.62, 0.7, 0.9))
# Ore pods each side on struts.
for sx in (-1, 1):
    prism("pod", 8, 4.6, 22.0, (sx * 11.0, -6.0, -0.6), chamfer=0.45)
    slab("pod_cap_f", (6.4, 1.6, 6.4), (sx * 11.0, 5.6, -0.6), chamfer=0.9, mat=M_ACCENT)
    slab("pod_cap_a", (6.4, 1.6, 6.4), (sx * 11.0, -17.6, -0.6), chamfer=0.9, mat=M_DARK)
    for y in (-12.0, -1.0):
        slab("strut", (4.0, 2.4, 1.6), (sx * 6.8, y, 0.4), chamfer=0.4, mat=M_DARK)
        slab("strut_low", (4.0, 1.2, 1.0), (sx * 6.8, y, -2.4), chamfer=0.3, mat=M_DARK)
    # Pod thrusters aft.
    cylinder("pod_bell", 1.3, 2.0, 2.6, (sx * 11.0, -19.6, -0.6), axis="Y", mat=M_NOZZLE)
# Two main bells aft.
for x in MAINS:
    cylinder("main_bell", 1.9, 2.9, 3.8, (x, -32.8, 0.0), axis="Y", mat=M_NOZZLE)
    cylinder("main_throat", 1.4, 1.4, 1.2, (x, -31.0, 0.0), axis="Y", mat=M_DARK)

# ---------------------------------------------------------------- armour plates
def plate(o, cuts_per_m=0.3, min_area=4.0):
    """Big faces cut into a grid of plates, each raised, recessed or flush (seeded), chamfered rims dark."""
    bm = bmesh.new()
    bm.from_mesh(o.data)
    big = [f for f in bm.faces if f.calc_area() > min_area and f.material_index != 3]
    for f in big:
        f.select = True
    edges = list({e for f in big for e in f.edges})
    if edges:
        span = max(max(e.calc_length() for e in edges), 1.0)
        cuts = max(1, min(6, int(span * cuts_per_m)))
        bmesh.ops.subdivide_edges(bm, edges=edges, cuts=cuts, use_grid_fill=True)
    faces = [f for f in bm.faces if f.select and f.calc_area() > 1.0]
    for f in faces:
        r = rnd.random()
        if r < 0.55:
            res = bmesh.ops.inset_individual(bm, faces=[f], thickness=0.16, depth=0.2, use_even_offset=True)
        elif r < 0.8:
            res = bmesh.ops.inset_individual(bm, faces=[f], thickness=0.22, depth=-0.18, use_even_offset=True)
        else:
            continue
        # The plate's rim in dark metal.
        for rf in res.get("faces", []):
            if rf is not f:
                rf.material_index = 2
        if rnd.random() < 0.07:
            f.material_index = 1
    bm.to_mesh(o.data)
    bm.free()

for o in list(parts):
    if o.data.materials[0] in (M_HULL, M_ACCENT) and o.name not in ("dish",):
        plate(o)

# Glazing: the bridge's faces looking forward and up, inset in a dark frame.
bm = bmesh.new()
bm.from_mesh(bridge.data)
glass = [f for f in bm.faces if f.normal.y > 0.35 and f.normal.z > -0.2 and f.calc_center_median().z > 0.6]
res = bmesh.ops.inset_individual(bm, faces=glass, thickness=0.22, depth=-0.06, use_even_offset=True)
for f in glass:
    f.material_index = 3
for rf in res.get("faces", []):
    if rf not in glass:
        rf.material_index = 2
bm.to_mesh(bridge.data)
bm.free()

# ---------------------------------------------------------------- greebles
def faces_world():
    out = []
    for o in parts:
        if o.data.materials[0] is M_NOZZLE:
            continue
        mw = o.matrix_world
        for p in o.data.polygons:
            if p.area > 2.0 and p.material_index in (0, 1):
                out.append((mw @ p.center, (mw.to_3x3() @ p.normal).normalized(), p.area))
    return out

spots = faces_world()
weights = [a for _, _, a in spots]
greebles = []
for k in range(420):
    c, n, a = rnd.choices(spots, weights)[0]
    if n.z < -0.6:
        continue
    # (Kept well inside its face: never hanging past an edge.)
    reach = math.sqrt(a) * 0.3
    # (Laid along the ship where the face allows: detailing runs fore and aft.)
    along = Vector((0, 1, 0)) - n * n.y
    t = along.normalized() if along.length > 0.3 else n.orthogonal().normalized()
    b = n.cross(t)
    off = t * rnd.uniform(-reach, reach) + b * rnd.uniform(-reach, reach)
    kind = rnd.random()
    bm = bmesh.new()
    if kind < 0.45:
        # A box: hatch, junction, housing.
        s = Vector((rnd.uniform(0.3, 1.0), rnd.uniform(0.3, 1.0), rnd.uniform(0.1, 0.5))) * min(1.0, math.sqrt(a) * 0.4)
        bmesh.ops.create_cube(bm, size=1.0)
        for v in bm.verts:
            v.co = Vector((v.co.x * s.x, v.co.y * s.y, v.co.z * s.z + s.z / 2))
        bmesh.ops.bevel(bm, geom=list(bm.edges), offset=min(s) * 0.15, segments=1, affect="EDGES")
    elif kind < 0.7:
        # A vent: a short cylinder.
        r = rnd.uniform(0.15, 0.5)
        bmesh.ops.create_cone(bm, cap_ends=True, segments=10, radius1=r, radius2=r * 0.85, depth=rnd.uniform(0.1, 0.4))
        for v in bm.verts:
            v.co.z += 0.1
    else:
        # A pipe run along the surface.
        L = rnd.uniform(0.4, 0.8) * math.sqrt(a)
        r = rnd.uniform(0.06, 0.16)
        bmesh.ops.create_cone(bm, cap_ends=True, segments=8, radius1=r, radius2=r, depth=L)
        bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "Y"))
        for v in bm.verts:
            v.co.z += r + 0.04
    m = Matrix((t, b, n)).transposed()
    for v in bm.verts:
        v.co = m @ v.co + c + off
    greebles.append(bm)

gb = bmesh.new()
for bm in greebles:
    me = bpy.data.meshes.new("tmp")
    bm.to_mesh(me)
    bm.free()
    gb.from_mesh(me)
    bpy.data.meshes.remove(me)
g = obj_from_bm("greebles", gb, M_DARK)
parts.append(g)

# ---------------------------------------------------------------- markings
def marking(text, loc, normal, size=1.4, mat=M_MARK):
    cu = bpy.data.curves.new("mark", "FONT")
    cu.body = text
    cu.size = size
    cu.extrude = 0.0
    # (One face, toward the outside: flat text filled both sides draws a mirrored twin.)
    cu.fill_mode = "FRONT"
    cu.align_x = "CENTER"
    o = bpy.data.objects.new("mark", cu)
    bpy.context.scene.collection.objects.link(o)
    n = Vector(normal).normalized()
    up = Vector((0, 0, 1)) if abs(n.z) < 0.9 else Vector((0, 1, 0))
    # (Reading left to right from outside, on either side: x = up × n.)
    x = up.cross(n).normalized()
    y = n.cross(x).normalized()
    # On its placard: a thin panel proud of any plate (0.26 m out), the text 4 mm on it,
    # so nothing floats over the plating casting a ghost of itself.
    lift = 0.26
    o.matrix_world = Matrix((x, y, n)).transposed().to_4x4()
    o.location = Vector(loc) + n * (lift + 0.004)
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
    bpy.ops.object.convert(target="MESH")
    o.data.materials.clear()
    o.data.materials.append(mat)
    o.select_set(False)
    parts.append(o)
    # (The text's extent in its own frame, with a margin.)
    xs = [v.co.x for v in o.data.vertices]
    ys = [v.co.y for v in o.data.vertices]
    w, h = max(xs) - min(xs) + size * 0.5, max(ys) - min(ys) + size * 0.4
    cx, cy = (max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        # (Sunk 0.3 m into the hull too: over a recessed plate it still sits on metal.)
        v.co = Vector((cx + v.co.x * w, cy + v.co.y * h, v.co.z * (lift + 0.3) + (lift - 0.3) / 2))
    bmesh.ops.bevel(bm, geom=[e for e in bm.edges], offset=min(0.05, lift * 0.3), segments=1, affect="EDGES")
    basis = Matrix((x, y, n)).transposed()
    for v in bm.verts:
        v.co = basis @ v.co + Vector(loc)
    parts.append(obj_from_bm("placard", bm, M_HULL))

# (On the plating, just proud of a raised plate (0.2 m): any farther and they
# float, casting their own shadow on the hull. A pod's side face is its
# radius × cos 22.5° out; a slab's, its half width.)
pod_face = 11.0 + 4.6 * math.cos(math.pi / 8)
for sx in (-1, 1):
    marking("HADLEY  OC-7", (sx * pod_face, -6.0, -2.6), (sx, 0, 0), size=1.1)
    marking("ORE CUTTER", (sx * 5.25, 9.5, -1.2), (sx, 0, 0), size=0.8)
    marking("07", (sx * pod_face, -6.0, 0.6), (sx, 0, 0), size=3.0)
    # (Low on the engine block's flat side, clear of its chamfers: z -3.6..3.6.)
    marking("MASS LIMIT 900 T", (sx * 7.5, -25.5, -3.0), (sx, 0, 0), size=0.5)

# ---------------------------------------------------------------- landing gear, hatch
# It sets down standing on its belly side, so legs, not wheels: four struts
# splayed out to round footpads (a shock absorber in each, braced fore and
# aft), each out of a bay whose doors stand open beside it. Two under the
# keel forward, two under the ore pods aft: a wide stance. The feet are
# at FOOT (the gear_* contacts: where the ship stands on the ground).
FOOT = -10.8

def rod(name, a, b, r, mat=M_DARK, segs=12):
    a, b = Vector(a), Vector(b)
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=segs, radius1=r, radius2=r, depth=(b - a).length)
    rot = Vector((0, 0, 1)).rotation_difference(b - a).to_matrix().to_4x4()
    bmesh.ops.transform(bm, matrix=Matrix.Translation((a + b) / 2) @ rot, verts=bm.verts)
    o = obj_from_bm(name, bm, mat)
    parts.append(o)

def panel(name, size, at, tilt_y=0.0, mat=M_DARK):
    """A thin box, turned `tilt_y` about the ship's length (fore-aft) axis."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co = Vector((v.co.x * size[0], v.co.y * size[1], v.co.z * size[2]))
    bmesh.ops.bevel(bm, geom=list(bm.edges), offset=min(size) * 0.3, segments=1, affect="EDGES")
    bmesh.ops.transform(bm, matrix=Matrix.Translation(at) @ Matrix.Rotation(tilt_y, 4, "Y"), verts=bm.verts)
    o = obj_from_bm(name, bm, mat)
    parts.append(o)

LEGS = [((sx * 2.2, 6.0, -5.9), (sx * 6.5, 8.0)) for sx in (-1, 1)] + [((sx * 11.0, -13.0, -4.85), (sx * 14.5, -14.0)) for sx in (-1, 1)]
for mount, (fx, fy) in LEGS:
    m = Vector(mount)
    knee = Vector((fx, fy, FOOT + 0.85))
    side = 1 if fx > 0 else -1
    # The bay: a dark well, its two doors open, hanging either side of the strut.
    panel("gear_bay", (2.4, 3.4, 0.2), (m.x, m.y, m.z + 0.02))
    for d in (-1, 1):
        panel("gear_door", (0.12, 3.2, 1.7), (m.x + d * 1.3, m.y, m.z - 0.8), tilt_y=-d * 0.25, mat=M_HULL)
    # The strut: a sleeve, the piston out of it, a knuckle and the pad.
    rod("gear_sleeve", m, m.lerp(knee, 0.58), 0.45)
    rod("gear_piston", m.lerp(knee, 0.5), knee, 0.28, mat=M_CHROME)
    for dy in (-2.6, 2.6):
        rod("gear_brace", (m.x - side * 0.4, m.y + dy, m.z + 0.1), m.lerp(knee, 0.72), 0.16)
    cylinder("gear_knuckle", 0.5, 0.5, 0.6, (fx, fy, FOOT + 0.85), axis="X", mat=M_DARK, segs=12)
    cylinder("gear_pad", 1.7, 1.25, 0.45, (fx, fy, FOOT + 0.225), axis="Z", mat=M_DARK, segs=24)
    cylinder("gear_pad_hub", 0.6, 0.45, 0.5, (fx, fy, FOOT + 0.65), axis="Z", mat=M_DARK, segs=12)

# The crew hatch, in the keel's belly (a stair drops aft from it, landed):
# a dark frame, the door inset, a lamp over it.
HATCH = (0.0, -4.0, -5.9)
panel("hatch_frame", (2.0, 3.0, 0.12), (HATCH[0], HATCH[1], HATCH[2] - 0.04))
panel("hatch_door", (1.6, 2.6, 0.1), (HATCH[0], HATCH[1], HATCH[2] - 0.07), mat=M_ACCENT)

# ---------------------------------------------------------------- lights
def lamp(loc, mat, r=0.18):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=8, v_segments=6, radius=r)
    for v in bm.verts:
        v.co += Vector(loc)
    o = obj_from_bm("lamp", bm, mat)
    parts.append(o)

lamp((-15.7, -6.0, 3.2), M_RED)
lamp((15.7, -6.0, 3.2), M_GREEN)
lamp((0, 1.0, 13.6), M_WHITE, r=0.15)
for y in (-20.0, -10.0, 0.0, 10.0):
    lamp((-5.6, y, 4.5), M_WHITE, r=0.1)
    lamp((5.6, y, 4.5), M_WHITE, r=0.1)
lamp((0, 25.4, -1.0), M_WHITE, r=0.25)
lamp((0, HATCH[1] + 1.8, HATCH[2] - 0.12), M_WHITE, r=0.12)

# ---------------------------------------------------------------- join, UVs, shading
bpy.ops.object.select_all(action="DESELECT")
for o in parts:
    o.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.join()
hull = bpy.context.active_object
hull.name = "hull"
bpy.ops.object.shade_auto_smooth(angle=math.radians(30))
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
# Box projection at real scale: a texture tile is 4 m on every face, seams the same size everywhere.
bpy.ops.uv.cube_project(cube_size=4.0, correct_aspect=True, scale_to_bounds=False)
bpy.ops.object.mode_set(mode="OBJECT")
# (cube_project's cube_size scales by 1/size in its own way: normalise the UVs to 1 tile per 4 m.)
me = hull.data
uv = me.uv_layers.active.data
lo = Vector((1e9, 1e9))
for l in uv:
    lo.x = min(lo.x, l.uv.x)
    lo.y = min(lo.y, l.uv.y)

# ---------------------------------------------------------------- conventions
def collider(name, size, loc):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    o = bpy.context.active_object
    o.name = name
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)

collider("COL_body", (11.0, 50.0, 9.0), (0, 0.5, 0.3))
collider("COL_engine", (15.0, 13.0, 10.0), (0, -26.5, 0.0))
collider("COL_dorsal", (6.4, 15.0, 4.6), (0, 3.5, 6.1))
for sx in (-1, 1):
    collider("COL_pod_%s" % ("l" if sx < 0 else "r"), (9.2, 25.0, 9.2), (sx * 11.0, -6.0, -0.6))

def empty(name, loc, toward):
    bpy.ops.object.empty_add(type="ARROWS", location=loc)
    o = bpy.context.active_object
    o.name = name
    o.rotation_mode = "QUATERNION"
    o.rotation_quaternion = Vector((0, 1, 0)).rotation_difference(Vector(toward))

for k, x in enumerate(MAINS):
    empty("nozzle_main_%d" % k, (x, -34.7, 0.0), (0, -1, 0))
for k, sx in enumerate((-1, 1)):
    empty("nozzle_main_pod_%d" % k, (sx * 11.0, -21.0, -0.6), (0, -1, 0))
for end, y in (("nose", 22.0), ("tail", -24.0)):
    for side, sx in (("left", -1), ("right", 1)):
        base = (sx * (4.2 if end == "nose" else 7.6), y, 3.0)
        for d, v in (("up", (0, 0, 1)), ("down", (0, 0, -1)), ("side", (sx, 0, 0)), ("fore", (0, 1, 0)), ("aft", (0, -1, 0))):
            empty("nozzle_%s_%s_%s" % (end, side, d), base, v)
for k, (x, y) in enumerate(((-2.0, 10.0), (2.0, 10.0), (-2.0, -16.0), (2.0, -16.0), (-11.0, -6.0), (11.0, -6.0))):
    empty("nozzle_lift_%d" % k, (x, y, -6.0 if abs(x) < 5 else -5.3), (0, 0, -1))
for k, (_, (x, y)) in enumerate(LEGS):
    empty("gear_%d" % k, (x, y, FOOT), (0, 0, -1))
empty("hatch", (HATCH[0], HATCH[1], HATCH[2] - 0.1), (0, 0, -1))
empty("cockpit", (0, 22.5, 2.6), (0, 1, 0))
empty("mount_hardpoint_1", (-3.0, 22.0, -2.0), (0, 1, 0))
empty("mount_hardpoint_2", (3.0, 22.0, -2.0), (0, 1, 0))
empty("mount_cargo", (-11.0, -6.0, -0.6), (0, 1, 0))
empty("mount_cargo_2", (11.0, -6.0, -0.6), (0, 1, 0))
empty("mount_utility", (0, 24.0, -3.0), (0, 1, 0))
bpy.context.scene["freefall_name"] = "ORE CUTTER"
bpy.context.scene["freefall_class"] = 3

bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_tangents=True, export_apply=True, export_yup=True, export_extras=True)
print("wrote", out, len(hull.data.polygons), "faces")
