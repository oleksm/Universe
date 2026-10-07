# 2026-10-07: registry audit, shared shapes

- `common.schema.yaml` defines `product_identity` (key, traded_as, name, maker, revision,
  description), `surface_position` (latitude, longitude with their bounds) and `staff`
  (profession, count) once. Equipment, hull, structure and gate identities derive from the first
  (allOf) and add their own words (slot, class, kind); sights and settlements share one position
  type; buildings and modules one staff type. Field names unchanged; the generators (build.rs,
  check.py) rebase a shape's own references when they inline it from another file.
