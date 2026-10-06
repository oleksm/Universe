# The ground's runtime detail: its slot (layer C groundwork)

- `world::detail`: the pure function the lab's generator will be (`offset(&Site)` over the ~150 m
  ground, Rust f64 the truth): a place in metres within its ~150 m tile, the tile's spacing, its
  heights and `fd150` fields by sample with a two-sample halo into the neighbouring tiles, the rock
  unit (the 5 km rock map), the world's seed (from its id), and the band `cell` (what's finer than
  about two cells left out). A stand-in adding nothing until the generator lands (`ACTIVE` false).
- Its WGSL twin (`engine/src/shaders/detail.wgsl`, the tile's samples supplied by the module
  that includes it) and the test that holds the two together within 5 cm (`AGREE_M`) on a
  synthetic tile (`engine/tests/detail_agree.rs`, 0.2 s; skipped without a GPU). Checked to fail
  when the twin is a metre off.
- Wired: the ground's height adds it (`Heights::detail_at`, tiles and fields through the same cache
  and budget), the physics at a 0.5 m band, each ground patch at its own cells' size; once `ACTIVE`,
  the patches go four levels finer (some 1.2 m cells), within a few kilometres of the eye. The
  halo's sampler is exercised once the generator reads it; past a cube face's edge it takes the
  tile's own nearest sample.
- The shape agreed with the lab (its answers: band limit, tile-relative coordinates for f32, a
  two-cell halo, boulders on an integer-hashed 4 m lattice).
- For the lab's generator (its asks): `Site::origin`, the tile's corner on its cube face in whole
  metres, `at` measured from it (the noise's lattices anchored on the face: no seams at tile
  edges; the twin the same in i32 and f32); `height` the ground's whole height (5 km, 600 m and
  ~150 m) at the tile's samples. `worlds::cube_dir`, `locate`'s inverse, with a test there and back.
- The rock unit under a place is the rock map's value over 8 (the map holds a unit's number
  times 8, as the scene's shader reads it): it was the raw byte, which the lab's layering table
  by unit would have missed.
