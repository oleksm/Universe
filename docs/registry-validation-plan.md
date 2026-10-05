# Registry validation: trim and fit (scheduled)

*Registry session, 2026-10-05. Asked for by the user; scheduled, not started. It follows the ship
equipment work in hand.*

## The aim (the user's)

- **A walker of about ten lines:** this is the folder; open every record under it; find its schema;
  run a plain schema validator from a standard library. Nothing else in the script.
- **The rules live in the schemas,** not in the script.
- **No record without a schema, ever.**
- **A record knows it is a physical thing.** A company is not; a bundle of 10 mm bar a metre long
  is. Every physical thing has its standard physical figures, and each is greater than nothing.

## Where it stands today

- `tools/standards/validate.py` is 328 lines: a table from a file's place to its schema, a
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
