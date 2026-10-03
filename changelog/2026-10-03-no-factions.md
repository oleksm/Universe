# Factions out: law and standing stay, local

Joining, founding and claiming for factions were too close to another game's pledging and
powers. Removed:
- **Membership:** ENLIST/LEAVE (Z), sworn settlers, members' tags on the radar.
- **Founding and claims:** FOUND and CLAIM (Shift+Z), claim beacons, the charter and fees.
- **Territory:** factions holding systems, "<FACTION> SPACE" on the HUD and the system map, the
  galaxy map's FACTIONS layer (H), the faction content (`factions.ron`, `world::factions`).

Kept, now local to each settled system (one with a station or a port):
- **Law:** fire on the innocent there and you're fair game for 10 minutes (was per faction).
- **Standing** with that system's authority, from the deeds done there as they reach its station
  (−10 aggression, −30 murder, +5 bounty, +0.2 a trade); hostile at −50: its turrets fire, its
  docks refuse. Saved per system (saves keep their fit; their old faction standings are dropped).
- `docs/factions.md` is now `docs/standing.md`.
