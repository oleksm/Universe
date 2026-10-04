# Request to the registry: one source of truth

*From the integration session on `main` to the registry session on `fso`, 2026-10-04. The user's
direction: "Our registry has to be SSOT to rule it all." The registry session solidifies the
registry, moves the data now held in Rust and hand-written RON into it, and creates a Dogma
registry. This lists what moves, what to consolidate, and where attributes are loose or missing.
File and line references are to `main` at 8ee71ed.*

## The target

- **The registry is the only copy.** The game loads `standards/` records directly into typed Rust
  structs, one struct per schema. No generated RON files, no mirrors in either direction.
- **The schemas are the engine's object types.** If a record needs something its schema lacks, the
  schema changes and the engine follows in the same merge, or the game refuses to load.
- **References, not copies.** A record names another by key, and the loader resolves every
  reference. A broken reference fails the load.
- **The engine works out what is derived** (a hull's mass from its parts, a stock item's mass from
  its material and size, gravity, tidal heat). The registry holds inputs, not results. The page's
  reports read the engine's results, so `build.py` keeps no formulas of its own.
- **To rebalance, change the spec and restart the world.** Each save carries a hash of the spec.
- **Guesses go in now, marked to review.** They are not held back for later (the user, 2026-10-04).
  Shock limits, thrust through the hull, small bodies, tidal heat and radiation are all to be built
  in the game on the registry's figures, with `review: true` showing where those figures are weak.
- **Physical stock is what ships haul** (decided 2026-10-04): items with mass, dimensions,
  temperature limits and shock limit, not abstract goods.

## 1. A Dogma registry

Dogma is the set of laws everything runs on. Today the laws are spread over four places:
`config/dogma.ron` (43 lines), `content/base/sheet.ron`, constants in Rust, and copies in
`build.py`. Proposal: a root `standards/Dogma/` with a `law` schema (key, value, unit, kind
real/simplified/invented, note, basis) and records grouped as today's sections. The game reads
it in place of `dogma.ron` and `sheet.ron`'s real entries.

What it takes in:

| Law | Where it is now | Copies to remove |
|---|---|---|
| SPEED_OF_LIGHT, STEFAN_BOLTZMANN | `dogma.ron` Nature | `game/src/fmt.rs:3` (`const C`); `build.py:1496` (`SIGMA`) |
| FIELD_COST, V_BEST_C | `dogma.ron` Field | none |
| TUBE_T_LY, TUBE_GAMMA, TUBE_EPS, TUBE_RHO, TUBE_K, TUBE_HOLD | `dogma.ron` Tube | `build.py:1429-1435` parses them with a regex and re-implements `physics/src/hyper.rs` in Python |
| G, AU, LIGHT_YEAR, SUN_MASS, SUN_RADIUS, EARTH_MASS, EARTH_RADIUS, DAY, YEAR | `world/src/units.rs:3-12` | light year again at `physics/src/hyper.rs:13` and `build.py:1431`; G three times in `build.py:2101,2102,2148`; AU at `build.py:1996`; DAY again at `services/src/economy.rs:48` |
| Standard gravity 9.81 | nowhere named | literals at `world/src/crew.rs:37`, `physics/src/atmosphere.rs:29`, `onfoot.rs:88`, `build.py:1401,1550,1647,1653` |
| SUTTON_GRAVES (entry heating) | `physics/src/atmosphere.rs:12` | none |
| Earth's air: 1.225 kg/m3, scale height 8,500 m, top at 14 scale heights | `physics/src/atmosphere.rs:29-30` | none |
| SOLAR_LUMINOSITY, AIR_CP, LAPSE_RATE, reference albedos | `content/base/sheet.ron` (marked `Real`) | none |

