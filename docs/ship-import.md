# Importing a ship from Blender (glTF)

A ship modelled in Blender (or any tool that writes glTF 2.0) becomes a hull like any other: it
renders as made (textured, physically based) and flies by its physics. Example and template:
`tools/blender/test_hull.py` (builds `assets/models/test_hull.glb` headless).

## Conventions

| In Blender | Means |
|---|---|
| Nose along **+Y**, up **+Z**, metres | the ship's frame (the exporter turns it into ours) |
| Meshes named `COL_…` | convex collision parts (`COL_body` first); hidden in the game. None: the whole model's convex hull |
| Every other mesh | what's drawn (glTF metallic-roughness materials: base colour, metallic/roughness, normal map, emission) |
| Empties named `nozzle_main_…` | the main drive's nozzles (driven by its drive slot) |
| `nozzle_lift_…` | lift jets (the lift slot) |
| any other `nozzle_…` | manoeuvring thrusters (the thrusters slot) |
| `gear_…` | landing contacts: put them at the bottoms of the feet. Set down, the ship stands on the lowest one (its centre that high over the ground or deck) |
| `dock_…`, `cockpit` | docking ports, the pilot's seat |
| `hatch` | the crew hatch; its +Y arrow down the ramp. Landed, you walk out that way to the ground (pointing straight down: aft; none: the generic port-side ramp) |
| `mount_hardpoint_…`, `mount_cargo…`, `mount_utility…` | one slot each (guns, racks, utility); other `mount_<slot>` set where that module sits |
| An empty's **+Y arrow** | a nozzle's exhaust; a port's way out; the pilot's view |
| Scene properties `freefall_name`, `freefall_class` (1-4) | its name; its size class (how big its slots are) |

Blender's `.001` suffixes are ignored.

**From your own .blend** (`tools/blender/export_hull.py`): exports the file as it stands, at a
frame of its rig (`--frame`), leaving out lights, cameras and what doesn't render (boolean
cutters, volumes), and places any conventions the file lacks from its parts' names (footpads
`*_Pad` → `gear_*`, `*EngineGlow*` faces → main nozzles, `*Glass*` → `cockpit`, `*Ramp*` →
`hatch`, `*Laser*_Head` → hardpoints, `Hull_*` → `COL_*` boxes, lift jets and thruster quads),
printing where. Your own empties win. Rerun it after every change:

    blender -b design.blend -P tools/blender/export_hull.py -- assets/models/mc07.glb --frame 50 --name MC-07 --class 3
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
