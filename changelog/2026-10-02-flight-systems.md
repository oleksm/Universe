# Flight systems: parked ships power down

- A ship that sets down powers down: landing on the ground, a pad or a station's deck. It's
  parked. Its drives, thrusters and turn don't answer, so nothing fires and it can't turn on the
  pad.
- **To leave, power up first:** **J** (POWER UP), then SHIFT+E to lift off. Landed, J also
  powers down again. Powering down is refused in flight.
- The cockpit says "FLIGHT SYSTEMS DOWN - J TO POWER UP" on landing, and the docked panel has a
  POWER UP / POWER DOWN cell. LIFT OFF is greyed out until the systems are on.
- **NPCs, and the route autopilot, power up as they leave** (`ShipCommands::power`, the same
  command a player gives).
- A ship off the ground always has its systems on. Powered down applies only to a ship that's
  landed.
- J is pinned: every letter of POWER is taken in flight (as X is for CANCEL).
- Test: set down on a station's pad, the ship stays put and unturned with its thrusters
  commanded and no jets firing; powered up, it lifts off.
