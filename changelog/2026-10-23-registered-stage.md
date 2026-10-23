# A registered hull's record fit is its fit; a fit that can't fly yet is a stage, not a guess

Registry `20261023T182935-registry-c9b8` and ships `20261023T182957-ships-4618` (the explorer's records before its
propulsion and avionics are chosen).

- `import`: a hull its record describes is fitted exactly as the record says, an empty fit included: `stock_fit` is
  only for an unregistered model (it made up a cheapest fit for an empty record). The MC-07's fit is unchanged.
- A registered hull whose fit can't fly yet (a base block missing, too little power, no drive) is built
  uncommissioned (`HullDef::build_uncommissioned`, `ClassSpec::commissioned` false), the reason first in its
  `limits`. It's shown, walked and inspected, not flown. Its mass must still be whole (partial or none refused).

## The ships' question: minimum native avionics, power and thermal (what main does, what it would need)

- Sensors: the base blocks need one. The only sensor the game makes is `sensors` (the 100 kW radar, priced). The
  camera (10 W) and altimeter (40 W) are kinds not made, and unpriced, so they're in no catalogue. Needed (engine):
  `Does::Camera`/`Does::Altimeter` from their records (their own figures, in the Sensors slot), standing for the
  base block. Not a radar's numbers on them.
- Power: the 50 kW fuel cell can't carry the radar: the assembler refuses draw over output. Batteries are a kind not
  made natively (the studio's device fit models them; the native assembler doesn't). Needed (engine): batteries as
  native energy stores (capacity, rate) drawn down while the plant can't cover the draw, refused at zero, not
  unlimited.
- Thermal: radiator and loop slots have no game kind, so their fit is dropped. Needed (engine): carried as inert
  modules (mass counted), as gear and command are now; heat itself isn't simulated (engines' heat is a stated limit).
- Until those land, the explorer's record can stand as an uncommissioned stage with exactly what it carries.

Test: `import::tests::import_empty_fit_is_an_uncommissioned_stage`; the workspace's library tests pass.

## Follow-up: the importer's nozzle check (ships `20261023T183414-ships-632b`)

- `hull_from_gltf` refused any model without a main nozzle before the stage path. Now only an unregistered model
  is refused for it; a registered one with no nozzle builds uncommissioned ("not commissioned: no main drive: …"),
  no nozzle made up. The assembler's main-drive check now covers hulls with no engine slots too.
- Test through the real importer: `import_registered_model_without_nozzles_is_a_stage` (a nozzle-less box GLB:
  refused unregistered, a stage under a registered model path).
