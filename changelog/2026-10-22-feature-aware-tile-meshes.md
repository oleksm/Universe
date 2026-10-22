# Feature-aware tile meshes (planets' `tile_mesh`, pyramid format revision 22 Oct)

A land pyramid level may carry `mesh` and its tiles `mesh_off`, `mesh_nv`, `mesh_nt`: the tile is then drawn on
its stored vertices and triangles; without them, on its full grid as before. Selection is unchanged.

- `world::land::TileMesh`: a tile's mesh read from its level's file and checked (vertices inside the tile,
  triangles' indices within them), with 8-cell buckets for finding the triangle under a point.
- `world::land::tile_height`: one height rule for a tile, on its triangles or its grid; the physics pyramid
  (`LandHeights`) and the renderer's `ground_at` both use it, so contact stays on the ground drawn.
- `build_model`: vertices at the listed samples (normals, colours and morph from the rasters as for the grid),
  the stored triangles (a coarse level's hole guard kept), skirts on every edge one triangle alone has (the grid's
  are the same edges as before). `drawn_ground_at_world` walks the drawn triangles near the point.

forge_e104-mesh against forge_e104-fine, same views (planet LOD app): 16.6 M triangles in place of 37.5 M, tile
memory 1.96 GB in place of 3.04, same 120 fps cap, no cracks, the Shasta disc's rim the same. Test:
`pyramid_tile_mesh_drawn_and_touched_on_its_triangles`.
