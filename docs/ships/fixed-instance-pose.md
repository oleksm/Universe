# Fixed-instance pose: drawing a motion package at rest, by explicit scope (engine, 2026-10-22)

Registry `20261022T192713-registry-ce37`. The CHS2 v2 package (`engine-ch-s2/v2`, model f55916dd) is static rest bytes
with `motion.json` beside them. `mounted` refuses any fitted visual with motion (`crates/game/src/mounted.rs:83`), so
a fixed CHS2 is never drawn. The registry prefers an explicit per-instance policy over a duplicate static variant.
The v2 package stays as it is; a hull opts one fitted instance into rest; the default stays refusal.

## The binding (minimal)

On the hull record, per fit entry (`hull.schema.yaml`, `fit.items`), one new optional property:

```yaml
fit:
  - { slot: engine_aft_port, item: equipment.engine.ch.s2, pose: rest }
```

- `pose: rest` (an enum with this one value for now): this instance is drawn in its model's default-scene node
  transforms (its rest), its motion contract not played, and nothing in the game moves it. Absent: as now, a
  fitted visual with motion is refused.
- It is per slot, so it binds to one real engine instance (the Engine slot route, one slot per instance); another
  hull, or another slot, fitting the same item without it is still refused.
- It claims nothing physical: gimbal stays "not simulated" in `ClassSpec::limits`; no thrust vectoring, no
  turbulence or startup playback. The record's mass, dimensions and identity are the item's own.
- Only for an item whose package has motion: `pose: rest` on one without motion is a record error (a check in the
  registry's validator; the engine refuses it on import too).

## Who does what

- Registry: add `pose: { enum: [rest] }` to `fit.items.properties` in `hull.schema.yaml` (and the validator rule
  above), regenerate. Then mail the engine.
- Engine, once the field is generated: `HullDef` keeps each fit entry's pose, `ClassSpec` carries the slots fitted
  at rest, and `mounted::item` draws a motion package at rest for those slots only (the rest unchanged, still
  refused). Test: one slot at rest drawn, the same item in another slot refused, MC-07 unchanged.
- Ships/Blender: place each instance by its own slot's mount node (see `explorer-propulsion-route.md`).
