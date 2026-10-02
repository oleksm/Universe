# Factions

The powers of the world: who holds which systems, whose law runs there, who defends it. Factions
are part of the world (seeded content, `content/base/factions.ron`), not Dogma: anyone can found
one, and nothing is held for good, preset factions included. (See `roadmap.md` §3.)

## The world's factions (base)

| Tag | Faction | What it is |
|---|---|---|
| HCD | Halden Concord | The founding worlds' union: strict law, open markets. Holds the home system. |
| FRA | Free Reach Assembly | Frontier settlements governing themselves: light law, cheap licences. |
| TSD | Tessaly Directorate | An industrial directorate: order, tariffs, its own yards. |

## Territory

- A faction **holds a system** when it owns its station, ports and gates.
- At the start (`world/src/factions.rs`): the first faction sits at the home system; each other's
  seat is a settled system as far by gates from the seats before it as can be (picked from the
  seed among the farthest); every settled system goes to the nearest seat.
- Unsettled space is **unclaimed**: no one's law.
- A faction's reach is also its **network**: its relays are its borders (`hypernet.md`).

## Build order (coarse first)

1. ✓ **Factions and territory:** content, each settled system's holder; shown on the HUD
   ("HALDEN CONCORD SPACE"), the system map's title, and as the FACTIONS layer on the galaxy
   map (held systems ringed in their colours, the holders listed).
2. **Standing:** each faction's view of every pilot, from deeds it has *heard* over the
   hypernet: attacks on its ships or in its space, trade at its markets, work for it.
3. **Faction law and security:** the holder's law (how long aggression stands, what's banned,
   docking refused to the hostile); its turrets and patrols enforce it; crimes are known where
   they're heard.
4. **Enlisting:** join a faction (an action at its station); NPC settlers belong to factions too,
   shown on their transponders.
5. **Founding and claiming:** a pilot founds a faction (a charter) and claims places; territory
   changes hands by building, buying or taking. Factions own assets (stations, fleets) that can
   be destroyed.
