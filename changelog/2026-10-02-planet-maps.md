# Planets surfaced per pixel

A globe's colour came from its mesh corners (about 250 km apart on a big world), blended between:
fine from afar, a blurry gradient up close. Now:

- **Surface maps:** each terrain world's terrain is sampled once onto a cube (512² a face, all
  cores, `terrain_view::globe_map`): height in relief units and crater-ness. It's the same height
  function the physics lands on. On the GPU it's an 8-layer cube array with mips
  (`GlobeMap`, `Frame::with_globe`); the least recently drawn layer gives way.
- **Colour per pixel** in the mesh shader (`globe_color`):
  - seas darker the deeper, with surf at the shore;
  - **coasts crisp at any zoom** (the height's own zero contour);
  - beaches; land in crisp-edged patches by moisture and height (forest, grass, dry ground,
    rock, snow), deserts toward the tropics;
  - other worlds in their colour, with outcrops and dark craters; ice at the poles.
- **Fine detail** (`globe_detail`): octaves of noise down to about the pixel, each fading in as it
  grows (no shimmer). It feeds the colour, the coast's raggedness, and a height detail with the same
  slope at every scale, so slopes shade at any zoom. This is drawing only; the physics is unchanged.
- **Globes lit per pixel:** slopes (surface-gradient normals) and a smooth terminator.
- `Terrain::height_and_crater`.

Also: **the canopy set lower,** half as tall and sunk into the plating; its ridge just clears the hull.
