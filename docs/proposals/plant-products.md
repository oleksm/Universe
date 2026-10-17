# Plant products: one model per product, not per unit (proposal)

2026-10-16. The owner: "it is building same factory like 3-5 times ... organize registry so it hold components / modules and layouts, not repeat same small or big stupid item 1000 times." Blender is stopped (thread 20261016T214805-registry-cf8f).

## What went wrong

Since 2026-10-13 there were 264 model installs. The arc furnace vessel was built 6 times (air v1 to v3, sealed v1 and v2, air v4 held). About 30 other units were built 2 or 3 times each. There were two causes, both the registry's.

1. **Models before interfaces.** Each unit was modelled, and only then given ports. Each layout converted to runs afterwards found something to change: a flange's size, a pipe's way past a crane's legs, a hood the furnace's tilt hits. Each change sent the unit back to blender. The units were made to fit the layouts, not the layouts the units.
2. **Variants copied, not composed.** 78 unit records stand for about 46 products:
   - five cooling towers differ only in their number of cells;
   - six bridge cranes and four gantry cranes differ in capacity and span, and the gantries add legs;
   - five transformers differ in rating;
   - the sealed arc furnace is the furnace with a sealing kit.

   Each was modelled whole.

## The registry's shape

| Record | What it is | Who models it |
|---|---|---|
| `product` (new) | A made thing a vendor sells, with its parameters (cells, MVA, capacity, span, pressure class) and its option kits (sealing, gantry legs, tongs). It has one model, or for a family a generator and its pieces (a tower cell, a pump skid). | blender, once |
| `unit` | A product as fitted in a layout: `product`, `params`, `options`. Its mass, ports and envelope are the product's at those params (the registry's generator works them out). It wants no model of its own. | nobody (drawn from its product) |
| run, connector | The kit families (section 8): one generator each, `looks_as` for look-alikes. | blender, once a family |
| `layout` | Data only: bays, runs, ground. | nobody |

**The order:**
1. Settle all 16 layouts' ports: the 9 layouts not yet laid of runs are converted first.
2. Freeze each product's interface: its ports as a rule of its params.
3. Blender models each product (or family generator) and each option kit once, in batches.
4. After that a layout adapts to its units. If it needs something they don't offer, it adds a run, an adapter, or a new option kit. Never a new unit model.

**Existing models stay.** They count as their product's model until that product's generator is made. Restructuring alone rebuilds nothing.

## The products

**Families: one generator, sized by a parameter** (25 units become 8)

| Product | Parameter | Units today | Pieces to model |
|---|---|---|---|
| Cooling tower | cells | cooling-tower, -1cell, -pellet, -h2, -shaft | a 12 m cell; the loop's pump skid |
| Radiator field | loop temperature (header metal) | radiator-field-330k, -345k, -485k | a panel; the header set |
| Transformer | rating, voltages; option kits: HV cable box, bus joint pads | furnace-transformer-14mva, -90mva, heater-transformer-131mva, rectifier-transformer-21mva, plant-transformer-35mva | a tank scaled by rating; bushings; the cable box kit |
| Baghouse | flow (compartments) | baghouse-200k, -450k, -800k | a compartment; the fan end |
| Overhead crane | capacity, span; option kits: gantry legs, ladle beam, stripping tongs, ingot tongs | hall-crane-20t, ladle-crane-150t, mill-crane-30t, stack-crane-20t, stripper-crane-20t, ingot-crane-10t, gantry-crane-20t-moon, gantry-crane-150t-moon, stripper-gantry-20t-moon, ingot-gantry-10t-moon | a bridge girder pair and end carriages; a trolley; the legs kit; the tools |
| Rail car | load, length; payload fixtures | ladle-car-80t, slag-pot-car, mould-car-12, ingot-car-40t | a chassis with bogies; the ladle seat, slag pot, mould set, ingot deck |
| Electrolyser block | pressure class | electrolyser-block-20mw, -10bar | one block (the class sets its walls) |
| Pressure hall | length | pressure-hall-30m, -30m-end | a section; a head |

**Products with a variant: an option kit or a parameter** (22 units become 11)

