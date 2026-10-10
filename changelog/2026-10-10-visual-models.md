# Models for the registry's things (`visual`, freefall-model/1)

- `world::assets`: a record of a kind that takes a model (equipment, modules, buildings, structures, gate rings,
  hulls) may carry `visual`; its package is read from the assets store (`UNIVERSE_ASSETS`, else
  `~/git/freefall-assets`), the manifest checked against the record's hash and `model.glb` against the manifest,
  as a world's ground is (docs/asset-contract.md). A package made on the spot is held by a test, tampering refused.
- The game draws a thing's model where it stands, loaded the first time it is seen (`visuals.rs`): a settlement's
  module or building standing on the ground at its block's centre, turned by its heading; a rig at its centre; every
  gate as the ring design's model. A thing without a model, or whose package fails a check (said once in the log),
  keeps its box.
- Not yet: equipment at its mounts and stations as structures (the game does not place them by record yet); no
  record has a `visual` yet, so the drawing has not been seen with a real model.
