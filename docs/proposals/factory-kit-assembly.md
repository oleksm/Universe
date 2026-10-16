# Factories snapped together from parts (proposal)

2026-10-15. The owner: "it builds a whole huge model for the factory. Instead, we should build manufacturable components / modules, and then have factory layout. The game will snap it together on the ground and render factory. I don't need a huge monolith model." Blender paused (thread 20261015T200117-registry-72c0).

## What is wrong now

A layout's model (`models/<layout>/v1/model.glb`) bakes everything into one file: copies of every unit's mesh, every pipe, belt and cable run, the ground works, the frames and the radiator fields. 12 of them are installed, 2 to 25 MB and up to 400k triangles each. One change to a unit or a route means rebuilding the whole file, and blender spends most of its time on fit work that a program can do from numbers.

Units are already separate products with their own models (77 unit records). What is monolithic is the assembly, and the ports. Each unit's connections (gas in, water flanges, terminal box) are written in its description in prose, so only a person or blender can read them.

## The split

| Thing | What it is | Who makes it | How the game shows it |
|---|---|---|---|
| Unit | a manufactured product (furnace vessel, compressor, crane, radiator panel) with mass, size, material and **ports** | registry record, one blender model each (already) | its model, placed at the bay |
| Run part | a standard part, per size class: pipe straight (per m), elbow, tee, flange, reducer, pipe support; belt section (per m), trestle, portal, head and tail; cable tray (per m); duct (per m), bend | registry record (mass per m, or per piece, from material and wall); one small blender model per part and size | straights stretched along the route, fittings at the corners, supports every n m |
| Frame part | kit struts and panels (SFO 24 / kit frames, already standard) | registry | instanced on the 0.5 m grid |
| Ground | slab, pad, berm, trench, cut and fill as shapes | layout data | the game cuts and builds the terrain |
| Layout | data only: bays (unit, place, quarter turn), runs (kind, size, from port to port, via points), frame, ground | registry (`plants.py`) | snapped together on the ground |

No layout model at all. A layout's mass stays the sum of units, runs (length × mass per m, plus fittings) and ground, which the generator already works out.

## Ports, inputs, outputs and connectors

Every unit gets structured ports in place of the prose. A port is where something goes in or comes out:

`{id, flow (in, out or both), carries (a stock or medium key: hydrogen at 8 bar, cooling water, sponge iron, 6.6 kV power), rate (kg/s or W at the unit's rating), connector (a standard's key), size, at [x,y,z], facing [x,y,z]}`

So a unit's record says what it takes and what it gives, at what rate, through which connector. The layout's inputs and outputs are the ports left open at its edge (ore in, sponge iron out, hydrogen make-up, power): a layout is itself a unit one level up, and two layouts join the same way.

**Connector standards** (a new SFO standard, like SFO 19 mounts for ship equipment): each standard fixes the mating faces, so that any vendor's part fits any vendor's port.
- Pipe flanges by size and pressure class (for example DN 1000 PN 16), with gasket and bolting.
- Vacuum and pressure couplings for airless sites, double O-ring and clamped.
- Belt transfer points: a chute and skirt to a receiving belt's loading zone, by belt width.
- Duct flanges, by size.
- Power terminations by voltage and current: cable lugs and boxes, bus ducts.
- Rail joints, by gauge and rail section.
- Hose couplings, for units that move (a tilting furnace).

**Connectors are made things.** Each connector is a part record with material and mass (a flange pair, a coupling, a cable termination, a chute), and a small model per size. The game snaps a run's end to a port only when they share a connector standard and size, and when one is an out and the other an in (or both carry the same medium). A run's ends name `bay:port`, never coordinates. The generator checks that the rates balance, that sizes and media match, and that ends face each other. A port left open where it should not be is a fault.

## Fit checks without blender

A program `tools/standards/fit_layout.py` checks clashes from unit boxes (or each unit's collision proxy, exported once with its model) and run cylinders. It also checks clearances, bend radii and support spans. Blender's layout fit work (most of the last two days) moves into that program, so fast and repeatable.

## Who does what

- **Registry:** ports (in, out, carries, rate, connector) on all 77 units; the connector standard and connector parts; run parts and their size classes; layout runs from port to port; the fit checker.
- **Blender:** unit models only, plus the run-part kit (about 10 parts × a few sizes), the connector parts (flanges, couplings, terminations, chutes) and a collision proxy per unit. No layout models.
- **Engine:** load the units' and parts' models, generate runs from the data, cut the ground, and instance radiators and struts. Its factory studio already places units from records, so it would move from boxes to these parts.

## What happens to what exists

- The 12 installed layout models stay on disk as reference pictures; the game never loads them.
- Unit models stay as they are.
- Measured link masses from blender's layout scenes stay in the records, labelled as now. New ones come from the run parts.
- The layout queue (shaft furnace airless, hydrogen plant airless, pellet plant airless, both airless yards) is cancelled as model work. The new units in those queues still need unit models.

## Order

1. Ports and connector schemas; the connector standard; ports on the airless shaft furnace's units, as the trial; runs from port to port in that one layout.
2. Run-part records, size classes and the fit checker; the shaft furnace airless passes it.
3. Blender: the run-part kit models.
4. Engine: the snap-together renderer on the trial layout.
5. Then the rest of the layouts.
