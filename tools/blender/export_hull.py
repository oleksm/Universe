"""A hull designed in Blender, into the game: its .blend as it stands, exported as a .glb with
the import conventions (docs/ship-import.md).

    blender -b -y design.blend -P tools/blender/export_hull.py -- assets/models/out.glb \
        [--frame 50] [--name "MC-07"] [--class 3] [--bake 4096] [--atlases 1]

Run it again whenever the design changes. The .blend isn't changed (nothing is saved).

- **The pose:** the scene at `--frame` as Blender has it there (its rig's state: gear down,
  doors shut, say). `-y` lets a rig's Python drivers run, as they do in the open file.
- **Left out:** lights, cameras, and whatever doesn't render (boolean cutters, volumes).
- **Conventions:** nodes the file has (`COL_*` meshes, `nozzle_*`, `gear_*`, `hatch`,
  `cockpit`, `mount_*` empties) are used as they are. Any kind it hasn't, this places from the
  parts' names and materials, and prints where:
    - `gear_*`: under each mesh named `*_Pad` (a landing foot), at its lowest point;
    - `nozzle_main_*`: each island of faces in an `*EngineGlow*` material, firing aft;
    - `cockpit`: the middle of the `*Glass*` faces farthest forward, looking forward;
    - `hatch`: the top of a mesh named `*Ramp*`, where it meets the belly, pointing down the
      ramp (the way out);
    - `mount_hardpoint_*`: each mesh named `*Laser*_Head`; `mount_cargo`, `mount_utility`;
    - `nozzle_lift_*`: under the hull, six, firing down; manoeuvring thrusters in quads,
      at the nose and the tail;
    - `COL_*`: a box round each big `Hull_*` mesh that isn't mostly inside the others
      (convex parts for contact and mass; overlaps count twice, so kept few).
- **Scene properties:** `freefall_name` and `freefall_class` from the arguments (or the file's).
- **Baked** (`--bake 4096`, the maps' size; `--bake 0`: not): node-made materials, which glTF
  can't hold, rendered into atlases the meshes share (`--atlases N`, balanced by surface; base colour, roughness, metalness,
  normal, ambient occlusion). Lamps, glows and glass keep their own materials.
"""
import sys
import bpy, bmesh
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
out = argv[0] if argv else "hull.glb"
opt = {argv[i]: argv[i + 1] for i in range(1, len(argv) - 1) if argv[i].startswith("--")}
scene = bpy.context.scene
if "--frame" in opt:
    scene.frame_set(int(opt["--frame"]))
    # Held there: each animated value as it is at that frame, the animation taken off (the
    # exporter evaluates the scene again, at a frame of its own choosing, animated).
    for o in scene.objects:
        ad = o.animation_data
        if not ad or not ad.action:
            continue
        a = ad.action
        try:
            curves = list(a.fcurves)
        except AttributeError:
            curves = [fc for layer in a.layers for strip in layer.strips for bag in strip.channelbags for fc in bag.fcurves]
        held = []
        for fc in curves:
            # (The value as the scene has it at this frame, not the curve's: a curve Blender
            # isn't applying would force a pose the scene doesn't show, as the MC-07's demo
            # sequence stowed its jackhammers while the file had them deployed.)
            try:
                if fc.data_path.startswith('["'):
                    v = o[fc.data_path[2:-2]]
                else:
                    target = o.path_resolve(fc.data_path)
                    v = target[fc.array_index] if hasattr(target, "__len__") and not isinstance(target, str) else target
                held.append((fc.data_path, fc.array_index, v))
            except Exception:
                pass
        ad.action = None
        for path, i, v in held:
            try:
                if path.startswith('["'):
                    # (A custom property: the rig's controls, read by its drivers.)
                    o[path[2:-2]] = v
                    continue
                target = o.path_resolve(path)
                if hasattr(target, "__len__") and not isinstance(target, str):
                    target[i] = v
                else:
                    setattr(o, path, v)
            except Exception as e:
                print("couldn't hold", o.name, path, e)
    bpy.context.view_layer.update()
if "--name" in opt:
    scene["freefall_name"] = opt["--name"]
if "--class" in opt:
    scene["freefall_class"] = int(opt["--class"])
dg = bpy.context.evaluated_depsgraph_get()


def shown(o):
    return o.visible_get() and not o.hide_render and o.type in ("MESH", "CURVE", "FONT", "SURFACE", "META")


