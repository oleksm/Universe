# The dashes on the ground close up, gone

- A grid of dark dashes lay over the ground close up (the lab found it, at 36.3 N 35.2 E from 2 to 3
  km). The slope shading took the globe map's own slopes on the near-ground patches too: its 512 a
  face texels (20 km on Harvest) are filtered to 1/256 of a texel, so its height stepped every 80
  m or so, and each step shaded as a line. A patch stands on the true heights, finer than that map:
  the map's slopes now shade a globe only (`scene.wgsl`, `lift`).
- On a patch a pixel's size comes from the eye's distance, the angle a pixel spans (`Globals::view`)
  and how obliquely the ground is seen, smooth from triangle to triangle (the screen's derivatives
  are one value a triangle); the globe maps' level there from it.
- The ground material's slope is the mesh's (`in.normal`), not the shaded normal's (the lab's).
