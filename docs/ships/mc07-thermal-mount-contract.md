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
