# Platform stations

Stations are now our own design: a **platform in orbit**. A square deck of 16 landing pads
(4 × 4, 150 m apart, like a spaceport's) has the station's **main structure** standing along one
edge. The structure is 250 m tall and holds the hangar, with its door and a band of windows facing
the pads. The spinning station with a docking slot is gone.

- **Physics:**
  - The station is built of boxes (a new `Blocks` collider: the deck slab and the structure).
  - Touching the deck's top gently (under 10 m/s) lands the ship belly-down where it touched;
    harder wrecks it.
  - Bumping the hull slowly bounces off.
  - A ship lifts off the deck along its normal on the lift thrusters (SHIFT+E), as from the ground.
  - The station turns once an orbit about its deck's normal (the orbit's), keeping the same side
    to its planet. That's about 0.001 rad/s, so there's nothing to match by hand.
- **Docking:**
  - Traffic control gives each docking ship **a pad of its own**, as at spaceports. If all 16 are
    taken it holds on a ring above the deck. Stations no longer have a one-ship-at-a-time
    corridor; gates keep theirs.
  - The docking computer flies to a point straight above its pad, around the station if it's
    below or beside it. It turns belly to the deck with its nose toward the main structure, then
    comes down slowly (from 60 m/s high up to 2 m/s near the deck).
  - From the spawn point it docks in 154 s, and the flight plan predicts the same.
  - The guide shows your pad's square on the deck and squares to come down through along its
    vertical, the next one brightest. The HUD shows the pad, height above it, offset from its line
    and descent speed. The phase banner reads APPROACH › OVER THE PAD › SET DOWN.
- **Ports in one model (`world::port`):** a spaceport and a station's deck are both ports, with
  pads, a hangar and taxiing between them. Traffic control keys pads by port. A long stay at a
  station is spent in its hangar, inside the main structure, freeing the pad, just as at
  spaceports (tested for both).
- **Old saves:** a ship saved docked at the old slot is put on the middle pad on load.
- **Messages:**
  - A pad request on its way says "PAD REQUESTED - STAND BY", not "ALL PADS TAKEN".
  - Small refuels show kilograms.
- Tests:
  - the collider (deck, hull, open air past the deck's edge, the structure's face);
  - the pads lie on the deck clear of the structure;
  - a gentle touchdown lands on pad 5 and says so, and a hard one wrecks;
  - lifting off clears the station;
  - docking from the spawn point, matched by the plan;
  - one ship leaves while another docks;
  - the hangar cycle at a station.
  Dev scenarios: `platform`, `platformdeck`.
