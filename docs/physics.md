# Physics: what's real, what's simplified, what's invented

Physical accuracy is the game's heart. Every feature is checked against this page: it uses the
real physics below, or one of the few invented devices under their written rules, and nothing
else. Travel, information, exploration and the economy all rest on it, so nothing is made up to
dodge a design problem.

## Real

| Area | What we do | How faithful |
|---|---|---|
| Flight | Newtonian motion; thrust from placed nozzles; torque, inertia, centre of mass; trim and balance | real |
| Propellant | mass falls as fuel burns, at an exhaust velocity | real (rocket equation) |
| Orbits | every body on a Kepler orbit about its parent: planets about the star; moons, stations and gates about theirs | real (exact two-body) |
| Gravity on ships | every massive body pulls on every ship | real |
| Contact | landing speeds, collisions, ground and sea; the terrain's heights are what you land on | real (materials simplified) |
| Air and heat | aerodynamic heating of the skin (Sutton–Graves), radiation away (Stefan–Boltzmann) | real, coarse |
| Light | starlight falling with distance; shadows, eclipses, planetshine | real |
| Sensing | radar and sight at light speed, limited by range | real |
| Time | one world clock at 1× for everyone | real |
| Aboard | no artificial gravity: magnetic boots aboard; station decks weightless (only their slow turn) | real |

## Simplified (approximations of the real, to refine)

- **Bodies on rails:** they don't pull on each other. A planet feels only its star; the star doesn't
  wobble; moons don't move their planet.
- **Small bodies have no gravity:** stations, gates and asteroids.
- **Atmospheres** are a height band with a density profile, not a full air model.
- **Heat** is the skin in air only: no sunlight, no cooling at night, no temperature of planets.
- **Stars don't move** across the galaxy.

## Planned (making the simplified real)

1. **Heat as an energy balance** (next):
   - **The ship's temperature, shown on the HUD.** In: sunlight on the lit side (distance, eclipses,
     shadows), a planet's reflected light and infrared, waste heat from the power plant and
     engines, air friction. Out: radiation at T⁴, exchange with the air.
   - **Planets' surface temperature field.** Equilibrium from starlight, distance and albedo; a
     greenhouse boost from the atmosphere; a day/night swing set by the air (thin air, large
     swings; thick, small); colder toward the poles and with height.
   - **Air temperature** is what a ship flies through and a walker feels.
2. **Barycentres:** the star moves about the system's centre of mass; a planet and its moons about
   theirs. Still exact (closed form).
3. **Small bodies' gravity:** stations, gates and asteroids pull too (tiny, and it matters for
   orbiting a rock).
4. **Full n-body later**, when player actions can change orbits (moving asteroids, terraforming).
   Its costs: no closed form (saves and late joiners replay or snapshot; every client integrates
   identically) and long-run chaos (systems must be generated stable).

## Invented: the hyper layer

Exactly two devices exist beyond known physics: the **hyperdrive** and **gates**, plus whatever is
built on the same medium (hypernet relays). They share one set of rules, and they're the only
exceptions.

### The medium (proposed)

- Space carries a **hyper-medium**. Its local speed limit is
  `v_h(x) = K × d(x)`, capped at `V_MAX`, where `d` is the distance to the nearest massive body's
  surface. It's slow deep in gravity wells, fast in open space. Near a body, `v_h` falls to nothing:
  masses block it.
- The **interlock**: nothing moves in it within `INTERLOCK` of a body's highest ground.
- Today `K = 2 /s`, `INTERLOCK = 1 km`, and there is no cap.

### Hyperdrive (proposed rules)

- A ship in the medium moves at up to `v_h` (times its throttle), relative to the dominant
  body's frame; dropping out keeps a chosen exit velocity.
- **It costs energy:** fuel (or power) per unit distance and mass, so range is limited, and far
  systems need fuel depots (exploration's chain: beacon, relay, fuel depot, and so on).
- **`V_MAX` sets interstellar travel.** Within a system nothing reaches the cap (at 1 AU from a star,
  `v_h` is about 1,000 c), so in-system trips stay seconds to minutes. Between stars the cap rules:

| V_MAX | 40 ly (a near neighbour) | 1,000 ly | Across the galaxy (19,000 ly) |
|---|---|---|---|
| 10⁴ c | 35 h | 36 days | about 2 years |
| 10⁵ c | 3.5 h | 3.7 days | 70 days |
| 10⁶ c | 21 min | 9 h | 7 days |

  Gates make the settled lanes fast (seconds), so beyond the gates the frontier is far but always
  reachable: the galaxy stays open.

### Gates (proposed rules)

- A gate pair is one short wormhole: matter and hyper-signals entering one ring leave the other
  after `TRANSIT_TIME` (10 s), within the ring's size and under its speed limit.
- **Made as a pair, at one place;** one ring is then hauled through the medium to its far end. Laying
  a lane is an expedition, and it takes time and fuel by the rules above. That's how players extend
  the network.

### Hyper-signals: the hypernet's carrier (proposed rules)

- Relays signal through the medium: a signal's time is the path integral of `1/v_h` along its way
  (fast in open space, slow out of wells, blocked by bodies), capped by `V_MAX`.
- **Strength falls with distance** (inverse square), so each relay's power and antenna set its
  reach. A message beyond reach needs another relay in between, a gate relay, or a ship to carry it.
- **Capture stays real:** events are sensed by light and radar at light speed, within range. Only
  carrying uses the medium.

### What doesn't exist

- No faster-than-light sensing (beyond relays' hyper-signals); no teleporting; no artificial
  gravity; no shields; no free energy.
- A new feature uses the above, or comes here first as a written rule with its costs.

## Open decisions

1. `V_MAX`: from the table above.
2. Hyperdrive energy: fuel per distance and mass, or power drawn at speed?
3. Hyper-signal reach: how strong relays are by brand and size.
