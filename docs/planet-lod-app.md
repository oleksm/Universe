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

**Withdrawn (the engine's review, 14 Oct):** the frame rates below measured an engine fault, not draw cost. The
app gave wire edges their own vertices, so the engine's edge normals fell back to a nearest-face search per edge
(edges x faces) on the render thread: 40 s to upload the tile groups, frames handed over after 50 ms timeouts.
Fixed on main (36cf1065: normals by the edge ends; the app's edges now index the face corners). After the fix,
captures on a hidden workspace still wait in 'acquire surface' (~31 ms a frame: the compositor, not the app);
on screen the engine measured 120 fps (the refresh cap) at 20 km held: render thread 1.1 ms, app 1.0 ms.
Flying 150 km to 24 km (after the fix): 21 to 110 fps, the app's mesh rebuilds 40 ms a frame at 150 km (next).

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
second (0.6-57 ms a frame): 8 km chunks are too coarse to decide by.
With 1 km decision cells (meshes still by 8 km chunks): LOD 5.6 ms at 130 km, 1.9 at 29 km, 0.8 at 3 km (the
floor is the margin test over all ~150k cells; a hierarchy would take it off); mesh rebuilds 4,370/s at 130 km
(22 ms a frame), 880/s at 29 km (2.9 ms), 200/s near the ground (0.7 ms). Rebuilds are bounded by the collapse
subtrees' chunk boxes; smaller mesh chunks would cut them but multiply draws (waits on the render-thread answer).

## Toward 120 fps (14 Oct, the owner: "do all 4")

Live, Yosemite (903k pattern vertices at 50 m, 0.5-0.9M triangles drawn): 61-116 fps flying. Per frame: LOD decisions
on the main thread 7.7 ms mean (25 worst), mesh upload on the render thread 7.3 ms mean (17 worst), GPU scene
0.3-0.7 ms. The GPU is idle; the CPU work scales with change.

1. Chunks sized by triangle count (about 3,000 finest triangles), not 8 km: a flip re-sends 100s of KB, not MBs. (planets)
2. Decisions in a worker thread: it owns what is drawn and takes the eye; the build worker takes its snapshots. (planets)
3. Smaller GPU vertices and a mesh pool. (engine, general API; see below)
4. A cluster DAG: fixed clusters of ~128 triangles per level, groups simplified to half with their borders locked,
   picked by on-screen error; nothing rebuilt, uploaded once, cached, the 1 s preload by the same selection. (planets;
   needs 3)

### Ask 5 of the engine: meshes for many small, often replaced parts

- Indexed meshes: shared vertices (positions as f32x3, normal and colour packed: about 16-24 B a vertex) instead of
  56 B per corner; faces as an index list. Today a 30k-triangle chunk is about 5 MB a rebuild.
- A pooled store: one large GPU buffer sub-allocated for thousands of small immutable meshes (clusters), each a
  range, drawn in few calls (instancing or indirect), freed into the pool. Upload of a new part is a copy into the
  pool, not a buffer creation.
- Flat-shaded faces with per-face colour must stay possible (the Real view), and the per-vertex morph band (ask 1)
  should work on pooled meshes.

## Object boxes (21 Oct)

Optional `boxes.json` beside the view, produced by planet-forge `tools/object_boxes.py`:
`legend: [{family, rgb:[0..255,0..255,0..255]}]`, `boxes: [{lat,lon,z,heading_deg,
length_m,width_m,height_m,rgba:[0..255,0..255,0..255,0..1],label,family}]`.
Coordinates and heading are degrees, dimensions and base altitude are metres; heading is
clockwise from north. Height exaggeration moves the base with terrain, retaining physical dimensions.
The optional label defaults to empty. Invalid sidecars report a load error; absent sidecars leave the viewer usable.

One shared cube supplies all geometry. Camera-relative per-axis transforms and linearised colours
are instances, culled against the view and horizon. The renderer sorts instances far to near and blends
after opaque terrain/models with depth testing, no depth writes and no shadows. Intersecting transparent
volumes use centre sorting, so intersection blending is approximate. Nearest 40 labelled objects whose
top anchors are in view receive terrain-depth-tested labels. OBJECTS has a coloured toggle per legend family.
`UNIVERSE_PLANETLOD_AT` accepts `boxes=all`, `boxes=none` or comma-separated family names;
observer status includes `objects_drawn` and enabled `object_families`.
