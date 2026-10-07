# Registry audit: duplication, scattered facts, coherence

2026-10-07, on fso. Three read-only passes (schemas; records; docs, page and tools) plus counts over
the tree. The registry is 2,617 records, 40 schemas, 7 roots, 46 docs and 13 tools. Findings are
ranked by what they cost; each carries one proposed consolidation. Two build bugs found by the audit
were fixed the same night (§0). Everything else is a proposal until the user picks.

## 0. Fixed tonight

- **The Mounts report showed the clocks.** The Clocks block inserted into build.py reused `rows`
  between the mount rows being built and the mounts report being written; the page said "mounts:
  12 rows" and no equipment-against-mount check reached it. Now 150 rows, 0 gaps.
- **Every piece of equipment was loaded twice.** SFO 16 and SFO 20 both said `records: equipment`;
  the loader read the folder once per standard and its duplicate check reset per standard. The page
  carried 302 equipment for 151 files, doubling parts, chains, dimensions and confidence. SFO 20 no
  longer claims the folder.

## 1. The big four (structure that multiplies work)

| # | Finding | Size | Cost | Consolidation |
|---|---|---|---|---|
| 1 | **Generated parts are boilerplate.** 889 part files in 185 folders; 838 are "fitted within" shares (mass × share, box scaled by the cube root), 50 measured from a model. Three identical basis sentences in each; the 15% cutting loss stated 474 times, twice per file. | 34% of all records | a change to the cut loss or the fitting rule is 475 edits; the files say nothing a rule doesn't | **Parts as a rule, not records**: an equipment record lists its parts as `{name, share, item, module}`; the build derives mass, box and cut loss from the record and the rule (SFO 12 text). Keep files only for measured parts (MC-07's 24, the drawn ones). Removes ~840 files. |
| 2 | **Every good has a mirror stock record.** 217 goods ↔ 217 `*-BULK` stock records, all pure wrappers (one good, form bulk, no size). Two stock folders (`stock/`, `mill-stock/`) under one key prefix. | 8% of records | the goods list exists twice by construction | **Kept, on reflection (2026-10-07):** deriving the stock would make the good name its market, which the user's rule forbids (only stock is sold; the validator catches a good that names one). The wrapper is that rule made explicit; it stays minimal and generated. |
| 3 | **Shared field groups are copied inline, and the generator ignores `allOf`.** The jet figures ×4 (drive, thrusters, lift, engine; the 140-word heat_to_hull text ×3), identity ×30, `traded_as` ×6, lat/long ×2, plan point ×5, `{profession, count}` ×3, reactivity enum ×3, zone use ×2, joining ×2. `physical_object` requirements never reach Rust: `Physical.mass` is `Option` everywhere. | ~60 copies | each copy is its own Rust type; required-ness lies | **Definitions in `common`** (`jet`, `product_identity`, `surface_position`, `plan_point`, `staff`, `composition`) referenced by `$ref`; shared enums into the dictionary; teach build.rs `allOf` or drop it. |
| 4 | **A second validator lives in build.py.** ~330 lines rebuild each record into its pre-SSOT shape and check fields against hand lists (makers, settlements, facilities, parts, admins, STANDARD_FIELDS, BODY_KINDS, ZONE_USES…) that the schemas already enforce (`additionalProperties: false`). Three validators overlap (validate.py, check.py, build.py); the hand lists drift (processes still allowed). | 330 lines + every schema change needs a list edit | the recurring "extend the hand list" tax; three places to be wrong | **One gate**: check.py (shape) + validate.py's reference pass; build.py keeps cross-record rules only (site plans, lines, chains, budgets) and reads records in schema shape. Delete the old view kind by kind. |

## 2. Scattered facts (say it once)

- **Boilerplate notes**: "fifteen years of service" ×151, "twenty-five years for plant" ×105, "50
  years EN 1990" ×25, "forty years for a hull" ×6; "a typical figure from memory" (bulk density) ×189;
  "from memory of real plant of this kind" ×122; Discovery II "brought to this product's weight"
  ×226; haul-truck body shares ×60; "a tenth more room and a quarter more weight" ×57 mounts plus
  SFO 19's text; "the game's early list… kept to be salvaged" ×56. **Rule**: a basis may name a
  rule (`rule: life.ship-system`) defined once in a standard's text or a `standards/rules.yaml`;
  the record's own note says only what differs.
