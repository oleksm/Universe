# Physics: what's real, what's simplified, what's invented

> **The numbers live in the physics sheet**, `config/physics.ron`: every constant with its unit,
> its kind (real, grounded, simplified, invented, tuning, planned) and its reason. The build makes
> the code's constants from it; `crates/world/tests/dogma.rs` checks the claims below against it;
> `docs/physics-sheet.md` is its generated report (`cargo run -p universe-world --example
> physics_sheet`). Change the dogma there, and the checks say what it breaks.

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

### Energy: fuel, reactors, capacitors, consumers

Everything runs on energy, accounted in joules and watts, through one chain:

    fuel  →  reactor (or engine)  →  capacitor bank  →  consumers (hyperdrive, shields, systems)
                    ↘ waste heat → radiators

**Fuels** are real substances, at their real densities and energies:

| Fuel | Energy | Density | Burnt in | Made from |
|---|---|---|---|---|
| Kerolox (kerosene + O₂) | about 1×10⁷ J/kg (exhaust 3.0–3.4 km/s) | about 1,030 kg/m³ | chemical engines | carbon-rich worlds |
| Methalox (methane + O₂) | about 1×10⁷ J/kg (exhaust 3.3–3.7 km/s) | about 830 kg/m³ | chemical engines | ice and CO₂: almost anywhere |
| Hydrolox (H₂ + O₂) | about 1.3×10⁷ J/kg (exhaust 4.4–4.5 km/s) | about 320 kg/m³ | chemical engines | water, by electrolysis (energy) |
| Hydrogen (nuclear thermal) | exhaust about 9 km/s | 71 kg/m³ | nuclear thermal engines | water, gas giants |
| Uranium-235 | 8.2×10¹³ J/kg | 19,000 kg/m³ | fission reactors | uranium ores (enrichment: energy and plant) |
| Deuterium–tritium | 3.4×10¹⁴ J/kg | about 200 kg/m³ | fusion reactors (wearing: neutrons) | D from water (1 in 6,400 of H: separation); T bred from lithium |
| Deuterium–helium-3 | 3.5×10¹⁴ J/kg | about 100 kg/m³ | fusion reactors, fusion torches | He-3 from gas giants and some regolith: rare |

