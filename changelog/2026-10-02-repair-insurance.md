# Repairs and insurance

- **Repairs** (`Universe::repair`): docked at a station, the hull is mended for `REPAIR_PRICE`
  (30%) of the frame's price for a whole hull, using `REPAIR_METALS` (5%) of the frame's mass in
  the station's metals.
  - It mends as far as the credits and the station's metals go: half a hull with the credits
    for a fifth mends a fifth.
  - The player's ship is repaired on docking at a station, if damaged: "HULL REPAIRED TO 70%
    FOR 9000 CR".
  - Any NPC mends its hull at a station stop, as it refuels (player parity: `Request::Repair`).
- **Insurance:** a lost ship comes back as the same hull and fit, with a full tank, 4 km behind
  the home station (`World::respawn_at` keeps the hull and fit).
  - The player pays an excess of `INSURANCE_EXCESS` (10%) of the ship's value (frame and modules
    at list prices).
  - Unable to pay it, they get the basic ship: the starting hull, stock fit.
  - Either way the cockpit says so ("INSURED: ...").
  - NPCs' operator stands its own losses.
- **Messages for the new events:** refitted, new ship, repaired, insured.
- Test: a partial repair limited by credits, then a full one; an interceptor lost comes back an
  interceptor for its excess; lost again with no credits, the basic ship.
