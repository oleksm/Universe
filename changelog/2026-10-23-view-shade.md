# View shade: faces shaded on the screen's colours, the planet lab's relief look

Planets `20261023T123028-planets-5f92` (the owner: "closer but detalisation is lost"): Lambert in linear light, then
gamma-encoded, flattens relief shading against the lab's views, which shade the displayed colours.

- `Frame::view_shade` (`ViewShade`: ambient, diffuse, sea, the world's centre and sea level): with it, lit
  non-globe faces are shaded as `sRGB(colour) × (ambient + diffuse max(0, n·l))`, under the sea level its departure
  from 1 times `sea`, and written so the tone-map-off screen (light × 0.9, gamma 2.2) shows exactly that. Two
  `Globals` fields at the end (`view_shade`, `view_world`); `scene.wgsl` `linear_to_srgb`, `EXPOSURE_RAW` (= the
  blit's).
- The planet LOD app sets it with its default look (0.3, 0.95, 0.6, its world): the lab's formula. `look=film`
  goes back to the game's light and curve.
- Normals as before: per triangle (the base tiles' lit by `light_exaggeration`), finer than a pixel at the far view.

lod_e124s43_v3 from 22,635 km beside the expected picture: the relief texture and colours match by eye. The
workspace's library tests pass.
