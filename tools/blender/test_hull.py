"""A test hull for the PBR pipeline, built headless:

    blender -b -P tools/blender/test_hull.py -- assets/models/test_hull.glb

Hard-surface, in the language of a working ship: a chamfered octagonal body,
a cockpit block forward, engine pods each side, a dorsal block, nozzles aft.
Textures are made here (numpy): painted plates with seams, hazard stripes and
grime (base colour), the seams and raised plates (normal map), paint vs bare
metal (metallic-roughness), small lamps (emission). Exported as glTF binary,
with tangents. Units: metres; the ship's nose along -Y in Blender (glTF +Z forward... see export axis).
"""
import sys, math
import bpy, bmesh
import numpy as np

out = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "test_hull.glb"
rng = np.random.default_rng(1984)

bpy.ops.wm.read_factory_settings(use_empty=True)

def prism(name, sides, radius, length, loc, rot=(0, 0, 0), scale=(1, 1, 1), bevel=0.15):
    bpy.ops.mesh.primitive_cylinder_add(vertices=sides, radius=radius, depth=length, location=loc)
    o = bpy.context.active_object
    o.name = name
    # (Its spin about its own axis first, z, then laid along the ship, x: order ZXY.)
    o.rotation_mode = "ZXY"
    o.rotation_euler = rot
    o.scale = scale
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
    m = o.modifiers.new("bevel", "BEVEL")
    m.width = bevel
    m.segments = 2
    m.limit_method = "ANGLE"
    return o

def block(name, size, loc, bevel=0.12):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    o = bpy.context.active_object
    o.name = name
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    m = o.modifiers.new("bevel", "BEVEL")
    m.width = bevel
    m.segments = 2
    m.limit_method = "ANGLE"
    return o

# The body: an octagonal prism along Y (nose at -Y), flattened a little.
# (Scaled in its own frame before it's turned: local z is its length, local y becomes up.)
parts = [prism("body", 8, 3.2, 26.0, (0, 0, 0), rot=(math.pi / 2, 0, math.pi / 8), scale=(1.0, 0.8, 1.0), bevel=0.25)]
# Cockpit block forward, raked: a box with its front cut by a slanted cube later (kept simple: two blocks).
parts.append(block("cockpit", (4.6, 4.2, 3.2), (0, -14.5, 0.6)))
parts.append(block("brow", (3.8, 2.0, 1.4), (0, -15.8, 2.2), bevel=0.2))
# Dorsal block and a spine of plates.
parts.append(block("dorsal", (3.6, 9.0, 1.6), (0, -2.0, 3.0)))
parts.append(block("dorsal_aft", (2.6, 5.0, 1.2), (0, 6.0, 2.8)))
# Engine pods each side on short pylons.
for x in (-5.6, 5.6):
    parts.append(prism("pod", 8, 1.9, 11.0, (x, 7.0, -0.4), rot=(math.pi / 2, 0, math.pi / 8), bevel=0.18))
    parts.append(block("pylon", (2.4, 4.0, 0.9), (x * 0.68, 7.0, -0.4)))
    parts.append(prism("nozzle", 12, 1.2, 1.6, (x, 13.2, -0.4), rot=(math.pi / 2, 0, 0), bevel=0.06))
# Main nozzles aft.
for x in (-1.4, 1.4):
    parts.append(prism("main", 12, 1.1, 1.8, (x, 13.6, -0.2), rot=(math.pi / 2, 0, 0), bevel=0.06))
# Side equipment boxes along the body.
for y in (-6.0, 0.0):
    for x in (-3.2, 3.2):
        parts.append(block("box", (0.9, 4.2, 2.0), (x, y, -0.3), bevel=0.1))

for o in parts:
    bpy.context.view_layer.objects.active = o
    for m in list(o.modifiers):
        bpy.ops.object.modifier_apply(modifier=m.name)
bpy.ops.object.select_all(action="DESELECT")
for o in parts:
    o.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.join()
hull = bpy.context.active_object
hull.name = "hull"

# Turned to the convention: the nose along +Y (it was built nose to -Y).
hull.rotation_euler = (0, 0, math.pi)
bpy.ops.object.transform_apply(rotation=True)