- **Engines** burn their own fuel: chemical engines at their exhaust velocity; the fusion torch
  (today's main drive, exhaust up to 10,000 km/s) turns its D–He3 into jet power at an efficiency,
  the rest heat.
- **Reactors** turn fuel into power: a rate (W), an efficiency, a fuel burn (g/s), mass. Their key
  figure is **power per kg** (today's best: 2.8 kW/kg, before radiators).
- **Capacitor banks** store energy: capacity (J), charge and discharge rates (W). They give bursts,
  charged slowly by a reactor or a station's grid and spent fast. Optimistic storage holds 10⁶–10⁷
  J/kg, so bursts, never long hauls.
- **Radiators** shed the waste heat at σ·ε·area·T⁴. Their area and mass are what really cap a ship's
  power (see Heat).
- **Prices come from production:** what it takes to extract, separate, enrich and haul each fuel.
- **Physics, not production, is dogma.** This page fixes what matter and energy are (a fuel's energy,
  density and abundance; what a device can do). Who makes what, where, and with which machines is
  not dogma: the seeded world is one reasonable starting state, and anyone (a player, a corporation,
  a faction) can collect any matter, build any plant, reactor or engine the physics allows, and run
  it however they want.
- **The seeded world, for now:** ships burn deuterium (catalysed D–D fusion, 3.45×10¹⁴ J/kg, liquid
  at 163 kg/m³). It's made where there's water (a tonne in about 31,000 t of sea or ice), and stations
  buy it in. Helium-3 is rare (gas giants' air holds about 1e-5 of it, regolith about 1e-8), but
  nothing stops anyone skimming it and building engines and generators to burn it.

### Hyperdrive: energy is the wall

A ship of mass `m` (everything aboard: hull, payload, crew, supplies, fuel) held in the field
at speed `v` draws power

    P = m · ( p_floor · s(d) + p_push · (v / v*)³ ) / η_field
    s(d) = min(1, (K · d / v_open)²)

- **`s(d)`, the slack:** the medium is stiff near masses and slack far from them. In a system
  `s ≈ 0` (at 1 AU, 10⁻⁶): the hyperdrive is as cheap as today. Between stars `s = 1`.
- **`p_floor` = 10 kW/kg:** holding the field at all in open space, per kg aboard.
- **`p_push` = 5 kW/kg at `v*` = 1,000 c:** pushing through, growing with the cube of speed (energy
  per distance with its square). No speed cap: speed is bought.
- **`η_field`** (0.5–0.8 by brand and wear): the field's practical efficiency; the rest is heat.
- `K = 2 /s`, `v_open = 10⁶ c` (the slack begins a few hundred AU out).

And three hard rules:

1. **The field forms only where the medium is stiff** (inside a system: `s` below 0.01). Between
   stars it must be held the whole way.
2. **If its power fails in open space, it collapses**, and the ship is stranded in deep space,
   beyond help but a rescue expedition's.
3. **Capacitors can't carry a crossing:** 10 kW/kg for days is 10⁹–10¹⁰ J per kg of ship, a
   thousand times what storage holds. The ship's own reactors must hold the field all the way.

So the wall is **power per kg, sustained**:

| Ship | Power per kg aboard | Between stars |
|---|---|---|
| Today's best (a Sprint, 14 MW in about 30 t) | about 0.5 kW/kg | impossible: below the floor |
| A ship that's all reactor (today's best plant) | 2.8 kW/kg | impossible |
| A future explorer (next-generation reactors, 30 kW/kg aboard) | 30 kW/kg | 1.4 v* (at η 0.8): 5 ly in about 1.3 days; 40 ly in about 10 days, held without a fault |

- **No ship today can cross even to a star 5 ly away.** Explorers come later, built around rare,
  costly reactors and huge radiators, mostly power plant with a sliver of payload. 40 ly is an epic:
  days of holding the field, where a reactor trip or a radiator leak means stranding.
- **Fuel quantity is not the wall** (fusion fuel is dense: an explorer burns kilograms on the way);
  **power density and heat are.** Fuel's rarity (He-3) and its production decide who can build and
  run such ships, and power the gates.
- **Time costs too:** crew eat, breathe and need power the whole way.

### Shields (to come: a rule to write)

Not yet. When they come, the proposal: a deflector on the same medium, a thin shell of field that
resists matter passing through. Its draw grows with the energy it stops, so it drains capacitors
under fire.

### Gates: justified, and limited

A gate pair is one wormhole throat: matter and hyper-signals entering one ring leave the other
after `TRANSIT_TIME` (10 s), within the ring's size and under its speed limit. A pair spans a
distance `S` between its rings.

- **Holding the throat open** takes continuous power, growing steeply with the span:

      P_gate = P₀ · (S / S₀)³,   P₀ = 1 GW, S₀ = 10 ly

  10 ly: 1 GW; 25 ly: 16 GW; 50 ly: 125 GW. Lost power closes the lane, and the
  powerplant is infrastructure that wears and breaks (maintenance).
- **A ring's class sets its greatest span** (its throat's strength): class I 10 ly, II 25 ly,
  III 50 ly (rarer, dearer, hungrier). Nothing bridges farther. Some stars may never be worth
  bridging: the power grows with the cube of the span (5 ly: 125 MW; 40 ly: 64 GW; 100 ly: 1 TW).
- **A transit costs** `E = τ · m · S`, with `τ` 2.6×10⁻⁹ J/(kg·m): 1,000 t across 40 ly costs about
  10¹⁵ J, roughly 100 kg of fuel: 10⁹ J per kg, against an explorer's 2×10¹⁰ J per kg held across
  40 ly (about 20 times cheaper). More: any ship can use it (no explorer's power needed), in 10 s
  instead of days, with no risk of stranding. That's why lanes carry
  trade, and why the gate's owner charges fees.
- **Laying a lane:** the pair is built together at one place. One ring (thousands of tonnes) is then
  hauled to the far end through the medium by an explorer's power, under the rules above: an
  expedition in itself. It's a corporation's or faction's project, not a pilot's afternoon.
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

- `p_floor`, `p_push`, `v*`, `v_open`, the push's exponent (3), `P₀`, `τ` and ring classes: a first
  cut, chosen so no ship today crosses even 5 ly, a future explorer makes 40 ly an epic, and gates
  are justified and limited.
- **The galaxy's density:** ours has 40,000 stars across 19,000 ly (median neighbour 41 ly; the Sun's
  are about 4–5 ly). A real-density galaxy, generated by region, is on the roadmap.
- How expensive life support is per crew-day (food, water, air, power).
