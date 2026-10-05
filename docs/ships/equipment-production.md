# Ship equipment: built from parts

*Registry session, 2026-10-05. The user: "the rudimentary implementation is in the game and we need
to get rid of it, that's the goal... let's start expanding the production chain in depth." The
review before this is `docs/ships/equipment-review.md`. The torch drive is kept (its option A).*

## How a piece of equipment is described now

As the MC-07's hull is: the product's record names a folder of parts
(`built_of.parts`, under `SFO/metadata/parts/`) and the module that puts it together (`making`).
Each part has a weight, what it is made from and the module that makes it. A part is either cut
from mill stock (plate, sheet, wire, tile) or is a good bought in whole and built in (a motor, a
pump, a heat pump, electronics, a computer, carbon).

Two reports check it: **Equipment: what each is built of** (the parts must sum to the product's
weight) and a **Chain** report for each product (each part followed back to a works).

## What is described: everything the MC-07 carries

| Product | Weight | Parts | By what |
|---|---|---|---|
| Torch drive S1 | 3,500 kg | 11 | NASA's Discovery II fusion engine, its shares |
| Thruster quads S1 | 700 kg | 11 | the same engine, small |
| Belly lift S1 | 1,100 kg | 11 | the same |
| Fuel tank 4 t | 350 kg | 5 | a ball of 2 mm aluminium, 41 m2; Discovery II's tank shares |
| Fusion plant S1 | 1,800 kg | 15 | Discovery II's reactor and its turbine set |
| Hyperdrive S1 | 2,500 kg | 6 | nothing: invented, as its physics is |
| Flight computer S1 | 200 kg | 5 | chosen |
| Nav computer | 100 kg | 3 | chosen |
| Transponder | 50 kg | 2 | chosen |
| Radar S1 | 300 kg | 4 | Sentinel-1's radar: nearly all antenna |
| Comm S1 | 40 kg | 4 | the Mars Reconnaissance Orbiter's radio |
| Life support S1 | 800 kg | 8 | the space station's water and oxygen plant |
| Mining rig | 2,500 kg | 9 | a roadheader, without the weight that holds it to the face |

107 lines in all (13 products and 94 parts). The figures are in
`standards/sources/research_ship_equipment.json`.

## What the chains say

71 of 107 lines are complete: the part can be made, somewhere, from what the registry describes.
The 36 that stop, stop at two places:

| Stops at | Lines | What it means |
|---|---|---|
| No facility is built to make it | 30 | Titanium plate, stainless plate and copper wire have recipes but no works stands at any port to run them |
| No recipe makes that stock | 6 | Silicon carbide tile (the first wall), insulation blanket (the tank), carbon composite plate (the plant's radiator panels): the materials are on file, nothing makes them |

So the next two pieces of work are plain: **place a titanium, stainless and copper works** (a
facility record, on a parcel: the engine reads those), and **describe how the three materials are
made** (three of the 26 parked processes: sintering, film casting with its metal coat, carbonising
and curing).

## What the real things say about the products

Bringing real designs to these weights shows how far each product is from anything built:

| Product | A real one | Ours | Apart by |
|---|---|---|---|
| Drive | Discovery II: 8.6 kW of jet a kg (a design, never built) | 1,300,000 kW a kg | 150,000 times |
| Fusion plant | Discovery II's set: about 0.08 kW a kg of electric power; the registry's station 0.022 | 2.2 kW a kg | 30 to 100 times |
| Mining rig | 433 kg for each kW of cutting head | 8 kg | 50 times (it has an anchor, and no weight to carry) |
| Life support | 293 kg a head (water and oxygen only) | 800 kg, for no stated crew | keeps about three |
| Tank | 10 to 11% of its fuel's weight | 9% | agrees |
| Radar, comm | Sentinel-1, the Mars orbiter | similar weights | agree |

The drive and the plant are the invented technology of this world, and now it can be said by how
much. Everything else is within reach of what has been built.

## Not done

- **The other 44 products:** the larger sizes, the second makers' drives and plants, racks, cabins,
  capacitors, weapons, the ground and station comms. The larger sizes can follow the same shares.
- **What is missing altogether:** radiators, a survey sensor, more mining gear, batteries. Each is a
  new kind of device, which the engine must be told of (`x-in-game: "not made"` until it is).
- **Wear:** a part has no life yet. The cutter's picks are the obvious first.
- **Parts of parts:** a coil case is one line. The MC-07's hull went three levels down.

## Closed (2026-10-05, later)

All 107 lines are complete: every product the MC-07 carries can be followed, part by part, to a
works that stands at Port Trethi.

- **Trethi Foundry** gained stainless ingot, titanium ingot (a vacuum arc furnace), copper ingot and
  bronze bar (a foundry), and silicon carbide tile (a carbide furnace).
- **Trethi Mill** gained stainless and titanium plate, copper wire (a wire mill), insulation
  blanket (a film works) and carbon composite plate (a composites works). Its power line went from
  150 to 250 MW.
- **Three new works** for the three materials: carbide furnace, film works, composites works.
  Figures: `standards/sources/research_ship_materials.json`.

**What "complete" does not mean.** A chain is complete when each step is described and somewhere
is built to do it. It does not check that the works' own inputs can be had there: quartz sand,
salt and limestone are a wet world's rock, and Port Trethi stands on an airless one; the metals
other than iron and aluminium still come as elements. And three stand-ins are in the recipes,
each said in its record: the registry's one plastic stands for polyester film, for the
polyacrylonitrile that carbon fibre is baked from, and for epoxy resin.