- **Sources uncited**: `research_plant_weights.json` (86 KB), `research_yard.json` (64 KB),
  `research_modules.json`, `research_asteroids.json`, `research_rock_structure.json` are cited by
  no record, while the 122 "from memory" plant-part notes sit exactly on the plant they cover, and
  the "chosen" 15% cut loss has a sourced range (60–70% manual, 82–90% automated nesting) in
  `research_yard.json`. 426 of ~700 citations point at one file; 148 cite "the game" as the source
  of the registry the game follows. **Rule**: cite the file; flag any "from memory" whose topic a
  source holds.
- **Facts in schema descriptions** (ISS leak and recovery figures, oleo 0.8, Discovery II and
  Daedalus in the engine-cycle values, Earth constants in body, formulas for sandwich panels, tidal
  heating and heat pumps, a date in a default). They end up in Rust docs and can't be reviewed as
  facts. **Rule**: a description says meaning and unit; the figure goes to a basis, a standard's
  text or Dogma. Jupiter's mass is hard-coded in two seeding records and not in Dogma.
- **Day-0 census written into 30 records** by census.py (settlement `census`, `independents`,
  resupply and warehouse comments, "one ship to about 220 people" restated in seeding, the patrol
  rule restated per fleet entry). Fleets add to 182 ships plus interceptors; resupply comments add
  to 39 haulers on runs against a freight fleet of 132: not reconciled. **Rule**: keep inputs
  (populations, seeding shares, fleets with a job); the census is one generated file or a report.
- **Facts copied across docs**: the MC-07's 154 t / 669 m³ / 3.05 m/s² in six docs; the 120-day
  reserve, outlawry, the Spire of Heath, "SFO 19" each in two or three. Two docs are running logs
  (`registry-ssot-response.md` 1,187 lines, `registry-ssot-request.md` 934) repeating the topic
  docs and keeping superseded answers; the ships note (§1–14) restates figures that live in records.
  **Rule**: figures live in records and reports, rules in topic docs, history in the changelog; the
  logs get a pointer at the top and stop growing.

## 3. Contradictions (fix the facts)

- **The MC-07 flies on twelve outdated records** (torch drive, tank s0, life s1, fusion plant,
  hyperdrive, quads, lift, computers, comm, transponder, radar) with `life.s1` keeping 3 people for
  a 6-seat cabin; its fuel capacity is "sourced" from the game's tank; its main thrust is 6 × the
  outdated torch. `tank-s0` carries `identity.revision: outdated` beside a top-level `revision:
  status: review`.
- **`physical.volume` means two things**: the sphere's volume on tank-s0..s4 (0.52 × the box), the
  box everywhere else, including the new propellant tanks.
- **Two drive families both claim Discovery II** (torch 900 kN / 3.5 t at 10,000 km/s; engine.ft
  1 kN / 30 t at 347 km/s), described three ways. No hull uses `mount.engine-*` yet. The heat rule
  exists three times and disagrees: mounts.py sizes cooling at a fixed 1e-6, Budgets reads
  `heat_to_hull` with a 1e-6 fallback and counts only drive/lift/thrusters (an `engine` fit reads
  no jet heat), the engine records derive it from isotropic loss.
- **Fuels**: deuterium, d-t, d-he3, helium-3 and hydrogen overlap; `engine.ft` burns D-He3 but no
  D-He3 stock exists; methane is a bulk gas stock on market.fuel beside methalox; oxygen has no
  good or material (`need.air` takes the element); polyethylene is both a good and a material.
- **Outdated but flown**: 56 outdated equipment records and 4 of 5 outdated hulls are in game
  content and referenced by up to 15 records each (seeding fills the sky with them). "Outdated"
  needs two meanings: *still in service* and *retired*.
- **Stale docs**: `registry-integration.md` describes the game of 10-04 (mirror files that no
  longer exist, `config/dogma.ron`, LAND_SPEED); `tick-tree.md` §1 "Today" is pre-clock; the ships
  note says 40 mounts and 42 pieces (57, 151) and "not yet on main"; the README's layout shows two
  roots of seven and a `processes/` folder; `docs/standards.md` gives a second id form; report
  descriptions hard-code "not yet said", "30 to 37%", "nothing says how concrete is made", all now
  false; processes are gone but build.py, page.html, the process schema and `installation.processes`
  still carry them.
