# The air's tables: a brighter sky

- **The lab's tables** (Hillaire 2020; planet-sim `air_luts.py`): a world's bake carries the sun's
  transmittance through its air (256 × 64) and the light of scattering's higher orders (32 × 32),
  described in `air_luts.json` (`worlds::Heights::air_luts`; the higher orders scaled by its
  `scale` on reading). Bound with the world's maps as half floats (bindings 9 and 10, a clamping
  sampler at 11; filterable without a GPU feature); `Globals::view.z` says they're there. The
  ground's air, the sky and the sea's reflection then use the lab's `_lut` entries
  (`world_air_ground`, `world_air_sky`): the sky about 1.5 to 2 times brighter, toward the horizon
  most, and cheaper (no march toward the sun at each step). Worlds without them as before.
- Harvest's bake installed (v1) has no tables yet; the lab's v3 has them (and Earth's mean haze).
  `UNIVERSE_AIR_LUTS=<folder>` uses a folder's tables meanwhile, for trying.
- A test window on workspace 8, hidden, is throttled by the compositor: read the GPU's time (F3),
  not frames a second, there.
