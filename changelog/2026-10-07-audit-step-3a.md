# 2026-10-07: registry audit, step 3a (structure: the tool-only part)

- One gate for a record's shape: validate.py (every schema says `additionalProperties: false`);
  the build's hand lists of allowed fields for facilities, pipeline parts and administrations are
  gone, and it adds only what crosses records.
- Shared words into the dictionary: `zone_use` (was inline twice), `joining` (twice),
  `reactivity` (three times), `magnetic_ordering`, `level` (flammability, toxicity). Records
  unchanged.
- `tools/standards/lib.py` holds `q`, `edit`, `sha`, the heat rule; the crewing and propulsion
  generators import them.
- Waiting on the registry crate (proposed to the integrator): parts derived from an equipment's
  part list (−840 files), bulk stock derived from goods (−217), `allOf` flattening so shared field
  groups (the jet figures, identity, positions, staff) become one definition, and the key rename
  pass with aliases. The records and the Python side switch together with the reader.
