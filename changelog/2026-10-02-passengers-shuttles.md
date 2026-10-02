# Passengers and shuttles

- **People waiting to leave book passage** (`Universe::bookings`).
  - Each is bound for a place that would have them (fed, with room) in the same system or one
    gate away, shared out by the room there.
  - Fare per head 40 CR, 160 one gate away, paid on arrival by the settlement fund of the place
    they settle at.
- **A passenger cabin** (new modules, in a cargo slot instead of racks) seats them:
  - PASSENGER CABIN 6, 14, 30, 200 (with their life support's power draw);
  - no cabin, no passengers.
  - A passenger counts for 100 kg; their room is the cabin's.
- **Board, land (for everyone):**
  - docked, take aboard as many of a booking as there are free seats (one destination at a
    time);
  - docked where they're bound, land them: they settle (the place's people grow), and the fares
    are paid.
  - Anywhere else, refused.
- **Shuttle pilots:** one slice in ten of the ships (from the settlers), Drovers with a 30-seat
  cabin.
  - They take the booking that pays most for their seats, fly there and land them.
  - With nothing booked, they go where most are waiting in the system, else on to a neighbouring
    system.
- **The passengers screen** (docked, G: PASSENGERS):
  - the passage booked from here (where to, how many, the fare, what your free seats would
    earn);
  - landing those aboard if this is where they're bound;
  - ↑/↓ pick, ENTER board or land.
- Tests: passengers book passage, board a cabin and settle where they booked for the fare (the
  ledger balanced); a shuttle takes the best booking, lands where bound, seeks out the waiting.
  Dev scenario `passengers`.
