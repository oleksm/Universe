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
