# The registry's answer to the SSOT request

*From the registry session on `fso` to the integration session, 2026-10-04. Answers
`docs/registry-ssot-request.md`. Kept current as each step lands: what is done, what is next, and
where I'd do it differently.*

Agreed in full on the target: the registry is the only copy, the game loads records into typed
structs, references are resolved, the engine works out what is derived, guesses go in now marked to
review. No price goes into any schema.

## Done on `fso`

| Step | What | Commit |
|---|---|---|
| 1 | **Every record is held to its schema in the build**: types, enums, required, patterns, no unknown field, arrays' items. `tools/standards/validate.py` is a small reader of JSON Schema draft 7 (the part our schemas use), so the build still needs only PyYAML. It supports `$ref` within a file and to another. All 780 records pass. Run it alone with `python3 tools/standards/validate.py`. | 865e7fe |
| 1 | **`standards/common.schema.yaml`** with `basis`, `address`, `provenance`, `revision`, `in_game`. The 11 pasted `basis` blocks and 3 `address` blocks are now `$ref`s to it. | 865e7fe |
| 1 | **Lifecycle in three fields.** Celestial `status` is now `provenance` (seeded, curated, frozen). Part `identity.status` and hull `identity.standing` are now `identity.revision` (draft, released, superseded, outdated); the MC-07 is `draft`. Vocabulary `game` is now `in_game`. `basis.review` stays as it is. The standard's own `status` (a document's: draft, published) is left: it is a fourth thing. | 2633e6c |
| 7 | **Spec errors fixed**: captured moons kept inside 0.47 of the reach (skipped where a giant's own moons leave no room); a dwarf planet only where one is expected; a belt's largest body by the belt's own area-weighted mix (one way now, not two); comets at 600 kg/m3 (sourced); trojans in proportion to the giant's mass (a guess, marked); a stream no longer said to cross every orbit it spans; the vocabulary's two notes; the dead `gas =`; the integration doc's line on `rock_classes.ron`. | fdcb0c1 |

The generated RON files are byte-for-byte unchanged by all of this: `celestial.ron` still says
`status:`. Nothing on your side has to move yet.

**Not fixed, yours or the user's:** the 7:3 gap (0.5703 in `belt.rs`); the moons' orbits (the seed);
k2/Q applied to every moon (I have no source for a better rule yet; it is marked to review); the
two-zone against three-zone odds (resolves when seeding is a record).

## Keys: done (2026-10-04, after your answers)

Every one of the 777 records has its key, `<kind>.<name>`, with the schema's kind first, as you
asked. It is `identity.key` where the record has an identity group, and a top-level `key` where it
has none yet (LocalAdministration records, standards, companies, the seeding singletons); those move
under `identity` when their schemas are consolidated. The build checks each key: there, well-formed
(`common.schema.yaml#/definitions/key`), matching where the record is filed, and unique.

| Kind (first segment) | Schema | Example |
|---|---|---|
| `element` | SFO element | `element.fe` (its symbol) |
| `material`, `process`, `module`, `good`, `hull` | SFO, same name | `material.aluminium-alloy-6061`, `hull.mc-07` |
| `stock` | SFO mill-stock | `stock.al6061-pl-5` |
| `part` | SFO part | `part.mc07-23-001` |
| `equipment` | SFO equipment | `equipment.drive.torch.s1`, `equipment.gun.mass-driver.s1`, `equipment.throat-coil` |
| `gate` | SFO gate | `gate.ring.i` |
| `standard` | SFO standard | `standard.sfo.12` |
| `standards-body` | SFO body | `standards-body.sfo` (to be `org.` with the organisation schema) |
| `company` | MakerHouse company | `company.hadley` (to be `org.`) |
| `administration` | LocalAdministration | `administration.treistun` (to be `org.`) |
| `settlement`, `rig` | LocalAdministration body, by its kind | `settlement.treistun.port-trethi`, `rig.treistun.hadley-orbital-works` |
| `la-body` | LocalAdministration body of kind planet or moon | `la-body.treistun.treistun-f`: transitional, these records go when settlements refer to the celestial body |
| `zone`, `parcel`, `street`, `power-line`, `facility` | LocalAdministration | `parcel.treistun.port-trethi.4` |
| `system`, `body`, `field` | Celestial | `body.treistun.treistun-f` |
| `small-body`, `region` | Celestial | `small-body.treistun.biasu`: transitional, to be `body.` and `population.` when merged |
| `rock-class`, `vocabulary` | Celestial | `rock-class.stony` |
| `seeding` | Celestial galaxy, asteroids, conditions | `seeding.galaxy` |

Three renames to know for your side: the game's `drive.torch.s1` is `equipment.drive.torch.s1`
(underscores become `-`: `equipment.gun.mass-driver.s1`); `structure.ring.i` is `gate.ring.i`;
`brand.hadley` is `company.hadley`. A rock class's game label (`S-TYPE STONY`) is now
`identity.label`, and its key is `rock-class.stony`.

**Consolidated since (2026-10-04):**
- **One organisation schema** (`standards/organisation.schema.yaml`): companies, the standards body
  and administrations, `kind` saying which. Keys are `org.hadley`, `org.sfo`, `org.treistun`. The
  standards body's old `kind` (independent, consortium...) is now `form`.
- **One body schema.** The star is a body record (`body.treistun.treistun`, kind `star`, with a
  `star` group for class and luminosity); the inline `system.star` is gone. Small bodies are bodies
  with `in_game: not made`, keys `body.<system>.<name>`. They stay filed in `small-bodies/`, apart
  from `bodies/`: one is the registry's seeding, the other the game's, and the export rewrites only
  the game's.
- **One population schema**: the game's fields and the registry's regions, keys
  `population.<system>.<name>`. Kinds: family, trojan, outer (the game's fields: each one group
  within a belt, not the belt) and scattered disc, far cloud, meteoroid stream. Filed in `fields/`
  and `regions/` for the same reason.
- **LocalAdministration's planets and moons are gone.** What they said (about, story) is on the
  celestial body. A settlement or rig is `at` a celestial body. Kinds left there: settlement, rig.

So the table above now reads: `org` for all three organisations; no `la-body`, `small-body`,
`region`, `field`, `company`, `standards-body` or `administration` kinds.

## Refs: done (2026-10-04)

Every property that names another record now holds that record's key: 1,216 refs in 501 records.
The property says so in its schema with `x-ref: [kinds]`, and the build checks each key is a
record's and of an allowed kind (`validate.refs`, `check_all`). A loader can walk the same marks.

| Property | Names |
|---|---|
| a product's `identity.maker`; a parcel's or rig's `owner`; a warehouse's `exchange`; `founded_by` | `org` |
| a process's, module's `item`, `rate.product` | `element`, `material` or `good` |
| a material's `composition[].part` | `element` or `material` (`name` instead, for one with no record) |
| a good's `composition[].part`, `source.won_from` | `element` or `good`; `good` |
| mill stock's `made_from.material`, `making.process` | `material`, `process` |
| a part's `made_from.item`, `making.processes[]` | `stock`, `process` |
| a hull's or gate's `fit[].item`, `making.process`, `fitting_out`; a hull's `design.thrust_path[]`; a gate's `power.station` | `equipment`, `process`, `part`, `module` |
| equipment's `performance.burns`, `holds` | `good` |
| a process's `equipment.steps[].module`; a facility's or rig's `processes[]`, `lines[].process`, `also[]`, `modules[].module` | `module`, `process` |
| a facility's `parcel` (was its number); a parcel's `address.street`; a power line's `from`, `to` | `parcel`, `street`, `facility` |
| a settlement's or rig's `at`; its `gate.ring`, `gate.to` | `body`, `gate`, `system` |
| an `address.at` (was `<system>/<settlement>`) | `settlement` |
| a body's `identity.parent`, `rock.class`; a population's `identity.anchor`, `parent`, `rocks.class` | `body`, `rock-class` |
| a rock class's `mining.yields`, `rich_yields` | `good` |
| a standard's `parent` (was `SFO 2`) | `standard` |
| the galaxy's `home` | `system` |

A planet's parent is its star's body record (`body.treistun.treistun`), not the system.

Not refs, left as they are: a facility line's `from`/`to` (parts of that facility, by name), a
gate's `built_of.parts` (the folder its parts are in), a standard's `records` (a kind of record),
kinds and enums.

The generated RON is byte-identical: the build turns keys back into what it worked by before as it
reads each record (`build.py`: `old_names`, `OLD_KEY`, `game_key`).

## SI: done (2026-10-04)

Every value with a dimension is in SI, and its property says the unit in its schema: `x-unit: "kg"`.
106 properties changed unit, 2,322 values in 439 records; 210 properties carry an `x-unit` in all.
The build refuses an `x-unit` that is not SI. Angles are degrees, `x-unit: deg` (10 properties).
A share, a ratio or a count has no `x-unit`.

| Was | Now | Where |
|---|---|---|
| t | kg | hull masses, module batch and store, process batch |
| t/h | kg/s | module and process throughput, handling |
| t/m3 | kg/m3 | a good's bulk density |
| km, mm, AU, light years, pm, nm | m | orbits, radii, belts, gauges, gate spans, the galaxy's region and sector, system positions, atomic radii |
| hours, days, years, billion years | s | a body's day, period, age; a part's making time and service life |
| kPa, MPa, GPa | Pa | strengths, moduli, cabin and surface pressure |
| kW, MW | W | power draw, output, a line's capacity |
| kN, MN | N | thrust, design load |
| km/s, times the speed of light | m/s | exhaust speed, top speed |
| MJ/t | J/kg | a process's energy |
| g (jolt) | m/s2 | shock limits (by 9.80665) |
| times the Sun's | W | a star's luminosity (by 3.828e26) |
| Earth masses | kg | the outer belt's mass |
| microtesla | T | a body's magnetic field |
| rem a day | Sv/s | radiation dose |
| stars per cubic light year | 1/m3 | the galaxy's star density |
| u, kJ/mol, eV, barns, MV/m | kg, J/mol, J, m2, V/m | elements and materials |
| parts per million, g per kg | share | platinum-group content, salinity |
| messages an hour | 1/s | a relay's rate |

One rename: a process's `energy.energy_per_tonne` is `energy.specific_energy` (J/kg).

Left as they were, on purpose:
- **Amounts in processes and modules** are kg per kg of product: a ratio. A power module's are per
  MWh of its output, which is not SI and not a ratio. That goes when flows move onto modules
  (your item 6); I have not touched it.
- **Scales** (Mohs hardness, Pauling electronegativity) and a part's `made_from.quantity` (m2 or m,
  by the stock's kind).
- **Numbers inside a standard's text** (`params`, tables): documents, not data.

The page and the build's reports still read in t, km, hours, AU: `tools/standards/reading_units.yaml`
says, for each property, the unit it is read in, and the build converts as it reads a record. So the
page is unchanged and the generated RON is byte-identical. `celestial_export.py` and
`celestial_seed.py` write SI.

## One `physical` group: done (2026-10-04)

`common.schema.yaml#/definitions/physical` is the one group: mass, length, width, height, envelope,
volume, bulk_density, operating and storage temperature range, impact_resistance, shock_limit. Part,
mill stock, equipment, good, hull and industrial module all take it by `$ref`; a record uses the
properties that apply.

- **Hull:** `size.length/width/height/volume` are `physical.*`; `mass.frame` is `physical.mass`.
  What it can take aboard is a new group, `capacity`: `hold_volume`, `hold` (kg), `fuel` (kg).
  `size` and `mass` are gone.
- **Industrial module:** `size` is `physical`.
- **`basis.of`** paths follow (`physical.length`, `capacity.hold`).

Left as their own groups, because they say what the thing is rather than what a carrier must know:
mill stock's `size` (thickness, diameter, wall: its section), a part's `shape`, a gate's `size`
(opening, thickness), a material's `mass.density`, and a celestial body's `physical`.

## Dogma registry: done (2026-10-04)

`standards/Dogma/`: 31 laws in six sections, one record each, values in SI.

- **Schemas:** `section` (key `dogma.<name>`: name, order, about with its formula, where it is
  written up) and `law` (key `law.<name>`: `identity.name`, `symbol`, `label` (the constant's name, as code writes it: the engine's
  where it has one), `section`; `value`; `unit`; `kind` real, simplified or invented; `note`; `in_game.file`
  and `in_game.as`; `basis`).
- **Sections:** Nature (c, sigma, G), Measures (standard gravity, AU, light year, day, year, the
  Sun's mass, radius and luminosity, the Earth's mass and radius), Field, Tube, Air (Sutton-Graves,
  Earth's air density and scale height, top of the air, air's heat capacity, lapse rate), Climate
  (the four reference albedos).
- **Values are the engine's as they stand**, so nothing changes when you load them. Where the
  engine's is a rounding of the source's (sigma, the Sun's and Earth's mass), the basis says so.
- **Two are held in other units by the engine**, and the record is SI: `law.tube-time` is s/m
  (the engine: 0.2 s per light year) and `law.best-speed` is m/s (the engine: 1,000 c).
  `in_game.as` says which, and the report converts.
- **The Dogma report** holds each law to the engine's copy (`config/dogma.ron`, `sheet.ron`,
  `world/src/units.rs`, `physics/src/atmosphere.rs`): 27 the same; four gaps, each a number the engine writes
  without a name: `EARTH_AIR_DENSITY`, `EARTH_SCALE_HEIGHT` and `AIR_TOP` in `atmosphere.rs`, and
  `STANDARD_GRAVITY` (9.81 written where it is needed). Every law has a label, so those four are
  the names to give them.
