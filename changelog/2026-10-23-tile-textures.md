# Tile textures: per-pixel colour and lit normals for the land pyramid's tiles

Planets `20261023T131938-planets-57ec` (forge_e124s43-mesh: colour blotchy where vertices are sparse): colour and
shading no longer follow vertex density.

- `engine::pool`: `PoolModel::uv` (a texel per vertex, a word after its colour, `F_UV`) and `PoolModel::texture`
  (`TileTexture`: colour sRGB and lit normal, up to `TILE_TEX` = 256 a side, shared by a tile's versions). The pool
  keeps them in two texture arrays (layers: 512, or the device's most: 256 here) with mips, each texture a layer,
  the least lately drawn let go; 8 MiB of new ones a frame. `GpuDraw` carries the layer (48 bytes, as
  `PoolDraw`); the arrays count in the pool's GPU memory.
- `scene.wgsl`: `fs_pool` (the pool's pipeline only; `fs_mesh` has no group 3) samples a resident tile's colour in
  place of its vertices', and under the view shade its normal, kept in each texel's own frame (along its columns,
  up × that, up from the world) and turned back by the screen's steps over the tile; `shade` is the shared body.
  The layer rides in `data.w` (rounded: interpolated, a constant came out a hair under itself and picked the wrong
  layer, a speckle).
- `terrain::pyramid::tile_texture`: each tile's samples' colours, and their normals from the light-exaggerated
  heights.

forge_e124s43-mesh over its pyramid (0.94° N 13.16° E, nadir, the view shade): at 60 km the tile meshes' star-shaped
vertex shading gives way to the ground's own relief per texel; at 300 km about the same, smoother
(`out/review/tile-textures/`). Base tiles next, from the package's new `base_color`/`base_height` cube. The
workspace's library tests pass.