Not laws, so not for Dogma: `sheet.ron`'s heat skin figures (hull and material), gate sizes (the
gate product), CAPACITOR_DENSITY (a technology limit, a standard's param), GROUND_MARGIN (a nav
product's spec). Section 5 says where each goes.

Emissivity is inconsistent: `sheet.ron` EMISSIVITY is 0.8 and `build.py` uses 0.9. It belongs on
the material, not in a law.

## 2. Common definitions for every schema

The 30 schemas repeat the same groups in different spellings. A generated loader would carry every
variant into the engine. Proposal: one `standards/common.schema.yaml` that every schema `$ref`s.

| Definition | Today | Proposal |
|---|---|---|
| **identity** | `identity.name` (what `build.py` keys on), file name (what references use), game key (`drive.torch.s1` vs file `drive-torch-s1`), codes (parts, mill stock), symbols (elements), top-level `key` (company, SFO body). LocalAdministration records have no `identity`. Celestial uses `about` where SFO uses `description`. | Every record has one `identity.key`, which is also the game's key. `name`, `description` and `story` are display text only. |
| **ref** | By file name (`fit.item`, `making.process`, `steps.module`, `made_from.material`, `yields`, `gate.ring`), by code (`made_from.item`, `thrust_path`), by brand key (`maker`, `owner`), by game key (`rate.stores: goods.food`, `game.goods`), by display name (Celestial `parent`/`anchor`, `at: "Treistun f"`, `gate.to: Liham`, `galaxy.home`, composition `part: "Fused silica"`), by path (`address.at`) | One form, a typed key such as `good.stony-ore` or `{kind, key}`, checked against an index of every record |
| **basis** | Pasted into 13 schemas. Missing from element, material and process: their sources are YAML comments the game never sees (`materials/aluminium-alloy-6061.yaml`) | One definition, allowed on every record and on any single value. The game uses it to show `review` in the dev overlay |
| **lifecycle** | Six vocabularies: Celestial `status` seeded/curated/frozen; part `identity.status` draft/released/superseded; hull `identity.standing` current/outdated; standard `status`; vocabulary `game` made/partly/not made; `basis.review` | `provenance` (seeded/curated/frozen), `revision` (draft/released/superseded/outdated), `in_game` (made/partly/not made) |
| **physical** | Spread across `size`, `physical` and `mass`, in mixed units | One group for every physical thing: mass, length/width/height (or envelope), storage and operating temperature range, impact resistance, shock limit, bulk density where it is bulk |
| **units** | Mass in kg (part, equipment) and t (hull); length in m and mm (mill stock); thrust in kN and MN; power in kW and MW; distance in km, AU and Earth masses; time in hours, days and s | **SI throughout**, as the game holds it (kg, m, s, W, N, K, rad). The page converts for reading. If a figure reads better in t or AU, that is the page's job, not the record's. |
| **address** | Copied verbatim into company, SFO body and administration | One `$ref` |
| **ratings, forms** | Material uses none/poor/fair/good/excellent; process uses none/low/medium/high; three different form enums (`material.form`, `process.inputs.form`, free text in mill stock and outputs); `joining` copied between material and part | One rating scale, one form list |
| **organisation** | SFO `body`, MakerHouse `company` and LocalAdministration `administration` are three schemas for one kind of thing (`parcel.owner` already accepts `brand.` or `body.`) | One `organisation` schema with `kind: company \| standards_body \| administration` |

Also: the schemas are not enforced. `build.py` checks group and property names one level deep by
hand (`build.py:694-704, 984-990`) and skips `fit`, `composition` and `open_questions`. Please
validate every record against its JSON Schema in the build (types, enums, required, patterns,
`additionalProperties: false`). The game's loader will be at least that strict.

## 3. Schemas to consolidate

1. **The three `body.schema.yaml` files are three different things with the same name.**
   - SFO `body` is an organisation; see `organisation` above.
   - LocalAdministration `body` with kind planet/moon repeats the Celestial body: `treistun-e` is in
     both, with `gravity` copied and kind spelled `planet` in one and `rocky planet` in the other.
     Drop the planet and moon kinds from LocalAdministration. A settlement or rig refers to its
     celestial body by key, and the engine derives gravity.
   - The settlement and rig kinds that remain need schemas of their own (item 6).
2. **`small-body` and the Celestial `body`.** They share identity, `orbit`, `physical` and
   `rock.class`, and both allow `kind: asteroid`. The only difference is that the game doesn't make
   small bodies yet. Merge them into one body schema with one `kind` list (star, rocky planet, gas
   giant, ice giant, moon, dwarf planet, comet, centaur, crossing asteroid, captured moon, asteroid)
   and `in_game`. Also give a **star** a schema of its own instead of the inline `system.star`, so a
   companion star can be recorded.
3. **`field` and `region`** are both a population of many small bodies. Merge them into one
   `population` schema: kind (family, trojan swarm, outer belt, main belt, scattered disc, far
   cloud, meteoroid stream), `of` (a ref), and an extent whose frame is stated.
4. **One product base.** `hull`, `gate` and `equipment` each repeat identity with maker, size,
   `fit`, `making` and `basis`. The industrial `module` has no maker, key, making or parts, although
   `parts/arc-furnace/` and others exist. Make one `product` base (identity, physical, making,
   built_of, revision, basis) that each kind extends: hull adds `design` and `fit`, gate adds
   `performance.span`, equipment adds `performance`, module adds `rate`, `inputs` and `outputs`.
   Stations, spaceports, outposts and orbital sites become product kinds (item 6 in section 5).
5. **One stock item base (SFO 8).** The standard says mill stock, parts, assemblies and hulls are
   alike, but each spells it differently: part `made_from.item` and `making.processes[]`; mill
   stock `made_from.material` and `making.process` with sizes in mm; hull mass in t. Make one
   `stock_item` base: code, `made_from[]` (typed refs, each with a quantity), `making`, `physical`.
   This is the unit ships will haul.
6. **Goods that copy stock.** `goods/ingot`, `bar`, `tube`, `sheet-and-plate`, `parts`, `hulls`,
   `forgings`, `cut-blanks` and `formed-panels` are bulk names for mill stock, parts and hulls.
   `module.rate.product` points at these goods, while process outputs give a material plus a form:
   three names for one output. With physical stock, a module's or process's output is a stock item
   reference. A good keeps meaning bulk matter traded by the tonne (ore, lime, slag, water, food,
   fuel).
7. **Flows in one place.** Modules and processes both carry inputs, outputs, rate and power, per
   tonne of different things (`modules/arc-furnace.yaml` and `process.equipment.steps`). Put flows
   on the module only. A process becomes an ordered list of steps (module refs), and the engine
   derives its totals. Merge `module.amounts` and `process.amount_list` into one definition.
8. **Facility and rig share an installation.** A rig in LocalAdministration `body.schema.yaml` has
   untyped `processes`, `lines: {type: array}` and `modules: {type: array}`, copies of the
   facility's. Make one `installation` definition (processes, lines, modules) used by `facility` (on
   a parcel) and by a new `rig` schema (owner, orbit or position, spin). Also, `process.equipment.facility:
   rig` is used, but `facility.kind` has no `rig`.
