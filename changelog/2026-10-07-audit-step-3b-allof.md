# 2026-10-07: registry audit, step 3b (iii): `allOf` is derivation

- The generator (`crates/registry/build.rs`, `flattened`), validate.py and check.py read a schema's
  `allOf` the same way: the shapes it derives from are flattened into the object's own properties
  and required fields, so one definition serves many kinds without nesting the YAML and without
  moving a field (`it.thrust` reads as before). A shape that only tightens a nested property's
  `required` (physical_object's companions) merges into that property.
- First use: `jet` (thrust, exhaust, efficiency, burns, heat_to_hull), once, for drive, lift,
  thrusters and engine; the 140-word heat_to_hull text is written once. Shared identity blocks,
  positions and staff follow the same way.