# Flat-ish shading with sharp chamfers: smooth by angle.
bpy.ops.object.shade_auto_smooth(angle=math.radians(35))

# UVs: islands per flat region, packed.
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.uv.smart_project(angle_limit=math.radians(40), island_margin=0.004, scale_to_bounds=True)
bpy.ops.object.mode_set(mode="OBJECT")

# --- Textures (numpy), 2048².
N = 2048
y, x = np.mgrid[0:N, 0:N].astype(np.float32) / N

def smooth_noise(cells, octaves=4):
    out = np.zeros((N, N), np.float32)
    amp, total = 1.0, 0.0
    for o in range(octaves):
        c = cells * 2 ** o
        g = rng.random((c + 1, c + 1)).astype(np.float32)
        gx, gy = x * c, y * c
        ix, iy = gx.astype(int), gy.astype(int)
        fx, fy = gx - ix, gy - iy
        fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
        a = g[iy, ix] * (1 - fx) + g[iy, ix + 1] * fx
        b = g[iy + 1, ix] * (1 - fx) + g[iy + 1, ix + 1] * fx
        out += amp * (a * (1 - fy) + b * fy)
        total += amp
        amp *= 0.5
    return out / total

# Plates: a grid of panels in UV space, each its own small tone, some raised.
cells = 24
px, py = (x * cells), (y * cells * 0.7)
cell_id = (np.floor(px) + np.floor(py) * 997).astype(np.int64)
tone = (np.sin(cell_id * 12.9898) * 43758.5453) % 1.0
seam = np.minimum(np.abs(px - np.round(px)), np.abs(py - np.round(py)))
groove = np.clip(1.0 - seam / 0.025, 0, 1)
raised = (tone > 0.72).astype(np.float32) * (1 - groove)
height = -groove * 1.0 + raised * 0.4 + smooth_noise(64, 2) * 0.05

grime = smooth_noise(6, 5)
paint = np.stack([0.80, 0.80, 0.78]) [:, None, None] * (0.93 + 0.07 * tone) * (0.88 + 0.12 * grime)
# Orange hazard stripes on some panels.
stripe = ((tone > 0.9) & (((px + py) * 6 % 1.0) < 0.5)).astype(np.float32)
orange = np.stack([0.85, 0.42, 0.10])[:, None, None]
base = paint * (1 - stripe) + orange * stripe
base = base * (1 - 0.45 * groove)
# Dark decal blocks (markings).
deco = ((tone > 0.55) & (tone < 0.58)).astype(np.float32) * ((px % 1 > 0.2) & (px % 1 < 0.8) & (py % 1 > 0.4) & (py % 1 < 0.55))
base = base * (1 - 0.8 * deco)
base_img = np.concatenate([np.clip(base, 0, 1), np.ones((1, N, N), np.float32)], 0)

# Normal map from the height.
dx = (np.roll(height, -1, 1) - np.roll(height, 1, 1)) * 6.0
dy = (np.roll(height, -1, 0) - np.roll(height, 1, 0)) * 6.0
nrm = np.stack([-dx, -dy, np.ones_like(dx)])
nrm /= np.linalg.norm(nrm, axis=0, keepdims=True)
normal_img = np.concatenate([nrm * 0.5 + 0.5, np.ones((1, N, N), np.float32)], 0)

# Metallic-roughness: paint (rough 0.45, not metal); seams and some plates bare metal.
bare = ((tone < 0.08) | (groove > 0.5)).astype(np.float32)
rough = 0.42 + 0.15 * grime + 0.2 * groove - 0.15 * bare
metal = bare * 0.9
mr_img = np.stack([np.ones_like(rough), np.clip(rough, 0, 1), np.clip(metal, 0, 1), np.ones_like(rough)])

# Emission: small lamps in some panels.
lamp = ((tone > 0.985) & (np.abs(px % 1 - 0.5) < 0.06) & (np.abs(py % 1 - 0.5) < 0.04)).astype(np.float32)
em_img = np.stack([lamp * 1.0, lamp * 0.95, lamp * 0.85, np.ones_like(lamp)])

