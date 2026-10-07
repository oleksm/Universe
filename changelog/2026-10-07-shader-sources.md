# Shader sources in one place (audit phase 4, #11)

- `engine::shaders`: every shader module as it's put together (its files in order), and the
  constants the Rust side and the shaders must agree on written once in Rust and prepended to
  every module as WGSL: the shadow texel (from `SHADOW_SIZE`), `GLOBE_SIZE`, `GEOMORPH_SPLIT` (the
  game's terrain LOD takes it from here), `MICRO_PERIOD`, `MAX_LIGHT`, `SPEC_MIPS`. The shaders'
  own copies (`1.0 / 4096.0`, `512.0`, `2.4`, `4096.0`, `4.0`, `7.0`) are gone. The three places
  that assembled modules by hand (renderer, models, cloud cache) and the six `include_wgsl!`s
  build from it.
- A test parses and validates every module with naga (0.01 s, no GPU): a file left out of a
  module, a name two files declare, a constant missing, fail it (checked: pbr without light.wgsl).
- Frames of the docked deck and of Heath's range look as they did. (The lab's own pairs, the
  cloud cache's `CC_N`/levels and the ground's `GM_PERIOD`, are to follow with the lab.)
