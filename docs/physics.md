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

Space carries a **hyper-medium**. Near a mass it holds anything moving through it (a drive's
field, a signal) under a local limit:

    v_lim(x) = K · d(x)

- `d`: distance to the nearest massive body's surface. Slow deep in wells, zero at a surface:
  bodies block it. `K = 2 /s` (at 1 AU from a star the limit is about 1,000 c).
- **In open space there's no speed limit.** The only limit is energy (below).
- The **interlock**: nothing moves in it within 1 km of a body's highest ground.

### Hyperdrive: speed is bought with energy

A ship of mass `m` held in the field at speed `v` draws power

    P(v) = m · q · (1/3) · (2 + (v / v*)³)

- The first part is the field's upkeep (holding the ship in the medium at all). The second is pushing
  through it, which grows like drag: power with the cube of speed.
- Over a distance `L` the energy is `E = m · L · ε* · f(v/v*)`, where `ε* = q / v*` is the cost per kg
  per metre at the best speed `v*`, and

      f(k) = (2/k + k²) / 3       (f(1) = 1, the least)

| Speed | Energy per distance | 40 ly takes |
|---|---|---|
| ½ v* | 1.4× | 70 h |
| v* = 10⁴ c | 1× (the least) | 35 h |
| 2 v* | 1.7× | 18 h |
| 3 v* | 3.2× | 12 h |
| 10 v* | 33× | 3.5 h |
| 100 v* | 3,300× | 21 min |

Energy comes from fuel (energy density `e`, times the drive's efficiency `η`, by brand). The fuel
burnt is mass carried, so like a rocket the range is exponential:

    L_max = (Λ / f(v/v*)) · ln(m_full / m_empty),    Λ = η · e / ε*

`Λ` is the drive's **range scale** at the best speed: each `Λ` travelled costs a factor e (2.72) in
mass. Going `k` times faster shrinks it by `f(k)`.

| Constant | Value | Meaning |
|---|---|---|
| `e` | 1×10¹³ J/kg | usable energy in fuel (fusion) |
| `η` | 0.6–1.2 | a drive's efficiency, by brand and grade |
| `v*` | 10⁴ c | best speed |
| `Λ` | 20 ly (at η = 1) | range scale at the best speed |
| `ε*` | 5.3×10⁻⁵ J/(kg·m) | energy per kg per metre at `v*` |
| `q` | 1.6×10⁸ W/kg | the field's upkeep (`ε* · v*`) |

What it means:

- **Within a system:** the well holds you well under `v*`; the upkeep per e-fold of distance is
  `(2/3) q / K`, about 0.5 kg of fuel per e-fold for a 90 t ship, 3–6 kg planet to planet. Quick
  and cheap, as today.
- **To the nearest stars (about 40 ly):** at `v*` it takes 35 hours and needs `m_full/m_empty = e²`,
  about 7.4. A ship must be 86% fuel to get there, with none to come back.

| Ship | Empty | Fuel | Range at v* (one way) |
|---|---|---|---|
| A Drover (stock) | 60 t | 30 t | 8 ly: can't reach a neighbour |
| An expedition ship | 200 t | 1,300 t | 40 ly: there, not back |
| The same, to 100 ly | 200 t | 29,500 t | not practical: needs depots |

- **Speed is logistics.** Going faster is always possible, but each step costs steeply more fuel. Over a
  long way the mass ratio runs away (exponential in `f(k) · L / Λ`), so speed comes only from
  refuelling often: a lane with depots every few light years can be run fast; a lone crossing must
  creep at `v*`. **So where fuel is made and stocked decides where travel and the hypernet are
  fast.** Fuel-rich systems (gas giants, ice) and the depot chains between them become the
  galaxy's arteries.
- **Expeditions** go one way at the best speed, carrying a mobile outpost that makes fuel from local
  matter to get home, or settle. If it fails, they're stranded. Going farther takes depots laid stage by
  stage.
- **Time costs too:** crew eat, breathe and need power the whole way (life support per person per
  day). Creeping slower than `v*` saves nothing and costs supplies.

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

- A relay's signal travels the medium under the same limit near masses: its time is `∫ ds / v_lim`
  climbing out of wells (blocked by bodies). In open space it's as fast as its energy buys,
  under the same law: a stronger relay sends faster, and farther.
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

- `Λ`, `v*`, the push's exponent (3), `P₀`, `τ` and ring classes: the numbers above are a first cut,
  chosen so the stock ships stay local, the nearest stars need an expedition, and speed is
  fuel logistics.
- How expensive life support is per crew-day (food, water, air, power).
