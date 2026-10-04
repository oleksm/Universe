# Planets: grown, not drawn

Status: **purpose and decisions agreed, design to come** (2026-10-04). This page says why and what for. How
comes next, in its own sections, once this is settled.

## Purpose

A world's surface is the record of its history. Its mountains, trenches, coasts, canyons,
craters, ice and seas are where they are because of what the world is made of, where it
orbits, and what happened to it over billions of years.

Freefall's worlds will be made the same way. Each one starts from a seed and a few physical
facts (its mass, its makeup, its star, its orbit, its spin, its moons). It is then **evolved**
under real physics, from a hot young body through cooling, a crust forming, plates or a
stagnant lid, volcanism, impacts, its air and water won or lost, its climate, and erosion,
until it is the age it is today. What is left at the end is its landscape.

We don't paint features onto a ball of noise. A river valley is there because rain fell on
high ground and ran to the sea. A mountain belt is there because two plates met. A world has
no oceans because it lost its water, and a sky is thin because its magnetic field died and the
star stripped the air away.

## Why

- **Physics comes first** (`docs/physics.md`). The ground a ship lands on is as much a part of
  the world as the orbit it flies. A surface invented to look good breaks that promise. Ours
  should hold up to anyone who knows the science.
- **What you see is what's there.** From orbit a canyon is a canyon, and when you fly down to
  it, it is still there, deeper and more detailed the closer you get. One surface serves
  everything: the view from orbit, the ground underfoot, the collision physics, and the
  economy that mines it.
- **Variety with reasons.** Every world differs because its history differs, not because a
  random number differs. A player who learns how worlds work can read a new one from orbit
  and guess where the water was, where the ore is, and where it is safe to land.
- **The surface carries the game.** Rock type, crust age, volcanism, ice and water decide
  where the resources are, where ports can stand, and what a world is worth. They come out of
  the same history as the landscape.

## What it must deliver

1. **A planet spec.** The full set of properties that define a world (bulk and interior
   composition, core, heat sources, magnetic field, atmosphere and water inventory, orbit, spin,
   tilt, tides, star, age), with the ones we seed and the ones that follow from them, and the
   real sources each rests on (NASA planetary fact sheets, published mass–radius relations and
   so on).
2. **An evolution.** The world's history, stage by stage, each stage using established science
   at the scale where it matters, each handing its results on to the next.
3. **One surface.** Height, rock and crust age, water, ice and climate over the whole world.
   It is coarse globally and refined toward the eye without ever contradicting itself. The
   renderer, the physics and the economy all read it.
4. **Made once, for everyone.** The game is one shared world: the service evolves each world
   once, keeps it, and serves the same world to every player. It is never re-made on players'
   machines.
5. **Explained outcomes.** Each world can say why it is the way it is: "plates stopped 1.2
   billion years ago", "lost its ocean when its dynamo died". That history is content the game
   can show.

## How we'll know it's right

The Solar System is the test. Given the real figures for each body, the evolution must come
out close to the real thing:

| Body | Must come out as |
|---|---|
| Earth | Plate tectonics; about 70% ocean; long mountain belts and ocean trenches; a lasting magnetic field |
| Venus | A runaway greenhouse; no oceans; resurfaced by volcanism; no plates |
| Mars | A stagnant lid; giant volcanoes; an early ocean lost; its dynamo dead early |
| Mercury, the Moon | Heavily cratered; their interiors long cold |

If the known worlds come out right, the unknown ones are believable by construction.

## What it is not

- **Not a full simulation at every scale.** No model can run billions of years at metre
  resolution. Each stage is a reduced model at its own scale, as planetary scientists use them.
  Detail at the smallest scales is refined from the evolved result, bounded by it, never
  invented against it.
- **Not certain where science isn't.** Some questions are open (why Earth has plates and Venus
  doesn't, for one). Where they are, we pick a published answer and write down which.
- **No unexplained numbers.** Every constant is real (cited), derived, or a stated invention,
  as `README.md` sets out.
- **Not a replacement for the world's other physics.** Orbits, flight and air drag stay as they
  are; the evolved world feeds them better data (its real surface, its real atmosphere).

## Decided

- **A world's type is a result, not an input.** Temperate, barren, ocean or anything else is
  what a world's history makes of it. For the settled region we evolve worlds and keep a
  collection of baked ones, choosing from it the worlds we want where we want them. Out in the
  wild, a world is simply what it turned out to be.
- **Where worlds are baked.** The few developed worlds (five to start, of some 40,000) are
  baked ahead of time, packaged with the game and built on. The rest are baked on the server,
  which works its way outward ahead of where people can reach, so a world is ready before
  anyone arrives. Either way it is the same simulation; only where and when it runs differs.
- **Civilisation follows the ground.** Ports, settlements and landing places are placed on a
  world after it is baked, on ground that suits them (the registry records them). Nothing
  already built is carried over: settled worlds are re-settled on their evolved surfaces, and
  new ports go where the land allows.

## The planet spec (draft)

What defines a world. Each property is one of three:

- **Seeded:** drawn when the system is made, from a distribution that rests on observation.
- **Derived:** follows from other properties by a stated law, at once.
- **Evolved:** comes out of the world's history (see the stages below). Never drawn.

