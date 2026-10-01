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

## Mining

- **Prospecting.** Among a field, the prospector scans the rock under the crosshair (else the
  nearest within 50 km), bracketed in green: its class, structure, size and spin, range to
  its surface and your drift against it. Within 2 km, a close survey: its make-up (water,
  organics, silicates, nickel-iron, volatiles, PGM ppm), the ore it yields and how fast it
  digs. Near a field, speeds and prograde read against its remnant.
- **Anchoring** (Y): within 30 m of the surface and drifting with it (under 0.5 m/s) the
  anchor holds; Y again lets go.
- **Digging** (H): the excavator's 300 kW against the energy to break the rock loose —
  gravel off a rubble pile 2 kJ/kg, ice 20, carbonaceous stone 30, stone 60, solid
  nickel-iron 400 — capped at 10 kg/s it can carry off. So a rubble pile fills the 20 t hold
  in about half an hour; a solid metal rock takes 7.4 hours. Ore by class: water ice, carbonaceous
  ore, stony ore, nickel-iron ore, PGM-rich ore (M-types over 30 ppm). Into the hopper, a
  tonne at a time into the hold — the ship weighs it.
- **Depletion.** Every rock holds what its mass holds; what's been dug from each is
  remembered (saved, and in the replay hash), and a worked-out rock stays worked out.
- **Selling.** The five ores are goods (after the catalog, the same in every galaxy):
  stations buy water and ore; each mined tonne is booked in the ledger from the world's
  account, caused by the dig.
- Dev scenarios: `asteroid`, `swarm`, `prospect`, `mining`.

## Miners, and closing on a rock

- **N closes on the rock you've scanned** (no ship locked): the follow program flies you to
  12 m off its surface and turns with it as it spins, gently, on the thrusters, keeping clear
  of the field's other rocks. When the prospector says Y TO ANCHOR, anchor; H digs. (Orbit and
  keep-at hold a kilometre off: they could never bring you within the anchor's reach.) X lets
  go of any follow program — the HUD says so now.
- Fixed in the follow program: small corrections lined up with the nose were handed to the
  main engine and then dropped, so it could stall short of its range.
- **NPC miners**: another tenth of the settlers (where their home system has asteroids). Each
  works the same way a player does: its route goes out to the field best worth working (an
  asteroid is now a route stop: a work site, held till the worker is done) and back to its
  home market; there it picks one of the best rocks by ore price × dig rate, closes, anchors,
  digs until the hold is full, lets go, flies to market, asks for quotes and sells its ore,
  then heads out again. Departures stagger like everyone else's. On radar they read MINING.

## Mining mode, and locking by list

- **Locking, everywhere: T.** A tap locks what's ahead (a ship in the beam; in mining mode,
  the prospected rock nearest the nose, within 10°). Held, T lists what can be locked —
  radar contacts, or in mining mode the rocks found — nearest first, with the one under the
  cursor bracketed in the view; move the mouse (or the wheel) to choose and let go of T to
  lock it. The mouse doesn't steer while the list is up. One lock at a time: ship or rock.
- **Mining mode (1)**: its own action panel — 2 PROSPECT, T LOCK, 3 APPROACH, Y ANCHOR,
  H DIG, N KEEP, U ORBIT, X LET GO.
- **Prospect (2)**: a pulse goes out to 30 km, a shell you see grow round the ship; every
  rock it passes is found: listed (numbered, nearest first: class, size, rubble or solid,
  ore, dig rate, range) and tagged with its number in the view.
- With a rock locked: N keeps at a range from it, U orbits it, 3 approaches it (12 m off its
  surface, turning with it), then Y anchors and H digs. The prospector reads the locked rock.
  (N no longer closes on the scanned rock by itself: 3 does, on the one you locked.)

Not yet: claims, refining, towing boulders,
the excavator's power drawn from fuel.

## The prospect pulse, seen from the ship

- Fixed: the pulse was left where it was sent, in the star's frame — which the ship and the
  field race through at tens of km/s — so it showed off to one side. It goes out round the
  ship now.
- From inside a shell its lines don't appear to move, so the pulse shows by what it does: a
  burst of two rings opening round the ship out past the edges of the view, then each rock
  flashing as the shell reaches it, near ones first, out to 30 km over 3 s.
