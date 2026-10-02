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

## Medium

The hyper-medium: near masses it holds what moves in it under v_lim = K·d; far from them it's slack, s(d) = min(1, (K·d / v_open)²).

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `HYPER_RATE` | 2 | 1/s | Invented | K: the limit per metre from the nearest surface. At 1 AU from a star, about 1,000 c. |
| `INTERLOCK` | 1000 | m | Invented | Nothing moves in the medium within this of a body's highest ground. |
| `V_OPEN_C` | 1e6 | c | Invented | Where the slack sets in (a few hundred AU from a star). |
| `STIFF_SLACK` | 0.01 |  | Invented | A field forms only where the slack is below this (inside a system). |

## Field

A hyper-field's draw: P = m·s·(P_FLOOR + P_PUSH·(v/v*)³) / η, η the device's. Between stars, power per kg is the wall.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `P_FLOOR` | 10000 | W/kg | Invented | Holding a field in open space, per kg in it. |
| `P_PUSH` | 5000 | W/kg | Invented | Pushing it through the medium at the best speed; grows with the cube of speed. |
| `V_BEST_C` | 1000 | c | Invented | v*: the speed the push is reckoned at. |

## Throat

A gate pair is one wormhole throat: holding it open takes P = P0·(S/S0)³ for a span S; a transit costs τ·m·S.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `MAX_TRANSIT_SPEED` | 300 | m/s | Invented | Faster through a throat and the transit wrecks what goes in. |
| `TRANSIT_TIME` | 10 | s | Invented | Through the throat, ring to ring (matter and hyper-signals). |
| `GATE_P0` | 1e9 | W | Invented | Holding a throat open at the reference span. |
| `GATE_S0` | 10 | ly | Invented | The reference span. |
| `GATE_TAU` | 2.6e-9 | J/(kg·m) | Invented | A transit's energy per kg per metre of span (the gate's, not the traveller's). |

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

## References

For the dogma's claims (not used by the code).

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `ETA_FIELD_MIN` | 0.5 |  | Planned | The field's efficiency, the worst drives. |
| `ETA_FIELD_MAX` | 0.8 |  | Planned | The field's efficiency, the best drives. |
| `EXPLORER_POWER` | 30000 | W/kg | Planned | A future explorer's power per kg aboard (next-generation reactors). |

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
| The wall between stars (holding the field, best drive) | 12.5 kW per kg aboard |
| Today's best plant on its own | 3.5 kW/kg (14 MW at most) |
| DROVER with the strongest plant | 0.15 kW/kg: can't cross |
| SPRINT COURIER with the strongest plant | 0.36 kW/kg: can't cross |
| BULK HAULER with the strongest plant | 0.05 kW/kg: can't cross |
| PROSPECTOR with the strongest plant | 0.13 kW/kg: can't cross |
| INTERCEPTOR with the strongest plant | 0.33 kW/kg: can't cross |
| A future explorer at 30 kW/kg | 1.41 × the best speed: 5 ly in 1.3 days, 40 ly in 10.4 days |
| The medium's slack at 1 AU / 40 AU / 2 ly | 1e-6 / 2e-3 / 1 |
| A gate spanning 5 ly holds open at | 125 MW |
| A gate spanning 10 ly holds open at | 1.0 GW |
| A gate spanning 25 ly holds open at | 15.6 GW |
| A gate spanning 50 ly holds open at | 125.0 GW |
| A gate spanning 40 ly holds open at | 64.0 GW |
| A gate transit, per tonne across 40 ly | 9.8e11 J |