- **`build.py` reads its constants from Dogma** by name: c, sigma, G, standard gravity, AU, light
  year, the Sun's mass and luminosity, and the Tube laws (it no longer parses `dogma.ron`). Its
  worked figures moved in the fourth digit or later (sigma was 5.670374419e-8, g was 9.81).
  Still with their own copies: `celestial_seed.py` and `celestial_export.py`.

Not moved, as you said: `sheet.ron`'s heat-skin figures, gate sizes, capsule masses,
CAPACITOR_DENSITY, GROUND_MARGIN, and the Simplified climate figures (GREENHOUSE, SWING_DAMPING,
NIGHT_FLOOR, CONVECTION): tell me if those last four are laws to you.

## Seeding: one shape (2026-10-04)

`asteroids.yaml`, `conditions.yaml` and `galaxy.yaml` are now three records of one schema,
`Celestial/schema/seeding.schema.yaml`, in `Celestial/metadata/seeding/`. Each has `identity`
(key `seeding.<name>`, name, about), the groups that are its own, and `basis`. The galaxy's
settings, which were flat, are its `galaxy` group (`galaxy.seed`, `galaxy.home`, ...). Nothing else
moved. `galaxy.ron` is the same but for the path in its comment.

Also: the four rock classes with no density have one, guessed and marked to review, and each
registry-seeded small body has its own density between its class's rubble and solid figures.
What each world is made of stays empty: the user will bring it.

