# Polished metal reflects its surroundings (and only metal's look changes)

- Chrome and polished metal (the MC-07's claws, cargo rams, gear struts and pistons) came out
  black: a fully metallic surface shows only what it reflects, and there was nothing to
  reflect but the sun's highlight. In Blender's viewport the studio light reflects in them,
  so they looked white.
- The environment the eye is in is drawn into a cube map each frame on the GPU (`engine::env`,
  `shaders/env.wgsl`): the dark sky's glow and the world nearest, lit by the sun on its day
  side in its colour (taken in as the eye takes light, as the ground is drawn); in the studio,
  its soft boxes. Eight mip levels hold it as surfaces of rising roughness mirror it (GGX,
  importance-sampled).
- Models use it **for reflections only** (split sum, Karis's fit of the reflectance), in place
  of the old planet glint and the metal's ambient term: metal shows the world below, the sky
  above and, in the studio, its light boxes. The lighting of everything else is as it was.
