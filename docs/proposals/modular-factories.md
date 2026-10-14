# Proposal: factories built like ships

Status: proposal, for the owner to decide. Written by registry, 2026-10-13.

The owner, 2026-10-13: "Factories should be modular and situational, it is more like ship assembly than a single unit."

## 1. What we have now

- A facility (LocalAdministration) is a list of lines. Each line lists industrial modules with a count. For example, Trethi Foundry's steel line is `ore-yard 1, pellet-plant 1, hydrogen-plant 3, shaft-furnace 1, arc-furnace 1, ladle-station 1, casting-bay 1, ingot-yard 1`. The top level is already modular.
- Each module (SFO 10, 110 records) is one fixed box with one rate and one power draw. For example, `module.arc-furnace` is 100 × 40 × 35 m, makes 26.7 t of steel an hour and draws 72 MW. The same box is used on every world: a moon, an orbit or Heath.
- A module's parts (`parts/<module>/`, 40 modules) were guessed by scaling the box. The arc furnace's "furnace transformer" is 54 × 22 × 19 m and weighs 150 t. Its "charging crane", at 300 t, is counted as part of the furnace. These parts can't be fitted, swapped or shared between modules.

So a works can't be smaller or larger than a whole box, can't be fitted to its site, and can't be built up from parts a player could buy.

## 2. The idea

A module becomes a fit: units fitted into a frame, the same way equipment is fitted into a hull.

| Ships | Factories (proposed) |
|---|---|
| Hull, sized from loads | Frame or hall from kit struts (SFO 13), sized from the units it carries and from local gravity |
| Slot offering a mount (SFO 19) | Plant bay offering a plant mount (new standard): room, floor load, power, feed in and out, heat out, exhaust |
| Equipment built to a mount, by a brand | Plant unit built to a plant mount, by a brand: furnace vessel, transformer, electrode mast, crane, conveyor, pump, tank, centrifuge |
| Design (studio fit) | Layout: a frame, bays, the units fitted to them, and the site it is meant for |
| Mass and size derived from parts | Same: a layout's mass, footprint, power, heat, staff and rate add up from its units |

The recipe moves from the module onto the unit that does the work. The furnace vessel carries the melt recipe; the transformer carries none but feeds power to it. A layout's rate is set by its slowest unit, multiplied by how many of that unit are fitted.

## 3. Situational: the site decides

A layout states which site it is for. The registry gives the physical rules; the layout's own figures must meet them. These are the cases that change the design. They follow from physics; the figures here are not yet sized.

| Site property | What changes |
|---|---|
| Gravity | Slag floats off metal, flotation cells settle, ladles pour, all by gravity. Settling speed goes as g (Stokes), so these are slower at low g: about 1/6 as fast on a 1.62 m/s² moon. In zero g they need a centrifuge unit or a different process. Frame members are sized from local g. In orbit the frame carries handling loads, not weight. |
| Air or airless | Airless: no convection, so heat leaves by radiators or into the ground; open frames instead of halls; only crew posts are pressurised; vacuum degassing comes free; lubrication and dust need vacuum-rated units. With air: open-air or covered works, cooling towers, and air as a process input. These are separate layouts, not one reused (rule: airless differs from air). |
| Power available | Sets how many units the site can run. A transformer unit is sized to its furnace. |
| Heat rejection | On airless sites radiator area is often the limit. Same physics as the power plants (SFO 22 radiators). |
| Scale | One small furnace or ten. Units come in size classes 1 to 4, as mounts do. Output follows the units fitted, not one box rate. |

## 4. Records (sketch)

```yaml
# units/eaf-vessel-s3.yaml: a plant unit, a product of a brand
identity: { key: unit.eaf-vessel-s3, name: "Arc furnace vessel, class 3" }
fits: plant-mount.melt-s3
physical: { length: 9, width: 9, height: 7, mass: ... }   # from material and loads
needs: { power: ..., heat_out: ..., gravity_min: 0.5 }      # tapping by gravity
recipes: [ ...the melt recipe now on module.arc-furnace, scaled to this vessel... ]

# layouts/arc-furnace-airless.yaml: a fit
identity: { key: layout.arc-furnace-airless }
site: { atmosphere: none, gravity: [0.5, 12] }
frame: { struts: [...], bays: [ { mount: plant-mount.melt-s3 }, ... ] }
fitted: [ unit.eaf-vessel-s3, unit.eaf-transformer-s3, unit.electrode-mast-s3, unit.radiator-s3 ]
# derived, not written: mass, footprint, power, heat, staff, rate
```

