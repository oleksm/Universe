# Sharper shadows close up; the planet as an area light

- **Shadows:** the near cascade tight round the eye (a 24th of the reach: ±104 m) and the map
  4096² (was ±312 m at 2048²): a ship close by gets about 5 cm a texel (was 30); 3×3 filtered
  samples (each 2×2) for soft edges, in the PBR and the mesh shaders. The ragged blotches on the
  test hull are gone.
- **Environment, first cut:** what a glossy surface reflects of the planet, treated as a light the
  size it looks (an area light) in the PBR shader: the reflection's lobe widens with roughness and
  catches the disc's share of itself, under Fresnel. (A captured, filtered environment cubemap
  comes later, with probes.)
