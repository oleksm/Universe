# Planet LOD as a mini app

*2026-10-14. The owner: migrate the browser Planet LOD viewer (planet-trees-pgs2 `tools/planet_lod.py`, page at
`~/planet-data/views/earth_s15-pattern/`) to a mini app on the engine. planets owns the migration (branch
`engine-claude-planetlod`); engine owns any general API it needs.*

## Why

The browser page walks every pattern vertex every frame (1.35M on the 3x3 grid: 59 ms in JS, 16 ms compiled),
rebuilds the whole index when anything changes (46 ms; 18 ms compiled) and uploads every position and edge colour
(44 MB) a frame. The engine's terrain already does the right thing: the CPU walks patches, never vertices, and the
vertex shader morphs each vertex toward its parent's shape by distance (`terrain_lod.rs`, `scene.wgsl` `vs_mesh`).

## The data (unchanged, read as is)

`dir.f32, z.f32, kind.u8, tri.u32 (planet outside the pattern, tile order), tiles.{f32,u32,e.u32}, lvl_N.u32 /
lvl_N.e.u32 (finest pattern triangles, edges), morph_vertex/anc/dptr/dep/dist/h0i/h0w, color.u8, lod.json`.
Every collapsed vertex: its stand-in (MA), its ring (DP/DE), its appearing and fully-in distances (Ao, Am = Ao/3)
and the coarser triangle it rises from (H0I/H0W).

## The app (`crates/game/src/planet_lod_app.rs`, `freefall --app planetlod [VIEW_DIR]`)

- **Cells, not vertices.** The finest triangles in 1 km cells (as the page's culling). Each cell is one `Mesh`
  built from its triangles through `res` (each vertex not drawn replaced by its drawn stand-in), rebuilt only when
  a vertex it uses changes state. The planet outside the pattern: one static mesh per tile, culled by tile.
- **What changes when the camera moves.** A vertex changes state only when the camera crosses its appearing
  distance. Each cell keeps the smallest |distance - Ao| of its vertices at its last decision: it is looked at
  again only after the camera has moved that far. A vertex that changes marks the cells its collapse subtree covers
  (bounding box of the vertices that collapsed into it, built at load), which re-resolve their `res`.
- **The rise on the GPU.** A drawn vertex carries its offset to the height it rises from (h0, from the coarser
  triangle's final heights) and its band, and the vertex shader blends it by distance: nothing per vertex per frame
  on the CPU.
- Views: Mesh (faces + edges coloured by rise) and Real (each triangle one colour from color.u8); the HUD the page
  has (altitude, LOD, sent/in-view counts), `#prof`-style timings from the engine profiler.

## Asks of the engine (general APIs)

1. **Morph band per vertex.** Today `vs_mesh` blends over [0.6 far, far] with far = 2*GEOMORPH_SPLIT*0.95*colour.a.
   The pattern needs [Am, Ao] per vertex (Am = Ao/3 now, configurable): e.g. a morph mode where colour.a is `far`
   (m) and a near/far ratio per mesh (or per vertex).
2. **The same morph on edges** (`vs_mesh_line`), so the wireframe stands on the drawn surface; ideally the edge
   colour blended by the same factor (grey at appearing, orange fully in).
3. **A face colour apart from the morph offset** (the morph uses colour.rgb): the Real view colours each triangle
   from the planet source.
4. **Many small meshes replaced often** (a few hundred cells a second while flying): the right way to swap a
   mesh's content (`Mesh::new` + `release_mesh` of the old id?), and any per-frame upload budget to respect.

## Gates (declared before the port)

- L1 the same drawn set as the page for the same camera (vertex for vertex, from `lod.json` descent cameras).
- L2 no cracks (edges used once = outer edges), no folds (as G4/G5).
- L3 frame time at 20 km, held and flying, on the RTX 5090: LOD CPU work under 2 ms a frame; 60 fps.
- L4 a vertex appears where the coarser surface stands: jump at appearing under 1 px (h0 from final heights
  instead of the page's live chain; measured).

## Measurements, stage A (14 Oct, release, RTX 5090, 1440x810 window on workspace 8)

App work with the camera held: LOD 0.02 ms, meshes 0.02 ms a frame (nothing decided, nothing rebuilt).
Frame rate held at 20 km (897 chunk meshes, ~490k triangles, edges on): 19.8 fps; the profiler puts 50 ms in
"hand over to the render thread" (main waits), GPU pass timings under 0.5 ms.

| what is drawn (20 km, held) | fps |
|---|---|
| 7 tile groups, no chunks (`UNIVERSE_PLANETLOD_MAXDRAW=0`) | 55.6 |
| + 100 chunks | 24.7 |
| + all 897 chunks, edges | 19.8 |
| all chunks, no edges (Real view) | 25.8 |
| all chunks, edges, no shadow casting | 19.8 |
| 600 km, all chunks, edges | 19.8 |

Flying (descent 200 km to 3 km, `UNIVERSE_PLANETLOD_FLY=1`): LOD 5-30 ms a frame, mesh rebuilds 150-9,000 a
second (0.6-57 ms a frame): 8 km chunks are too coarse to decide by (next: finer decision cells).
