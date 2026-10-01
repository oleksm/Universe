# Asteroids (mining, part 1: the world)

Asteroid fields, placed where physics puts them, in every star system:

- **Main belt** families between the first gas giant's 4:1 and 2:1 resonances, with the
  Kirkwood gaps (3:1, 5:2, 7:3) empty; with no giant, around the frost line. Stony and metallic
  inside, carbonaceous outside.
- **Trojan** groups 60° ahead of and behind giants (L4/L5): carbonaceous or icy.
- **Icy outer belt** families past the outermost giant, from its 3:2 resonance.

A field is a collisional family's largest remnant (1–12 km, a body of the system, named after
it; it pulls, faintly) and a swarm of 120–260 fragments bound to it: Kepler orbits around the
remnant, inside its Hill sphere, of hours to days, at centimetres to a metre a second. One
class per family (C, S, M, icy) with composition (water, organics, silicates, nickel-iron,
volatiles, PGM ppm) varying by family grade and rock; sizes on a power law from 15 m boulders;
big ones rubble piles (spinning no faster than 2.2 h), small ones monoliths (minutes);
elongated, lumpy shapes that are also their collision surface.

Flying among a field: its swarm joins the bodies the ship moves among (generated on first
approach, so nobody else pays for it), with fine substeps near small bodies. Hitting a rock is
a collision: a bounce, and the energy lost goes into the hull (3 m/s scrapes it, 40 m/s
wrecks the ship). The **anchor** holds a ship to a rock within 30 m of its hull if it drifts
with the surface there (under 0.5 m/s): it then rides the rock's orbit and spin; let go, it
drifts with that surface.

## Seeing them, getting there

- Every rock is drawn with its own faceted mesh built from its shape — the same surface it
  collides with. Remnants show from across the system; a field's swarm within 300 km of it.
  Rocks too small to make out are marked by small diamonds in their colour, fading out to
  60 km, so the swarm reads at a glance.
- Fields are nav targets: listed on the nav map by class (S-TYPE, M-TYPE, ICY…) with their
  distance, and marked on its chart between the planets' rings. The hyperdrive autopilot
  takes you there and drops out just outside the swarm, moving with it (13 s from the home
  station to a family 1.3 AU out). Keep-at and orbit (N, U) work round a remnant.
- Fixed: the own ship (drawn over the HUD in chase view) no longer covers the nav map or
  the market screen.

Next: digging, ore and markets.
