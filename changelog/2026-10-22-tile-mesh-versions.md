# Land pyramid: nested tile mesh versions (pyramid-format revision "nested tile meshes", tile_mesh 2)

The owner rejects the selection density cap (it shortened the sight): `UNIVERSE_TERRAIN_TRI_PX` now defaults to 0
(off). Instead each meshed tile carries `mesh_lod`, coarser nested meshes at 2x-32x the allowed error, and the
selection stays as it was.

- `Tile::mesh_lod` (`MeshLod`, kept `Copy`) and `Tile::meshes()`; `TileMesh::read_at` reads any version.
- A tile is built in all its versions from one read of its samples (`samples`, `build_version`): each version's
  vertices morph to the next coarser version's surface (nested: shared vertices stay put), the coarsest to the
  parent's as before.
- `Source::version`: per drawn tile, the coarsest version whose triangles average no more than
  `UNIVERSE_TERRAIN_VERSION_PX` (default 2; 0: always the finest) pixels on the render surface (foreshortened
  footprint over each version's triangle count), continuous between versions by their counts (log), and never
  finer than its morph to its parent asks (it steps through its versions before meeting its parent). Only the
  version drawn is uploaded; one not up yet is stood in for by the nearest that is, unmorphed.
- Drawn-ground queries walk the version drawn.

forge_e104-mesh4, -33.75° 97.4°, versions at a 1080-high surface (0.474 on the test window): 24.8 km 9.6 M
triangles → 3.4 M, 2 km 8.4 M → 3.4 M, the sight and ridges kept (14 % of pixels change, from 42 % under the
cap); a 24-frame pan steady (no pops). The tile cache holds every version: ~4.0 GB here against ~2.9-4.0 for the
finest alone (within its 8 GB). Mesh3 (no versions): unchanged. Tests: the versions in
`pyramid_tile_mesh_drawn_and_touched_on_its_triangles`.

Later the same night: `UNIVERSE_TERRAIN_VERSION_PX` defaults to 0 (always the finest version). The owner finds the
versions blurry: colour and shading are per vertex, and big triangles spread them. Off, a tile also no longer
steps through its versions before meeting its parent: the finest, morphing to its parent, as before versions
(mesh4, 24.8 km: 12.7 M triangles; the "all finest" figure above, 9.6 M, still stepped). On again once tiles carry
their own colour and normal textures.
