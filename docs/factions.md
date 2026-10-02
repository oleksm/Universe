# Factions

The powers of the world: who holds which systems, whose law runs there, who defends it. Factions
are part of the world (seeded content, `content/base/factions.ron`), not Dogma: anyone can found
one, and nothing is held for good, preset factions included. (See `roadmap.md` §3.)

## The world's factions (base)

| Tag | Faction | What it is | Fair game for | Enemy at |
|---|---|---|---|---|
| HCD | Halden Concord | The founding worlds' union: strict law, open markets. Holds the home system. | 15 min | -40 |
| FRA | Free Reach Assembly | Frontier settlements governing themselves: light law, cheap licences. | 5 min | -70 |
| TSD | Tessaly Directorate | An industrial directorate: order, tariffs, its own yards. | 10 min | -50 |

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
2. ✓ **Standing** (`sim/src/standing.rs`): each faction's view of every pilot (the player,
   settlers, pirates alike), -100 to +100, from the deeds done in its space as they reach its
   seat's station over the hypernet (a deed nobody's relay saw never counts):
   opening fire on the innocent -10, destroying the innocent -30, destroying the fair game +5,
   a trade at its market +0.2. Words: HOSTILE (-50), UNWELCOME (-10), NEUTRAL, FRIENDLY (+10),
   HONOURED (+50). Shown: STAND in the instruments (with the holder of the space you're in), and
   with each faction on the galaxy map's FACTIONS layer. Not yet: standing fading with time,
   work done for a faction, deeds against its ships outside its space.
3. ✓ **Faction law and security:** each faction's law (`factions.ron`): how long one who opens
   fire on the innocent stays fair game in its space (`aggression`), and the standing at or below
   which it treats a pilot as an enemy (`hostile`). **Unclaimed space has no law:** firing there
   makes no one fair game. In its space the holder's turrets fire on the fair game and on its
   enemies; its docks refuse its enemies ("REFUSED - <FACTION> TREATS YOU AS AN ENEMY"). A ship
   lost pays its debt where it comes back: no longer an enemy there (no friend either). The HUD
   warns "ENEMY OF THE <FACTION> - ITS GUNS FIRE, ITS DOCKS REFUSE". Not yet: patrols, bans by
   faction, crimes known only where heard (the law still rules in a system at once).
4. ✓ **Enlisting:** ENLIST (Z, docked at a station) swears you to its holder (+10); docked at one
   of its stations, the same key leaves it (-5). Not while it finds you UNWELCOME or worse, nor
   while sworn to another. Settlers swear to the holder of the first station they stop at
   (their own call, as clients; pirates never do). A deed against a member counts with its
   faction wherever the faction hears of it, not only in its space. Members' transponders carry
   the tag ("SETTLER 90 [HCD]"); your STAND row says MEMBER. Saved with the game: your oath and
   standings. Not yet: ranks, duties and pay; a faction's members defending each other.
5. **Founding and claiming** (`sim/src/realm.rs`: the realm, territory as live world state):
   - ✓ **Founding** (Shift+Z docked at a station, a name typed): the charter, 250,000 CR, paid to
     the station's market; you're sworn to it (+10). Its tag from its name, its colour from it
     too; default law (10 min fair game, enemy at -50). Not while sworn to another.
   - ✓ **Claiming** (Shift+Z in flight, sworn to a faction, in an unclaimed system): a claim
     beacon planted where you are (the beacon relay and a 50,000 CR fee): the system is your
     faction's. The beacon is a hypernet relay there (the backbone where there's no station).
   - Founded factions and claims are saved with the game.
   - Next: buying and taking held places, and assets (stations, beacons, fleets) that can be
     destroyed.