| Product | Variant | Units today |
|---|---|---|
| Arc furnace vessel, 80 t | sealing kit (roof clamps, glands, feed lock, bellows) | eaf-vessel-80t, -sealed |
| Ladle furnace, 80 t | sealing kit | ladle-furnace-80t, -sealed |
| Hydrogen shaft furnace, 200 t/h | pressure class, hot discharge | shaft-furnace-200th, -hp |
| Recycle compressor | power, pressure class | recycle-compressor, -8bar |
| Hydrogen heater | power, pressure class | gas-heater-122mw, -119mw-8bar |
| Hydrogen purifier | pressure class | hydrogen-purifier, -10bar |
| Water treatment | sealing kit | water-treatment, -sealed |
| Sampling laboratory | pressurised shell kit | sampling-lab, -sealed |
| Crating line | crate kind | crating-line, -steel |
| Off-gas cleaning | baghouse (above) or sealed candle filter | offgas-filter-sealed stays its own product |
| Top gas cooling | scrubber (air) or condenser (8 bar) | two products: different machines |

**One-off products: one unit each, modelled once** (27)

alloy-bins-8, ball-mill-4m4, balling-disc-7m5, crew-post, crop-saws, degassing-stand, disc-filter-2, dri-bin-200t, dri-cooling-loop, filter-press-2, forklift-2t5, induration-grate-125th, ingot-tipper, offgas-cooler-sealed, ore-lock-hopper, pellet-gas-loop, pellet-shaft-63th, recuperator, stacker-reclaimer-1500th, stair-tower-12m, top-gas-condenser-8bar, top-gas-scrubber, truck-dump-hopper, vacuum-degasser-80t, yard-tripper-1500th, offgas-filter-sealed, hall heads' kits (in the pressure hall family).

**In all:** 78 units become about 46 products. They need about 20 family pieces, 27 one-off models and about 10 option kits. Most already exist as today's models.

## Next

1. Schema: a `product` record (params, pieces, option kits, port rules); `unit` gains `product`, `params` and `options`; `next_asset` asks models of products and pieces, not units.
2. Convert the 9 remaining layouts to runs, without modelling, so every product's ports are known.
3. Freeze the interfaces. Then one brief to blender: the family generators and the products still without a model, each once.

## What is built, and what is reused (inventory, 2026-10-16)

Every unit model was checked against its record today: the ports and size the record held when the model was installed, against those it holds now.

- **Units: 71 of 78 models match their records.** The other 7 differ only in their ports, except one:
  - baghouse-800k, cooling-tower, dri-bin-200t, filter-press-2, ball-mill-4m4: ports added or resized since; their bodies are unchanged.
  - pellet-shaft-63th: its record's width became a centred box round the same model (it reaches 5.5 m on one side for its power box); the model is right.
  - eaf-vessel-80t (air, v3): the one real gap. Its off-gas elbow points up, and the tilt sweep showed that design collides. The record now has a horizontal elbow and a sliding sleeve, which is a different body.
- **Kit: 125 of 126 models match.** flange-dn1000-pn16-double is 8 mm thicker in its record than in its model, which is not worth a rebuild. The 2 newest pieces are drawn by `looks_as` with no model of their own.
- **Layouts:** the 12 old monolith models stay reference pictures only.

**Rules from now on:**
1. **A port is not a reason to rebuild.** The game draws the run's connector (flange, termination, chute, hood) at every port a run ends on, from the kit's models. A unit's model needs no stub there. A unit is rebuilt only when its body changes: a part moves, is added, or turns out wrong, as the furnace's elbow did.
2. **An envelope is measured, not built.** Units without one (cooling-tower, baghouse-800k, hall-crane-20t, eaf-vessel-80t) get their boxes measured from their installed models.
3. **Converting the 9 remaining layouts makes no model work.** Their units all have models already. Only a body conflict a layout can't route round goes to blender, with the reason.
4. **The product families' generators are built only when a new size is needed.** Today's units all have their models.

**So blender's whole list today:**
- the air furnace's v4 (the horizontal elbow, its posts and the telescoping feed chute: a body change);
- four envelopes measured from installed models.

The section 9 unit versions for cooling-tower and baghouse-800k are cancelled: they were ports only.
