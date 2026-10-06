# Mountains cast shadows

- A fourth shadow cascade, the ground's: a box 60 km across the light and 160 km along it round the
  eye (15 m texels), drawn from the ground's patches alone (`Frame::ground_shadow`, in place of
  `no_shadow` for them), with no depth bias (a slope's bias at a low sun is hundreds of metres, more
  than the hills); its receivers step 2.5 texels off along their normals. Every lit mesh multiplies
  it in (`sunlit_ground`, scene and PBR shaders): the ground, and ships and stations on it, lie in a
  mountain's shadow. Faded out toward the box's edge.
- Seen on Heath's highest peak (8,920 m) with the sun 16° up: shadows kilometres long west of it,
  where a ray march over the baked heights puts them; none without the cascade. Heath's 600 m
  ground is mostly gentler than the sun, so most places show none yet (the lab's steeper relief and
  finer levels will show more). GPU cost too small to measure on the test workspace.
- `UNIVERSE_SHADOW_DEBUG` tints the shadowed red on baked ground too (it read the vertex light,
  which a baked world's ground doesn't use).
- Dev: `UNIVERSE_HOURS` starts a scenario that many hours on (the sun elsewhere). Heath turns 17.8°
  of longitude an hour.
