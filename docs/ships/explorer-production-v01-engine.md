# Explorer production-v01: what main supports (engine, 2026-10-22)

The registry's question (`20261022T185008-registry-1eee`): the supported route for a new hull into the game, ramp,
boarding and gear today, and the tests to pass before preview or commissioning. Read from main at 782bdd65f; no
claims beyond the code. production-v01 holds only `review/fit-inputs.json` so far (`export/`, `source/` empty).

## Import and preview

- One importer: `world::import::hull_from_gltf` / `commission` (`crates/core/world/src/import.rs`). A `.glb`, Blender
  axes (nose +Y, up +Z). Named nodes: `nozzle_*` (at least one `nozzle_main_*`, else refused), `mount_*`, `gear_*`,
  `dock_*`, `door_*`, `cockpit`, `hatch`; `COL_*` meshes are its convex collision (none: the whole model's hull);
  any mesh with `Ramp` in its name (and what hangs under it) is its ramp. Scene extras `freefall_name`,
  `freefall_class` optional. Blender `.001` suffixes ignored.
- Its key is `hull.import.<file stem>`. A registry hull record whose `model` is the file's path supplies its name,
  parts' mass, price, fit and hold; without one the frame mass is from its size class.
- Preview: `UNIVERSE_HULL=path.glb freefall` flies it as the player's ship (set down at its gear height). The
  studio (`freefall --studio`, `--report/--fit/--compare <design>`) works on registry hulls whose record names the
  model. Both are the ships session's tools; the hull fit itself is theirs.
- Look only, no hull: `UNIVERSE_SCENARIO=showcase UNIVERSE_MODEL=review.glb UNIVERSE_MODEL_CLEAN=1 freefall` shows the
  model ahead of the eye, slowly turning (`crates/game/src/lib.rs:1271`, `scene.rs:112`; CLEAN hides the player's
  own ship). It commissions nothing: no collision, nodes, ramp or gear are read. Distinct from `UNIVERSE_HULL`, the
  flight import above. (Registry's audit addition, 2026-10-22.)

## Ramp and boarding

- A ramp is rigid, one hinge: an authored mesh named exactly `CargoRamp` (its origin the hinge, its local +X the
  axis) if present, else the `hatch`
  node with the axis across the line to the ramp's middle. It swings down only when landed at a hatch-usable
  place, by `asin((hinge.y − lowest gear.y) / length)`, i.e. until its far end is level with the feet; else shut.
  No intermediate states, no animation curve, no second ramp, no doors that move.
- Boarding: on foot, a ship with a ramp is walked up its ramp's triangles (`crew::inside_hatch`, `ship_colliders`);
  a ship without one boards at the `hatch` node. The cockpit seat is the `cockpit` node.

## Gear

- Contact: the `gear_*` nodes are the feet (`ground::struts`), sharing the hull record's legs (SFO 15 struts:
  force, stroke, efficiency, sink speed: `world::legs`); or a design's fitted leg devices. Rest height = the
  lowest foot.
- Static only. A fitted landing-gear visual with motion (`has_motion`) is refused
  (`crates/game/src/mounted.rs:83`), and an overlay replacing native nodes too (`:43`). No retraction, no stowed
  state, no animated oleo: the gear-strut-s0 hinge/oleo nodes would be drawn fixed.
- The explorer's intent is retractable gear. A preview on main shows it fixed, as a limit of the engine, not an
  acceptance of fixed gear: retraction stays an open engine capability (an articulated mounted consumer).

## Before preview or commissioning

- Import: `UNIVERSE_HULL=…glb freefall` loads with no error (the importer's refusals are its checks: geometry, a
  main nozzle, node names).
- Thrusters: `cargo run -p universe-world --example mc07_rcs_review` is the pattern (the MC-07); a copy pointed at
  the new file for its RCS authority.
- Ramp: `cargo run -p universe-world --example ramp_threshold <glb> <from> <to>` (walker across the ramp's threshold),
  `cargo run -p universe-engine --example cargo_ramp_fixture` for an authored ramp motion fixture (diagnostic).
- Landing: `ground::tests::the_hardest_landing_lies_between_what_the_legs_hold_and_what_breaks_them` and
  `test_drive::local_flight::tests::dropped_too_hard_its_legs_give_way_and_the_hull_takes_the_fall` cover the leg
  rules; a flight in the test drive with the real file (the ships session's) is the end-to-end check.
- Mass, equipment, key: the ships session and the registry record.
