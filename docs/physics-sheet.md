# The physics sheet

Generated (run `cargo run -p universe-world --example physics_sheet`): edit the sheets, not this. The charter is `docs/physics.md`; the dogma's claims are checked in `crates/world/tests/dogma.rs`.

# Dogma's laws

From `config/dogma.ron`.

## Nature

Constants of physics.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `SPEED_OF_LIGHT` | 2.99792458e8 | m/s | Real | c: light, radio, radar, and every capture of an event. |
| `STEFAN_BOLTZMANN` | 5.670374e-8 | W/(m²·K⁴) | Real | σ: how a body radiates heat: σ·ε·A·T⁴. |
| `SOLAR_LUMINOSITY` | 3.828e26 | W | Real | The Sun's power: stars' luminosities are reckoned in it (1361 W/m² at 1 AU). |

## Medium

The hyper-medium: it holds what moves in it under v_lim = K·d, d the distance from the nearest surface (slow close to bodies, fast in the open). The same everywhere: no inside, no outside.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `HYPER_RATE` | 2 | 1/s | Invented | K: the limit per metre from the nearest surface. At 1 AU from a star, about 1,000 c. |

## Field

A hyper-field's cost, drawn from the tank: dE/dx = m·E0·(1 + (v/v*)²) / η, η the device's. Anywhere: what limits a ship is the fuel it carries. See docs/world/hyperspace.md; tools/experiments/hyper_fuel.py.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `FIELD_COST` | 0.002391 | J/(kg·m) | Invented | E0: the energy per kg per metre, going slow. Target: a normal ship (a Drover's tank, an S2 drive) goes about 1.5 ly on a full tank at v*: never the 4-7 ly to the next star; an explorer that's mostly tank about 5 ly. |
| `V_BEST_C` | 1000 | c | Invented | v*: at this speed the cost is double the slow cost (it grows with the square of speed). |
| `V_TOP_C` | 3000 | c | Invented | A field's top speed. Target: full throttle costs ten times the slow cost (rushing burns a tank in a tenth of the distance). |

## Tube

A gate or relay holds a tube of the medium open along its route (as long as the span; millions of c inside). Crossing in time t: E = eps·m·S·e^(t_nat/t), t_nat = T_LY·(S/1 ly)·(m/1 kg)^GAMMA. A tube of diameter d weighs as mu(d) = RHO·(d/1 m)^K: opening it is the same formula, holding it a share of that. See docs/world/hyperspace.md; tools/experiments/.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `TUBE_T_LY` | 0.2 | s/ly | Invented | Natural crossing per light year for a 1 kg payload. Target: data crosses at 200 ms per ly; faster gets punishing. |
| `TUBE_GAMMA` | 0.3333333333 |  | Invented | Heavier is naturally slower, by the cube root of mass. Target: a 100 t ship under a minute through a 5 ly gate, a capital ship several minutes. |
| `TUBE_EPS` | 2.24e-12 | J/(kg·m) | Invented | A crossing's energy scale. Target: a 100 t ship at natural speed through a 5 ly gate costs an S2 plant-hour. |
| `TUBE_RHO` | 406 | kg | Invented | A 1 m tube's equivalent mass. Target: opening a 3 km, 5 ly gate costs a year of a 100 GW industry. |
| `TUBE_K` | 3 |  | Invented | A tube weighs by its diameter cubed. Target: opening a one-ship tube for one pass costs thousands of passes through a held gate; a gate pays at about 9 ships a day. |
| `TUBE_HOLD` | 2.592e8 | s | Invented | Holding a tube costs its opening over this (100 months: 1% a month). Target: small next to opening, big in absolute terms: a lapse is ruinous. |
| `TUBE_SETTLE` | 3 | s | Invented | After a throw a tube's flow settles this long before the next can be caught: the cadence data is batched at. Target: news crosses a relay link in 1-2 s. |
| `MAX_TRANSIT_SPEED` | 300 | m/s | Invented | A ring catches what enters it slower than this; faster and the capture wrecks it. |

# The base world's numbers

From `content/base/sheet.ron`.

## Drives

The world's drives as built today (each engine's exhaust and efficiency are its product's: modules.ron).

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `GROUND_MARGIN` | 20000 | m | Tuning | Flying along the nose at a world, a hyperdrive drops out this far above its ground (or its air's top). |

## Gates

The world's gate rings.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `GATE_RADIUS` | 1500 | m | Tuning | A ring's centreline radius; its opening is a little smaller. |
| `RING_TUBE` | 60 | m | Tuning | Half the ring structure's thickness. |
| `RELAY_TUBE` | 0.01 | m | Tuning | A hyper relay's tube: capsules only, no ship fits (a gate's is its ring's opening). |
| `GATE_CAPSULE` | 1 | kg | Tuning | A gate's data capsule: a casing and hardened storage (target: the 200 ms per ly is a 1 kg capsule's). |
| `RELAY_CAPSULE` | 0.01 | kg | Tuning | A relay's data capsule, small enough for its tube. |

## Technology

How far the world's engineering goes today.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `CAPACITOR_DENSITY` | 1e7 | J/kg | Grounded | The most a capacitor bank stores per kg (superconducting magnetic storage, optimistic; batteries about 1e6). |

## Heat

The hull's skin in air: Sutton–Graves heating in, Stefan–Boltzmann radiation out. Sunlight and night: planned.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `NOSE_RADIUS` | 2 | m | Tuning | For the stagnation heating. |
| `HEATED_AREA` | 150 | m² | Simplified | The area taking the heating at the stagnation rate (underside and leading edges, averaged). |
| `RADIATING_AREA` | 500 | m² | Simplified | The area radiating it away. |
| `EMISSIVITY` | 0.8 |  | Real | How well the skin radiates (oxidised metal). |
| `SKIN_CAPACITY` | 3e6 | J/K | Simplified | The skin's heat capacity: about 3 t of metal. |
| `SKIN_LIMIT` | 1500 | K | Real | Beyond it the hull burns (refractory alloys). |
| `AMBIENT` | 290 | K | Simplified | What the skin settles to with nothing heating it: to be replaced by the energy balance (sunlight, night). |
| `AIR_CP` | 1005 | J/(kg·K) | Real | Earth-like air's specific heat at constant pressure. |
| `HULL_ABSORPTIVITY` | 0.3 |  | Real | How much sunlight a hull's paint takes in (white paint about 0.25–0.35). |
| `INTERNAL_LEAK` | 0.01 |  | Simplified | The share of a ship's power use that reaches its skin as heat; the rest goes to its radiators (built in for now: radiators come as modules). |
| `CONVECTION` | 25 | W/(m²·K) | Simplified | Heat exchange between a hull and Earth-like air at sea level (scaling with the air's density to the half power). |

## Climate

Worlds' surface temperatures: radiative equilibrium from their star, their albedo, a greenhouse from their air; day and night swings damped by air.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `ALBEDO_TERRAN` | 0.3 |  | Real | An Earth-like world's albedo (Earth's: 0.3). |
| `ALBEDO_DRY` | 0.15 |  | Real | A dry rocky world's (Mars: 0.17). |
| `ALBEDO_CRATERED` | 0.12 |  | Real | A cratered moon's (the Moon: 0.12). |
| `ALBEDO_GIANT` | 0.5 |  | Real | A gas or ice giant's cloud tops (Jupiter: 0.5). |
| `GREENHOUSE` | 33 | K | Simplified | What Earth-like air at sea level adds to a world's mean (Earth's: 33 K); grows with the air's density to the 0.6. |
| `SWING_DAMPING` | 0.05 | kg/m³ | Simplified | Air this dense halves a world's day–night swing; Earth-like air (1.2) all but flattens it. |
| `LAPSE_RATE` | 0.0065 | K/m | Real | Air cooling with height (Earth's standard atmosphere). |
| `NIGHT_FLOOR` | 0.35 |  | Simplified | An airless world's night, as a share of its mean (the Moon: about 100 K of 270). |

## References

For the dogma's claims (not used by the code).

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `ETA_FIELD_MIN` | 0.5 |  | Planned | The field's efficiency, the worst drives. |

# The base world's materials

From `content/base/materials.ron`.

| Material | Density (kg/m³) | Energy (J/kg) | Process | Trades as | Note |
|---|---|---|---|---|---|
| DEUTERIUM | 163 | 3.45e14 | Fusion | goods.fuel | Catalysed D–D fusion (the tritium and helium-3 it makes burnt too). A tonne in about 31,000 t of water. |
| HELIUM-3 | 59 | 0 | None | - | Burnt with deuterium (D–He3, few neutrons). Rare: gas giants' air about 1e-5, regolith about 1e-8. |
| D–HE3 BLEND | 100 | 3.5e14 | Fusion | - | Deuterium and helium-3, 2:3 by mass: the cleanest fusion fuel. |
| D–T BLEND | 200 | 3.4e14 | Fusion | - | Deuterium and tritium (bred from lithium); its neutrons wear a reactor. |
| ENRICHED URANIUM | 19000 | 8.2e13 | Fission | - | U-235 fission. |
| METHALOX | 830 | 1e7 | Chemical | - | Methane and oxygen (exhaust 3.3–3.7 km/s): made from ice and CO₂ almost anywhere. |
| KEROLOX | 1030 | 1e7 | Chemical | - | Kerosene and oxygen (exhaust 3.0–3.4 km/s). |
| HYDROLOX | 320 | 1.3e7 | Chemical | - | Hydrogen and oxygen (exhaust 4.4–4.5 km/s): the best chemical fuel, bulky. |
| HYDROGEN | 71 | 0 | None | - | Reaction mass for nuclear thermal engines (exhaust about 9 km/s). |

## What it adds up to

| Claim | Figure |
|---|---|
| DROVER on a full tank (S drive, 0.6) | 3.0 ly slow, 1.5 ly at 1,000 c |
| SPRINT COURIER on a full tank (S drive, 0.6) | 1.9 ly slow, 0.9 ly at 1,000 c |
| BULK HAULER on a full tank (S drive, 0.6) | 2.9 ly slow, 1.4 ly at 1,000 c |
| PROSPECTOR on a full tank (S drive, 0.6) | 2.5 ly slow, 1.2 ly at 1,000 c |
| INTERCEPTOR on a full tank (S drive, 0.6) | 0.9 ly slow, 0.4 ly at 1,000 c |
| An explorer, nine tenths tank (0.75) | 5.1 ly at 1,000 c, 5 ly in 1.8 days |
| A gate spanning 1 ly: opened at / held at | 6.3e17 J / 2.4 GW |
| A gate spanning 5 ly: opened at / held at | 3.2e18 J / 12.2 GW |
| A gate spanning 10 ly: opened at / held at | 6.3e18 J / 24.4 GW |
| A gate spanning 40 ly: opened at / held at | 2.5e19 J / 97.5 GW |
| data (1 kg) through a 5 ly gate at natural speed | 1.0 s, 2.9e5 J |
| a 100 t ship through a 5 ly gate at natural speed | 46.4 s, 2.9e10 J |
| a capital ship (100 kt) through a 5 ly gate at natural speed | 464.2 s, 2.9e13 J |