def image(name, arr, colorspace):
    img = bpy.data.images.new(name, N, N, alpha=True)
    img.colorspace_settings.name = colorspace
    # Blender's pixels go bottom up.
    img.pixels = np.flip(np.transpose(arr, (1, 2, 0)), 0).ravel().tolist()
    img.file_format = "PNG"
    img.pack()
    return img

mat = bpy.data.materials.new("hull_paint")
mat.use_nodes = True
nt = mat.node_tree
bsdf = nt.nodes["Principled BSDF"]
def tex(img):
    n = nt.nodes.new("ShaderNodeTexImage")
    n.image = img
    return n
tb = tex(image("base", base_img, "sRGB"))
nt.links.new(tb.outputs["Color"], bsdf.inputs["Base Color"])
tm = tex(image("mr", mr_img, "Non-Color"))
sep = nt.nodes.new("ShaderNodeSeparateColor")
nt.links.new(tm.outputs["Color"], sep.inputs["Color"])
nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
tn = tex(image("normal", normal_img, "Non-Color"))
nm = nt.nodes.new("ShaderNodeNormalMap")
nt.links.new(tn.outputs["Color"], nm.inputs["Color"])
nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
te = tex(image("emission", em_img, "sRGB"))
nt.links.new(te.outputs["Color"], bsdf.inputs["Emission Color"])
bsdf.inputs["Emission Strength"].default_value = 4.0
hull.data.materials.append(mat)

# --- The hull's conventions (see crates/core/world/src/import.rs).
def collider(name, size, loc):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    o = bpy.context.active_object
    o.name = name
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    o.display_type = "WIRE"
collider("COL_body", (6.6, 31.0, 6.0), (0, 1.5, 0.6))
for x in (-5.6, 5.6):
    collider("COL_pod_%s" % ("l" if x < 0 else "r"), (3.9, 11.5, 3.9), (x, -7.0, -0.4))

def empty(name, loc, toward):
    """An empty whose +Y arrow points `toward` (a unit axis: exhaust, way out, view)."""
    bpy.ops.object.empty_add(type="ARROWS", location=loc)
    o = bpy.context.active_object
    o.name = name
    o.rotation_mode = "QUATERNION"
    from mathutils import Vector
    o.rotation_quaternion = Vector((0, 1, 0)).rotation_difference(Vector(toward))
    return o

# Main drive aft (the nose is +Y, so aft is -Y): exhaust astern.
for k, x in enumerate((-1.4, 1.4)):
    empty("nozzle_main_%d" % k, (x, -14.4, -0.2), (0, -1, 0))
# Manoeuvring quads at the four corners: up, down, out, fore, aft.
for end, y in (("nose", 11.0), ("tail", -10.0)):
    for side, sx in (("left", -1), ("right", 1)):
        base = (sx * 3.4, y, 1.2)
        for d, v in (("up", (0, 0, 1)), ("down", (0, 0, -1)), ("side", (sx, 0, 0)), ("fore", (0, 1, 0)), ("aft", (0, -1, 0))):
            empty("nozzle_%s_%s_%s" % (end, side, d), base, v)
# Belly lift jets: exhaust down.
for k, (x, y) in enumerate(((-2.0, 8.0), (2.0, 8.0), (-2.0, -6.0), (2.0, -6.0))):
    empty("nozzle_lift_%d" % k, (x, y, -2.6), (0, 0, -1))
# Landing gear under it.
for k, (x, y) in enumerate(((-2.2, 9.0), (2.2, 9.0), (0.0, -8.0))):
    empty("gear_%d" % k, (x, y, -2.7), (0, 0, -1))
empty("cockpit", (0, 14.5, 1.0), (0, 1, 0))
empty("mount_hardpoint_1", (-2.0, 12.0, -2.4), (0, 1, 0))
empty("mount_hardpoint_2", (2.0, 12.0, -2.4), (0, 1, 0))
empty("mount_cargo", (0, -1.0, 0.0), (0, 1, 0))
empty("mount_utility", (0, 2.0, 2.6), (0, 1, 0))
bpy.context.scene["freefall_name"] = "TEST MINER"
bpy.context.scene["freefall_class"] = 2

bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_tangents=True, export_apply=True, export_yup=True, export_extras=True)
print("wrote", out, len(hull.data.polygons), "faces")
