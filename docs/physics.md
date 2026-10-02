# Physics: what's real, what's simplified, what's invented

> **Why "Dogma".** Some of it is invented (the hyper layer), but in the game it's settled, not up for
> debate: nothing in the world bends, bargains with or designs around it. It changes only by a
> deliberate review, out of the game: edit the sheet, and the dogma checks say which promises break.
>
> **Dogma and the world.** The core engine is Dogma; the world is built on it (see
> `docs/architecture.md`). **Dogma's laws** are `config/dogma.ron` (nature's constants, the
> hyper layer's laws: `universe_physics::laws`, `universe_physics::hyper`): no materials, devices
> or designs. **The base world** (`content/base/`) brings its matter (`materials.ron`: real
> substances at real properties), its devices (`modules.ron`: a plant says what it burns and how
> well, a tank what it holds) and its fixed design numbers (`sheet.ron`). Every constant carries a
> unit, a kind and a reason; `crates/world/tests/dogma.rs` checks the claims below against both;
> `docs/physics-sheet.md` is the generated report (`cargo run -p universe-world --example
> physics_sheet`).

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

- **Bodies on rails, two by two:** each pair (a body and its parent) moves about its shared centre
  of mass (the star wobbles, a planet round its moons'), but a planet doesn't feel its neighbours:
  no n-body perturbations.
- **Atmospheres** are a height band with a density profile, not a full air model.
- **Heat** is the skin as one body (no hot and cold sides), its own power's heat going to built-in
  radiators but for a leak (radiators come as modules); worlds' climates are equilibrium-and-swing
  models, not weather.
- **Stars don't move** across the galaxy.

## Planned (making the simplified real)

1. **Heat as an energy balance** (done, coarse: `heat`, `climate`):
   - **The ship's temperature, shown on the HUD.** In: sunlight on the lit side (distance, eclipses,
     shadows), a planet's reflected light and infrared, waste heat from the power plant and
     engines, air friction. Out: radiation at T⁴, exchange with the air.
   - **Planets' surface temperature field.** Equilibrium from starlight, distance and albedo; a
     greenhouse boost from the atmosphere; a day/night swing set by the air (thin air, large
     swings; thick, small); colder toward the poles and with height.
   - **Air temperature** is what a ship flies through and a walker feels.
2. **Barycentres** (done): the star moves about the system's centre of mass; a planet and its
   moons about theirs. Still exact (closed form).
3. **Small bodies' gravity** (done): stations and gates pull too (faintly); asteroids already did.
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

### Gates and relays: tubes

A gate pair (or a relay pair, a thin one) holds a **tube** of the medium open along its route: as
long as the span, millions of c inside. Crossing, opening and holding are one formula by mass
(a tube weighs by its diameter cubed); data crosses in capsules, thrown and caught at the flow's
settle cadence (light can't be caught reliably in the tube's unstable flow). The laws, their
targets and the numbers: **`docs/world/hyperspace.md` §3**; Dogma's `Tube` laws; the calculations
in `tools/experiments/`.

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
