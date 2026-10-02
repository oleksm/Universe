# Factions 5a: founding and claiming

- **The realm** (`sim/src/realm.rs`): the factions as they stand (the world's, then any founded)
  and who holds which system, as live world state kept by the tick and shared through the view
  (no longer fixed in the charts). Claim beacons are hypernet relays in their systems.
- **Found a faction** (Shift+Z, docked at a station): type its name; the charter, 250,000 CR, goes
  to the station's market; you're sworn to it. Its tag from its name ("OPEN REACH": ORE), a colour
  from it, default law. Not while sworn to another ("LEAVE IT FIRST").
- **Claim a system** (Shift+Z in flight, sworn, in an unclaimed system): a beacon planted where you
  are (the beacon relay's price and a 50,000 CR fee); the system is your faction's: its law, the
  galaxy map's factions layer, the HUD's "<FACTION> SPACE". (Every letter is taken in flight and
  docked: one chord, Shift+Z, founds docked and claims flying.)
- **Saved:** founded factions and claims, by key; your oath and standings with founded ones too.
- The kill feed starts under the status strip (it ran into the wider button bar).
- The hull purchase test also founds a faction, claims an unsettled system, and checks both
  survive a save. Dev scenario `founding`. Input: a text prompt can swallow the frame's keys.
