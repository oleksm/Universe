# The player's ship is vessel 0 (audit phase 5, #6)

- `sim::vessel::Vessel` (what was `traffic::Craft`): a ship and its pilot's side of it (system,
  status, inbox, last posting, sleep). `Universe.vessels` (`Vessels`) holds every one by `ShipId`:
  the player's first, then the crafts (`vessels[id]`, `vessels.crafts()` for the NPCs alone).
  `Universe.ship`, `ship_system`, `player_status`, `player_inbox` and `crafts` are gone; the save
  format keeps its own fields and is unchanged.
- Written once now, for every ship: the step's aftermath (`after_step`: services, records, flight
  recorder, crash log), a pilot's posting (`post`), the combat phase (one events list by id, one
  pass for the law, kills and statistics), the snapshot, the pilot view, the recorder's slices,
  ground-ahead and gate frames, ship names and lookups. Commerce's `if pilot == PLAYER { … } else
  { crafts[pilot - 1] … }` are `vessels[pilot]`.
- The player/NPC drift goes with it: the player's crashes, transits, collisions, kills and route
  stops now count in the traffic statistics, and its wrecks go in the crash log, as every craft's
  did. Two things stay the player's own, said where they are: its client runs every frame (a
  posting's sleep is ignored for it), and the dead-man rule watches the crafts (the game stops the
  world when it stops).
- Frames of the deck and Heath's range as before; 98 tests pass (the replays among them).
