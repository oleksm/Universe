# Manual thrusters

- **G (THRUSTERS, in NAV mode, in flight) turns the flight computer off.** Every thruster fires
  only while you hold its key; nothing steadies the ship. G again turns it back on.
  - The stick and throttle answer nothing; the ship turns and moves only by the torque and push
    of what you fire, about its real centre of mass.
  - Going manual, the autopilots let go. Taking one up (a route, a docking, keep, orbit) turns
    the flight computer back on.
  - Refused on the ground and in the hyperdrive.
- **The numpad fires the thrusters, W the main drive.**
  - Each of the 16 numpad keys is given to the jet nearest where it sits on the ship seen from
    above, nose up: computed for any hull, designed ones too.
  - More jets than keys: the extras are unbound.
- **A small plan at the left of the screen** (not over the view): the ship from above, each
  thruster with its key, lit while it fires.
  - The NAV instruments show the manual set: G THRUSTERS, NUM FIRE A JET, W MAINS, F7 THRUSTERS
    PANEL.
  - F7 still opens the full thrusters panel.
- **In the world:** `ShipCommands::manual` (flight computer off or on) and `jets` (the bits held);
  `Ship::drive_manual` fires exactly those at full.
- Test: the stick does nothing; one nose jet alone fires and yaws the ship; let go, it keeps
  turning; the mains push it forward; back on, nothing held. Dev scenario `manual`.