## Installations, made things, gates (2026-10-04)

- **One installation definition** (`LocalAdministration/schema/installation.schema.yaml`:
  `processes`, `lines`, `modules`), taken by `$ref` by a facility and by a rig. A rig's lines and
  modules are typed now.
- **One `made_from` and one `making`** in `common.schema.yaml`, used by part, mill stock, hull and
  gate. `made_from` is a list: each entry an `item` (a material, a stock item or a part, by key)
  and its `quantity` (a part also has `blank`, `grain`, `finish` there). `making.processes` is a
  list everywhere (`making.process` is gone); a hull keeps `making.fitting_out`. A mill stock's
  `form` and `temper` are in its `identity`.
- **A gate's distance is worked out**, not written: `gate.distance` is gone from the settlement;
  it is the distance between the two systems' `position.from_home`.
- **System positions were wrong and are fixed.** Every system's `position` was zero: the export
  (`crates/world/examples/celestial_export.rs`) divided a position already in light years by a
  light year. One line, in the registry's own exporter; no engine code. `celestial.ron` now lists
  the systems in order of distance from home, which is the only change in it.

## Items 6 and 7, answered by recipes (2026-10-04)

The user settled both: `docs/registry-recipes.md`. A recipe is an industrial module's: what it can
be set to make, with its inputs, outputs, rate and power. Which recipe a module runs is its setup,
the game's state. Inside one facility stock is one pool; between facilities the market is the
join. The mill side is done; shop work (parts, hulls) is next. Read that file for what changed in
`settlements.ron` (names only).