9. **Gates.** The SFO `gate` is the product, and the placed gate is a LocalAdministration
   settlement with `gate: {ring, to, distance}`. Settlements and rigs should say which product they
   are (`structure: <product ref>`). `to` should be a system ref, and `distance` is derived from
   system positions. `built_of.parts: halcyon-ring` is one parts folder shared by classes I, II and
   III.
10. **Seeding laws.** `asteroids.yaml`, `conditions.yaml` and `galaxy.yaml` are single sets of
    generator settings, not objects. Give them one shape, `seeding`, kept apart from Dogma: Dogma
    is how nature works, seeding is how our seed makes a world.

## 4. Loose and missing attributes (fragmentation risks)

**Free text that should be a ref:**
- `equipment.performance.burns`/`holds: deuterium`: no such element or material (a good exists).
- `equipment.identity.slot`, `hull.fit.slot`, `hull.identity.class`.
- `part.making.facility`, `process.equipment.facility`/`machine`.
- `fasteners.kind`, `pipelines.carries`.
- `amount_list.name` for unregistered items (`processes/carbonising.yaml`).
- Composition `part`, which can be a symbol, a good or a material.
- `rock.structure`, which is not even an enum.
- Settlement facility lines name products by display name ("Ingot", four times on the mill), so
  the game cannot tell 4340 ingot from any other.

**Rock class has three spellings:** file name `stony` (small bodies), label `s-type stony` (bodies,
fields), key `"S-TYPE STONY"` (`rock-classes/stony.yaml`). Use the key (section 2).

**Structure that lives only in folders:** hull → parts and assembly → children exist only as
folder layout. `build.py` adds `parent`, but no schema has `parent` or a bill of materials. Needed:
`built_of` on the product and `parent` on the part, or a bill of materials list. Assemblies (SFO 14)
have no schema at all. `0012-parts.yaml` says parts are filed by hull, but `parts/<module>/` and
`parts/halcyon-ring/` are parts of other products.

**Equipment `performance`** is a grab-bag of 19 optional fields, where the game has a tagged union
(`does: Drive(...)`, `world/src/modules.rs:15`). Proposal: `function` (the kind of device:
power plant, drive, thrusters, lift, tank, capacitor, rack, cabin, hyperdrive, flight computer,
sensors, comm, gate relay, hyper relay, nav computer, gun, laser, mining rig, transponder, life
support…), with the parameters that function needs. Plus the game's fields the schema has no home
for: size class, seats, capacitor charge rate, nav `interlock` and `governor`.

**Missing fields:**
- Mass on modules and gates.
- An orbit or position for stations and rigs.
- An owner on a facility (today inferred from its parcel).
- Storage temperature and shock limit on goods (the stock rule says every physical thing has them).
- On the MC-07: `key`, `maker`, frame mass.
- Hulls: shape or model ref (`assets/models/mc07.glb`), radius, drag area, hull strength (or its
  material, so the engine derives it), slot sizes, and the nozzle → thruster slot share table (the
  schema has only `fit.nozzles` as a count).
- Price: no schema has one. See the open question at the end.

**Untyped arrays:** LocalAdministration `body.schema.yaml` `lines` and `modules`.

## 5. What moves into the registry

### 5a. The hand-written content files

