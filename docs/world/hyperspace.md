# Hyperspace

*A world article: how the hyper layer works in the universe, all in one place. The laws are
Dogma's (`config/dogma.ron`, every constant marked Invented); the reasons and the rules of the
invented are in the charter (`docs/physics.md`). Kept current as the hyper layer changes.*

## In one breath

Space carries a **hyper-medium**: it holds anything moving in it to a speed limit that grows with
distance from the nearest surface (slow close to bodies, fast in the open). A ship's **field** (its
hyperdrive) moves through it burning fuel by the metre, more the faster it goes, by one formula
that's the same anywhere: no zones, no walls; what limits how far a ship goes is the fuel it
carries. A normal ship's tank won't reach the next star; an explorer that's mostly tank just
will. **Gates** are **tubes** laid along the real route between two rings: a 5 ly gate is 5 ly
long, but inside it anything can go millions of times faster than light, paying by its mass and
how fast it insists on going, about a billion times cheaper per kilogram than a field. Light can
travel a tube too, but its flow is unstable and a light signal can't be caught at the far end
reliably, so **information crosses in physical capsules**, thrown and caught like any payload: by
the same rule, data crosses a light year in about 200 ms, a regular ship in tens of seconds, a
capital ship in minutes.

(*How the numbers are chosen*: each invented constant comes from a stated design target, see
`docs/world/README.md`; the calculations are in `tools/experiments/`.)

## 1. The medium

No laws of its own: no zones, no limits, the same everywhere. What moving through it costs is the
field's law (§2); what holding a tube in it costs, the tubes' (§3). The rest is products':

| Spec | On | Value (base world) |
|---|---|---|
| Top speed | a hyperdrive | Kestrel 3,000 c, Halcyon 3,500 c |
| Governor: `v ≤ k · d` near bodies (d: distance to the nearest surface) | a nav computer | k = 2 /s (about 1,000 c at 1 AU from a star, 20,000 km/s at 10,000 km from a world) |
| Interlock: drops out rather than come within this of a body's ground | a nav computer | 1 km |
| Capture: the fastest a ring catches a ship entering it | a gate ring | 300 m/s |
| Cadence: a batch of capsules thrown this often | a hyper relay, a gate relay | 3 s |

Without avionics a drive goes as fast as the throttle says, wherever it's pointed.

## 2. The field: hyperdrives

A ship's hyperdrive holds a field round it and moves it through the medium, burning fuel from its
tank by the metre (Invented law; the shape borrowed from drag, where pushing through a medium costs
energy per distance growing with the square of speed):

    dE/dx = m · E0 · (1 + (v / v*)²) / η

| | Kind | Value | From the target |
|---|---|---|---|
| E0 (FIELD_COST) | Invented constant | 2.4×10⁻³ J/(kg·m) | **a normal ship (a Drover: 91.5 t, 30 t of deuterium, an S2 drive at 0.6) goes about 1.5 ly on a full tank at v*: never the 4-7 ly to the next star** |
| v* (V_BEST_C) | Invented constant | 1,000 c | the cost doubles there over going slow: **5 ly in about 2 days at v*** |
| Top speed (V_TOP_C) | Invented constant | 3,000 c | **full throttle costs ten times the slow cost** |
| η | the drive's (a product spec) | 0.6-0.75 | brands |

What comes out (`tools/experiments/hyper_fuel.py`; deuterium at 3.45×10¹⁴ J/kg):

| | Slow | At v* | Full throttle |
|---|---|---|---|
| A Drover on a full tank | 3.0 ly | 1.5 ly | 0.3 ly |
| An explorer, nine tenths tank, η 0.75 | 10.2 ly | 5.2 ly | 1.0 ly |

| Trip, a Drover | Fuel |
|---|---|
| A 1 AU hop | 0.2 kg |
| 40 AU round a system | 7 kg |
| 200 AU out and back | about 130 kg |
| A light year at v* | 20 t |

- **Nothing walls a ship in.** Fuel and money limit how far it goes; exploration is logistics
  (tankers, depots, drop tanks, ships that are mostly tank).
