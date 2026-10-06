# From the integration session to the ships session

*2026-10-05. Main and `ships` are level at the merge of 67617e2 (the studio resizing placed
modules). All 86 tests pass. What on main touches ships, and what is left on your side.*

## On main, in or near your code

- **A registry hull is built as its record says** (your `registry-fit-request.md`): its `fit`, its
  `capacity` as a bay built into the frame (`HullFrame::bay`), its `flight` radius, drag area and
  strength. `import.rs` finds the record by `model`.
- **The MC-07 weighs what its parts do,** parts made of parts included (legs, ramp, clamps,
  laser): a 154.2 t frame, about 165 t dry fitted.
- **Landing on legs, `world::legs`:** the legs worked out from the hull's parts (each strut's stock
  tube against its material's yield and Euler buckling, the shortest stroke, `strut_efficiency`),
  the same reckoning as the registry's structure report. At touchdown the sink is held to
  `Legs::hardest(mass, g)`: past it a leg gives way (wrecked), past `designed` a hard landing and
  its jolt (`ShipEvent::HardLanding`). `ground_check` gives lift against weight. You said FRAME's
  landing case will read its figures from here: `Legs { force, stroke, efficiency, designed }`,
  `hardest`, `jolt`. If FRAME comes to give the legs' strength better than a column per strut, say
  so and `legs` can take it from `world::frame`, so there is one answer.
- **No clearance to land where the ship's lift or legs can't hold it;** the landing readout gives
  gravity, lift against weight and what the legs take.
- **Shock:** stock and parts carry `shock_limit`; a jolt past it breaks that cargo
  (`ShipEvent::CargoBroken`).
- **A mining rig is its product's** (`mining::Rig`: excavator power, throughput, anchor reach and
  speed from the fitted rig's record; no rig, no anchor). **A survey is the fitted sensor's**
  (`belts::Survey`: `resolves`, `survey_range`). **Radar range is the fitted sensor's**
  (`radar::range(spec)`, no constant).
- **Outdated hulls fly crowded** (the five stock hulls, `revision: outdated`): placement's "no room"
  is listed in `spec.crowded`, not an error. The balance and sizing tests skip them.
- **`Design::default()` is the MC-07's size** (66 × 31 × 19 m); the import test uses `mc07.glb`.
- **Ships are built from the bill:** parts, equipment and hulls are stock, counted by the piece;
  the yard's company sets its modules; Trethi Yard builds an MC-07 in about 24 days. A port's market
  sells modules and hulls from its warehouse.

## Left on your side, when you want it

1. **Gun and laser figures:** `weapons.rs` still holds `GUN_*` and `LASER_*` as constants; the
   registry has them on the gun and laser records (`equipment.function`). The handler's
   `gun`/`laser` methods now receive the whole kind (`it: &EquipmentFunctionGun`), so reading
   them is a field each.
2. **Mounts (SFO 19):** equipment names `fits: mount.<slot>-s<class>` and each hull slot its
   `mount`; the game still fits by slot kind and size class. Fitting by mount (envelope, weight
   borne, nozzle thrust, what the hull feeds) is yours.
3. **The MC-07's sizing is deferred until it is flown** (its record is a draft). As it stands its
   lift is 0.48 to 0.83 of its empty weight on Treistun's four heavy worlds; the user wants that kept
   as a problem for players to solve. Its home world (Treistun e, below the starting station) is one.
4. **The stock fleet at real equipment sizes** is off balance (the Drover keeps 67% of its lift
   authority): NPC traders fly it. The user's word on the outdated fleet: "if it breaks it breaks".
5. **Imported hull strength:** the MC-07's record has no `flight.hull_strength`, so it still takes
   `STRENGTH_PER_KG` × frame mass. FRAME may give a better answer.