A facility line then lists layouts instead of modules. A player or NPC builder can also make a new layout from units, the same action a ship designer takes (player parity).

## 5. What stays

- Every sourced number on today's modules: rates, inputs, outputs, power, batch sizes and their sources. They move onto units unchanged and are scaled by size class.
- Facility lines and counts.
- The rule that what a works can make is a maximum, while what it does make is up to the economy.
- The 110 modules keep their keys. Each becomes the reference layout for a world with air and standard gravity, now built from units, and keeps its current derived rate during the change.

## 6. Pilot: the steel line

Start with one line, from ore to cast steel: `pellet-plant`, `shaft-furnace`, `arc-furnace`, `ladle-station`, `casting-bay` (with `hydrogen-plant` feeding it).

1. Plant mount standard (SFO 24 proposed) and schemas for units and layouts.
2. Split the five modules into units. Size the units from real plant (sourced where possible, labelled invented where not). Weights come from material, dimensions and loads.
3. Two layouts each: one for a world with air at standard gravity (must give today's rates, so nothing in the game moves) and one airless at low gravity (a moon).
4. A generator derives the layout figures and checks them in `build.py`, as `frames.misfits()` does for struts.
5. Then a generator moves the other modules over, in batches.

## 7. Who does what

- **Registry:** standard, schemas, units, layouts, generator, checks.
- **Engine:** reads facility lines as layouts. Until it does, the module records keep their derived figures, so nothing breaks.
- **Blender:** models units, not whole works. A layout is assembled from unit models on a frame, as ships are.
- **Ships / studio:** the studio's fit tools could lay out a works later. No work is asked of them now.

## 8. Open questions for the owner

1. Approve the direction, and the steel line as the pilot?
2. Should players build works layouts in the studio, as they do ship fits, or should works layouts stay registry-only for now?
3. Should each unit carry its own staff (operators per unit), or should staff stay per layout?

## 9. The first example (2026-10-13)

The owner approved one example, for blender to model. `tools/standards/plants.py` writes it:

- `standards/SFO/metadata/0024-plant-mounts.yaml`: SFO 24, plant mounts, units and layouts (schemas `plant-mount`, `unit`, `layout`).
- `plant-mounts/bay-s{1,2,3}.yaml`: bays 6, 12 and 24 m square.
- `units/`, eight units, each with its mass from its material and dimensions (model version in brackets):

  | Unit | Mass |
  |---|---|
  | Arc furnace vessel, 80 t (v3) | 307 t |
  | Furnace transformer, 90 MVA (v1) | 144 t |
  | Sponge iron bin (v1) | 17 t |
  | Baghouse (v2) | 198 t |
  | Cooling tower (v1) | 157 t |
  | Ladle car (v1) | 56 t |
  | Slag pot car (v1) | 27 t |
  | Hall crane, 20 t (v2) | 27 t |

- `layouts/arc-furnace-air.yaml`: the arc furnace shop for a world with air at 1 g.
  - It has 5 bays, 2 rail cars and 8 links. Pipes, the duct and the conveyor follow routed paths (`via`).
  - The hall is 33 × 30 × 27 m, a lattice of 3 m struts checked for roof and crane loads, with framed openings for the duct and the conveyor.
  - The ground works have their shapes in notes: the furnace platform with its ladle slot, slag tunnel and slag well, the transformer vault, the sunk tower basin, the pipe trench.
  - Totals: the module's 26.7 kg/s at 74.2 MW, 18.4 MW rejected by the tower, 10.2 kg/s of water, 1,208 t of plant and frame, 3,426 t of concrete (1,428 m³).
  - By contrast, the module record was one 100 × 40 × 35 m box at 72 MW, with no water.

Result, 2026-10-14: blender modelled all eight units and assembled the shop (`models/arc-furnace-air/v1`, 257,238 triangles). The game's loader reads every model; drawing units and layouts is engine's. Fitting the units together found about 30 faults that one box per module could never show. Examples: the slag fell onto the furnace's own cradle until a spout was added; the slag car did not fit the hall; the crane's ends touched the walls; the pipes ran through walls; the bin's cone was missing from its height. Each was fixed in `tools/standards/plants.py`, and where blender measured a part, its mass is bound as measured. Still open: anything that moves (tilt, crane, cars), electrical clearance, wind and footing loads, combustion in the off-gas gap.

Blender's brief: `python3 tools/standards/next_asset.py --key unit.<name>` for each unit, and `--key layout.arc-furnace-air` for the assembly.

## 10. The steel line, next (2026-10-14)

The owner left the next step to registry's judgement ("things are made of anything that look reasonable for the situation and conditions"). So the steel line goes on as units, layout by layout, along the ladle rail:

- `layouts/ladle-station-air.yaml`: an 80 t ladle furnace and a vacuum tank degasser under a crane hall, the heat arriving on its car from the arc furnace shop. New units: ladle furnace (its transformer rated for 4 degrees a minute of heating, 14 MVA), vacuum degasser, alloy bins, a 200,000 m3/h baghouse, a one-cell cooling tower, a 150 t ladle crane (its girders the lightest that meet span/800 and 150 MPa). The hall's wall struts are chosen for the crane's wheel loads. 2,868 t, 8.9 MW against the module's 7.5.
- `layouts/casting-bay-air.yaml`: top teeming into small ingots, each sized to one heavy crate (2 t): 1,592 kg of sound ingot, 1 m long, 0.48 to 0.42 m square. A heat fills 44 moulds on a train of four mould cars; a second train stands while its ingots freeze; a 20 t stripper crane strips them onto an ingot car for the ingot yard. New units: mould car (grey cast iron moulds, a new material), stripper crane, ingot car. No cooling water: the moulds and ingots cool in the hall's air. 3,380 t, 0.67 MW against the module's 1 MW.
- `layouts/shaft-furnace-air.yaml`: direct reduction by hydrogen, 200 t of sponge iron an hour, in an open braced tower (no hall, so a hydrogen leak rises away). The gas flow comes from the reduction zone's heat balance: hydrogen brings no heat of its own, so the shaft's 112 MW must come in as hot gas, 14.3 kg/s at 900 °C, 4.8 times what the reduction uses. New units: the shaft (6.6 m inside, 50 m with its hopper, about 2,300 t of burden, hung from a ring girder on eight box columns), a 122 MW electric hydrogen heater and its transformer, a recuperator, a top gas scrubber that condenses the 26.9 kg/s of water the reduction makes (back to the hydrogen plant), a 20 MW recycle compressor (hydrogen is light: 14 stages), a cooling loop for the sponge iron, and four cooling towers for 130 MW. 152 MW and 14,700 t against the module's 22 MW: the module's power left out heating the gas.
- `layouts/hydrogen-plant-air.yaml`: five 20 MW pressurised alkaline electrolyser blocks in a hall with an open ridge (hydrogen rises and must find its way out), a rectifier transformer each outside, a purifier, water treatment and a cooling tower. The cells are sized by Faraday's law at 1.9 V and 0.4 A/cm2: 931 cells a block, 4 stacks of 233 in parallel at 43 kA; 50.5 kWh a kg in the cells, 99.2 MW in all against the module's sourced 100. Nickel electrodes (a new material, `material.nickel-200`). One shaft furnace at its full rate needs about 5.7 of these plants.
- `layouts/pellet-plant-air.yaml`: 125 t of pellets an hour in one crane hall: a ball mill grinds the fines wet, disc filters dry the slurry, four balling discs roll green balls, and a straight induration grate (4 x 28 m) dries, fires and cools them. Its firing heat, the module's sourced 0.5 GJ a tonne, comes from electric plasma torches, since there is no fuel to burn. The grate needs a class 4 bay. 26.1 MW against the module's 23, and no cooling water. One shaft furnace needs about 2.3 of these plants.
- `layouts/ore-yard-air.yaml` and `layouts/ingot-yard-air.yaml`, the line's two ends. The ore yard's two beds come from the module's 150 kt, at the ore's bulk density and a 37 degree angle of repose (105 x 40 x 15 m each), worked by a stacker-reclaimer on its rail and fed by a truck dump hopper. In the ingot yard, a crane lays the hot ingots on racks in a cooling hall for a day (9.5 MW to the air); a lab samples each heat; a crating line puts one ingot in each heavy crate. Forklifts then stack the crates two high, the crate's limit: 30,452 crates need a 140 x 171 m yard, more than the module's 150 x 100 m box. Layout links can now be roads, for the forklifts.
- So the steel line for a world with air is laid out, ore to crate: eight layouts, 48 units. Next: the airless variants.

Sizing helpers shared by the generators: `frames.lattice_count`, `frames.strut_capacity`, `frames.lightest_section`.

