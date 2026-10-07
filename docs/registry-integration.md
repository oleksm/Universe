# Integrating the game with Freefall Facts

*For an agent working on the game. Written 2026-10-04 from branch `fso`; brought up to date 2026-10-07
(the registry audit): the game now reads the registry directly, nothing is mirrored. The
short form, with what is waiting to be built, is the `freefall-integration` skill. The detail of
that work: `docs/registry-ssot-response.md` (keys, refs, units, merged schemas) and
`docs/registry-recipes.md` (how things are made).*

Freefall Facts is the registry the game is to be sourced from: YAML records under `standards/`,
built into a browsable page and into RON files the game loads. This says what the game can rely
on, what it reads today, where the registry's spec is ahead of the game, and how the two branches
are kept apart.

## The rules, in short

1. **The registry is the spec.** Where a record differs from what the game does today, the record
   is what is meant. The user decides when a spec is solid enough for the game to follow; until
   then, don't bend the registry to the game and don't rush the game to the registry.
2. **Never hand-edit a generated file.** `standards/index.html`, `changes.yaml`, `game-keys.yaml` and
   the engine's generated types are written by the build; change the YAML and run it.
3. **Branches.** Registry work is on `fso`, in its own worktree `~/git/universe-fso`. The registry
   session does not merge, push or commit to `main` unless the user asks for that merge. Merging
   `main` *into* `fso` to stay current is fine. If you need something from `fso`, ask the user.
4. **Platform, not outcomes.** Records say what things are and the most they can do (a line's
   maximum rate, a hold's rated load). What is actually produced, bought or flown is the game's
   economy to decide. Don't read a maximum as a target.
5. **Every number has a tier.** `sourced`, `derived` or `invented`, and guesses carry
   `review: true`. Treat an invented figure as provisional: build the mechanism, expect the value
   to move.

## Building

    python3 tools/standards/build.py            # validates, writes standards/index.html and the RON files
    python3 tools/standards/celestial_export.py # writes what the seed makes into the celestial records

The build exits non-zero and lists problems if a record is wrong; it then does **not** update the
game's files. Its second line lists gaps per report: gaps are open work, not errors.

Open `standards/index.html` in a browser to read the registry. The Reports section is the quickest
way to see what joins up and what doesn't.

## What the game loads

One way, from `standards/` and nothing else: `crates/registry/build.rs` generates a Rust type from
every schema (`standards/**/schema/*.yaml`, `standards/*.schema.yaml`, with `allOf` shapes
flattened), and `universe_registry::Registry::read` reads every record into them at the world
crate's build (`crates/world/build.rs`); a record that does not fit fails the build. Equipment,
hulls, parts (derived from their equipment's `built_of.list`), stock, goods, materials, needs,
clocks, marks, settlements, bodies: all come this way, by key. `crates/physics/build.rs` generates
the Dogma constants (`laws.rs`) from `standards/Dogma`. The seed is `seeding.galaxy`, required.

Beside the records the game keeps only its own hand-kept files in `content/base`: `aliases.ron`
(old key → new key, so saves resolve), `prices.ron` (what the game asks for each thing the
registry describes; the registry holds no prices), `shapes.ron`, `sheet.ron`. The eight RON files
the build once generated (bodies, brands, celestial, galaxy, industry, rock_classes, settlements,
standards) are gone (2026-10-07): nothing read them. Units everywhere are SI; a record's unit is its
schema's `x-unit`.

There are no mirrors: the registry copies nothing from the game and the game keeps no second copy
of a record. Where the game's code still holds a value a record also says, the clocks and Dogma are
the pattern: read it from the record.

## Where the spec is ahead of the game

These are registry facts the game does not yet follow. None is a bug; each is work to schedule
with the user. The page's reports say which: a kind marked `x-in-game: "not made"` in its schema
(fittings, crewing, the engine family, marks' stamping) has a handler stub and no behaviour yet.

**The MC-07** (`standards/SFO/metadata/hulls/mc-07.yaml`): the game builds it from its parts, so
its mass is the registry's. Its fit is the game's early list, twelve records marked `outdated`
(in service, not to be balanced against): its life support keeps three for six seats, its torch
drive's jet no pad stands (SFO 22, 23). Its `open_questions` list is current. The ships session
designs its successor from the current families.

**Breaking.** SFO 15 (Shock) says how the registry carries it: a hull's landing jolts, a part's and
a stock item's `physical.shock_limit` (g), a material's `mechanical.fracture_toughness`. All shock
limits are guesses marked to review. The game has no breaking mechanics yet.

**Only in the registry:** rigs (`kind: rig`, movable industry with no zoning); the gates as
settlements; equipment kinds not made (see above); the yard, mill, smelter and orbital chains as
lines with maxima; the Engine root's clocks beyond the ones bound so far.

**Marked outdated** (`identity.revision: outdated`): the five stock hulls and the game's early
equipment: in service, early guesses, not to be balanced against. `retired` is what is no longer
built or flown at all.

**How things are made** (`docs/registry-recipes.md`): an industrial module lists its recipes; a
setup (a module set to one recipe) is game state; a works is one pool of stock kept in its storing
modules; between works the market is the join. Processes are retired (SFO 7 superseded).

## Keys and names

- Every record has one key, `<kind>.<name>`, the kind being its schema's: `hull.mc-07`,
  `part.mc07-23-001`, `stock.al6061-pl-5`, `equipment.drive.torch.s1`, `gate.ring.i`,
  `module.arc-furnace`, `good.stony-ore`, `org.hadley`, `settlement.treistun.port-trethi`,
  `body.treistun.treistun-f`, `law.tube-hold`, `clock.realtime`. A record names another by its
  key; the property is marked `x-ref` in its schema, and the registry test checks every reference.
- The generated RON still carries the game's old names for brands, bodies and systems
  (`standards/game-keys.yaml`): the build turns them as it writes. That stops when the game loads
  those by key too.
- A rock class's and a law's `identity.label` is the game's name for it (`S-TYPE STONY`,
  `SPEED_OF_LIGHT`).
- Page keys, for linking into `standards/index.html#<key>`: `hull:`, `part:`, `stock:`, `mod:`,
  `good:`, `eq:`, `gate:`, `mk:`, `bd:`, `fc:`, `pc:`, `cs:`, `cb:`, `cf:`, `cr:`, `dg`, `dl:`,
  `rep:`.

## Asking for a registry change

Say what the game needs to read and in what shape. The usual path:

1. A schema property (`standards/<root>/schema/*.schema.yaml`), with its unit in the description.
2. The records.
3. The build writes it into a RON file, in SI.
4. A struct in the game with `serde::Deserialize`, loaded in `crates/world/src/content.rs`.
5. If the game's own code also holds the value, a test that holds the two together.

A property that doesn't apply to a record is left out, not written empty.

## When branches meet

- `standards/index.html` is generated and conflicts on almost every merge. Take either side, run
  the build, commit the result.
- The generated RON files likewise: never resolve by hand, rebuild.
- `tools/standards/build.py` and `page.html` are large single files. If you add a record kind to
  the registry from the game side, tell the registry session; the same touch-points are needed in
  both files.

## Where to look

| For | Read |
|---|---|
| How the registry is organised and its rules | the `freefall-facts` skill, `standards/README.md` |
| What joins up and what doesn't | Reports, in `standards/index.html` |
| Cited sources for sourced figures | `standards/sources/*.json` |
| What changed and when | `changelog/2026-10-0*.md` |