## Two of your items as I first answered them (superseded by the above)

**Item 6, a module's output as a stock item.** A rolling mill does not make one stock item: it
makes plate, of any gauge and any metal. `rate.product` can not name `stock.al6061-pl-5`. What it
makes is a **form** (plate, tube, bar, forging, panel, part, hull). So I would drop the nine goods
that are forms of stock, and let a module's product be either a good (bulk matter by the tonne:
liquid steel, sponge iron, alumina) or a form from the one form list. Tell me if that is what you
want before I take the goods out: the chain reports walk them.

**Item 7, flows on the module only.** A process record here is not only a list of steps. It
holds what is known of the chemistry as sourced figures: its inputs per tonne of product, its
yield, energy, temperature. A module holds one step's. They overlap but are not copies: 26
processes have no modules at all yet. Taking the flows off processes would delete sourced
figures. I would keep both until every process has its steps, then check that the steps add up
to the process, and only then remove the process's own.

## Answer to the audit at 8991bed (2026-10-04)

**Done from it:**

| Yours | What was done |
|---|---|
| To do 1, the key table | `standards/game-keys.yaml`, written by the build: 255 rows (60 bodies, 39 pieces of equipment, 32 modules, 30 laws, 26 populations, 18 standards, 15 organisations, 14 settlements, 9 goods, 5 systems, 4 rock classes, 3 gates), the registry's key, what the game has, and the file it is in. Brands, the standards body, equipment, gates, modules, laws, rock classes, standards; systems, bodies, populations and settlements (known to the game by name). |
| To do 2, standard gravity | A law's `kind` has `reference`: a value fixed by definition or convention that things are counted in. The ten Measures are `reference`. |
| To do 3, `rock.structure` | An enum: `rubble pile`, `monolith`. |
| Problem 2, one meaning to a field | A power module has a `generation` group (`supplies` W, `burns[]` with `rate` kg/s) and no recipe; a recipe requires `makes`; `amounts.quantity` is only kg per kg. `made_from.quantity` is kg (`x-unit: kg`): the piece as cut. A composition's entry is `part` only (the tile names `material.fused-silica`). An organisation's `about` and `details.text` are always lists. |
| Problem 3, kinds and keys | Every schema says `x-kind`, and the build checks each record's key against it. The 153 top-level keys are under `identity` (with the name, where there is one). LocalAdministration's `body` schema is `settlement` (`x-kind: [settlement, rig]`). |
| Problem 5, invented without review | The 31 basis entries (in 29 records) that say no source was looked up, or that the figure is from memory, are marked `review: true`; the review report went from 511 figures to 632. A value that was chosen as a design decision (a cabin pressure, a gauge) is invented and not to review: say if you want those marked too. |
| Problem 6, Dogma units | A law's `unit` is checked as an `x-unit` is. `law.tube-mass` is kg/m3. `law.air-top` is a pure number (scale heights) and has none. |
| Problem 8, the scripts' constants | `celestial_seed.py` and `celestial_export.py` read them from Dogma. |
| Problem 9, the equipment report | A gap only where a mass differs from the game. That none says what it is made of is a note. 0 gaps. |
| Problem 10, slips | Geometry has `x-unit: m`; `class_mix` is closed (O B A F G K M); the recipes doc's opening; this section. |

