# The lab's shader leftovers, done here (audit phase 4, #10/#11)

The lab session has moved to another project and handed these back:

- Dead code out: the uncached `clouds_over` and `clouds_shadow` (the cloud cache replaced them),
  `sea_hash`/`sea_noise` and `gm_noise`, none called. Every remaining function is called or an
  entry point.
- Paired constants from Rust (the shared block): `CC_N`/`CC_LEVELS` from the cloud cache's `N` and
  levels; `CLOUD_WRAP_S` (`shaders::CLOUD_WRAP_S`, the game wraps the clouds' time by it); the
  ground material's `GM_PERIOD` is `MICRO_PERIOD`, which it was a copy of.
- No shared `noise.wgsl`: the four hashes (fine grain, clouds, ground material, detail) are
  different formulas making different patterns; one hash would change how each looks.
- Frames of Heath's range and the deck look as before; the shader test validates every module.