def world_points(o):
    e = o.evaluated_get(dg)
    m = e.to_mesh()
    pts = [o.matrix_world @ v.co for v in m.vertices]
    e.to_mesh_clear()
    return pts


def box(o):
    p = world_points(o)
    return Vector([min(v[i] for v in p) for i in range(3)]), Vector([max(v[i] for v in p) for i in range(3)])


have = {o.name for o in scene.objects}
has = lambda prefix: any(n.startswith(prefix) for n in have)
meshes = [o for o in scene.objects if o.type == "MESH" and shown(o)]
made = []


def empty(name, at, toward):
    o = bpy.data.objects.new(name, None)
    o.empty_display_type = "ARROWS"
    scene.collection.objects.link(o)
    o.location = at
    o.rotation_mode = "QUATERNION"
    o.rotation_quaternion = Vector((0, 1, 0)).rotation_difference(Vector(toward))
    made.append((name, tuple(round(c, 2) for c in at)))


# The ship's extent (what renders).
lo, hi = Vector((1e9,) * 3), Vector((-1e9,) * 3)
for o in meshes:
    a, b = box(o)
    lo = Vector(map(min, lo, a))
    hi = Vector(map(max, hi, b))

if not has("gear_"):
    for k, o in enumerate(sorted((o for o in meshes if o.name.endswith("_Pad") and "Gear" in o.name), key=lambda o: o.name)):
        a, b = box(o)
        empty("gear_%d" % k, ((a.x + b.x) / 2, (a.y + b.y) / 2, a.z), (0, 0, -1))

if not has("nozzle_main"):
    k = 0
    for o in meshes:
        slots = [i for i, m in enumerate(o.data.materials) if m and "EngineGlow" in m.name]
        if not slots:
            continue
        bm = bmesh.new()
        bm.from_object(o, dg)
        bm.transform(o.matrix_world)
        faces = {f for f in bm.faces if f.material_index in slots}
        while faces:
            # (One island: faces joined by their edges.)
            seed = faces.pop()
            island, todo = [seed], [seed]
            while todo:
                f = todo.pop()
                for e in f.edges:
                    for g in e.link_faces:
                        if g in faces:
                            faces.discard(g)
                            island.append(g)
                            todo.append(g)
            c = sum((f.calc_center_median() * f.calc_area() for f in island), Vector()) / max(sum(f.calc_area() for f in island), 1e-9)
            empty("nozzle_main_%d" % k, (c.x, c.y - 0.2, c.z), (0, -1, 0))
            k += 1
        bm.free()

if not has("cockpit"):
    glass = []
    for o in meshes:
        slots = [i for i, m in enumerate(o.data.materials) if m and "Glass" in m.name]
        if not slots:
            continue
        bm = bmesh.new()
        bm.from_object(o, dg)
        bm.transform(o.matrix_world)
        glass += [f.calc_center_median() for f in bm.faces if f.material_index in slots]
        bm.free()
    if glass:
        front = max(p.y for p in glass)
        near = [p for p in glass if p.y > front - 6.0]
        c = sum(near, Vector()) / len(near)
        empty("cockpit", (0.0, c.y - 2.0, c.z - 0.6), (0, 1, 0))

if not has("hatch"):
    ramps = [o for o in meshes if "Ramp" in o.name and o.name.split("_")[0].endswith("Ramp")]
    if ramps:
        # (Its top, where it meets the belly: shut or lowered, that's the way in.)
        # (Pointing down it to its foot when it's lowered: the way out.)
        o = ramps[0]
        pts = world_points(o)
        top, foot = max(pts, key=lambda p: p.z), min(pts, key=lambda p: p.z)
        a, b = box(o)
        way = Vector((0.0, foot.y - top.y, foot.z - top.z))
        empty("hatch", ((a.x + b.x) / 2, top.y, b.z), way if way.length > 1.0 else (0, 0, -1))

if not has("mount_"):
    heads = sorted((o for o in meshes if "Laser" in o.name and o.name.endswith("_Head")), key=lambda o: o.name)
    for k, o in enumerate(heads):
        a, b = box(o)
        empty("mount_hardpoint_%d" % (k + 1), ((a.x + b.x) / 2, (a.y + b.y) / 2, (a.z + b.z) / 2), (0, 1, 0))
    mid = (lo + hi) / 2
    empty("mount_cargo", (0.0, mid.y, mid.z), (0, 1, 0))
    empty("mount_utility", (0.0, hi.y - 4.0, lo.z + 3.0), (0, 1, 0))