| Game file | Records | Registry home | Needs adding first |
|---|---|---|---|
| `modules.ron` (56) | ship equipment | SFO 16 `equipment/` (39 today) | The 17 missing records: cabin.s1–s4, capacitor.s3, comm.long.s1, drive.kestrel.k2, hyperdrive.halcyon.s1–s3, power.aurel.s1–s3, rack.s2, tank.s2, terminal.ground, transceiver.station. The `function` union (section 4). Units in SI (the schema has kW, kN, km). Names as the game's (`messages` → capacity, `top_speed` → top_c, `exhaust_speed` → exhaust) or the game follows the registry: either way, one name. |
| `hulls.ron` (5) | hulls | SFO 2 `hulls/` | The game needs a playable fleet, and the five stock hulls are `outdated`. Either they stay loadable as outdated records, or their replacements arrive. The fields listed under "Missing fields" above. |
| `structures.ron` (7) | stations, spaceports, outposts, orbital sites, gate rings | the rings are in `gates/`; the rest have no kind | A structure product kind (section 3, item 4) with size, built_of, fit, power. The ring size now copied in `sheet.ron` (GATE_RADIUS, RING_TUBE) comes from `gate.size`. |
| `materials.ron` (9) | fuels | SFO 5 `materials/` (24 structural, no fuels) | Energy per kg, process (fusion, fission, chemical), stored density. Records for the 8 missing fuels: helium-3, D–He3, D–T, uranium, methalox, kerolox, hydrolox, hydrogen. |
| `ores.ron` (5) | ores | goods with `kind: rock` already cover all 5 | Nothing but price. Rock classes' `yields` already point at them. |
| `goods.ron` (20) | market kinds | none (goods records point at them via `game.goods`) | Decide what a market kind is once stock is physical. Probably a category over stock items and goods (food, metals, machinery…), with price range, unit mass, basket and bulk density. Its `adjectives`/`nouns` generate item names: seeding vocabulary. |
| `recipes.ron` (12) | coarse works (MINE, FARMS, FACTORY…) | processes and modules, at material level | To be retired, not copied: the economy runs on facilities' lines. Processes are needed for what recipes cover and the registry lacks: farms, artisans, pharma, fab, factory, wells. |
| `places.ron` (4) | settlement types | settlements and their facilities | Population on the settlement. `sells` and `wants` are outcomes ("platform, not outcomes"), for the economy to derive, not to record. `ship_fuel` on the port facility. |
| `markets.ron` (1) | trade bans | none | Bans are administration law: per administration, beside zoning. The ban chances are seeding. |
| `sheet.ron` (28) | mixed | split | Real laws go to Dogma (section 1). Heat skin goes to hull and material. Albedos go to Celestial. Gate sizes go to `gate`. Capsule masses go to the relay equipment. CAPACITOR_DENSITY becomes a standard's param. GROUND_MARGIN goes to the nav product. |
| `shapes.ron` (5) | geometry and models | stays an asset | The hull record refers to its model. The nodes the world depends on (nozzles, mounts, gear, dock, cockpit) are hull spec; your call whether they move to the record. |
| `aliases.ron` | old key → new key | game | Or a `renamed_from` on each record. |

### 5b. World data now in Rust

Grouped by where it goes. Engine tuning (render, LOD, network, physics stepping, UI, avionics)
stays in code.

**Seeding (how the seed makes the world):**
- `world/src/galaxy.rs:103-114`: REGION, STAR_DENSITY, REGION_CENTRE, SECTOR. Already copied in
  `galaxy.yaml`; the game reads the Rust and a test keeps them equal.
- `galaxy.rs:59-68`: the star class mix (duplicates `class_mix`).
- `galaxy.rs:20`: `StarClass::params`.
- `galaxy.rs:125`: the galaxy shape's own seed, hard-coded to 1984.
- `galaxy.rs:152-222`: the spiral's settings (arm pitch, thickness, 40k samples, SHAPE_CELLS,
  SHAPE_REACH).
- `world/src/system.rs:186-338`: the system generator.
  - Star mass ±15%; frost line 2.7 AU·√L; first orbit 0.25 AU·√L; spacing ×1.5–2.1.
  - Rocky planet mass and radius; what counts as temperate.
  - Giant odds, sizes and masses; ring odds; eccentricity, day and tilt ranges.
  - Moon density 3,000 for every moon, icy or not (`:298`); moon orbit 3–6 planet radii up to 0.35
    of the Hill radius; moon sizes and spacing.
  - Station mass and radius (`:423,433`), gate mass (`:502`), GATE_CLEARANCE (`:183`).
  - Belongs on each vocabulary kind's seeding block.
- `system.rs:155-159`: the body colour palettes go to the vocabulary kinds.
- `world/src/belt.rs`:
  - SMALLEST (`:37`, already in `asteroids.yaml`), SWARM_MAX, SWARM_HILL (`:39-42`).
  - Belt edges (`:345`; the no-giant pair is already in `asteroids.yaml`).
  - Kirkwood gaps and tolerance (`:348`).
  - Family class odds (`:289-300`), trojan odds (`:363`), outer belt range (`:378`).
  - Spin ranges (`:261-262`).
- `world/src/terrain.rs:14-77`: pad flats, relief settings, relief amplitude, crater counts per
  terrain kind. PAD_FLAT_INNER is copied at `services/src/land.rs:32`.

**Rock classes and ores:**
- `belt.rs:83-158`: RockClass density, albedo, colour, composition.
- `world/src/mining.rs:43-55`: cut energy, ores.
- `mining.rs:39`: PGM_RICH.
- `goods.rs:195`: the `Ore` enum.

