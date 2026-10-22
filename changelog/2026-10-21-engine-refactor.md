# Engine review: fixes and consolidation, one by one

From the engine review of 2026-10-21 (duplication, fragmentation, control, performance).

1. A frame replaced on its way to the render thread (not taken within its patience) dropped what it asked of
   the GPU besides drawing: mesh uploads, pooled uploads and the releases sent only once (the land pyramid's
   evictions), so ranges stayed resident till pool pressure. The new frame takes them on
   (`Frame::carry_requests`); the old frame is freed after the slot's lock is let go.
2. Contact asks each substep's body motion once: `ground::GroundFrame` holds the body's centre, velocity (its
   orbit chain solved once), spin and turn, and answers every contact point's three height samples; before,
   each sample solved the chain and the rotation again. The land pyramid's raster cache is in 16 shards locked
   apart (the parallel ships' queries no longer queue on one lock); a query no longer allocates. Not done:
   sharing tile reads between the renderer and physics (they read different rectangles; the OS page cache
   already shares the file reads), nor one copy of the anchor mesh (the renderer's f64 vertices would need
   rewriting for about 0.5 GB).
3. The pair grid's walk took every cell in a mover's reach: by the cube of its sweep (at warp or high speed,
   millions of empty cells a mover). It now takes the group's occupied cells in that box when they are fewer,
   in the walk's order so the contacts are the same. Collisions take the world's cached body positions
   (`rails_at`) instead of solving them again. (Rebuilding the grid each tick stays: everything moved.)
4. The world's shared caches no longer do their work holding the lock the parallel ship steps ask: a system is
   made (its worlds read from disk) with the systems cache let go, the first made kept; ephemerides and field
   ephemerides are worked out unlocked; a field's dug rocks are only gathered when its total has changed (a
   flight step asked it, allocating, every time). The 64-system trim runs on insertion only.
5a. A landed ship's step solved its world's orbits (the planet's and its moons' pull) for its velocity, every ship
    every step: 1,577 landed ships took 7.8 ms a frame in a busy port scene. The world keeps body velocities as it
    keeps positions (`velocities_at`, frozen with the rails while ships step side by side; one two-moment cache
    for both), and `Weld::place_moving` takes it: 2.0 ms. The world step went from 14.8 to 9.1 ms.
5b. Meshes were packed for the GPU (face and edge vertices, normals, edge normals through hash maps) on the render
    thread when first drawn: 6.6 new terrain meshes a frame took 1.4 ms of it in the busy port scene. `Mesh::pack`
    packs them where they are made (the planet LOD build worker, its tile groups, the game's patch pool); the upload
    takes the packed copy (not kept twice). Upload 1.48 -> 0.33 ms, the render step 1.93 -> 1.32 ms. Profiler
    scopes for the upload's parts (new meshes, batching, casters).
5c. Ships near a belt (Hoar E's traffic) spent 4 ms a frame asking which asteroid field they are among: the belt
    patches' bodies were cached 256 at a time and the whole cache cleared when full (patches made again, under
    one lock, spikes to 31 ms), and every rock of every patch near was solved for where it is. The patch cache
    now lets go its longest-unasked past 1,024 and makes patches unlocked; a rock whose distance from the star
    (between its periapsis and apoapsis) can't come within reach is passed over unsolved. Field look 4.1 ->
    2.3 ms (no spikes), the world step 9.1 -> 7.5 ms. Profiler scopes for the flight step's parts.
5d. Small per-frame work: the PBR uploads' order worked out once per upload (its importance scanned every draw
    inside the sort's comparisons); ATC's change lists sorted by their text once each (`sort_by_cached_key`,
    the same order); the pool's releases checked against the frame's mesh index, not a scan. Left as they are,
    measured small in the busy scene: the pilots' copies of awake ships (0.15 ms: sharing them would put every
    ship behind an Arc), the crash log's `remove(0)` (a short list), ship shape pairs (already culled by spheres).