if not has("nozzle_lift"):
    span = hi.y - lo.y
    # (Six, as a pair fore, amidships and aft: enough to hover under a world's pull.)
    for k, (sx, f) in enumerate(((-1, 0.75), (1, 0.75), (-1, 0.5), (1, 0.5), (-1, 0.25), (1, 0.25))):
        empty("nozzle_lift_%d" % k, (sx * 2.5, lo.y + span * f, lo.z + 3.0), (0, 0, -1))

if not any(n.startswith("nozzle_") and not n.startswith(("nozzle_main", "nozzle_lift")) for n in have):
    # Quads at the nose and the tail, each side, high on the hull: up, down, out, fore, aft.
    span = hi.y - lo.y
    for end, y in (("nose", hi.y - span * 0.12), ("tail", lo.y + span * 0.12)):
        for side, sx in (("left", -1), ("right", 1)):
            base = (sx * (hi.x - 2.0) * 0.5, y, hi.z - 4.0)
            for d, v in (("up", (0, 0, 1)), ("down", (0, 0, -1)), ("side", (sx, 0, 0)), ("fore", (0, 1, 0)), ("aft", (0, -1, 0))):
                empty("nozzle_%s_%s_%s" % (end, side, d), base, v)

if not has("COL_"):
    hulls = [o for o in meshes if o.name.startswith("Hull_")]
    # (The big ones: a box each. Small trim stays out of contact and mass.)
    sized = sorted(((box(o), o.name) for o in hulls), key=lambda t: -(t[0][1] - t[0][0]).length)
    first = True
    kept = []

    def inside(a, b):
        # (How much of box a..b the boxes kept already hold, about: by the share of a grid of points.)
        n, hit = 0, 0
        for i in range(5):
            for j in range(5):
                for k in range(5):
                    p = Vector((a.x + (b.x - a.x) * (i + 0.5) / 5, a.y + (b.y - a.y) * (j + 0.5) / 5, a.z + (b.z - a.z) * (k + 0.5) / 5))
                    n += 1
                    hit += any(all(c[0][q] <= p[q] <= c[1][q] for q in range(3)) for c in kept)
        return hit / n

    for (a, b), name in sized:
        size = b - a
        # (Small trim, or mostly inside the boxes already kept: left out. Overlaps count twice.)
        if size.x * size.y * size.z < 60.0 or inside(a, b) > 0.4:
            continue
        kept.append((a, b))
        bpy.ops.mesh.primitive_cube_add(size=1, location=(a + b) / 2)
        c = bpy.context.active_object
        c.name = "COL_body" if first else "COL_" + name[5:]
        first = False
        c.scale = size
        bpy.ops.object.transform_apply(scale=True)
        made.append((c.name, tuple(round(v, 1) for v in size)))

# Flat text drawn from one side only (filled both ways, it shows a mirrored twin).
for o in scene.objects:
    if o.type == "FONT" and o.data.extrude == 0.0:
        o.data.fill_mode = "FRONT"

for name, at in made:
    print("placed", name, at)
print("ship %s: %.1f x %.1f x %.1f m" % (scene.get("freefall_name", "?"), hi.x - lo.x, hi.y - lo.y, hi.z - lo.z))

# ---------------------------------------------------------------- baking
# Node-made materials (procedural textures: brick, noise, an AO node's grime, a bump) don't
# exist in glTF: exported, they come out plain white. Baked here into one texture atlas the
# meshes share: base colour, roughness, metalness, normal (the bump), and ambient occlusion
# (where light from round about can't reach: seams, the corners between armour layers).
bake_size = int(opt.get("--bake", "4096"))
# How many atlases the hull is baked into (each `bake_size`): more, sharper, more memory.
atlases = max(1, int(opt.get("--atlases", "1")))


def procedural(m):
    return m is not None and m.use_nodes and any(n.type.startswith("TEX_") and n.type != "TEX_IMAGE" for n in m.node_tree.nodes)


