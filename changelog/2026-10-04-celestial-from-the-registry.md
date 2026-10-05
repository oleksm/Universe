# The charted systems read from the registry

- **The systems, their stars, planets, moons and asteroid fields come from the registry's records**
  (`system.*`, `body.*`, `population.*`), not `content/base/celestial.ron`, which the game no longer
  loads. Each record is read whole and strictly typed: orbit, physical, bulk, interior, magnetic
  field, crust, water, surface, air, life, rock. The game takes the parts it makes. A body's parent
  and a field's rock class are followed by key; angles are turned from degrees, and a star's mass
  and light are counted in Suns.
- **The small bodies and regions** (comets, centaurs, crossing asteroids, captured moons, dwarf
  planets, the scattered disc, the far cloud, meteoroid streams) are read too, and left aside while
  their `in_game` says the game doesn't make them yet.
- **Nothing changes in play:** every record is still `seeded`, and the celestial guard holds the seed
  to all of them. What's curated or frozen is taken from its record as before.
- The registry built into the game is now 49 KB, and still decoded once at start.
