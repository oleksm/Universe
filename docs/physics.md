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
built on the same medium (hypernet relays). They share one set of rules, below, and they're the
only exceptions. The rules are chosen so the game's shape falls out of them: travel within a
system is quick and cheap; between stars it's an expedition; gates are justified, and limited.

### The medium

Space carries a **hyper-medium**. Anything moving through it (a drive's field, a signal) is held
under a local speed limit:

    v_lim(x) = min(K · d(x), V_MAX)

- `d`: distance to the nearest massive body's surface. Slow deep in wells, fast in open space,
  zero at a surface: bodies block it.
- `K = 2 /s`; `V_MAX = 10⁵ c`. In a system the well rules (at 1 AU from a star the limit is about
  1,000 c); between stars, the cap.
- The **interlock**: nothing moves in it within 1 km of a body's highest ground.

### Hyperdrive: speed and energy

A ship of mass `m` held in the field at speed `v` draws power

    P(v) = m · (q + c_h · v²)

- `q`: the field's upkeep (holding the ship in the medium at all), per kg.
- `c_h · v²`: pushing through it.

Over a distance `L` at steady speed, the energy is `E = P · L / v = m · L · (q/v + c_h · v)`.
That has a **best speed**, `v* = √(q / c_h)`: slower wastes energy on upkeep, faster on the push.
At `v*` the cost is `2 m L √(q c_h)`, and at `k` times `v*` it's `(k + 1/k)/2` times that.

Energy comes from fuel (energy density `e`, times the drive's efficiency `η`, by brand). The
fuel burnt is part of the mass carried, so like a rocket the range is exponential:

    L_max = Λ · ln(m_full / m_empty),    Λ = η · e / (2 √(q c_h))

`Λ` is the drive's **range scale**: each `Λ` travelled costs a factor e (2.72) in mass.

| Constant | Value | Meaning |
|---|---|---|
| `e` | 1×10¹³ J/kg | usable energy in fuel (fusion) |
| `η` | 0.6–1.2 | a drive's efficiency, by brand and grade |
| `v*` | 10⁴ c | best speed (at η = 1) |
| `Λ` | 20 ly (at η = 1) | range scale |
| `q` | 7.9×10⁷ W/kg | upkeep (from `v*` and `Λ`) |
| `c_h` | 8.8×10⁻¹⁸ /s | push (from `v*` and `Λ`) |

What it means:

- **Within a system:** the well holds you well under `v*`, and the upkeep per e-fold of distance is
  `q/K`: about 0.4 kg of fuel per e-fold for a 90 t ship, 3–5 kg planet to planet. Quick and cheap,
  as today.
- **To the nearest stars (about 40 ly):** at `v*` it takes 35 hours, and needs `m_full/m_empty = e²`,
  about 7.4. A ship must be 86% fuel to get there, with none to come back.

| Ship | Empty | Fuel | Range (one way) |
|---|---|---|---|
| A Drover (stock) | 60 t | 30 t | 8 ly: can't reach a neighbour |
| An expedition ship | 200 t | 1,300 t | 40 ly: there, not back |
| The same, to 100 ly | 200 t | 24,000 t | not practical: needs depots |

- **So expeditions** go one way, carrying a mobile outpost that makes fuel from local matter (ice,
  gas) to get home, or settle. If the outpost fails, they're stranded. Going farther takes depots
  laid stage by stage. Going faster than `v*` burns range: at 3×, a third less.
- **Time costs too:** crew eat, breathe and need power the whole way (life support per person per
  day). A slow, cheap crossing needs more supplies.

### Gates: justified, and limited

A gate pair is one wormhole throat: matter and hyper-signals entering one ring leave the other
after `TRANSIT_TIME` (10 s), within the ring's size and under its speed limit. A pair spans a
distance `S` between its rings.

- **Holding the throat open** takes continuous power, growing steeply with the span:

      P_gate = P₀ · (S / S₀)³,   P₀ = 1 GW, S₀ = 10 ly

  10 ly: 1 GW; 25 ly: 16 GW; 50 ly: 125 GW. Lost power closes the lane, and the
  powerplant is infrastructure that wears and breaks (maintenance).
- **A ring's class sets its greatest span** (its throat's strength): class I 25 ly, II 50 ly,
  III 100 ly (rarer, dearer, hungrier). Nothing bridges farther.
- **A transit costs** `E = τ · m · S`, with `τ` 2.6×10⁻⁹ J/(kg·m): 1,000 t across 40 ly costs about
  10¹⁵ J, roughly 100 kg of fuel. That's 20,000 times cheaper than hyperdrive. That's why lanes carry
  trade, and why the gate's owner charges fees.
- **Laying a lane:** the pair is built together at one place. One ring (thousands of tonnes) is then
  hauled to the far end through the medium, under the range law above: depots on the way, an
  expedition's worth of fuel. It's a corporation's or faction's project, not a pilot's afternoon.
- So the galaxy is open (anything's reachable by expedition) but **lanes are earned**. Bridging far
  takes chains of rings and their power, each one built, fuelled and defended.

### Hyper-signals: the hypernet's carrier

- A relay's signal travels the medium under the same limit: its time is `∫ ds / v_lim` along the way.
  It's quick in open space, slow climbing out of wells, and blocked by bodies.
- **Its strength falls with the square of the distance:** a relay sending `P_tx` reaches a receiver of
  sensitivity `p_min` within `R = √(P_tx · G / p_min)` (`G`: the antennas' gain, by brand and size).
  So reach is bought with power and size, and the far frontier needs relays laid out to it.
- Between stars a signal pays the same steep reach. A **gate relay module** sends it through the
  throat instead (at `TRANSIT_TIME`). The network's backbone is its gates.
- **Capture stays real:** events are sensed by light and radar at light speed, within range. Only
  carrying uses the medium.

### What doesn't exist

- No faster-than-light sensing (beyond relays' hyper-signals); no teleporting; no artificial
  gravity; no shields; no free energy.
- A new feature uses the above, or comes here first as a written rule with its costs.

## To tune (playtest)

- `Λ`, `v*`, `V_MAX`, `P₀`, `τ` and ring classes: the numbers above are a first cut, chosen so the
  stock ships stay local and the nearest stars need an expedition.
- How expensive life support is per crew-day (food, water, air, power).
