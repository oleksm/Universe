# The physics sheet

Generated from `config/physics.ron` (run `cargo run -p universe-world --example physics_sheet`): edit the sheet, not this. The charter is `docs/physics.md`; the dogma's claims are checked in `crates/world/tests/dogma.rs`.

## Nature

Constants of physics (real).

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `SPEED_OF_LIGHT` | 2.99792458e8 | m/s | Real | c: light, radio, radar, and every capture of an event. |
| `STEFAN_BOLTZMANN` | 5.670374e-8 | W/(m²·K⁴) | Real | σ: how a body radiates heat: σ·ε·A·T⁴. |
| `AIR_CP` | 1005 | J/(kg·K) | Real | Air's specific heat at constant pressure (Earth-like air). |

## Medium

The hyper-medium (invented): its local limit v_lim = K·d near masses; slack in open space.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `HYPER_RATE` | 2 | 1/s | Invented | K: the medium's limit per metre from the nearest surface. At 1 AU from a star, about 1,000 c. |
| `INTERLOCK` | 1000 | m | Invented | Nothing moves in the medium within this of a body's highest ground. |
| `GROUND_MARGIN` | 20000 | m | Tuning | Flying along the nose at a world, the drive drops out this far above its ground (or its air's top). |
| `V_OPEN_C` | 1e6 | c | Planned | Where the slack sets in: s(d) = min(1, (K·d / v_open)²). A few hundred AU out. |
| `STIFF_SLACK` | 0.01 |  | Planned | A field forms only where the slack is below this (inside a system). |

## Hyperdrive

Draw P = m·(P_FLOOR·s + P_PUSH·(v/v*)³)/η. Between stars, power per kg is the wall.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `HYPER_FUEL_FLOW` | 0.2 | kg/s | Simplified | Today's flat fuel draw while engaged, at full throttle: to be replaced by the energy model. |
| `P_FLOOR` | 10000 | W/kg | Planned | Holding the field in open space, per kg aboard. Today's best plant gives 2.8 kW/kg on its own. |
| `P_PUSH` | 5000 | W/kg | Planned | Pushing through the medium at the best speed; grows with its cube. |
| `V_BEST_C` | 1000 | c | Planned | v*: the best speed between stars (an explorer at 30 kW/kg goes 1.6 v*). |
| `ETA_FIELD_MIN` | 0.5 |  | Planned | The field's efficiency, the worst drives (the rest is heat). |
| `ETA_FIELD_MAX` | 0.8 |  | Planned | The field's efficiency, the best drives. |
| `EXPLORER_POWER` | 30000 | W/kg | Planned | The reference future explorer's power per kg aboard (next-generation reactors), for the dogma's claims. |

## Gates

A gate pair is one wormhole throat (invented): power cubic in span, limited by ring class.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `GATE_RADIUS` | 1500 | m | Tuning | The ring's centreline radius; its opening is a little smaller. |
| `RING_TUBE` | 60 | m | Tuning | Half the ring structure's thickness. |
| `MAX_TRANSIT_SPEED` | 300 | m/s | Invented | Faster through a ring and the transit wrecks the ship. |
| `TRANSIT_TIME` | 10 | s | Invented | Through the throat, ring to ring (matter and hyper-signals). |
| `GATE_P0` | 1e9 | W | Planned | Holding a throat open at the reference span: P = P0·(S/S0)³. |
| `GATE_S0` | 10 | ly | Planned | The reference span. |
| `GATE_TAU` | 2.6e-9 | J/(kg·m) | Planned | A transit's energy per kg per metre of span (the gate's, not the ship's). |
| `RING_SPAN_I` | 10 | ly | Planned | A class I ring's greatest span. |
| `RING_SPAN_II` | 25 | ly | Planned | A class II ring's greatest span. |
| `RING_SPAN_III` | 50 | ly | Planned | A class III ring's greatest span: nothing bridges farther. |

## Drives

Today's main drive: a fusion torch.

| Name | Value | Unit | Kind | Why |
|---|---|---|---|---|
| `EXHAUST_VELOCITY` | 1e7 | m/s | Grounded | A fusion torch (Daedalus-class, 3% of c). Each drive burns thrust / this, in kg/s. Its power isn't yet accounted (the energy chain). |

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

## What it adds up to

| Claim | Figure |
|---|---|
| The wall between stars (holding the field, best drive) | 12.5 kW per kg aboard |
| Today's best plant on its own | 2.8 kW/kg (14 MW at most) |
| DROVER with the strongest plant | 0.16 kW/kg: can't cross |
| SPRINT COURIER with the strongest plant | 0.31 kW/kg: can't cross |
| BULK HAULER with the strongest plant | 0.06 kW/kg: can't cross |
| PROSPECTOR with the strongest plant | 0.13 kW/kg: can't cross |
| INTERCEPTOR with the strongest plant | 0.31 kW/kg: can't cross |
| A future explorer at 30 kW/kg | 1.41 × the best speed: 5 ly in 1.3 days, 40 ly in 10.4 days |
| The medium's slack at 1 AU / 40 AU / 2 ly | 1e-6 / 2e-3 / 1 |
| A gate spanning 5 ly holds open at | 125 MW |
| A gate spanning 10 ly holds open at | 1.0 GW |
| A gate spanning 25 ly holds open at | 15.6 GW |
| A gate spanning 40 ly holds open at | 64.0 GW |
| A gate spanning 50 ly holds open at | 125.0 GW |
| A gate transit, per tonne across 40 ly | 9.8e11 J |
