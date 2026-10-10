# MC-07 thrusters: intended topology and per-nozzle rating (registry, 2026-10-10)

For engine's task 20261010T172036-engine-5632, after ships' radiator screen failed on the RCS cores. Ships owns
placement; engine checks control authority. Status: proposal (the hull record's `on_conflict: propose`).

## Topology

Four pods, one at each corner of the hull's plan: nose left, nose right, tail left, tail right, as near the hull's ends
and sides as the structure allows (the farther from the centre of mass, the more turning per newton). Each pod has five
nozzles, one per direction: **fore, aft, up, down, outboard** (`side`). Twenty nozzles, the names already in the hull
record (`nozzle_<nose|tail>_<left|right>_<fore|aft|up|down|side>`). Inboard needs no nozzle: the opposite pod's
outboard gives it.

What that gives: translation along every axis (pairs and quads of like nozzles), roll from up/down on the left against
the right, pitch from up/down at the nose against the tail, yaw from outboard at the nose against the tail, and fore/aft
couples as a second yaw.

A pod is a real socket: a compact block (about a metre) whose five nozzles exit on its faces, a few tens of
centimetres apart, not four co-located bounding boxes. The exits need not stand above the roof or ahead of the nose; the
block sits at the hull's outer corner (or on an outrigger) so that no nozzle's core cone (15 degrees) cuts the hull or
anything fitted to it. That is the placement test ships runs. Radiators and sensors keep
out of those cones (docs/ships/exhaust-keepouts.md).

## Per-nozzle rating

**Baseline (as the game reads it today):** a nozzle's thrust is the device's rating times its `share`
(crates/core/world/src/modules.rs, ship.rs: no division). `equipment.thrusters.quad.s1` is 120 kN and every MC-07
thruster nozzle has `share: 1`, so **each of the 20 nozzles is 120 kN** (2.4 MN in all; 600 GW of jet a nozzle at
10,000 km/s). (Corrected: this section first said 6 kN, reading share as a split.)

**Proposed (not made):** about 6 kN a nozzle, which turns the ship well enough (below). Migration, when agreed: not by
shares (share 0.05 would leave a 120 kN product whose nozzles never give it), but by a product rated for it: a chemical
thruster block of 6 kN a nozzle (next section), fitted with `share: 1`. Until then the baseline stands.

A rough authority check for the proposal (the hull's 66.1 m, about 156 t, pods about 30 m from the centre of mass, its
moment of inertia taken as a uniform rod's, m L^2 / 12, about 5.7e7 kg m2): two nozzles in a couple give
2 x 6 kN x 30 m = 360 kN m, 0.0063 rad/s2: a quarter turn in about 31 s from rest to rest. All rough, review.

## The jets themselves (open, flagged)

The quad record burns deuterium at 10,000 km/s (an early game figure; the record is marked outdated): 120 kN at that
speed is 600 GW of jet a nozzle (30 GW even at the proposed 6 kN), which makes every RCS core a beam no radiator survives near. Manoeuvring thrusters in
practice are chemical (a few km/s): 6 kN of hydrolox at 4.4 km/s is 13 MW a nozzle. A chemical thruster product for
the MC-07 is the registry's to add (a proposal, not made yet: it changes what the ship burns and carries). Until then
the keep-out cones apply as they are.
