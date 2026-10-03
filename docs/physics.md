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
- **Ship against ship** is swept exactly through each tick, so no speed passes through a hull
  unseen. But within a tick each ship keeps its turn (it moves, it doesn't rotate), and contact is
  a point of one (a corner, an edge's middle) entering a convex part of the other: two edges
  crossing between those points aren't seen.

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

Space carries a **hyper-medium** that a drive's field moves through and tubes are held open in.
It has no zones and no limits of its own: what a field costs is the one law below
(*Hyperdrive*), the same everywhere. How fast a drive goes, how its avionics slow it near bodies
(a **governor**, `v ≤ k · d` with d the distance to the nearest surface) and keep it out of them
(an **interlock**) are **products' specs**, by brand, not laws.

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

### Hyperdrive: fuel is the limit

One formula, the same everywhere (no inside or outside, no zones): a ship of mass `m` (everything
aboard) held in the field burns, per metre it goes at speed `v`,

    dE/dx = m · E0 · (1 + (v / v*)²) / η

drawn straight from its tank (the fuel's energy: deuterium 3.45×10¹⁴ J/kg).

- **`E0` = 2.4×10⁻³ J/(kg·m)** (Invented). Target: a normal ship (a Drover's 30 t tank, an S2
  drive) goes about 1.5 ly on a full tank at `v*`, never the 4-7 ly to the next star; an explorer
  that's nine tenths tank, with the best drive, about 5 ly.
- **`v*` = 1,000 c** (Invented): the cost is double the slow cost there, growing with the square
  of speed (the shape of drag: borrowed, not derived). Speed is bought.
- **Top speed 3,000 c** (Invented): full throttle costs ten times the slow cost.
- **`η`** (0.6-0.75 by brand): the drive's efficiency.
- How fast a drive can go (its top speed) and its avionics' governor near bodies are products'
  specs: Kestrel drives 3,000 c, Halcyon 3,500 c; the nav computers' governor 2/s (k × the
  distance to the nearest surface). Without avionics: as fast as the throttle says, anywhere.

So: **nothing walls a ship in; fuel and money limit how far it goes.** A hop round a system costs
kilograms; leaving and coming back from a few hundred AU, about a hundred; the next star, more
than a normal ship can carry. Exploration is logistics: tankers, depots, drop tanks, ships that
are mostly tank. The numbers: `tools/experiments/hyper_fuel.py`, `docs/world/hyperspace.md`.

The interlock that keeps a drive from carrying a ship into a body is a **feature of its avionics**
(a product's spec), not a law: without one, the drive goes where it's pointed.

### Shields (to come: a rule to write)

Not yet. When they come, the proposal: a deflector on the same medium, a thin shell of field that
resists matter passing through. Its draw grows with the energy it stops, so it drains capacitors
under fire.

### Gates and relays: tubes

A gate pair (or a relay pair, a thin one) holds a **tube** of the medium open along its route: as
long as the span, millions of c inside. Crossing, opening and holding are one formula by mass
(a tube weighs by its diameter cubed); data crosses in capsules, thrown and caught at their
relays' cadence (light can't be caught reliably in the tube's unstable flow). The laws, their
targets and the numbers: **`docs/world/hyperspace.md` §3**; Dogma's `Tube` laws; the calculations
in `tools/experiments/`.

### What doesn't exist

- No faster-than-light sensing (beyond relays' hyper-signals); no teleporting; no artificial
  gravity; no shields; no free energy.
- A new feature uses the above, or comes here first as a written rule with its costs.

## To tune (playtest)

- The field's `E0`, `v*` and top speed, and the tubes' constants: a first cut from the targets in
  `docs/world/hyperspace.md`.
- How expensive life support is per crew-day (food, water, air, power).
