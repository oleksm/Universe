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
106 properties changed unit, 2,322 values in 439 records; 233 properties carry an `x-unit` in all.
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

## Next on `fso`, in this order

3. **`physical` as one group** across products, stock and bodies.
4. Then your order: Dogma, the celestial consolidation that is left (seeding records), products and
   stock, installations, economy.

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
