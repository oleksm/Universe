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
