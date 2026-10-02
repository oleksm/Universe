# Balancing a ship at the yard

- **A ship's centre of mass is rarely on its main drive's thrust line.** The tank and the hold
  aren't at the centre, and the centre of mass moves as fuel burns and cargo loads.
  - Under the flight computer, the thrusters hold the nose straight against it (the steady trim
    burn).
  - Flown manually on the mains alone, it turns (a Drover with a full tank: 0.05 rad/s after
    two seconds).
- **Trim (`world::trim`), set at a station's shipyard and kept on the ship:**
  - **Trim cells:** up to 15% of the fuel aboard pumped to cells at the hull's six ends (found
    inside the hull, out along each axis from the tanks). They move the centre of mass, and
    drain with the rest as the fuel burns.
  - **Main shares:** each main nozzle turned down to as little as 70%, swinging the centre of
    thrust sideways (for less drive). Side by side mains can't move it up or down: that's the
    cells' work.
  - The centre of mass, inertia, thrusters and authority all fly with the trim, under the flight
    computer and in manual.
  - A new hull starts untrimmed; the insurer's replacement keeps the trim.
- **Auto-balance:** the trim that puts the drive's thrust through the centre of mass for the
  load aboard. It minimises the drive's torque, then the drive given up, then the fuel moved.
  - The Drover with a full tank: 6.8 cm above and 0.9 cm left of the centre of mass to on it;
    the trim burn 0.58% to 0.
- **The shipyard's BALANCE page** (a fifth tab):
  - rows for the fore/aft, down/up and port/starboard cells and each main's share; ←/→ sets
    them (SHIFT ×5);
  - AUTO-BALANCE works the trim out; TRIM THE SHIP has it done (docked at a station; the yard's
    pumps are heard);
  - the readout: where the thrust line passes the centre of mass and the trim burn, as trimmed
    now and with this trim, with the tank full and near dry (how a trim drifts as fuel burns);
  - a bubble level, ×500: the centre of mass a cross, the thrust line's point a dot;
  - the ship from above and the side, with its trim cells (lit by what they hold), its tank and
    its centre of thrust.
- **Keys:**
  - CLEARANCE is now DOCKING, on O (as asked);
  - IMPACT WARNING is now IMPACT (still P);
  - ECONOMY moves to N;
  - MEND HULL is now OVERHAUL (O, docked).
- **The radio's voices are gone** (they didn't sound good); traffic control chirps again.
- Tests: auto-balance puts a Drover's drive through its centre of mass; balanced at the yard, it
  goes straight on its mains alone (2,000 times less turn than untrimmed). Dev scenarios
  `balance`, `balanced`.
