# Registry validation: trim and fit (scheduled)

*Registry session, 2026-10-05. Asked for by the user; begun 2026-10-07 as step 4 of the registry audit (docs/registry-audit.md). It follows the ship
equipment work in hand.*

## The aim (the user's)

- **A walker of about ten lines:** this is the folder; open every record under it; find its schema;
  run a plain schema validator from a standard library. Nothing else in the script.
- **The rules live in the schemas,** not in the script.
- **No record without a schema, ever.**
- **A record knows it is a physical thing.** A company is not; a bundle of 10 mm bar a metre long
  is. Every physical thing has its standard physical figures, and each is greater than nothing.

## Where it stands today

- `tools/standards/validate.py` is about 300 lines: a table from a file's place to its schema, a
  hand-written validator for a subset of JSON Schema (no `allOf`, no `exclusiveMinimum`), and rules
  that are not about shape (keys, references, units).
- 1,283 of 1,286 records already name their schema on their first line
  (`# yaml-language-server: $schema=...`).
- `common.schema.yaml` has one `physical` definition every physical kind points to. Nothing in it
  is required and nothing must be above nothing. 26 of 35 stock items and 29 of 163 goods have no
  physical figures at all.

## The steps

1. **Each record names its schema; the walker follows it.** Fix the three records with no schema
   line. A record with none fails. The place-to-schema table goes.
2. **A standard validator library** (`jsonschema`) in place of the hand-written one.
3. **Rules into the schemas:** a key's form as a `pattern` on each kind's key; units stay as
   `x-unit`.
4. **`physical_object` in `common.schema.yaml`:** a base that requires a `physical` group, with its
   magnitudes (mass, length, width, height, volume, density, radius) above nothing. Each kind that
   exists in the world derives from it (`allOf`) and adds the figures it must have: a part its mass
   and size, a bulk good its density, a body its mass and radius, stock its gauge. An organisation
   does not derive from it.
5. **Fill what then fails:** the stock and goods with no physical figures, as guesses marked to
   review where nothing better is known.
6. **Run it as a test,** not only in the build.

## What stays outside a schema

One rule: a reference points at a record that exists. A schema checks one file and cannot look at
another. It stays as a second, small pass with no knowledge of any kind.

## What it needs from the engine's side

- The type generator (`crates/registry/build.rs`) must learn `allOf`.
- A `required` figure becomes a plain field in the generated type, so tightening a kind the engine
  reads lands with the engine's code in one merge.

## To decide when it starts

- The per-kind sets of required figures: a table for the user to correct.

## Done (2026-10-05)

| Step | State |
|---|---|
| 1. Each record names its schema | Done. Every record's first line names it; the two root descriptors gained a schema (`root.schema.yaml`). A record that names none fails. The table from place to schema is gone from `validate.py` |
| 2. A standard library | Done, beside the old one. `tools/standards/check.py` is the walk with `jsonschema` (35 lines, 3 seconds); it passes the whole registry. It needs `pip install -r tools/standards/requirements.txt`, which this machine's own Python lacks, so the build still runs `validate.py`, taught `allOf` and `exclusiveMinimum` so the two agree |
| 3. Rules into schemas | Partly. A key's form by kind is still checked in `validate.py` (a `pattern` beside a `$ref` needs `allOf` on a string, which the engine's generator has not been tried on) |
| 4. `physical_object` | Done. Kinds derive from it with `allOf`; the engine's generator ignores `allOf`, so the game's types did not change and nothing on its side had to move |
| 5. Fill what fails | Done: 48 goods' bulk densities, 5 materials' densities, the throat coil's volume. Guesses, marked |
| 6. As a test | Not yet: `check.py` can be one once the library is installed where tests run |

**What each kind must have** (to be corrected by the user):

| Kind | Must have, above nothing |
|---|---|
| Part | a `physical` group (a part made of parts takes its weight from them) |
| Equipment | mass, volume |
| Module | length, width (a field has no height) |
| Hull | volume (its mass is its parts') |
| Building | mass, length, width, height |
| Good | bulk density |
| Material | density |
| Body | mass, radius |
| Rock class | density as rubble and as one piece |
| Stock, gate, structure, element | nothing yet: stock's gauge depends on its form |
| Organisation, law, need, trade, zone, standard | not physical things: they do not derive from it |

Mass, length, width, height, volume and bulk density must be above nothing wherever they appear.

## Dimensions are required (the user, 2026-10-05)

"Make dimensions to physical items required." So a length, a width and a height, each above
nothing, are now required of:

| Kind | Must have |
|---|---|
| Equipment | mass, volume, length, width, height |
| Part | length, width, height (its mass too, unless it is made of parts) |
| Hull | volume, length, width, height |
| Building | mass, length, width, height (as before) |
| Module | length, width (a field has no height) |

Goods, materials and stock are bulk: they have a density or a gauge, not a size.

**599 records had none, and now carry a stand-in:** a cube of the volume on record (equipment), of
the room its own material takes (a part: its weight over its material's density), or a box by the
radius the game flies it at (the five old hulls). Each says so in its basis ("Not worked out..."),
marked to review. A tank is the one exception: a ball of its volume, which is worked out.

A new report on the page, **Dimensions**, counts them: 1 of 6 hulls, 5 of 57 pieces of equipment
and 58 of 595 parts are sized; the rest are owed a real size. The gate now refuses a physical thing
with no size; the report says which sizes are real.
