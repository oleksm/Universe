# Factions 2: standing

- **Each faction's view of every pilot** (`sim/src/standing.rs`), from -100 to +100, kept in the
  world's tick (it will drive law next): the player, settlers and pirates alike.
- **From deeds in its own space, as heard:** a faction listens at its seat's station (the news
  machinery, `news::Knowledge`), so a deed far off counts when its news arrives, and one that no
  relay saw never does. Opening fire on the innocent -10; destroying the innocent -30; destroying
  the fair game +5; a trade at its market +0.2. (Turrets' kills don't count for anyone.)
- **Shown:** STAND in the instruments, with the holder of the space you're in ("+0 NEUTRAL HCD");
  and on the galaxy map's FACTIONS layer, beside each faction ("YOU: +0 NEUTRAL"). Words:
  HOSTILE, UNWELCOME, NEUTRAL, FRIENDLY, HONOURED.
- Trade records name their pilot by id; the hypernet's listener takes sightings (a ship opening
  fire, seen where it was) and the things to hear come as one `Happenings`.
- The pirate test checks the holder of the space hears the attack and the pirate's standing falls.
