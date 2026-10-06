# Harvest in its own colours

- **A world's own colour** (`docs/planet-studio-plan.md`, step 2): a globe map may carry the
  world's colours texel by texel (`GlobeMap::colors`, sRGB), uploaded beside its heights into a
  second cube array (`renderer::Globes::colors`, binding 6; alpha 0 where a world has none, the
  palette as before). The globe and the near ground draw them, with the fine detail's light and
  shade over them.
- Harvest's from its bake's `globe_color.jpg` (8192 × 4096, the planet simulation's true colour:
  deserts, forests, tundra, the shelves' pale blue, snow), read once into the globe map's 512 a
  face (about 20 km a texel) and let go (`worlds::Heights::colour`, `Equirect`; `zune-jpeg`).
- Up close the colour is that coarse (a valley one brown): the near ground's finer colour, the
  sea's glint and the air's haze come next (the plan's steps 2 and 3).
- Dev: the low-flight scenario takes `UNIVERSE_LAT`, `UNIVERSE_LON` (over that place, looking
  north) and `UNIVERSE_DOWN` (how far down it looks).
