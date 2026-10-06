# Worlds from the planet simulation: the store and the package readers

- **`world::worlds`** (`docs/survey-contract.md` §2a, §3), all by reference:
  - `root()`: where the registry's files lie (`UNIVERSE_ROOT`, else the source tree).
  - `store()`: the worlds store (`UNIVERSE_WORLDS`, else `~/git/planet-sim/out` where it exists).
  - Every package's `manifest.json` is checked against the hash on the body's record, and every
    file read against its manifest's hash; a mismatch is refused (SHA-256, `sha2`).
  - `World::load(body)`: the survey (the rock map, 3,756 deposits, districts, what each rock unit
    yields as bulk rock) and the energy package (282 basins, oil and gas fields, 89 coalfields), in
    SI (barrels and billions of cubic feet to m³, megatonnes to kg).
  - `Bake::open(body)`: the surface bake in the store (Harvest's 0.7 GB, 1,328 files), each file
    fetched by name and checked.
- Nothing reads them in play yet: next, Harvest's heights as its ground, then the map layers and
  mines on deposits.
- Found: Harvest's oil fields add up to 1.7% more oil in place than its basins' total (1.207e12
  against 1.187e12 m³, the figure on the record). For the lab.
- Test: `harvest_reads_from_its_packages` (0.2 s; reads the bake too where a store is present).
