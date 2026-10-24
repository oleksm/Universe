# Camera, altimeter, batteries and thermal fittings made in the game

Owner (2026-10-23): settle what the Explorer needs to be buildable without fakes. Registry flipped the four kinds
(fso 694f51a6c, merged).

- `Does::Camera`, `Does::Altimeter`: sensors from their own records (field of view; range, accuracy). They fill the
  sensors base block and draw their own power (10 W, 40 W). Nothing shows a camera's picture or reads an altimeter
  yet (stated in `limits`). No radar figures on them.
- `Does::Battery`: store, rate and efficiency from its record, heat capacity (specific heat × mass) and cut-off from
  its physical record (without them: carried inert, warned). `ClassSpec::batteries` go into the ship's device state
  (`Ship::own_propellants`). The assembler refuses a fit whose draw exceeds plant output plus battery rate. Past the
  plant alone it's commissioned "on its batteries", its endurance on a full charge stated.
- Device state:
  - packs are drawn down for what the plant can't give (as before);
  - they're now recharged from the plant's spare power, its fuel burned for it;
  - a ship whose plant and packs can't carry its steady draw is dark (no thrust, no flight computer) whatever the
    switch says. Never more than the pack holds.
- `SlotKind::Thermal`: radiator, coolant-loop and exchanger slots are now fitted. They used to be dropped. Radiators
  (`Does::Radiator`) and loops are carried, their mass counted. Heat isn't simulated (stated). The MC-07's
  radiators and loops are now in its dry mass.
- `Does::carried`: one rule for what a record fits unpriced at 0 (command, gear, engines, camera, altimeter,
  batteries, radiators, thermal fittings).

Test: `import::tests::import_battery_powers_by_its_charge_then_goes_dark` (the MC-07 refitted: battery-only refused
past its rate, commissioned within it, drained running, dark when flat).
