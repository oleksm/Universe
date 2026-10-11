# Exhaust keep-outs: a screening rule (registry, 2026-10-10)

For engine's task 20261010T170858-engine-864d and ships' radiator study (20261010T171500-ships-29d4). Status:
**screening, not certification**. Every angle and fraction below is invented (review) until a plume model is sourced;
ships labels results made with it as screening.

## Why the jets rule everything near them

A jet's power is half its thrust times its exhaust speed. From the records the MC-07 fits:

| Device | Thrust | Exhaust | Jet power |
|---|---|---|---|
| `equipment.drive.torch.s1` (6 nozzles) | 900 kN | 10,000 km/s | 4.5 TW |
| `equipment.lift.belly.s1` (6 nozzles) | 220 kN | 10,000 km/s | 1.1 TW |
| `equipment.thrusters.quad.s1` (per slot) | 120 kN | 10,000 km/s | 0.6 TW |
| `equipment.engine.ch.s2` (for scale) | 110 kN | 4.4 km/s | 0.24 GW |

A part that catches even a millionth of a fusion jet takes megawatts. So the rule is not "how much flux may a radiator
take" but "where may nothing be".

## The rule

Per nozzle, from its frame in `docs/ships/mc07-exhaust-frames.json` (engine, main 23587da4): apex at the nozzle exit,
axis along the exhaust direction.

| Zone | Where | What may be there |
|---|---|---|
| **Core** | within half-angle 15 degrees of the exhaust axis, any distance | nothing, ever (no part, no radiator, no line of a docked ship) |
| **Fringe** | 15 to 45 degrees | no radiator or sensor face; structure only if it is the nozzle's own shield |
| **Backflow** | 45 to 90 degrees, within 10 nozzle exit diameters of the exit | no radiator face that sees the exit; contamination expected |
| **Behind** | over 90 degrees | free of the jet (still under the radiation below) |

For a fusion drive the plasma also shines (X-rays, bremsstrahlung) in every direction: that is a radiation dose and
heat load on everything in sight of the plume, unmodelled here. It is the next open item, not a reason to wait.

Nozzles that fire only briefly (RCS) keep the same core and fringe: a radiator is in place for every firing.

## The rear pod's HOT EXHAUST

No device in the registry vents gas there: the MC-07's heat leaves by radiators, and its jets are the drive, lift and
thrusters. The marking sits on the authored louver and heat-sink bank, so it is **a hot radiating surface, not a plume**:
nothing may shade it from space (a radiator in front of it robs it of its view), and its own heat reaches what faces it.
Its temperature and power belong to the MC-07's thermal design (the hull's heat sink), still open; until then treat it as a
1,000 K surface for screening. If the owner meant a gas vent, it needs a device record first (registry).

## What stays open

- A sourced plume model (divergence of a magnetic nozzle's exhaust, chemical bell exit angles).
- The fusion plume's radiation, in every direction.
- The heat sink's temperature and power (the HOT EXHAUST bank).
- Radiator structure, mass, coolant routes and the two-face qualification (ships' list).
