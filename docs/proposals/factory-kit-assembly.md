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

## The trial, as built (2026-10-15)

The owner approved the proposal; the trial is the airless shaft furnace.

- **SFO 25** (`standards/SFO/metadata/0025-plant-connectors.yaml`) sets the ports, connectors and runs. There are new schemas `connector.schema.yaml` and `run.schema.yaml`. `unit.schema.yaml` gains `ports` (with the shared `#/definitions/port`), and `layout.schema.yaml` gains the layout's own `ports` and, on links, `run`, `ends`, `taps`, `count` and `fittings`.
- **`tools/standards/plant_kit.py`** sizes connectors and runs from pressure, bore and material. Sizes come from the EN DN series, the next size up from what the flow needs. Walls come from Barlow at B31.3 stresses. Flanges use a simple bolt-and-ring rule, not a flange code. Supports are spaced by the span a full pipe bears at 1 g. Hydrogen lines on airless ground have double seals. Only the classes a layout asks for are written: 19 connectors and 19 runs.
- **Ports** are on the layout's 8 unit kinds. A layout `ports` list holds its 9 own ports: pellets in, sponge iron out, hydrogen in, condensate out, and 6.6 kV, 132 kV and 400 V feeds.
- **`layout.shaft-furnace-airless`** has 46 runs, each joining port to port. They include the radiator loop as flow and return mains (laid as a parallel pair that never crosses), a pair of row mains along each row with each field's ports tapped off, and power to every unit that draws it. The old layout had no cable to the furnace, the crew post or the 43 radiator pumps, and no 132 kV feed to the transformer.
- **`tools/standards/fit_layout.py`** checks a layout in half a second. It finds ends that are not at their ports or that leave a port the wrong way, connector, flow and medium mismatches, tees off their main or inside its elbow, legs too short for their elbows, runs through units, other runs or the frame outside its openings, open ports, and power not fed. On its first run it found the pellet belt clipping the top of the furnace's box, so the head now stands 3 m over the hopper and the roof opening was widened one cell. It also found the heater's cables reaching its terminal house through the unit, so they now enter on the unit's west face. It reports no faults now.
- **Mass:** 9,530 t, up from 9,288 t. The weight moved because the old links counted the loop mains only, while the runs count the row mains (8 rows × 2 × 290 m), the trestles and stands, the cable trays, and the flanges at every port.

Not done yet:
- the units' collision proxies (the checker uses each unit's box, so a manifold under radiator panels is let through by its taps);
- rate balance along a branched network;
- the frame's inner faces;
- the other 15 layouts;
- blender's run-part and connector models;
- engine's snap-together renderer.

## The second layout (2026-10-16)

`layout.hydrogen-plant-airless` is laid of runs too: 131 runs and 13 layout ports, 0 faults. Each electrolyser block takes 43 kA of direct current from its rectifier. On airless ground a bare bus can only radiate, and at lunar noon it would need plates about 4 m wide a pole. So SFO 25 gains the run kind `busbar` (two poles of water-cooled hollow copper, 7 tubes a pole, 6.8 kW lost a metre into its water) and the connector kind `bus-joint`. Each bus passes in a covered trench under the maintenance gantry's rail. The rectifier transformers gain a 33 kV cable box, a feeder each from the east edge. The radiator rows are shared with the shaft furnace (`radiator_rows`). The two layouts mate: the hydrogen plant's `h2-out` is the shaft furnace's `h2-in` flange (DN 250 PN 16, stainless, double seal).


## The third layout: the airless pellet plant (2026-10-16, in progress)

Laying the pellet plant from ports showed what the old drawn layout had hidden (its belts turned corners, its pipes passed through the hall's wall where it had no openings). The registry's choices:

- **Kit and checker (3974d56c):**
  - A port can be `optional`: a spare, blinded when unused (the hall heads' pass-throughs).
  - A belt's head can drop `onto:` another belt, 0.5 m over it.
  - A bulk tap reaches its belt by a short chute: 1.6 m aside, 1 m under or 0.8 m over.
  - Belts can be `sealed`, in a 3 m gallery tube.
  - A PN 1 class exists for the galleries' joints.
  - A belt's height follows its width.
  - The fit check tests a run's section, not its round, against units' boxes.
- **The hall's heads (v2):** each head gets two nozzles, both optional:
  - the deck-level nozzle, for the green balls going out east;
  - a high nozzle at z 14.5 for the ore coming in west.

  The ore gallery then runs straight from the lock to the mill's hopper. Each head also gets a service panel of pass-throughs (water pair, condensate, nitrogen, 6.6 kV, 400 V), each an outer and an inner port, all optional. The lock pumps get a power port.
- **The ore lock** stands on a 6 m plinth of sintered regolith, so its outlet feeds the gallery at the high nozzle.
- **The green balls:**
  - a collector belt along the aisle under the four screens;
  - a sealed gallery out of the east nozzle;
  - a second sealed gallery rising at 15 degrees to a reversible shuttle over the shafts' charge hoppers.
- **The gas loops' dust** falls into a bin under each lock (gas loop v2), emptied by forklift: no dust conveyor.
- **The balling discs** return their undersize to their own feed (disc v2).
- **The radiator field 330 K** takes the 345 K field's ports at its south-west corner (v2), so the shared `radiator_rows` lays its rows.
- **The plant transformer** gets cable boxes (v2), as the heater transformer did.
- **Power:**
  - 6.6 kV from the transformer by a main with joint boxes, to the torches and the mill;
  - 400 V from the site for the fans, the lock, the post and the fields;
  - inside the hall, one 400 V main through the west head's panel, branched to the presses, the discs, the sections' fan coils and the heads' lock pumps.