Today a world is a handful of seeded numbers and a colour, and the colour decides its type
(`system.rs`: a "terran" colour makes a Terran world, which alone gets air). Mass and radius of
giants are drawn independently, so their densities are arbitrary; moons are all 3,000 kg/m³;
no world has a composition, core, field, water, age or heat. The spec replaces all of that.

### The star

| Property | Kind | Law or source | Today |
|---|---|---|---|
| Mass | Seeded | Class mix of the solar neighbourhood (have it) | ✓ class × U(0.85, 1.15) |
| Age | Seeded | Uniform over the disc's star-forming history, capped by the star's main-sequence life | none |
| Metallicity [Fe/H] | Seeded | Solar-neighbourhood distribution (mean about −0.1, spread about 0.2 dex) | none |
| Luminosity, temperature, radius over time | Derived | Main-sequence relations with brightening over age (the Sun was about 70% as bright at birth); M dwarfs' early flares and wind | luminosity = M^3.5, fixed |
| X-ray and UV output, wind, over time | Derived | Activity–age relations (young stars far more active): what strips air | none |

### Bulk

| Property | Kind | Law or source | Today |
|---|---|---|---|
| Formation distance | Seeded | Where it formed relative to the frost line; may differ from where it is now (migration) | equal to its orbit |
| Mass | Seeded | Planet occurrence by mass and distance (exoplanet surveys) | ✓ log-uniform by kind |
| Rock / iron / ice / gas fractions | Seeded within derived bounds | Star's Fe/Mg/Si (from metallicity); formation distance (ice beyond the frost line); mass (gas kept above a few Earth masses) | none |
| Radius | Derived | Mass–radius relations for that composition (Seager et al. 2007; Zeng et al. 2016, 2019) | drawn separately |
| Density, surface gravity, escape velocity | Derived | From mass and radius | gravity only |
| Core mass and radius | Derived | Iron fraction and the structure that holds it | none |
| Radioactive heat (U, Th, K) | Derived | Metallicity and the galaxy's chemical history at its birth, decaying with age (Turcotte & Schubert, *Geodynamics*) | none |
| Heat of formation | Derived | Accretion energy and core formation, by mass | none |
| Water and other volatiles | Seeded within derived bounds | Formation distance, late delivery by impacts | none |
| Impact history | Seeded | Crater rate over time from lunar and Martian dating (Neukum et al.), scaled by the system's debris | 4–70 craters by kind |

### Orbit, spin and neighbours

| Property | Kind | Law or source | Today |
|---|---|---|---|
| Orbit (a, e, i…) | Seeded | ✓ as now | ✓ |
| Spin at birth, tilt | Seeded | Giant impacts set both | day U(10, 60) h, tilt U(0, 30)° |
| Spin and tilt over time | Evolved | Tides from star and moons slow the spin and lock close worlds; large moons steady the tilt | moons locked; planets fixed |
| Moons | Seeded | As now, with real densities from their composition | 3,000 kg/m³ for all |
| Tidal heating | Derived | Eccentricity, distance, the body's stiffness (what melts Io) | none |

### Evolved: the world as it is

Not drawn but grown, by the stages below: the mantle's temperature and the surface regime
(moving plates, a stagnant lid, or volcanism through the crust), the magnetic field and how long
it lasts, the atmosphere's makeup and pressure, oceans and ice, climate by latitude and season,
and the surface itself: its height, rock, crust age, soil, water, ice, and the ore its history
left behind. Its type (temperate, desert, ocean, ice, hothouse, dead) is read off the result.

## The stages (outline)

Each stage is an established model at the scale where its process acts, handing fields on to the
next. The stages are coupled (climate drives erosion; the field decides what air survives; the
air decides the climate), so they step forward together through time, not one after another.

| Stage | What it settles | Science it rests on |
|---|---|---|
| Interior | Mantle and core temperatures over time; whether the core's convection runs a dynamo; the surface regime | Parameterised mantle convection (Stevenson et al. 1983; Turcotte & Schubert); dynamo scaling (Christensen 2010) |
| Air and water | What the interior breathes out; what the star strips away (less under a field); the carbon cycle that holds a climate steady; a runaway greenhouse | Outgassing models; escape (Jeans, hydrodynamic, wind); carbonate–silicate cycle (Walker et al. 1981; Kasting) |
| Tectonics | Plates and their motion; ridges whose depth follows the age of the floor; trenches, arcs, mountain belts, rifts, hotspot chains; or, on a stagnant lid, great volcanoes and lava plains | Plate kinematics; half-space cooling (seafloor depth ∝ √age); isostasy |
| Impacts | Craters by size through time, erased by how fast the world resurfaces | Crater chronologies (Neukum et al.; Hartmann) |
| Climate | Temperature, wind and rain by latitude and season; rain shadows; ice caps and glaciers | Energy-balance and circulation scalings; builds on `climate.rs` |
| Erosion | Rivers and their valleys, canyons, deltas, coastal plains and beaches, glacial valleys; sediment where it settles | Stream-power erosion and hillslope diffusion (Braun & Willett 2013, FastScape); glacial erosion |
| Refinement | Detail toward the metre, near where it is seen | Erosion re-run locally, bounded by the global result |

The order of work comes next: what each stage reads and writes, its resolution and time step,
and how the Solar System checks each one.
