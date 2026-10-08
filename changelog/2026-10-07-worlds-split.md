# Worlds in four files (audit phase 5)

- `world/worlds.rs` (1,196 lines) is a directory: `worlds/mod.rs` (the store, packages checked by
  hash, the bake, clouds and air tables, directions), `survey.rs` (rock map, deposits, districts,
  bulk rock, basins, fields, coalfields), `heights.rs` (the bake's height tiles by level, read
  where wanted) and `releases.rs` (the store's index, a world's history). Re-exported as before,
  so `worlds::Heights`, `worlds::Survey` and the rest keep their paths. Heath's ground in a frame
  as before.
