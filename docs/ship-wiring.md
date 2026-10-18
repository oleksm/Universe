# Ship wiring

The studio wires every design itself (`crates/game/src/interior/wiring.rs`), from the
registry's cables (`standards/SFO/metadata/runs/`, `template.wire` and
`template.feeder`), connectors and converters. The harness weighs into the design's
mass, its budget, the report and the test drive. The studio's WIRING layer draws it.

## Buses

| Bus | Loads | Cable |
|---|---|---|
| 28 V DC | up to 1 kW: avionics, sensors, small systems | hook-up wire |
| 270 V DC | up to 30 kW: command, life support, pumps | hook-up wire |
| 3 kV DC | above 30 kW: fans, drives, hyperdrives; every source feeds it | 3 kV feeders, copper or aluminium |

A low bus draws from the nearest converter to its voltage
(`equipment.power.converter.*`). The report names what to fit when there is none, or
too little.

## Routes

- The hub is the switchgear if there is some, else the biggest source.
- Each source and converter is fed from the hub, and each load from the hub or its
  converter, along the frame's members (the shortest way). Where the frame doesn't
  reach, a run goes straight.
- Power-bus runs are laid biggest first, and each goes round members that already
  carry feeders, so the feeders spread.

## Sizing

Each run gets the fewest parallel pairs (at most four), and of those the lightest
cable, that:

- carries its current in vacuum, derated for its bundle (runs of its bus sharing a
  member). A run to something that only works in air (a fan) carries nothing in
  vacuum. A source's run carries no more than everything that works in vacuum draws.
- carries its current in air (the registry's `current_air`, or `current_air_bundle`
  in a bundle).
- loses no more than 5% of its voltage, out and back, by its conductor's
  conductivity.

Mass: both poles' length, and a connector at each end (one shell per pair on the low
buses, one per pole on the power bus). Runs are bundled only along members: at a
module's terminals they fan out.

Invented and to review: the bus limits (1 kW, 30 kW), the vacuum bundle derating
(`bundled`), the 5% drop limit, at most four pairs, and the spreading weight.