6. A world's heights come from one `Source` (procedural, a bake, lines, a land package, the development graph)
   in place of four optional fields that every query sorted out for itself (56 references). Lines, land and
   bake answer through one `Terrain::ground(dir, detail, cell)`; the sea and the ports' plains are put over it
   as before. Two slips the fields allowed go: a lines world also kept a bake (`baked()` true, the ground-ahead
   prefetch reading its tiles for nothing), and a world from the store still worked out the seed's crater
   noise for `classify` (always 0 there). Captures of a baked world, a moon and Hoar E match before/after.
7. One home for the planet lab's latitude-longitude frame: `land::lab_direction` and `lab_lat_lon` (the terrain
   crate's `unit` re-exports it; land's own copy and its test's go), beside `to_body` and the game's
   `worlds::direction`. The crew's north-east tangent is the spaceport's (they were the same code). The ATC
   presence check over a pad stays its own: a column at any height, where `pad_at` is resting on it.
8. The game's developer settings (`UNIVERSE_*`, 124 of them) are read from one snapshot of the environment taken
   at the first ask (`devenv::var`, `var_os`, as `std::env`'s): 122 direct reads in 20 files go through it, among
   them about ten a frame (each a lock and a copy). A test keeps it so. `UNIVERSE_OBSERVING` meant "set" in the
   game and "= 1" in the studio and observer: one `devenv::observing()` now (any value but empty or 0).
9. How a frame draws what's drawn now is one `DrawState` (front or scene, shadow casting and the ground's cascade,
   surface, globe and its place and grain, air) in place of eight fields each saved and put back by hand; the
   seven `in_front`, `in_scene`, `with_surface`, `with_globe`, `with_air`, `no_shadow`, `ground_shadow` are one line
   each over `with_state` (the whole state put back, nothing forgotten). Captures of a baked world, a moon and
   Hoar E match before/after.
10. Mesh drawing: the renderer's three instance batchers (by mesh, casters, overlays back to front) share one
    `runs` (each only picks its order); the overlay and PBR axis scalings share `Instance::scale_axes`. The six
    `model_*` calls stay: each is already one line over the frame's one `mesh()` (a style struct would change 37
    callers for no gain).
11. One orbiting eye (`orbit_cam::OrbitCam`: drag, keys, zoom, camera) for the observer and the planet, scene,
    plant and factory apps, each with its own buttons, tilt and range; five copies of the same yaw, pitch and
    distance code go. The observer saves its orbit as before (flattened). Captures match. Left to the ships
    session (its code): the studio's and test drive's own orbit (other drag and zoom rates).
12. Small shared pieces: `Input::shift()` and `ctrl()` (12 hand-written either-key checks; the ships session's
    interior and studio code left to it); `scene::star_light` lights a system as the game's scene, the planet app
    and the scene app did each with the same lines; `scene::studio_sun` and `scene::ground_grid` for the plant
    and factory apps. Captures match. Not done: one theme for `palette` and `hud_kit` (the HUD's look, a design
    decision for the owner) and splitting `hud.rs`, `interior.rs`, `dev.rs` (the last two the ships session's).
15. Caches: `Design::spec` leaked a whole hull spec (its shape mesh with it) for every set of knob values and kept
    it, and nothing called it: gone. Kept, bounded in practice: hull specs per fit (`&'static`, 196 callers rely on
    it; one per distinct fit), the per-world height caches (one per world with a bake, lines or a land package;
    physics needs them while ships are there), the contact hulls by shape (guarded against a reused address).
14. The game's background jobs run on one small named pool (`background`, two threads) beside the world's and
    the pilots': opening a world's land, reading a world's maps, laying the lines round the eye, the planet LOD
    app's view-box count (a new thread every 0.3 s before). Captures match. Not done: one GPU residency manager
    (one budget, frame counter and upload allowance for meshes, pool, PBR, globes, world maps): a redesign of
    the renderer's memory, to plan with the owner, not a refactor.
13. Observing (SHIFT+F3, the live port) is one `observe::Observing` and one `observe::serve` for the game, the
    studio and the apps' host, in place of three copies of the toggle, the port's first opening, the hitch watch,
    the answers and the recorded frames; the game keeps its own "OBSERVING OFF" and F3's debug cycle (now F3
    without SHIFT). Checked live: the game's and an app's port answer /status and /profile.
