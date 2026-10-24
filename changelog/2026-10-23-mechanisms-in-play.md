# Hull mechanisms in play: states on the ship, posed floors, ladders, ground contact, drawn parts

Registry `20261023T184542-registry-0607`: the discrete consumer of a hull record's `mechanisms` and `exits`.

- `Ship::mechanisms`: each mechanism's state, physical, kept from tick to tick (saved; empty: defaults). The sim
  settles them once a tick for the ships in the player's system (`crew::settle_mechanisms`):
  - gear: deployed when landed, stowed otherwise;
  - `always`: deployed;
  - `landed`: deployed where the hatch opens (landed on a world), else its default;
  - `pilot`: as held (no control yet).
  A deploy with a ground `contact` is taken only where the marker, in the candidate deployed pose at the ship's
  attitude, sits between 0.05 m into the ground and 0.45 m (the walker's step) over it, on standable slope.
  Otherwise the state before is kept. Engine gameplay limits, not engineering figures.
- Importer: the meshes under each owned node (the nearest owned node above them) are that node's floor, not the
  hull's fixed walk mesh. A hull with mechanisms gets no legacy `*Ramp*` reading.
- `crew::ship_colliders`: each floor where its state puts it. `on_ladder`: an open climb exit's top-to-foot line,
  held within 0.6 m, inside and outside. `walks_out`: true for any hull declaring exits, so no hatch step or teleport
  when they're shut.
- Drawing: each owned node is a part of its own (exact names, `=Name`, in `PbrModel::load_gltf_parts`), drawn at its
  state's move.
- Test: `crew::tests::declared_mechanisms_pose_floors_and_deploy_onto_the_ground` (real importer, a two-leaf ramp
  with walk and climb exits: floor posed, ladder only when open, deploy on a generated rocky world at 0.1 m, kept
  shut at 5 m). The empty-fit unit test was dropped (the importer stage test covers it).

Not yet: no record carries mechanisms, so nothing has been seen drawn in a window; gear states don't gate the
legs' contact; no motion between states; exits aren't chosen by reachable route (the walker reaches whatever it
can walk or climb to).
