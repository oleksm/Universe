# A world's full-resolution maps, and the slot for the ground's material

- **World maps** (`engine::worldmaps`, `docs/planet-studio-plan.md` step 2): a world's own maps at
  full resolution (equirectangular): its colour, its plants and soil, its normals, its climate, its
  rock map. Encoded where they're made (BC1 with mips for colour, ground and normals; the rock map
  one byte a texel, read exactly), on a thread of their own (Harvest's 8192 × 4096 maps in 2.5 s),
  then bound whole as group 2 of the mesh shaders for the world nearest the eye (within six radii;
  `Frame::world_maps`; the globe layer they're for in `Globals::look2.w`).
- **Drawn:** the world's colour at full resolution (5 km a texel, four times the globe map's), and
  close up (a pixel under 2.5 km) over land, the ground's material: `ground_material(GroundIn)` in
  `shaders/ground_material.wgsl`, the lab's to write (its signature as the lab proposed: the maps
  sampled at the pixel, the climate decoded, the rock unit, height, slope, a ground-steady position
  and the pixel's size). This stand-in returns the plants and soil.
- `MeshOut::up`: straight up from the world, as drawn (the slope is reckoned from it).
- Bake images read whole (`worlds::Heights::image`, `Terrain::bake_image`: JPEG or PNG to RGBA8).
- Dev: the low-flight scenario logs where the sun stands overhead.