- **Speed:** `v = throttle × min(governor, top speed)`, both products' specs (§1).
- Out of fuel in hyperdrive, it drops out where it is.

## 3. Gates: tubes along the route

**Model (invented law):** a gate pair holds a **tube** of the medium open along the real path
between its rings, inside which the medium's speed limit is lifted. Real-physics idea: the
**Krasnikov tube** (Krasnikov 1997, [gr-qc/9702049](https://arxiv.org/abs/gr-qc/9702049)), a
channel along a route inside which travel can be faster than light; real versions need absurd
negative energy, ours is the invented medium's. Nothing is shortened: **the tube is as long as
the span**; it's the speed inside that's extraordinary.

### Crossing a tube (the golden formula)

    t_nat(m, S) = 0.2 s × (S / 1 ly) × (m / 1 kg)^(1/3)     natural crossing time
    E(m, S, t)  = ε × m × S × e^(t_nat / t)                  energy to cross in time t

| | Kind | Value | From the target |
|---|---|---|---|
| Form: exponential above a natural speed | Invented law | | rushing must punish steeply; going slow saves little (a floor, `ε·m·S`), so nobody crawls for free |
| 0.2 s per ly | Invented constant | | **data (a 1 kg capsule) crosses at 200 ms per light year at natural speed; faster gets punishing** |
| Mass exponent ⅓ | Invented constant | γ = ⅓ | **a regular ship under a minute through a typical (5 ly) gate, a hauler a couple of minutes, a capital ship several minutes** (¼ and ½ tried: too fast, too slow; the cube root goes with a payload's size) |
| ε | Invented constant | 2.2×10⁻¹² J/(kg·m) | **a 100 t ship at natural speed through a 5 ly gate costs about an hour of an S2 fusion plant (8 MW)** |

What comes out (`tools/experiments/gate_transit.py`):

| Payload | 5 ly gate, natural | Average gate (3 ly) | Cost at natural, 5 ly |
|---|---|---|---|
| Data capsule 1 kg | 1 s | 0.6 s | 2.9×10⁵ J |
| Ship 100 t | 46 s | 28 s | 2.9×10¹⁰ J (an S2 plant-hour) |
| Hauler 1,000 t | 1.7 min | 1 min | 2.9×10¹¹ J |
| Capital ship 100 kt | 7.7 min | 4.6 min | 2.9×10¹³ J |
| Super-capital 1 Mt | 17 min | 10 min | 2.9×10¹⁴ J |

- **Rushing**, any payload: 2× faster costs 2.7×, 3× 7×, 5× 55×, 10× about 8,000×.
- **Cost is linear in mass and span**: the gate's owner prices by both, and by the speed asked.
- The tube is entered under the ring's capture speed (its spec: 300 m/s; faster and the capture fails),
  a docking rule at the mouth, not the speed inside.

### Data through a tube

Light could cross, but the tube's flow is unstable: a light signal can't be caught at the far
end reliably, so it's practically useless. Data goes in **capsules** (a casing and hardened
storage: a world product), thrown and caught by the gates:

- **Batched:** traffic is huge, so it's gathered and thrown in batches; a capsule's mass is mostly
  its casing, so a link pays per throw, not per bit.
- **Both ways, by turns:** capsules can't pass each other in the flow, so the two ends take turns;
  casings circulate (caught, reloaded, thrown back), so a link pays about two throws a cycle
  whatever the traffic's balance.
- On a 5 ly gate (1 kg capsules): one-way latency 0.5 s needs 23 MW; **1 s 1.6 MW**; 2 s 290 kW;
  5 s 58 kW; 20 s 8.5 kW.
- **Relays** are the same physics with thin tubes (1 cm: capsules of 10 g only, no ship fits):
  next to free to hold, carrying the hypernet between a system's sites in capsules too.

### Opening and holding a tube: the same formula

A tube weighs by its diameter, `μ(d) = ρ · (d / 1 m)^K`; **opening** it is a crossing by that
equivalent mass (the same formula, the same exponential for opening it faster); **holding** it
costs its opening over `TUBE_HOLD` (it leaks, topped up continuously).

    μ(d)    = ρ × (d / 1 m)^3,                 ρ = 406 kg
    E_open  = ε × μ × S × e^(t_nat(μ,S) / t)   (natural pace: t = t_nat)
    P_hold  = E_open,natural / TUBE_HOLD,      TUBE_HOLD = 200 months (0.5% a month)

| | Kind | Value | From the target |
|---|---|---|---|
| Diameter exponent | Invented constant | K = 3 | **opening a tube sized for one ship (100 m) just for one pass costs thousands of passes through a held gate; a held gate pays once about 5 ships a day use it** (K = 2 tried: holding always pays, at 0.15 ships a day) |
| ρ | Invented constant | 406 kg | **opening a typical gate (a 3 km ring, 5 ly) costs about a year of a 100 GW industry** |
| TUBE_HOLD | Invented constant | 5.18×10⁸ s | **holding costs about 0.5% of the opening a month (halved 2026-10-04, to ease upkeep): small next to opening, big in absolute terms; a lapse (re-opening) is ruinous** |

| Tube | Opening (natural pace) | Takes | Holding |
|---|---|---|---|
| Gate 3 km, 1 ly | 6.3×10¹⁷ J | 74 min | 1.2 GW |
| **Gate 3 km, 5 ly** | **3.2×10¹⁸ J** | 6 h | **6.1 GW** (about 760 S2 plants) |
| Gate 3 km, 10 ly | 6.3×10¹⁸ J | | 12 GW |
| One-ship tube 100 m, 5 ly | 1.2×10¹⁴ J (4,000 passes) | 12 min | 0.2 MW |
| Relay tube 1 cm | next to nothing | | microwatts |

(`tools/experiments/tube_holding.py`.) *Open for one pass* is a comparison, not a thing ships do:
no ship today could carry the energy to open a tube of its own.

### Data's cadence

Capsules can't pass each other in a tube's flow, so data goes in batches, thrown by turns: how
often is the relay's **cadence** (a product's spec, 3 s on the base world's relays; target: news
crosses a relay link in 1-2 s). Data across a tube takes half a cadence (the wait for the next
throw) plus the capsule's crossing: a relay hop in a system about 1.5 s (the crossing is
microseconds), a 5 ly gate about 2.5 s (+ its relay's handling).

## 4. Typical gates

A typical gate spans **up to about 5 ly** (40 ly is an outlier). The charted region has stars at
real density (`galaxy.md`: neighbours 4-6 ly), and the seeded lanes run about 4-7 ly: a 91 t
Drover crosses in 37-63 s.

## Open questions

1. **Natural speed and the owner's power:** should more holding power buy a faster tube (premium
   lanes, a neglected gate slowing down)? Leaning yes.
