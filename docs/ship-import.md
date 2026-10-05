# Importing a ship from Blender (glTF)

A ship modelled in Blender (or any tool that writes glTF 2.0) becomes a hull like any other: it
renders as made (textured, physically based) and flies by its physics. Example and template:
`tools/blender/test_hull.py` (builds `assets/models/test_hull.glb` headless).

## Conventions

| In Blender | Means |
|---|---|
| Nose along **+Y**, up **+Z**, metres | the ship's frame (the exporter turns it into ours) |
| Meshes named `COL_…` | convex collision parts (`COL_body` first); hidden in the game. None: the whole model's convex hull |
| Every other mesh | what's drawn (glTF metallic-roughness materials: base colour, metallic/roughness, normal map, emission), and what's walked on and bumped into on foot: its carved spaces and floors as modelled (steps up to 0.45 m, floors to 50°) |
| Empties named `nozzle_main_…` | the main drive's nozzles (driven by its drive slot) |
| `nozzle_lift_…` | lift jets (the lift slot) |
| any other `nozzle_…` | manoeuvring thrusters (the thrusters slot) |
| `gear_…` | landing contacts: put them at the bottoms of the feet. Set down, the ship stands on the lowest one (its centre that high over the ground or deck) |
| `dock_…`, `cockpit` | docking ports, the pilot's seat (getting up, you stand on the floor under it) |
| `hatch` | the crew hatch; its +Y arrow down the ramp. Landed, you walk out that way to the ground (pointing straight down: aft; none: from the port side, amidships); coming in, you stand on the floor at its top |
| `door_…` | the middle of a doorway (a crew door's, shut): the interior studio's door points |
| `mount_hardpoint_…`, `mount_cargo…`, `mount_utility…` | one slot each (guns, racks, utility); other `mount_<slot>` set where that module sits |
| An empty's **+Y arrow** | a nozzle's exhaust; a port's way out; the pilot's view |
| Scene properties `freefall_name`, `freefall_class` (1-4) | its name; its size class (how big its slots are) |

Blender's `.001` suffixes are ignored.

**From your own .blend** (`tools/blender/export_hull.py`): exports the file as it stands, at a
frame of its rig (`--frame`), leaving out lights, cameras and what doesn't render (boolean
cutters, volumes), and places any conventions the file lacks from its parts' names (footpads
`*_Pad` → `gear_*`, `*EngineGlow*` faces → main nozzles, `*Glass*` → `cockpit`, `*Ramp*` →
`hatch`, `*Laser*_Head` / `*Hammer*_Head` → hardpoints, `Hull_*` → `COL_*` boxes, lift jets and thruster quads),
printing where. Your own empties win. Rerun it after every change:

    blender -b -y design.blend -P tools/blender/export_hull.py -- assets/models/mc07.glb --frame 50 --name MC-07 --class 3 --bake 8192 --atlases 4

The MC-07 now ships as vector: no textures, its detail all in the geometry (panel edges, insets,
lettering), each procedural paint given its flat colour first by the design's `flatten.py`
(`~/git/blender/mc07-assembly/tools/`), so nothing blurs at any distance (8 MB, 3 s):

    blender -b -y mining_ship.blend -P mc07-assembly/tools/flatten.py -P tools/blender/export_hull.py -- assets/models/mc07.glb --frame 50 --name MC-07 --class 3 --bake 0
 Export: glTF Binary (`.glb`), **Tangents** and **Custom
Properties** on, **+Y Up** on.

## What follows from it

Its slots are standard for its class; its stock fit the cheapest module for each with a plant big
enough; its frame's mass from its size (as a design's); then everything about how it flies from
the physics: its mass and inertia from where its modules sit inside it, its thrust and turning
from where its nozzles push. Nothing is balanced by hand.

## Trying it

    UNIVERSE_HULL=your_ship.glb cargo run --release

(you fly it), or to look at it alone:

    UNIVERSE_SCENARIO=showcase UNIVERSE_MODEL=your_ship.glb UNIVERSE_MODEL_CLEAN=1 cargo run --release
