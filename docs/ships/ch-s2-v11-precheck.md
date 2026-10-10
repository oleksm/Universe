# ch.s2 refinement-v11: pre-acceptance compatibility check (registry, 2026-10-10)

Not an install (the candidate is not accepted). Files as delivered (`engine-ch-s2/refinement-v11/delivery.yaml`), hashes
checked: `exports/engine-ch-s2-rest.glb` f55916dd...32b6aa, `motion.json` c371f530...bd05ace.

## Run

| Check | Result |
|---|---|
| `install_model.py ... --motion=motion.json` (dry run, fso 3c519667) | pass: size, nodes, 46,276 triangles, schema, nodes in model, rigid binds, gimbal limit = record, actuators, links, bellows rims, all 9 reference poses |
| engine's CPU consumer, `cargo run --release -p universe-engine --example ch_s2_fixture` (main a1ecf04e, built on fso ea6fbd3a) | pass: 140 nodes, 16 sleeves, 131 primitives, 23 parts, 9 references, 513-pose sweep; bind error 4.8e-7, pose 3.7e-7, placed vertex 1.07e-6 m, normal 5.5e-7; 2 actuators |

## What still stands between this and moving in play

| Gate | Owner |
|---|---|
| The owner's acceptance of the candidate; open source QC (pipes, manifolds, datum, mass and heat, final hull fit) | owner, blender |
| Install (`--about`, `--motion`, `--push`) and the registry's validation entry (`docs/asset-validation.yaml`) | blender installs, registry validates |
| The game reads `motion.json` from the package (today it reads `model.glb` only) | engine |
| Equipment drawn at its hull mount (frame composition, roll, datum) | engine (step 3) |
| A per-engine gimbal command and applied state, clamped by the record's limit, saved and replayed; the same angle for thrust and drawing | engine (step 4a) |
| Live rigid motion in the renderer (per-instance pose, 23 parts, part 0 the static body) | engine (step 4b) |
| GPU appearance and cost: frames at play distance (workspace 8), draw count, shadows, motion bounds | registry looks, engine measures |
| Full-assembly clearance through the swing with the final hull | blender, then ships for the hull |