2. **Capsules as products:** casing mass and hardened storage density per brand (today a gate's
   1 kg and a relay's 10 g are tuning values in the world sheet).
3. **Gate economics in play:** owners paying to open and hold, fees by mass and speed, ships
   choosing their crossing speed (today every crossing is at natural speed, unpaid).

*In code (2026-10-02):* Dogma's Tube laws (`config/dogma.ron`), `hyper::tube_*`; gate transits
take the natural time for the ship's mass and the lane; the hypernet's relay hops and gate data
cross in capsules at their relays' cadence; the Dogma checks test the targets above.

## Numbers at a glance

| | |
|---|---|
| Speed limit at 1 AU from a star | about 1,000 c |
| A Drover's full tank at v* | 1.5 ly (never the next star) |
| An explorer's (nine tenths tank) at v* | about 5 ly, 2 days |
| Data through a gate, natural | 200 ms per ly (1 s for 5 ly) |
| A 100 t ship through a 5 ly gate, natural | 46 s, an S2 plant-hour |
| A capital ship (100 kt) through a 5 ly gate | 7.7 min |
| Rushing 2× / 3× / 10× | 2.7× / 7× / 8,000× the cost |
| Opening / holding a 5 ly gate | 3.2×10¹⁸ J / 6.1 GW |
| A relay hop in a system | about 1.5 s |