def principled(m):
    return next((n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None) if m and m.use_nodes else None


def glowing(m):
    b = principled(m)
    if b is None:
        return False
    strength = b.inputs["Emission Strength"].default_value
    colour = b.inputs["Emission Color"].default_value
    return strength * max(colour[0], colour[1], colour[2]) > 0.05 or "Glass" in m.name


if bake_size:
    import time
    # (Each step's line as it happens, not when Blender quits.)
    sys.stdout.reconfigure(line_buffering=True)
    started = time.time()
    targets = [o for o in scene.objects if o.type == "MESH" and shown(o) and any(procedural(slot.material) for slot in o.material_slots)]
    bpy.ops.object.select_all(action="DESELECT")
    for o in targets:
        o.select_set(True)
    bpy.context.view_layer.objects.active = targets[0]
    # The geometry as drawn (booleans, bevels applied): what the maps are laid on.
    bpy.ops.object.make_single_user(object=True, obdata=True)
    bpy.ops.object.convert(target="MESH")
    # The meshes shared out among the atlases, each about the same surface (the largest first,
    # each to the atlas with the least so far): more atlases, more texels a metre.
    surface = {o: sum(p.area for p in o.data.polygons) for o in targets}
    groups = [[] for _ in range(atlases)]
    loads = [0.0] * atlases
    for o in sorted(targets, key=lambda o: -surface[o]):
        k = loads.index(min(loads))
        groups[k].append(o)
        loads[k] += surface[o]
    groups = [g for g in groups if g]
    print("baking %d meshes into %d atlas(es) of %d px" % (len(targets), len(groups), bake_size))
    # Each atlas's UV layout: its meshes' islands packed together (the same texels a metre).
    for k, g in enumerate(groups):
        bpy.ops.object.select_all(action="DESELECT")
        for o in g:
            o.select_set(True)
        bpy.context.view_layer.objects.active = g[0]
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        # (Smart projection packs the meshes' islands together itself, a texel apart. Packed again
        # after, a hull this detailed (tens of thousands of islands) came out as specks: the hull
        # sampled the black between them and looked burned.)
        bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=1.0 / bake_size, scale_to_bounds=False)
        bpy.ops.object.mode_set(mode="OBJECT")
        print("  atlas %d: %d meshes, %.0f m2: about %.1f texels a metre" % (k, len(g), loads[k], bake_size / max(loads[k], 1.0) ** 0.5 * 0.65))

    # (The collision boxes are the game's, not the ship's: out of the light while baking, or
    # they'd wrap the hull and every point would look shut in.)
    for o in scene.objects:
        if o.name.startswith("COL_"):
            o.hide_render = True
    scene.render.engine = "CYCLES"
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        for kind in ("OPTIX", "CUDA", "HIP", "ONEAPI", "METAL"):
            try:
                prefs.compute_device_type = kind
            except TypeError:
                continue
            prefs.get_devices()
            if any(d.type == kind for d in prefs.devices):
                for d in prefs.devices:
                    d.use = d.type == kind
                scene.cycles.device = "GPU"
                print("  on the GPU (%s)" % kind)
                break
    except Exception as e:
        print("  on the CPU (%s)" % e)
    scene.render.bake.margin = 4
    scene.render.bake.use_clear = True
    if scene.world is None:
        scene.world = bpy.data.worlds.new("bake")
    scene.world.light_settings.distance = 1.5

    def image(name, colour):
        im = bpy.data.images.new("hull_" + name, bake_size, bake_size, alpha=False)
        im.colorspace_settings.name = "sRGB" if colour else "Non-Color"
        return im

    maps = [{kind: image("%d_%s" % (k, kind), kind == "base") for kind in ("base", "rough", "metal", "normal", "ao")} for k in range(len(groups))]
    mats = {slot.material for o in targets for slot in o.material_slots if slot.material}
    for m in mats:
        m.use_nodes = True
    bakers = {}
    for m in mats:
        n = m.node_tree.nodes.new("ShaderNodeTexImage")
        bakers[m] = n

    # Each atlas's meshes as one object to bake: baked one by one, Blender sets the whole scene up
    # again for each (on the CPU, the GPU idle meanwhile), every pass. All of them in the light
    # (each atlas's occlusion sees its neighbours), the originals out of it, or the copies and
    # they would shut each other in.
    joined = []
    for g in groups:
        bpy.ops.object.select_all(action="DESELECT")
        for o in g:
            o.select_set(True)
        bpy.context.view_layer.objects.active = g[0]
        bpy.ops.object.duplicate()
        bpy.ops.object.join()
        joined.append(bpy.context.view_layer.objects.active)
    for o in targets:
        o.hide_render = True
    for j in joined:
        j.hide_render = False
    print("  joined into %d object(s) to bake (%d faces)" % (len(joined), sum(len(j.data.polygons) for j in joined)))

    def bake(kind, samples, **kw):
        t = time.time()
        scene.cycles.samples = samples
        for k, j in enumerate(joined):
            for m, n in bakers.items():
                n.image = maps[k][kind]
                m.node_tree.nodes.active = n
            bpy.ops.object.select_all(action="DESELECT")
            j.select_set(True)
            bpy.context.view_layer.objects.active = j
            bpy.ops.object.bake(**kw)
        print("  %s: %.0f s" % (kind, time.time() - t))

    # (The materials' own grime from Ambient Occlusion nodes stays out of the colour: the
    # occlusion map carries it, once. Baked in too, against all this detail every face came
    # out nearly black, then darkened again by the map: burned, not painted.)
    for m in mats:
        for n in [n for n in m.node_tree.nodes if n.type == "AMBIENT_OCCLUSION"]:
            for sock in n.outputs:
                for l in list(sock.links):
                    to = l.to_socket
                    m.node_tree.links.remove(l)
                    if hasattr(to, "default_value"):
                        try:
                            to.default_value = (1.0, 1.0, 1.0, 1.0) if sock.type == "RGBA" else 1.0
                        except (TypeError, ValueError):
                            pass

    bake("base", 8, type="DIFFUSE", pass_filter={"COLOR"})
    bake("rough", 4, type="ROUGHNESS")
    bake("normal", 4, type="NORMAL", normal_space="TANGENT")
    bake("ao", 32, type="AO")
    # (Metalness has no pass of its own: baked as a colour, each material's base colour its metalness.)
    for m in mats:
        b = principled(m)
        if b is None:
            continue
        v = b.inputs["Metallic"].default_value
        for l in list(b.inputs["Base Color"].links):
            m.node_tree.links.remove(l)
        b.inputs["Base Color"].default_value = (v, v, v, 1.0)
    bake("metal", 1, type="DIFFUSE", pass_filter={"COLOR"})
    for j in joined:
        bpy.data.objects.remove(j, do_unlink=True)
    for o in targets:
        o.hide_render = False

    # Each atlas's baked material, for every face not lit of itself (lamps, glows and glass keep theirs).
    group = bpy.data.node_groups.get("glTF Material Output") or bpy.data.node_groups.new("glTF Material Output", "ShaderNodeTree")
    if "Occlusion" not in [i.name for i in group.interface.items_tree]:
        group.interface.new_socket("Occlusion", in_out="INPUT", socket_type="NodeSocketFloat")
    for k, g in enumerate(groups):
        baked = bpy.data.materials.new("Hull_Baked" if len(groups) == 1 else "Hull_Baked_%d" % k)
        baked.use_nodes = True
        nt = baked.node_tree
        b = principled(baked)
        tex = {}
        for kind, im in maps[k].items():
            tex[kind] = nt.nodes.new("ShaderNodeTexImage")
            tex[kind].image = im
        nt.links.new(tex["base"].outputs["Color"], b.inputs["Base Color"])
        nt.links.new(tex["rough"].outputs["Color"], b.inputs["Roughness"])
        nt.links.new(tex["metal"].outputs["Color"], b.inputs["Metallic"])
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nt.links.new(tex["normal"].outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], b.inputs["Normal"])
        # (Occlusion, the way the glTF exporter takes it: a node group of this name.)
        out_node = nt.nodes.new("ShaderNodeGroup")
        out_node.node_tree = group
        nt.links.new(tex["ao"].outputs["Color"], out_node.inputs["Occlusion"])
        for o in g:
            for slot in o.material_slots:
                if slot.material and not glowing(slot.material):
                    slot.material = baked
    print("  baked in %.0f s" % (time.time() - started))

# Out: what renders, the nodes and colliders.
for o in scene.objects:
    o.select_set(shown(o) or o.type == "EMPTY" or o.name.startswith("COL_"))
bpy.ops.export_scene.gltf(
    filepath=out, export_format="GLB", use_selection=True, export_apply=True, export_yup=True,
    export_extras=True, export_tangents=True, export_lights=False, export_cameras=False, export_animations=False,
    export_image_format="JPEG", export_jpeg_quality=92,
)
print("wrote", out)
