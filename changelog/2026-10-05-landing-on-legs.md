# Landing on legs

The registry's design (SFO 15, the MC-07's `design`): legs built for a 3.05 m/s sink, failing near
8; landing empty. Now the game's:

- **A hull's legs are its parts'** (`world::legs`): each strut a column of its stock's tube, taking
  what its material yields at or buckles under (Euler, pinned ends), whichever is less; the legs
  together stopping the ship over the shortest strut's stroke, at its strut efficiency. The same
  reckoning as the registry's structure report.
- **What they take is worked out at each touchdown,** for the ship as it is (its mass, cargo and
  all) on the ground it sets down on: v = √(2 (F − m g) s η / m). Faster than that, measured as
  the sink into the surface, **a leg gives way** and the ship is wrecked. Faster than its design but
  under that, it lands **hard**, and the jolt aboard is told (HARD LANDING: 6.2 M/S DOWN, 0.8 G).
- **The MC-07:** empty, its legs take 7.3 to 10 m/s (a 1.1 to 2 g jolt), by the world. With its
  1,200 t hold full, they can't stand at Trethi or the four heavy worlds, and take only 1.5 to
  2.8 m/s on the five light ones: it lands empty, as its record says.
- Hulls without legs in the registry (the five outdated stock hulls) land as before (the pad's
  30 m/s, the deck's 10).
- The landing test now also sets an MC-07 down: under its design, hard, and past its legs.

## Shock: what a jolt breaks

- **Every stock item and part carries its shock limit** (SFO 15: the hardest jolt it takes while
  carried; 9 of 35 mill stock items and 52 parts say, the rest none: they take any jolt).
- **A hard landing's jolt breaks what in the hold takes less:** it's written off, the hold weighs
  what's left, and the pilot is told (BROKEN IN THE JOLT: 2 OF ...). NPC ships alike.
- Even the hardest landing an MC-07's legs survive puts about 2 g aboard, and the lowest limit is
  10 g: the legs keep the cargo whole. Harder jolts (a collision's) are next.