- **Smaller**: `org.hearth-line` named "Hoar Line"; `settlement.schema.yaml` titled "Body";
  `designer` uses a stale `^brand\.` pattern no key can match; `throat-coil` has no revision and no
  mount; `hours` carries `x-unit: s`; angles in rad against the stated degrees rule; `common` says
  shares carry no unit while nearly all carry `"1"`.

## 4. Naming (one word per concept)

- **People**: persons / seats / crew / people / count. **Period**: every / interval / cadence /
  cycle / cycle_time / period; `rate` is kg/s, W or rounds/s. **Maker**: maker / designer / owner
  (an org key in parcels, a session enum in revision) / holder. **Text**: description / about /
  purpose / note / who / what / means; `story` inside identity on bodies, top-level elsewhere.
- **Same name, different shape**: `holds` is a material key, a free string, newtons, kilograms or
  kelvin; `capacity` is kg, J, messages/s, W or an object; `life` is seconds, an object or biology;
  `fit`, `address`, `batch`, `details`, `extent`, `trade_bans` each have two shapes; `revision`
  means a design stage, a record's firmness, or free text.
- **Keys**: equipment follows five patterns (`equipment.tank.water.s1`, `equipment.tank.deuterium.s1`,
  `equipment.cargo.food-store.s1`, `equipment.engine.ft.s1`, `equipment.gate.throat-coil.s3`); 77 distinct key
  middles for 151 records; file names differ from keys for gear, ore bays, doors, lockers, 13
  makers, gates and all parcels; mill-stock files upper case, keys lower.
- **Rule**: `equipment.<slot>.<family>.<size>`; file name = key's last part; `persons`, `period`,
  `maker`, `about`; fields named by quantity (`capacity_mass`, `energy`, `load_limit`).

## 5. Page and reports

- 153 per-thing chain reports plus equipment-parts, mass and equipment walk the same bill: one
  Chains report with a picker. Hulls, budgets, volume, dimensions are four per-hull tables: one
  Hulls report with tabs. Stock / takers / goods / modules and the overview all ask "is it made and
  used": one Balance. Confidence / invented / review (10,538 rows, all gaps) say one thing.
  Report keys don't match titles (`modules` is "Balance", `stock` is "Materials").
- Mounts, needs, professions, buildings, markets, structures, clocks and sights have no page of
  their own; the page links no docs. Add a `docs:` field to each report and a page per kind.
- Generators copy each other (`crew.py`, `propulsion.py`: `q`, `edit`, `part`, `equipment`;
  `law()` ×2, `sha()` ×2, `r()` ×2); three use cwd-relative paths; three patch schemas by string
  replacement inside "idempotent" scripts; the fittings, sockets, ore-bay, members, nodes and panel
  generators are not in the repo. One `tools/standards/lib.py`; schema changes as commits.

## 6. Proposed order

1. **Facts first** (done 2026-10-07, changelog/2026-10-07-audit-step-1.md; the MC-07's fit stays outdated-in-service by design, its successor the ships session's): the MC-07 onto current equipment or "in service" marks; one volume
   rule; the heat rule in one function used by mounts, Budgets and the studio, counting `engine`;
   D-He3 stock; "outdated" split into in-service / retired; the stale docs and report texts; the
   process remnants removed; the small renames.
2. **Say it once** (done 2026-10-07, changelog/2026-10-07-audit-step-2.md; the census stays in the records until the engine reads one generated file): basis rules (`rule:`) for lives, cut loss, mounts margin, bulk
   density, plant weights; cite the five uncited sources; census as a report; freeze the two logs.
3. **Structure** (3a done 2026-10-07: one shape gate, dictionary enums, lib.py; 3b: the equipment naming rule and parts as a rule done the same day with the registry reader deriving parts, bulk stock kept by the user's rule; `allOf` is derivation in the generator and both validators, with `jet` as its first use; shared identity, position and staff definitions follow the same way): parts as a rule (−840
   files); bulk stock derived (−217); shared definitions and dictionary enums; one validator;
   the naming rule applied with a key-rename pass (keys are referenced; the rename is a script).
4. **Page** (a day): merged reports, pages per kind, docs linked, `lib.py`.

Each step leaves the validators at 0 and the game building; the integrator regenerates types
after step 3 and the ships session re-reads keys after the rename.
