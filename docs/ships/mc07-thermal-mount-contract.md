# MC-07 thermal equipment mount handoff

2026-10-10. Engine reply to ships task `20261010T170338-ships-35a4` for
`equipment.thermal.radiator.s2`. Ships owns hull placement and clearance.
This records current importer behavior and the export frame to preserve for
integration; it does not claim a working thermal equipment/deployment consumer.

## Names and binding

Export meshless Blender empties with these exact, unique names, rigidly parented
to the hull structure (not a moving panel):

| Hull empty | Registry slot | Offered mount | Fitted item |
|---|---|---|---|
| `mount_radiator_2` | `radiator_2` | `mount.thermal-s2` | `equipment.thermal.radiator.s2` |
| `mount_radiator_3` | `radiator_3` | `mount.thermal-s2` | `equipment.thermal.radiator.s2` |
| `mount_radiator_4` | `radiator_4` | `mount.thermal-s2` | `equipment.thermal.radiator.s2` |

These slots/fits already exist on fso in `standards/SFO/metadata/hulls/mc-07.yaml`.
The convention is `mount_<slot name>`, not an equipment key or mount-standard key.
Do not change the S3 `radiator_1` slot into another S2 slot. Keep stable names and
export the full rigid transforms, metres, unit scale, no reflected/negative scale.
The mount standard specifies capacity/envelope/attachment pattern, not the actual
locations on this hull.

## Datum and axes for this radiator

The installed radiator candidate's `mount_root` and its `RadiatorS2_Root` ancestor
are identity transforms in the neutral GLB. Use `mount_root` as the equipment's
placement datum, not the panel centre or hinge pivot. Its exported model basis is:

- +X: from the root edge toward the tip (about 12.5 m).
- +Y: panel thickness/outward normal for the stowed placement (0 to about .08 m).
- +Z: the remaining width axis (about -3.13 to +3.13 m).

In Blender, the corresponding axes are +X toward the tip, +Z outward from the
hull surface, and +Y maps to glTF -Z. Orient each hull mount accordingly; preserve
roll rather than supplying only a surface normal. Source the final root/attachment
locations from the actual equipment GLB, not a guessed centred box.

Compose placement as `hull_world * hull_mount * inverse(equipment_mount)`.
Here equipment_mount is identity, but retain that composition as the contract.
Record each full hull-root mount matrix (column-major glTF basis) with the hull
handoff, alongside its slot/key and clearances. Do not manually subtract the
physics shape's centre from exported empties: the importer applies its own
`made_centre` shift when building the shape. Future full-frame retention must
apply that same shift once.

`mount_root_left/right` and `mount_tip_left/right` are equipment attachment
witnesses, not four additional independently fitted modules. Their transforms
follow from the selected equipment placement; the tip interface includes released
hooks for deployment, so clearance planning needs the source sequence.

## Current runtime boundary

- `crates/core/world/src/import.rs` recognizes meshless `mount_*` nodes. It retains
  position and transformed glTF -Z direction (Blender +Y), **not a full rotation**.
  That direction is the legacy node-forward convention, not this panel's normal.
- `ship.rs` uses `mount_<slot>` to locate supported fitted modules; absent nodes
  fall back to the shape centroid. Do not rely on that fallback for radiator QC.
- Imported hull slots currently come from `standard_slots` (cargo, hardpoints,
  utility and flight systems). Adding `mount_radiator_*` does not create thermal
  slots. `SlotKind::from_record` also excludes thermal. The new inert-fitting
  support in `48253227` is limited to supported slots, so it does not yet admit
  radiator fittings. Thermal slot/mass integration is separate engine work.
- Registry visual packages are not yet automatically drawn at ship equipment
  mounts. Full mount-frame retention and mounted equipment rendering are still
  required; adding empties alone does not display the installed radiator package.
- Candidate-v01 is a **static stowed package**, without `motion.json`. There is no
  deployed device state, generic radiator hinge driver or thermal emission model.
  Source animation and hinge metadata do not enable runtime deployment.

For ships' clearance sweep only, the supplied metadata says `RAD2_panel` pivots at
`(0.04, 0.04, 0)` about `(0, 0, -1)` in equipment-root glTF coordinates, from 0 to
`-pi/2` rad. `RAD2_latch_sliding*` first translates +.10 m along equipment +X;
fixed hinge groups remain on the mount. Transform this whole sequence through
each mount and test the actual hull, hammers, gear, ramp and plume keep-outs.
These are the candidate's authored motion values, not a newly accepted runtime
motion schema. Deployment and thermal qualification remain explicitly deferred.

Sources: candidate-v01 `delivery.yaml`, `about.yaml`, and
`exports/radiator-s2.glb` under `~/git/blender/thermal-radiator-s2/`; registry
`docs/ships/radiator-s2-targets.md`, `docs/asset-contract.md` and
`standards/SFO/metadata/mounts/thermal-s2.yaml`. Engine's general placement/bind
rules are also in `docs/ships/ch-s2-motion-contract.md`.

## Exhaust clearance inputs, 2026-10-10

Reply to ships task `20261010T170802-ships-33c5`: **no authoritative plume
exclusion envelopes were found** in the current engine/registry contracts.
`docs/ships/mc07-exhaust-frames.json` supplies the current engine GLB's meshless
nozzle origins, normalized exhaust directions and slot classification, with the
source hash. It is model-root geometry before the physics `made_centre` shift.
Regenerate/compare these frames for the actual ships candidate export; the
working Blender source can differ from the engine's installed GLB.

Importer rule: accumulate the entire glTF node hierarchy; origin is transformed
zero, exhaust direction is transformed local -Z (Blender +Y). `nozzle_main*`
binds to drive, `nozzle_lift*` to lift, remaining `nozzle_*` to thrusters. Physics
push is the negative of exhaust direction. Runtime positions subtract the
shape's `made_centre`, then use the ship's world rotation/translation. Hull-root
clearance work must keep the uncentred hull and nozzle geometry in the same frame.

`crates/game/src/scene.rs::jets` uses PLUME=0.035 times square-root thrust and
GLOW=0.0016 times square-root rated thrust, plus flicker and rendering-specific
clamps. These are visual rules, not gas/radiation exclusion cones, heat-flux
limits or certified safe distances. Do not use the glow boundary as final
radiator clearance.

The rear pod's HOT EXHAUST marking is near the authored louver/heat-sink bank
(`build_mining_ship.py`, engine-bay section, VX/VY/VZ/VW/VH). The source does not
specify a plume cone, outlet flow, operating temperature or allowable exposure
there. A label or visible recess does not identify a physical flow axis. Registry
must define whether these are exhaust outlets, cooling surfaces or another device,
with stable interface frames and their operating/exclusion conditions.

Missing inputs for certification: envelope shape/extent or a flux-and-exposure
criterion per outlet, operating power/throttle and environment assumptions,
steering/divergence range where applicable, simultaneous operating cases and
radiator exposure limits. Ships may use explicitly assumed screening volumes
and report their parameters/results, but must keep final exhaust/thermal clearance
open. The radiator's own thermal/material qualification is separate as well.
