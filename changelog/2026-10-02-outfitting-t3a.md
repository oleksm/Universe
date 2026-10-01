# Refitting (T3a)

- **A ship's own fit:** a ship may carry a fit of its own (`Ship::fit`; None is its hull's stock
  fit), saved by module keys. Its numbers come from hull and fit together (`ClassSpec::assemble`,
  shared with the hull's own build), worked out once per distinct fit and kept (`ship::fitted`),
  so any number of ships on one fit share them. The ship keeps them to hand.
- **Refitting:** `Universe::refit` for the player, `refit_as` for any pilot (player parity), done
  docked at a station.
  - **Paying:** the new module's price goes to the station's market, less `BUYBACK` (60%) of the
    price of the one taken out, through the ledger.
  - **Refused, with the reason:** not docked at a station; a module that doesn't fit its slot; a
    base block taken out; a plant that can't carry the load; a hold too small for what's in
    it; not enough credits.
  - A smaller tank keeps what it holds.
- **What's fitted counts:**
  - **in the world:** the gun and laser fire only if fitted; the anchor needs a mining rig; the
    hyperdrive won't start without one (`ShipEvent::NotFitted`: "NO HYPERDRIVE FITTED").
  - **in the avionics:** each autopilot needs its nav computer to run it — the approach autopilot
    (docking, landing, gate, by target), the hyperdrive autopilot, the route, the follow
    programs ("NO ROUTE AUTOPILOT IN THE NAV COMPUTER").
  - Stock fits carry everything, so nothing changes for anyone flying one.
- **More modules to choose from:** a 14 MW plant, a lighter drive, a 15 t tank, 10 t racks, a basic
  nav computer (docking and landing only).
- Test: refused undocked; smaller racks lighten the ship and shrink the hold, at the right price;
  the gun out doesn't fire; the plant can't go; a basic nav computer runs no route; the fit
  survives a save and load.
