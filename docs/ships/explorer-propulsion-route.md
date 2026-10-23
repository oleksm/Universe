# Explorer propulsion: the smallest truthful consumer route (engine, 2026-10-22, scope only)

Registry `20261022T191820-registry-eeb0` (chemical engines for the explorer, no alias to the torch, no fake
nozzle) and ships `20261022T192155-ships-8631` (four independent engines, two aft and two down). Read from main at
f012e7490. Nothing is implemented yet; this is the route and what it needs first. The importer and `ship.rs` stay
the engine's to edit (ships: diagnostics only, as now).

## What main does now

- `engine` equipment (`engine.ch.s1`-`s4`, SFO 22) is `not_made`: no module, so a fit entry naming one is held.
  The registry's `engine` slot kind has no game kind (`SlotKind::from_record` → None), so the slot is dropped too.
- A thruster is a nozzle driven by the module in its slot, and only `drive`, `thrusters` and `lift` slots drive
  nozzles (`ship.rs`, "the thrusters"); its role (Main, Rcs, Lift) is the slot's kind.
- The importer binds nozzles by name prefix alone (`nozzle_main_*` → `drive`, `nozzle_lift_*` → `lift`, else
  `thrusters`, share 1) and refuses a model with no `nozzle_main_*`. The record's `thrusters[{nozzle, slot,
  share}]` are not read.
- Every ship must carry a Drive and a Thrusters (RCS) module among its base blocks.
- One fuel aboard: tanks, plants and engines must all hold or burn one material.

## The route (one engine commit, the world crate)

1. `Does::Engine { thrust, exhaust, efficiency, burns }` from `Kinds::engine` (throttle floor and gimbal read and
   kept on the module, not simulated: see the limits); `SlotKind::Engine` from the registry's `engine` slots.
   Each engine is its own slot (e.g. `engine_aft_port`), its own module instance, its own mass.
2. Nozzles bound to an `engine`-kind slot by the hull record's `thrusters` list: exact nozzle name, slot and share.
   Every other nozzle keeps the prefix binding, so the MC-07 and other hulls are unchanged. A record that binds
   an engine nozzle to a slot it doesn't have, or a nozzle the model doesn't have, is refused, not dropped.
3. An engine's role from where it pushes in the hull frame: along the nose (within 30°) Main, upward (within 30°)
   Lift; any other direction is refused (an engine is not RCS). The "no `nozzle_main_*`" refusal becomes "no
   thruster with a Main role", so a forward-pushing engine meets it without a fake nozzle name.
4. Base blocks: a Main-role engine stands for the Drive block. Nothing stands for Thrusters (RCS).
5. Fuel: the existing one-fuel rule already admits hydrolox throughout (`tank-hydrolox-*`,
   `power-fuelcell-s1`, `engine-ch-*` all hold or burn `material.hydrolox`).
6. Tests (fast): role and binding on a fixture record (two aft, two down: two Main, two Lift thrusters, each its
   own thrust and mass); a binding to a missing slot or nozzle refused; MC-07 thrusters, fit and mass unchanged.

## Prerequisites (the registry's and the ships session's, before step 1 helps the explorer)

- The record: `engine` slots per instance with sizes and mounts, `fit` naming the chosen `engine.ch.*` in each,
  and `thrusters` binding each real nozzle empty to its slot. The exported model needs those nozzle empties
  (the intake has none: `main_nozzles: []`), each at the engine's real exit and pointing along its exhaust.
- Attitude control: four fixed engines give no turning. The game doesn't simulate gimbal (0.07 rad on the ch
  engines). The explorer needs an RCS fit (a `thrusters` slot with its nozzles), or gimbal physics as a separate,
  larger engine change. Without one it fails the base-block check, as it should.
- A whole mass (`goods::hull_mass` must be Recorded or Parts: the import refuses partial or none).
- Power and tanks sized for hydrolox (fuel cell or batteries; hydrolox tanks).

## Limits, stated

- Throttle floor (the ch engines' 0.2) not enforced: thrust is continuous 0 to full. Gimbal not simulated.
- Startup, restart, chill-down and ignition limits not modelled.
- Engine heat, the record's `heat_to_hull`, is not fed to the hull.
- No exhaust plume or ground erosion.
