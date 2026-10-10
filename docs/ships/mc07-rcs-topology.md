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

A pod is a real socket: its five nozzles share one block, each exit a few tens of centimetres from its neighbours,
not four co-located bounding boxes. Each nozzle's core points away from the hull: up nozzles above the roof line,
down below the keel, outboard beyond the side, fore ahead of the nose, aft behind the tail. Radiators and sensors keep
out of those cones (docs/ships/exhaust-keepouts.md).

## Per-nozzle rating

`equipment.thrusters.quad.s1`: 120 kN for the slot, shared over its 20 nozzles (`share: 1` each): **6 kN a nozzle**.
A rough authority check for engine (from the hull's 66.1 m length, its mass about 156 t, pods about 30 m from the
centre of mass, its moment of inertia taken as a uniform rod's, m L^2 / 12, about 5.7e7 kg m2): two nozzles in a
couple give 2 x 6 kN x 30 m = 360 kN m, 0.0063 rad/s2: a quarter turn in about 32 s from rest to rest. All rough, review.

## The jets themselves (open, flagged)

The quad record burns deuterium at 10,000 km/s (an early game figure; the record is marked outdated): 6 kN at that
speed is 30 GW of jet a nozzle, which makes every RCS core a beam no radiator survives near. Manoeuvring thrusters in
practice are chemical (a few km/s): 6 kN of hydrolox at 4.4 km/s is 13 MW a nozzle. A chemical thruster product for
the MC-07 is the registry's to add (a proposal, not made yet: it changes what the ship burns and carries). Until then
the keep-out cones apply as they are.
