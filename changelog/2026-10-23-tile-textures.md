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

## The base tiles: the package's base cube

Then the base, from the cube the planets session shipped (`base_cube.json`, `base_color.u8`, `base_height.f32`: six
faces of 1,024, as wgpu samples a cube):

- `engine::PlanetCube` (colour, and lit normal in the planet's frame) set on `Frame::planet_cube`; the world-maps group
  binds it (19 to 21; a blank one otherwise), made whole on the GPU once with its mips (~67 MB here, counted with
  the world maps), bound again when another comes.
- Base tile corners carry their direction on it in `data` (w: `PLANET_CUBE_DATA`); `shade` samples colour, and under
  the view shade the normal, by that direction. The normals are the planet's frame: right for a planet drawn
  unturned, as the planet LOD app's is.
- `terrain::Ground` loads the cube when the package ships one (`planet_cube`: colour, normals from the light-
  exaggerated heights) and hands it to each frame; the base tiles' vertex colours stay as the fallback.

forge_e124s43-mesh from 10,000 km: the vertices' blotches (angular coasts, patchy shelves) give way to the cube's
smooth coasts (`out/review/tile-textures/far_cmp.png`). A faint honeycomb of dots on some land is in both, before and
after: not this.

## Far relief: coarse tiles take the cube's normals

Planets `20261023T134425-planets-94ba`: far relief had faded (their specks 42 against the reference's 1,034). All the
land at 10,000 km is the pyramid's coarse levels, whose texels (tens of km) were finer than nothing but coarser than
the cube's (~9.8 km): their normal textures, mip-averaged too, flattened it. Now a tile's texel span (`TileTexture::
texel_m`, in its draw: `GpuDraw::texel_m`, `data.z`) is compared with the cube's, and under the view shade a coarser
tile takes the cube's normal by the pixel's direction from the world's centre (its frame unturned, as the planet LOD
app's). The cube's normals carry the package's ×13 (land) and ×3 (sea).

forge_e124s43-mesh from 10,000 km: detail (a 2 px high-pass's spread) 1.52 → 6.25 (`far_cmp2.png`).

The honeycomb of darker dots on some pyramid land: the view shade's sea rule (sea floor where a pixel lies under the
sea level's radius) caught low coastal plains near or under 0 m in patches of the tile meshes' triangles, pulling
their shade toward 1. A pyramid tile is ground (its mask keeps no sea): the rule now only for the base. Found by
splitting colour from shade (colour clean, the dots all in the shade). Left: much fainter, lighter hexagonal
specks in the sunlit land.

## The light exaggeration tapered by a pixel's ground

Planets `20261023T151411-planets-fb4a`: ×13 is right for the far view's ~19.5 km pixels but blackens slopes close in (a
4 km range at 60 km). The exaggeration is baked into the normals (tile textures, the cube, the vertices), so the view
shade takes it back: a normal's slope part over its up part is the ground's gradient times what was given (land over
the sea level, sea under: `ViewShade::land_light`, `sea_light`, the package's), scaled to what's wanted at the pixel's
ground: the whole of it at `TAPER_FAR` (19.5 km) and over, ×1 at `TAPER_NEAR` (100 m) and under, log-linear between.
`Ground::light_exaggeration` gives the app the package's.

forge_e124s43-mesh: their 60 km oblique view's near-black pixels 1.7 % → 0 (`obl_cmp.png`); from 10,000 km (~9 km
pixels here, under 19.5 km) the far detail 6.35 → 4.9.
