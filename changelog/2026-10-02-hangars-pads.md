# Hangars; 4×4 pads

Ports were filling up: since settlers were given hours-long stays (for the fuel economy), they
spent them **on the landing pads**. A port had nine, every home port sat at 9/9, and arriving
ships (the player too) were left holding.

- **Hangars:** a spaceport has a hangar beside its pad grid.
  - **Moving in and out:** a landed ship can move off its pad into the hangar, or out of it onto
    a pad, with `ShipCommands::hangar` (`Enter` / `Leave { pad }`). The world checks it physically:
    in only from a pad of that port, out only onto one of its pads, which traffic control gives.
  - **Inside:** the ship stays landed, out of sight. Radar, weapons, collisions and traffic
    control's presence all leave it out, so its pad frees itself, and nothing lifts off from
    inside.
  - **Events:** `EnteredHangar`, `LeftHangar`, `HangarRefused`.
- **The route autopilot uses them:** a stay at a spaceport longer than two turnarounds starts
  with a turnaround on the pad (`TURNAROUND`, 60 s, time to trade) and is spent in the hangar.
  When the stay is up, it asks traffic control for a pad, comes out onto it, and lifts off as
  before. The same actions are open to the player (player parity); a key for it can come later.
- **4×4 pads:** 16 pads per port (`GRID`); the touchdown zone that counts as the port grew to
  400 m to take in the corner pads.
- **Measured** (1,000 settlers, 3 minutes): before, every home port held 9/9; after, the busiest
  holds 12/16. The pad holders are ships coming and going, and traders, miners and pirates
  still within their start-up wait.
- Test: a long stay goes to the hangar after the turnaround, its pad is freed, and when the stay
  is up it gets a pad, comes out and leaves.