The records already exist; the game must read them instead of being checked against them. Still to
add: colour, and density, composition and yields for basaltic, enstatite and stony-iron (without
them they can't be made).

**Hulls and equipment:**
- `world/src/design.rs:25-30`: FRAME_PER_AREA, STRENGTH_PER_KG (the frame material's), PRICE_PER_KG,
  PRICE_PER_SLOT_SIZE.
- `design.rs:109`: KNOBS; `design.rs:447`: standard_slots. These belong to the hull standard.
- `ship.rs:137`: PASSENGER_MASS goes to a person record. `ship.rs:640`: SHIP_RADIUS goes to the hull.
- Device specs are products: `weapons.rs:27-70` (gun, laser), `missiles.rs:20-31`,
  `turrets.rs:21-108`, `radar.rs:12`, `mining.rs:35-37` (excavator 300 kW, 10 kg/s), `mining.rs:70-74`
  (anchor).
- `heat.rs:24`: ABLATION goes to the material.

**Settlements, facilities, structures:**
- `station.rs:20-35`: deck geometry, DECK_SPEED, BUMP_SPEED.
- `spaceport.rs:10-21`: PAD_RADIUS, LAND_SPEED, GRID, PAD_SPACING, PAD_SIZE. `build.py:182` reads
  this Rust file back.
- `spaceport.rs:72`: the vending list.
- `traffic.rs:15-19`: DOCK_RANGE, LAND_RANGE_RADII.
- **Landing speed is a hull's spec, not a port's.** The MC-07's sourced 3.05 m/s design and 8.2 m/s
  failure replace LAND_SPEED 30.

**People:** `world/src/crew.rs:28-39`: eye height, walk, run, jump, reach, climb go to a person
(species) record.

**Administration and economy:**
- `services/src/land.rs:22-39`: LAND_PRICE, MODULE_PRICE, BUILD_RATE, POWER_PRICE, LEAST_SIDE,
  STREET_HALF go to the administration's office.
- `services/src/economy.rs:28-47`: population, growth, hunger, migration, death go to the
  settlement/population.
- `services/src/market.rs:36-173`, `outfitter.rs:20-24`, `sim/src/commerce.rs:16-647` (buy-back,
  fuel price, fares, insurance, repair, settler credits) go to market rules.
- `sim/src/standing.rs:25-38` and `services/src/law.rs:12` go to law.
- `sim/src/universe.rs:32` STARTING_CREDITS and `ship.rs:15` STARTING_HULL go to a start record.
- `sim/src/newsroom.rs:19-58`: DIGEST_EVERY, mastheads go to the news outlet.
- Not for the registry: `sim/src/operator.rs:31-42` (pirate odds, minimum profit). These are NPC
  intentions; by the no-intentions rule they belong to the NPC client.

## 6. What `build.py` stops doing

- **Writing RON.** All eight generated files go once the game loads the records. Until then each
  goes as soon as its loader lands on `main`; I'll say which.
- **Reading game files back in:**
  - `build.py:182` reads `spaceport.rs`.
  - `:193` reads `ores.ron`.
  - `:194` reads `goods.ron`.
  - `:1408` reads `modules.ron` (the source of the Equipment report's 40 false gaps; the report
    compares mass only, while every row flags its empty "Made of").
  - `:1429` reads `dogma.ron`.
  - `:1432` reads `structures.ron`.

  Each goes away when its data is a record.
- **Working out what the engine works out.** Gate costs, tidal heat, gravity, balance points, mass
  sums. The engine will have a command that writes its derived figures as JSON for the page to
  read. Until then, mark any Python formula that copies an engine law as such.
- `celestial_export.py` (the game's seed written into seeded records) stays. It is the one
  deliberate flow from the game into the registry, and the guard test needs it.

## 7. Errors found in the spec

- **Tidal heat.** Recomputed from the records: Treistun j I 531 W/m², i I 1,860, h I 45, i II 44 (Io
  is about 2). Two causes: the seed puts first moons 1.5–2.2 Roche radii out on undamped random
  eccentricities (`system.rs:288,301`), and Io's k2/Q of 0.015 is applied to every moon. The seed
  is moving to the registry (5b), so fix it there. The charted world will change.
- **Small bodies' rock classes.** 13 small bodies use classes the game lacks, and basaltic and
  enstatite have no density, so those bodies have no mass. Comets use icy-monolith density 950;
  real comets are about 500–600.
- **Two ways of picking a belt's class.** `celestial_seed.py` picks the largest body's class by warm
  or cold only. The build weights three zones by area. They disagree.
- **Asteroid moons.** The minimum separation of 300 km is more than a 1 km main-belt rock's Hill
  radius (about 250 km). The figure comes from large trans-Neptunian binaries.
- **Meteoroid streams** are counted as crossing every planet between perihelion and aphelion. At up
  to 35° inclination, only planets near the two nodes are crossed.
- **Captured moons** can be placed past the 0.47 limit the vocabulary claims. A dwarf planet is
  always made, even where the expected count is near zero.
- **Trojan counts:** 500k per swarm whatever the giant's mass.
- **Small numbers:** the 7:3 gap is 0.5684, not 0.5703. A dead `gas =` assignment at
  `build.py:2004`.
- **Vocabulary notes:** "two to five fields a system" (Treistun has 8) and "outer belt, one field"
  (the game makes 1–2).
- **`docs/registry-integration.md`:** `rock_classes.ron` is on `main`, not only on `fso`.
- **Belt rock-class odds:** the spec uses three zones (cut at 0.93 and 1.04 of the frost line); the
  game uses two (cut at 0.9). This resolves itself once seeding is a record.

## 8. Suggested order

1. `common.schema.yaml` (identity/key, ref, basis, lifecycle, physical, SI units, address) and
   schema validation in the build. Everything else builds on these. This is the one change that
   touches every record, so it goes best before the records multiply.
2. The Dogma registry. Small, and every other part reads it.
3. The celestial consolidation (one body schema, star, population, seeding) and rock classes
   complete. The game already reads this area, so it's the first to switch to direct loading.
4. The product base and the stock item base. Equipment completed from `modules.ron`, structures,
   fuels. The goods/stock split.
5. Installations, rigs, settlements' structure refs, flows on modules, typed products in lines.
6. Economy and administration figures (5b), market kinds, the start record.

As each step lands on `fso` and the user has it merged, I switch the game to load that part
directly, delete the RON file or Rust constants it replaces, and say which `build.py` writer or
mirror can then go.

## Answered by the user (2026-10-04)

- **Prices are volatile data, not spec.** No price goes into the registry. Prices will come from a
  stock exchange and a mechanism of their own, to be designed. So: drop price from every request
  above, `goods.ron` price ranges and the module, hull and land prices stay game-side until that
  exists, and no schema gains a price field.
- **The playable fleet is outdated, with no future.** No decision to make on it: the five stock
  hulls are not to be preserved. If moving to the registry breaks them, it breaks them. Don't add
  fields or records to keep them working.
- **Ship model nodes** (nozzles, mounts, gear, dock) are the spaceship engineer's work on `ships`,
  not finished yet. Leave them out of the hull schema until that work says what it needs.

## Answers to the registry's response (2026-10-04)

Replying to `docs/registry-ssot-response.md`.

**The key format: dotted strings, and the first segment is the schema's kind.** `<kind>.<name>`,
lower case, words joined by `-`. The name may have further dotted segments where a record lives
under another (`body.treistun.treistun-f`, `equipment.drive.torch.s1`). A ref is that string. The
loader takes the first segment, finds the schema, and checks it against the kinds the field allows.
No `{kind, key}` objects: strings read better in YAML and carry the same information.

One change to your proposal: **don't keep the game's keys where their first segment isn't a schema
kind.** `drive.torch.s1` would make `drive` look like a kind when the record is equipment, and
`structure.ring.i` is a gate. The loader needs exactly one schema per first segment. The game
renames on its side. That's cheap: the worlds restart on a spec change anyway, the content files
are moving into the registry, and `aliases.ron` exists for old saves if we want them. So
`equipment.drive.torch.s1`, `gate.ring.i`, `hull.mc-07`, `good.stony-ore`. Organisations take
whatever kind the organisation schema is named (`org.hadley`); until it lands, `company.hadley`.

**Angles: degrees in records, as the one exception to SI.** Records are written and read by people,
and an inclination of 1.13 reads; 0.0198 doesn't. Mark it on the schema property in a form the
loader can read (`x-unit: deg`), and the loader converts to radians when it loads. Everything else
is SI as agreed (kg, m, s, W, N, K, Pa). No other exceptions: not t, not km, not AU. The page
converts those for reading.

**Structure and logistics checks without a game world: agreed.** The laws they use will live in a
crate that needs only the records (physics and the registry loader, no world, no simulation). It
gets a command-line tool that runs the checks on the records alone, and the build calls it. Until
then the build keeps its formulas, each marked as a copy of an engine law, and the checks stay
yours.

**Standard gravity as a named reference value in Dogma: agreed.** 9.80665, exact by definition,
marked as a reference, not a law. A world's own `g` is derived.

**`rock.structure` as an enum (rubble, monolith): agreed.** It's a property, not a ref.

**The ones left to me:**
- The 7:3 gap.
- The moons' orbits (the seed).
- Two zones against three in the belt odds.

All three change the charted world and are seeding, so they move with the seeding records (section
5b) rather than being patched in `belt.rs` and `system.rs` now. k2/Q marked to review is fine.

**RON writers:** I'll say which writer can go each time a loader lands for a kind on `main`.

## Audit of the registry at 8991bed (2026-10-04)

The integration session checked the work merged at 8991bed against this request and the answers
above. The build passes, 874 records fit their schemas, the game's 85 tests pass, and the RON files
are byte-for-byte the same, so nothing changed in play.

**Done well:**
- One key on every record. The first segment is always a kind, keys are checked unique and
  matching their file, and no key carries an old game prefix.
- About 1,200 references by key, each marked `x-ref` with the kinds it takes, and checked.
- SI throughout, with angles in `deg` the one exception. Every unit is declared as `x-unit`.
- Organisation, one body schema (the star a body, small bodies bodies) and one population schema.
- One physical group, one `made_from`/`making`, installation, seeding.
- Dogma as 31 laws, with the build reading its constants from them.
- A gate's distance worked out. The zero system positions fixed.
- No price anywhere.

**Still to do, as agreed:**
1. **An old→new key table.** The mapping exists only as code in `build.py:50,73,1779` (`OLD_KEY`,
   `old_name`, `game_key`), about ten families of rename. `docs/registry-integration.md:211` lists
   three. The game needs it as data to rename. Also, `game.goods`, `game.ore` and `capacity.stores`
   still hold old game keys (9 goods records, tank farm, general warehouse): make them refs, or drop
   them.
2. **Standard gravity** is `kind: real` under the key `law.standard-gravity`. The law schema needs a
   way to mark a reference value.
3. **`rock.structure`** is still free text ("rubble pile", "monolith") at
   `Celestial/schema/body.schema.yaml:141`. It should be an enum.
4. **The product base:**
   - Maker and mass on modules (0 of 32 have either). Mass on gates.
   - The MC-07's maker and frame mass.
   - A part's `parent`/`built_of` (the parts tree is still only folders).
   - `gate.built_of.parts` names a folder.
   - Structures (platform, spaceport, outpost, orbital) are not yet records.
5. **Equipment:**
   - The `function` union (`performance` is still 19 optional fields).
   - `slot` as a ref.
   - The 17 records missing from `modules.ron`.
   - Fuels as materials: there are none yet.
6. **Stock and goods:** `good.parts` and `good.hulls` remain, and are what the shop modules make.
   `good.hot-ingot-6061` sits beside `stock.al6061-ingot`.
7. **Settlements and rigs:**
   - Their schema is still named `body` (LocalAdministration), which clashes with the celestial
     `body`.
   - There is no orbit or position for stations and rigs.
   - No structure ref, no population.
8. **Population:** no `main belt` kind, no `of` ref, two optional extents.

**Problems to fix:**
1. **The build still runs on the old shapes.** `build.py:73-288` rewrites every record back to the
   old keys, units and shapes before using it, and flattens a module's recipes and a `made_from`
   to their first entries. "Byte-identical RON" proves the shims, not the new schema. Retire them
   kind by kind; I'll say when each loader lands.
2. **Fields that change meaning, which a typed loader can't hold:**
   - A recipe has either `makes` or `supplies` (no `oneOf`).
   - `amounts.quantity` is kg/kg or kg/s depending on the recipe.
   - `made_from.quantity` is m² or m depending on the stock, with no `x-unit`.
   - Composition takes `part` or `name`.
   - Text-or-blocks `oneOf` in standard and organisation.

   Proposal: a separate power recipe, and one meaning, with one unit, per field.
3. **The kind→schema mapping is implied** (`stock` → mill-stock, `dogma` → section,
   `settlement`/`rig` → LocalAdministration body), found only by path rules in `validate.py`. Let each
   schema declare its kind (`x-kind`), and move the 153 top-level keys under `identity`.
4. **Nearly everything is optional.** Hull, equipment, part and good require only `identity`, and
   12 groups of the body schema require nothing. Require what every record of a kind must have,
   or the engine carries `Option` everywhere and checks by hand.
5. **105 invented values without `review`**, including all 32 modules (`welding-bay.yaml` says no
   source was looked up), the MC-07's cabin pressure and minimum gauge, 49 parts, gate size and
   capture speed.
6. **Dogma units are free text** (no `x-unit`). `law.tube-mass` says kg where `TUBE_RHO·d³` makes
   it kg/m³. `law.air-top` has no unit. The Dogma report's 4 gaps are engine literals with no
   name (Earth's air density, scale height, air top, 9.81 where the law is 9.80665). They are mine
   to fix on the engine side.
7. **Shop recipes make goods from nothing** (`makes: good.parts`, no inputs), against "every recipe
   balances".
8. **Seeding scripts keep their own constants:** `celestial_seed.py:18` (Sun mass 1.98847e30 against
   Dogma's 1.989e30) and `celestial_export.py:30`. Point them at Dogma.
9. **The Equipment report's 40 gaps are written in:** `build.py` (~1795) always passes `row("gap",
   ...)` for "Made of: not yet said". Every mass matches. Drop the forced gap.
10. **Small slips:**
    - LocalAdministration geometry (`outline`, `line`, `point`) has no `x-unit`.
    - `seeding.galaxy.class_mix` is an open object.
    - `docs/registry-recipes.md` opens "Nothing is migrated yet".
    - The response's "Next" still waits on answers given.

**The recipes proposal** (`docs/registry-recipes.md`) is sound against the rules: a setup is game
state chosen by an owner, so the engine holds no intentions, and players and NPCs set up modules
alike. Two notes:
- A facility line's `makes` and the seeded setups are a world's starting state, not the module's
  limits. The game will load them as ordinary setups an owner can change.
- The rules for markets, full stores and "no transport inside a facility" are economy mechanics.
  They belong to the game, with the doc as their spec.

Moving stock inside a facility instantly, at no energy, is coarse-first; label it so.

**Suggested order for the registry:** items 1–3 of "Still to do" and problems 2–3 first. These are
what the game's loader needs before it can read records directly. Then the product base and
equipment, then the rest.

## Landed on main

**Dogma (95059b2).** The engine makes its laws from `standards/Dogma/metadata` at build time:
- The section records and their law folders.
- Each law as a constant under its `identity.label`, valued as the record holds it, in SI.

`config/dogma.ron` is deleted. `universe_world::units` re-exports Dogma's measures. The world
sheet's real entries (SOLAR_LUMINOSITY, AIR_CP, LAPSE_RATE, the four albedos) are gone from
`sheet.ron`. STANDARD_GRAVITY, EARTH_AIR_DENSITY, EARTH_SCALE_HEIGHT and AIR_TOP replace the bare
numbers.

What the registry can now drop:
- `in_game` on every law, and the Dogma report: the engine has no copy left to hold the records to.
  The report shows 31 gaps until it goes.
- The `as: per light year` / `as: times the speed of light` conversions: the engine works in the
  records' SI.

Needed from the registry for the loader:
- **The section record's `order` and the law's `label` are what the engine keys on:** keep labels
  stable.
- **A law's `note` is optional, but the engine documents each constant with it:** please give every
  law one.

**The registry crate (this commit).** `crates/registry` reads `standards/` into typed records when the
game is built and carries them in the binary. Today it reads `seeding.galaxy` (the galaxy's
settings, now the game's only source of them), `rock-class.*` and `system.*` (identity and position).
- **`galaxy.ron` and `rock_classes.ron` are no longer loaded:** their writers in `build.py` can go.
- **`seeding/galaxy.yaml`'s comment** "The game does not read this record yet" is now wrong.
- **`seeding.schema.yaml`, `galaxy.home`:** the description is cut at a colon, leaving a stray
  property `by its key.: null`.
- **The loader is strict:** a record of a kind the game reads (`seeding.galaxy`, `rock-class`,
  `system`) with a field its type doesn't know fails the game's build. Tell me when a schema the
  game reads gains a property, and I'll add it in the same merge.

**Rock classes (this commit).** `belt::RockClass` is now a handle on a `rock-class.*` record:
density, albedo, colour, composition, cut energy and yields all come from it. `good.*` records
are read as well (strictly; the `physical` group is typed for every kind that uses it).
- **`rock-class.icy` `letter: comet-like`:** the game shows `letter` (in capitals) in tight lists
  (asteroid lists, lock read-outs, about 9 characters). "comet-like" isn't a spectral type. Either
  give icy a short letter, or add a short `tag` property the game shows instead.
- **The game maps a yield to its ore through `good.*.game.ore`.** That stays until ores are
  goods in the game (`ores.ron` moves into the registry). Don't drop `game.ore` before then.
- **`RUBBLE_ENERGY` (2,000 J/kg) is still a code constant:** the rock-class schema's
  `cut_energy` description names it. It belongs to the registry, either in Dogma or as a seeding
  value. Say which, and I'll read it.

**Celestial systems (this commit).** `system.*`, `body.*` and `population.*` are read whole, strictly
typed, every group of the body schema included. The game builds its charted systems from them.
- **`celestial.ron` is no longer loaded.** With `galaxy.ron` and `rock_classes.ron`, that's three
  writers in `build.py` that can go.
- **What the game reads from a body is fixed by its schema:** adding a property to `body`,
  `population` or `system` needs the game's types to follow in the same merge (the build stops
  otherwise). Tell me, and I'll add it.
- **The loader takes a system's bodies from `body.<system>.*`** and its fields from
  `population.<system>.*` (kinds family, trojan and outer), where `in_game` isn't `not made`. A body
  of a kind the game doesn't make, without `in_game: not made`, stops the game at start. Keep
  marking them.

**Organisations and standards (this commit).** `org.*` and `standard.*` are read whole, strictly
typed.
- **`brands.ron`, `bodies.ron` and `standards.ron` are no longer loaded.** That's six of the eight
  writers that can go: those three, plus `galaxy.ron`, `rock_classes.ron` and `celestial.ron`. The
  game's hand-written files now name makers `org.*`.
- **`standard.refs` are still citations** (`SFO 2`, pattern `^[A-Z]{2,6} [0-9]+$`). Make them keys
  (`standard.sfo.2`, `x-ref: [standard]`) like every other reference; the game turns them into
  keys today.
- **A standard's body is found from its key** (`standard.sfo.*` → the body whose prefix is SFO).
  Fine as long as keys keep the prefix; an explicit `body` ref would be clearer.
- **`organisation.schema.yaml`, `note`:** the description is cut at a comma, leaving a stray
  property `as the game shows it.: null` (the same slip as `seeding.galaxy.home`).
- **Branches are still the topics, flat.** If the register is to be a tree (`parent`), say so, and
  the game's standards view will follow the parents.
