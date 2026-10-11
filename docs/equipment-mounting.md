# Mounting equipment models on a hull (registry contract, 2026-10-10)

For engine's generic renderer (task 20261010T194308-engine-8630). Static placement only; motion stays in
`freefall-motion/1` and is gated.

## Binding

- A hull record's `slots[]` name the slots; its `fit[]` puts an equipment key in each (`standards/SFO/metadata/hulls/*.yaml`).
- The equipment record's `visual` names its package (`freefall-model/1`, `docs/asset-contract.md`).
- The hull model carries one meshless node per slot, `mount_<slot name>` (engine's convention in
  docs/ships/mc07-thermal-mount-contract.md): its origin is the slot's mount point, its frame the slot's frame.
- The equipment model's **primary mount node** is, in this order: a node named exactly `mount_<slot name>` for the slot
  it is fitted to, else one named exactly `mount`, else the model's root (origin, identity). Other `mount*` nodes (feet,
  bolt points, a radiator's second hinge) are attachment detail, not the frame.
- Placement: `world = hull_placement * hull_node(mount_<slot>) * inverse(model_node(primary))`. No centring by bounds,
  no scale. A fitted item with no `visual` draws as its box (`docs/asset-contract.md`, Model or box).

## Frame convention

Model space is glTF: metres, +Y up. The primary mount node sits on the contact face, and its frame is the slot's frame as
the equipment sees it: what the hull node says for the slot, the model's node says for the equipment. The MC-07's
extracted equipment is authored in the ship's own axes with its primary node at the origin and no rotation, so its
slot nodes on the hull carry the same axes (rotation-free) at the mount points; generic equipment (comm, radar, tanks,
avionics) has its `mount` node at the origin, the contact face, and expects the slot node to point the right way.

## Installed static models today (fso; validation in docs/asset-validation.yaml)

| Key | Package | Primary node | Note |
|---|---|---|---|
| equipment.engine.ch.s2 | models/engine-ch-s2/v1 | root (`mount_CHS2` at origin) | motion.json present, gated |
| equipment.access.anchor-clamp.mc07-aft | access-anchor-clamp-mc07-aft/v1 | `mount_clamp_aft` | |
| equipment.access.anchor-clamp.mc07-fwd | access-anchor-clamp-mc07-fwd/v1 | `mount_clamp_fwd` | |
| equipment.access.cargo-ramp.mc07 | access-cargo-ramp-mc07/v1 | `mount_cargo_ramp` | |
| equipment.gear.front.mc07, .rear.mc07 | gear-front-mc07/v1, gear-rear-mc07/v1 | `mount_gear_front`, `mount_gear_rear` | stowed |
| equipment.hardpoint.mining-hammer.mc07 | hardpoint-mining-hammer-mc07/v2 | one primary mount at the origin, identity (v2, installed 9ec2e645; the old offset marker is now `attachment_reference_hardpoint_l`) | |
| equipment.thermal.radiator.s2, .s3 | thermal-radiator-s2/v1, -s3/v1 | root (`mount_root` at origin) | stowed |
| equipment.thermal.coolant-loop.s1, .s2 | thermal-coolant-loop-s1/v1, -s2/v1 | root (only foot nodes) | |
| equipment.tank.air.s1, .water.s1 | tank-air-s1/v1, tank-water-s1/v1 | `mount` | |
| equipment.comm.basic.s1 | comm-basic-s1/v1 | `mount` | |
| equipment.sensors.radar.s1 | sensors-radar-s1/v1 | `mount` | |
| equipment.avionics.nav-full.s1, .computer.fbw.s1, .transponder.s1 | their v1 packages | `mount` | |

## Reading it without a merge

The records live on fso (`~/git/universe-fso/standards`), the packages in the assets store (`~/git/freefall-assets`).
Until the owner merges fso into main, read the registry from the fso worktree for this work (point the build at it, or
copy nothing: read in place); the packages need no merge.