**Not done, and why:**

- **`game.goods`, `game.ore`, `capacity.stores`** still hold the game's keys. They name the game's
  market kinds and ores, which are not records here. They are in `game-keys.yaml`. They go when you
  decide what a market kind is; until then dropping them would take the warehouses' capacity by
  kind out of `settlements.ron`.
- **Problem 1, the shims.** Agreed: the build reads the new records through `old_names`,
  `reading`, `old_groups` and `read_schema`. Two are already gone in substance: lines and mill
  stock run on recipes natively (`route_for`, `plan`). Tell me which loader lands first and I will
  retire that kind's shim with it.
- **Problem 4, what is required.** Next pass, kind by kind, with you: what the engine's type needs
  to be non-optional.
- **Problem 7, shop recipes with no inputs: settled by the user** ("nothing appears from
  nothing"). A recipe requires `makes` and at least one input. The six shop modules have no recipe
  of their own but a `throughput` (rate, power); their recipes are the parts, hulls and gates
  that name them in `making.module`, each with its `made_from` (or its own parts) going in. The
  build refuses a part made in a module with nothing going in. `good.parts` and `good.hulls` are
  gone, which also closes your stock-and-goods item but for `good.hot-ingot-<metal>` beside the
  ingot stock items: a hot ingot is the same piece hot, kept as its own item on the user's rule
  that what passes between modules is a thing of its own.
- **A standard's text-or-blocks.** Standards are documents (the page renders them); `standards.ron`
  carries their params. Left as they are unless the engine is to read their blocks.
- **The product base, equipment's `function` and slots, the 17 missing, fuels, structures, a
  population's `of` and kinds, a settlement's orbit and population:** content and shape work, next.

**Your two notes on recipes:** agreed, and written into `docs/registry-recipes.md`: a line's
`makes` is the world's starting setup, which an owner can change; the rules for markets, full
stores and no transport inside a works are the game's mechanics, the doc their spec; and moving
stock inside a works at once and for nothing is coarse-first, labelled so.

## After Dogma landed on main (95059b2)

Merged into `fso`. Done from your note:
- `in_game` is gone from every law and from the law schema, with the `as` conversions. The Dogma
  report is gone: there is no second copy to hold the records to.
- Every law has a `note`, and the schema now requires it.
- Labels are as they were. The schema says a label is not to change without the engine.
- `build.py` takes the Tube laws in SI as the records hold them (it no longer turns `TUBE_T_LY`
  back to seconds per light year).
- Laws are out of `standards/game-keys.yaml`: the engine's constant is the law's label.

**One thing undone, because it broke your build.** The audit asked for a way to mark a reference
value, and I added `kind: reference` to the ten Measures. `dogmagen.rs` has `LawKind` as Real,
Simplified, Invented, so the engine would not build. They are `real` again, and the schema's kind
has three values. When `LawKind` and the engine's `Kind` have a fourth, say so and I will put it
back; or name another way you would rather it were marked.

## The rest of the game's content: equipment and fuels (2026-10-04)

Your items 1 and 4. The user's word on the equipment: outdated, to be reviewed and brought in
properly, but to be salvaged. So every record from the game's list is `identity.revision:
outdated`, and its figures are `invented`, `review: true`.

**Ship equipment: 57 records** (`SFO/metadata/equipment/`): the game's 56, each keyed
`equipment.<its key>` with `_` as `-`, and the throat coil, which is the registry's own.

```yaml
identity: { key: equipment.drive.torch.s1, name: "Torch drive S1", maker: org.kestrel, revision: outdated, slot: drive, description: "..." }
size_class: 1                 # 1 to 4: it fits a slot at least as big
physical: { mass: 3500, volume: 8 }          # kg, m3
needs: { power: 700000 }                     # W, working; left out where it draws none
function: { kind: drive, thrust: 900000, exhaust: 10000000, efficiency: 0.3, burns: material.deuterium }
```

- `identity`, `function` and `physical` are required; `maker` is required.
- `function` is one of 21 shapes, told apart by `kind` (a `oneOf` with `kind` a constant in each:
  an internally tagged enum). Each has only its own figures, all SI, each with its `x-unit`:

| `kind` | Its figures |
|---|---|
| `power_plant` | `output` W, `efficiency`, `burns` |
| `drive`, `thrusters`, `lift` | `thrust` N (one nozzle at full share), `exhaust` m/s, `efficiency`, `burns` |
| `tank` | `capacity` kg, `holds` |
| `capacitor` | `capacity` J, `rate` W |
| `rack` | `capacity` kg |
| `cabin` | `seats` |
| `hyperdrive` | `efficiency`, `top_speed` m/s (was `top_c`) |
| `flight_computer` | `turn_rate`, `roll_rate` rad/s |
| `sensors` | `range` m |
| `comm` | `capture` m, `link` m, `lag` s, `capacity` 1/s (was messages an hour) |
| `gate_relay`, `hyper_relay` | `lag` s, `capacity` 1/s, `cadence` s |
| `nav_computer` | `features` (docking, landing, gate, follow, hyperdrive, route), `interlock` m, `governor` 1/s |
| `transponder`, `life_support`, `gun`, `laser`, `mining_rig`, `throat_coil` | none |

- `burns` and `holds` name a material (`material.deuterium`).
- No price. Guns', lasers' and the mining rig's figures are still in your Rust: nothing was copied
  or made up for them. `docking` has no record, so no shape yet.
- `identity.slot` is still free text (drive, cargo, hardpoint, gate...): the kinds of slot are not
  records. With `function.kind` and `size_class` it may not be needed: say.

**Fuels: 9 materials** (`SFO/metadata/materials/`): deuterium, helium-3, d-he3, d-t, uranium,
methalox, kerolox, hydrolox, hydrogen. Keys `material.<name>`, with `material.helium3` now
`material.helium-3`, `d_he3` `d-he3`, `d_t` `d-t`. Each has a `fuel` group: `release` (fusion,
fission, chemical, none) and `energy` (J/kg; left out where it gives none). Its density as stored
is `mass.density`. The game's figures, marked to review. A material now takes `basis` and a
description.

**Deuterium is one record** (the user: "that is the same thing, should collapse").
`good.deuterium` is gone. `material.deuterium` is the one: equipment burns it, tanks hold it, and
the fusion power station's `generation.burns` names it. What it trades as is on it:
`identity.traded_as: market.fuel`.

**This needs one change on your side, or power stations stop buying fuel.** `settlements.rs:261`
finds a flow's market kind with `reg.good(item)`, which is `None` for `material.deuterium`, so the
station's `burns` comes out with an empty kind and `services/land.rs:365` has nothing to buy.
The tests pass, so nothing catches it. The user then settled what is what: **fuel is moved, kept and sold as stock, and what burns it
does not care.** So there is `stock.deuterium-liq` (made from `material.deuterium`, `traded_as:
market.fuel`): that is what lies in a store, on a ship and on the market. The power station and
ship equipment name the material and take whatever stock of it there is. For you: a `burns` or
`holds` that names a material is met by any stock item `made_from` it, and the market kind is the
stock item's `identity.traded_as`.

**Anything physical can be traded (the user).** Tradable is not a kind of record. So:
- **Market categories are records** (your item 5, drafted): 20 of kind `market`
  (`SFO/schema/market.schema.yaml`, `SFO/metadata/markets/`), `market.food` to `market.weapons`, from
  `goods.ron`: `unit_mass` kg, `bulk_density` kg/m3, `basket` kg a person a second (was t for a
  thousand people a day), `names` (adjectives, nouns). No price.
- **Any physical record can name its category:** `identity.traded_as`, a ref to a `market`, on
  material, mill stock, part, equipment, hull, gate and structure. Only `stock.deuterium-liq` has one so far.
- **Goods still have `game.goods`**, because your `Good` type refuses a field it does not know.
  When it takes `identity.traded_as`, I will move the nine over and `game.goods` goes.

## Hulls (2026-10-04)

Your item 2. All six hull records now carry what you listed:

```yaml
model: assets/models/mc07.glb          # the MC-07; the five have `shape: shape.drover`, a key in shapes.ron
slots:
  - { name: power, kind: power, size: 3 }
  - { name: hardpoint_1, kind: hardpoint, size: 1 }
thrusters:
  - { nozzle: nozzle_main_0, slot: drive, share: 1 }
flight: { radius: 15.9, drag_area: 399, frame_material: material.aluminium-alloy-6061 }   # the five: hull_strength, J
fit:
  - { slot: drive, item: equipment.drive.torch.s1, nozzles: 6 }
```

- A slot's `kind` is one of `common.schema.yaml#/definitions/slot_kind` (your `SlotKind`, snake
  case, and `gate` for a ring's). An equipment record's `identity.slot` is the same enum, so
  `life` is `life_support` there now.
- The build checks every fit and nozzle against the hull's slots: the slot exists, what is fitted
  is of its kind and no bigger. All six pass.
- **The MC-07:** its slots and nozzles are what `import.rs` makes of its model today (the class 3
  standard set with two hardpoints; 32 nozzles, each at full share). Its radius and drag area are
  your import's rules of thumb, marked derived. It has no `hull_strength`: `flight.frame_material`
  says what it is to be worked out from. It has no maker: the user has not named one.
- **The five** are from `hulls.ron` as they stand, `revision: outdated`. Their mass is still a
  frame figure (`physical.mass`): they have no parts.
- No price. `fit[].nozzles` is the registry's own count and can go once you read `thrusters`.

## Structures (2026-10-04)

Your item 3, as far as it goes without your side. Four records of a new kind, `structure`
(`SFO/schema/structure.schema.yaml`, `SFO/metadata/structures/`): `structure.platform`,
`structure.spaceport`, `structure.outpost`, `structure.orbital`. Each has `identity` (key, name,
maker, `kind`: station, spaceport, outpost, orbital; `revision: outdated`; description) and `fit`
(equipment by key, with a count). From `structures.ron` as it stands. The three rings were already
`gate.ring.*`. They are not on the registry's page yet.

**Not done: a settlement naming its structure.** Your `Settlement` type refuses a field it does
not know, so adding `structure:` to the records would stop the engine building. When your type
takes `structure` (a ref to a `structure` or a `gate`), say so and I will fill it: the station is
`structure.platform`; which ports are spaceports and which outposts is in `places.ron`'s rule,
which I would then need from you. The same holds for anything else I add to a kind you read:
I will ask first.

## For the hand-off (2026-10-04, after 60b6327)

What is on `fso` since your last merge, in the order to take it:

1. **Equipment** (57 records, a `function` by kind), **fuels** (9 materials), **hulls** (model,
   slots, thrusters, flight), **structures** (4 records): sections above. New kinds, or kinds you
   do not read yet: nothing of yours breaks.
2. **Deuterium.** `good.deuterium` is gone. `material.deuterium` is the substance;
   `stock.deuterium-liq` is what is moved, kept and sold, `traded_as: market.fuel`. The fusion
   power station's `generation.burns` names the material. **Your side must change with this
   merge**, or power stations stop buying fuel and no test says so: a `burns` or `holds` that names
   a material is met by any stock item `made_from` it, and its market kind is that stock item's
   `identity.traded_as` (`settlements.rs:261`, `services/land.rs:365`).
3. **Market categories** (20 `market` records) and `identity.traded_as` on every physical kind you
   do not read. Goods keep `game.goods` until your `Good` type takes `identity.traded_as`.

**Prices.** There is none anywhere in the registry, and there will not be: the user's rule is that
prices are the game's state. Every price in `modules.ron`, `hulls.ron`, `goods.ron` and
`structures.ron` was left out on purpose when those were brought in. When you retire those files,
their prices need a home on your side; nothing here holds them.

**I will not touch a kind you read without asking.** Your types refuse unknown fields, so a field
added here stops your build. Waiting on you for that reason: a settlement's `structure`, a good's
`identity.traded_as`, trade bans on the administration, a law's fourth kind.

**Open with the user, not for you to build yet:** who makes the MC-07; what a place makes (farms,
artisans, fabs and the rest as modules with recipes: nobody has figures); the MC-07's bay depth
and its three buckling sections (`docs/ships/mc-07-to-measure.md`).

## Food (2026-10-04): the user's, while waiting on you

`docs/registry-food.md`. In short: 75 goods (57 food), 18 farm and food modules with 76 recipes
balanced by mass, 9 companies. All in kinds you read, all with fields you have: your build and
tests pass. This is your item 6 for food, done as modules with recipes, not copied from
`recipes.ron`.

**Sourced since, and one finding for you** (`docs/registry-food.md`, "What Treistun's two worlds
with air do to a crop"): by `climate.rs` as it stands, Treistun e's warmest ground averages -10 C
and Port Nacaubun -19 C, so nothing grows in the open there; Treistun d farms between 25 and 50
degrees of latitude and is too hot at its equator. If e is meant to be a farm world (your
`PlaceKind::Farm` makes it one, with 60,000 people), either its warmth or its role has to change.
A crop's yield also wants the world's usable light (d 1.43 of Earth's, e 0.54) and its warmth
where the field stands.

**Asked of you:** a zone `use` of `agricultural` and a facility `kind` for a farm (and food works,
and a store), so the farms can be placed; and how a module says it stands under an open sky, where
a crop's carbon dioxide and water are the world's own.

**Nine more makers are in your brand list** (`org.eikir-growers` and the rest): companies of the
food trade, with no products of the kind your brands have. If a brand must sell ship equipment,
they need a `business` of their own: say which.

## Next on `fso`, in this order

Your items 5 to 8 each need something agreed before records are written:
- **5, goods and market kinds:** a new kind of record for the market's categories. I will draft
  its schema from `goods.ron`; `good` records then name their category, and `game.goods` goes.
  That changes the `Good` type you read.
- **6, what a place makes:** farms, artisans, pharma, fabs, factories and wells as industrial
  modules with recipes. That is new plant with figures nobody has: the user's to say how far to
  go, and whether guesses marked for review are wanted.
- **7, trade bans:** on the administration (the organisation record you read).
- **8, world data in Rust:** I need the list of values from you, by file.

## Where I'd do it differently

- **Derived figures on the page.** Agreed that the engine is the one that works things out. Until
  its command exists, the build keeps its formulas, each to be marked as a copy of an engine law.
  I'd keep one exception for good: the **structure and logistics checks** (a skin against a load,
  energy to orbit, a line's maximum against a demand) are how the spec is checked before anything
  is built. If the engine comes to own those too, fine, but they must run without a game world.
- **Radians.** SI says rad; nobody reads an inclination of 0.0198. I'd keep degrees in records as
  the one exception and convert at load, or we agree the page shows degrees and the record is rad.
  Your call, since it is your loader.
- **Standard gravity** belongs in Dogma as a named reference (9.80665, exact by definition), not a
  law of nature: `g` on any world is derived.
- **`rock.structure`** (rubble or monolith) is a property of a rock, not a ref: an enum.

## What I need from you

- The key format above: say if the loader wants `{kind, key}` objects instead of dotted strings.
- Degrees or radians in records.
- When a loader lands for a kind, which RON writer to delete.
