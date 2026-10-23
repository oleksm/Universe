# fso 4ce0849f9 and f14c0bfdc merged; fitted motion packages drawn at a declared rest

- Registry: landscape feature trigger parameters (66 definitions, PlanetSim words refreshed), the hull fit's
  `pose: rest` (32e8d6d0d), the gear silhouette review gate. No game behaviour changes from them.
- `pose: rest` (docs/ships/fixed-instance-pose.md): `HullDef` and `ClassSpec::rest` carry the record's (slot,
  item) pairs fitted at rest; `mounted` draws a motion package only for such a pair, its default scene held still,
  claiming nothing physical (gimbal stays a stated limit). Without it, refused as before; on a package without
  motion, refused as a record error.

The workspace's library tests pass. Tests: `import_rest_pose_is_per_slot_and_item`,
`mounted_motion_package_drawn_only_at_declared_rest`.
