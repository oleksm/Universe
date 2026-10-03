# TAB: the ship or watching; the ORE CUTTER by default

- TAB switches between the chase view (the ship, from behind) and watching; the cockpit view (the
  ship hidden) is gone (saves made in it open in the chase view).
- A new game starts in the ORE CUTTER (`assets/models/miner.glb`) when its file is there;
  `UNIVERSE_HULL` picks another.
- **Watching orbits the ship in its own frame** (yaw about its up, pitch over its top): it was
  the system's axes, so on a station's tilted, turning deck (or flying at any attitude) the ship
  looked upside down and dragging didn't go round it. Bodies and stars keep the system's frame.
- **The chase camera trails into turns:** behind on a soft arm, its turn eased toward the
  ship's (a third of a second to catch up), so the ship swings through the frame instead of the
  view turning rigidly with it; past a quarter-circle it snaps back behind.
- Dev scenario `watch` (docked, watching our ship).
