# Request to the Scientist: propulsion, an audit and the options to experiment with

*From the ships session on `ships`, 2026-10-06. The user: "Do we even need belly lift, or would normal
thrusters placed downward do the job? What other engineering solutions can we ask? What about a
rotatable gimbal?" and "create request to Registry for options, then we can experiment with each
one." Also the user's standing ask: audit the ship records against physics, critically, and raise
what's wrong. Part 1 is that audit for propulsion; Part 2 is what's asked; Part 3 is how the studio
will try each option.*

## Part 1. Audit: what the records say and what physics says

Figures from `standards/SFO/metadata/equipment/`. Jet power is ½ × thrust × exhaust speed; fuel
flow is thrust ÷ exhaust speed.

| Record | Thrust | Exhaust | Jet power | Fuel flow |
|---|---|---|---|---|
| drive.torch.s1 | 900 kN | 10,000 km/s | **4.5 TW** | 0.09 kg/s |
| lift.belly.s3 | 1.6 MN | 10,000 km/s | **8 TW** | 0.16 kg/s |
| lift.belly.s1 (per nozzle) | 220 kN | 10,000 km/s | 1.1 TW | 0.022 kg/s |
| thrusters.quad.s1 | 120 kN | 10,000 km/s | **0.6 TW** | 0.012 kg/s |

### 1. Landing on a torch destroys the pad (critical)

A belly lift holding design-2 (156 t) at 1 g puts 8 TW onto the ground: about 1.9 kt of TNT every
second. Nothing near a landing ship survives; a port could not exist. Torch exhaust is a deep-space
engine. Real landers use low exhaust speed: chemical about 3–4.5 km/s, nuclear thermal about
9 km/s. At 4.4 km/s, hovering 156 t takes 3.5 GW of jet power (a launch pad's scale), but
**364 kg/s of propellant: a 60-second landing burns about 22 t**. That's the true price of landing a
heavy ship on a 1 g world, and it's the kind of hard physics the user wants as a game constraint.

### 2. Manoeuvring thrusters are torches too (critical)

Each quad block is 0.6 TW. Docking beside a station with them would cut it apart. Real attitude
thrusters are cold gas (about 0.7 km/s) or small chemical ones (about 3 km/s), at kilonewtons, not
120 kN.

### 3. Where the drive's lost 70% goes (critical, and invented)

Efficiency 0.3 means each watt of jet costs 2.3 W of waste: about **10 TW** for the S1 drive. The
heat budget counts `heat_to_hull` (1e-6, invented) of the jet power aboard and silently lets the
rest leave with the plume. If the true share is 1e-4, the ship must radiate 450 MW; at 1e-3,
4.5 GW, against 60 MW of radiators. The whole heat budget hangs on one invented number. Asked: a
basis (a design where the reaction and its neutrons stay outside the ship, magnetic nozzle, aneutronic
fuel) or a figure with a source, and the waste split stated (jet, plume, aboard).

### 4. Deuterium's energy is barely enough (review)

Exhaust at 10,000 km/s carries ½v² = 5×10¹³ J per kg. At efficiency 0.3 that needs 1.7×10¹⁴ J
per kg of fuel burned. Plain D-D fusion yields about 8.7×10¹³ J/kg; only a fully catalysed burn
(the tritium and helium-3 burned too) reaches about 3.5×10¹⁴. Either the exhaust carries extra
inert mass (and the record should say so), or the figures are at the edge of possible: please check
and state it.

### 5. "Lift", "drive" and "thrusters" are roles, not things (design)

All three burn the same fuel at the same exhaust; they differ in where they point. Separate families
make players carry two engine sets (the lift idles in cruise, the drive idles landing) and hide the
real choices: where engines point, whether they swivel, what exhaust they use.

### 6. The lift's size is its array (design)

`lift.belly.*` is sized as the MC-07's six-nozzle spread (38.6 × 6.5 m), and the S3 scaled from it
(17.5 × 5.9 × 38.6 m, about 4,000 m³ for 7 t). The studio places it as a solid box that fills the
lower frame. A record should be one engine, its own size.

### 7. Crew "down" changes between phases (design, for the seats)

Command stations face the thrust. On a belly-lander, thrust is aft in flight but the belly is "down"
when hovering: the crew lie on their backs to land. A record can't fix that; designs can (a
tail-sitter, vectored engines, seats that turn). Asked: whether a seat can turn (a figure for it).

## Part 2. What's asked

1. **Engines as one family of single units**, replacing drive, lift and thrusters as families: in
   about five thrust classes, each with thrust, exhaust, efficiency, fuel, mass, its own size, a mount,
   and:
   - **throttle range** (the least thrust as a share of the most: soft landing needs deep throttling);
   - **gimbal range** (degrees: 0 fixed, about ±10° to steer);
   - `heat_to_hull` with a basis (Part 1, item 3).
2. **Engines of other kinds**, the low-exhaust ones a lander needs:
   - **chemical** (bipropellant, about 3.5–4.5 km/s; its propellants as stock and its tanks);
   - **nuclear thermal** (about 9 km/s, hydrogen);
   - **cold gas** and small chemical **attitude thrusters** (0.7–3 km/s, about 0.5–5 kN).
3. **Swivels:** an actuator that turns an engine through 90° (vectored thrust), its mass and power by
   thrust class. Far heavier than a gimbal; that's the trade.
4. **Landing without engines**, for worlds with air (later is fine): parachutes (mass, drag area, the
   most mass they slow), a heat shield (areal mass, heat it takes), and air density at the surface on
   each world's record (the climate data may hold it already).
5. **A landing pad standard (later):** what a pad must stand (jet power on it, temperature): what
   lets ports exist next to landing ships.

## Part 3. How the studio will try each option

With engine units, these become design choices, not product families. The studio adds a balance
check (net thrust through the centre of mass, within what gimbals and thrusters can correct), an
engine-out check (still hovers with one unit lost), the crew's "down" in each phase, landing
propellant (litres burned to land from 1 km at design gravity), and jet power on the pad. Then:

| Variant | Engines |
|---|---|
| A. downward units | four or more low-exhaust units pointing down round the centre of mass |
| B. gimballed main | torch units on gimbals to steer; cold-gas attitude thrusters |
| C. vectored | main units on 90° swivels, aft in flight, down to land |
| D. tail-sitter | the ship stood up, landing on its main units; decks across the thrust |
| E. orbit only | torch only; no landing (shuttles land) |
| F. aero | parachutes and a heat shield where there's air, engines for the last metres |

Each built as a design-2 variant, the budgets compared side by side.
