# Hyperspace

*A world article: how the hyper layer works in the universe, all in one place. The laws are
Dogma's (`config/dogma.ron`, every constant marked Invented); the reasons and the rules of the
invented are in the charter (`docs/physics.md`). Kept current as the hyper layer changes.*

## In one breath

Space carries a **hyper-medium**. Near masses it's stiff and holds anything moving in it to a
speed limit that grows with distance from the nearest surface; between the stars it goes slack.
A ship's **field** (its hyperdrive) moves through the stiff medium cheaply; in slack space
holding a field costs power per kilogram, a wall no ship today climbs. **Gates** are **tubes**
laid along the real route between two rings: a 5 ly gate is 5 ly long, but inside it anything
can go millions of times faster than light, paying by its mass and how fast it insists on going.
Light can travel a tube too, but its flow is unstable and a light signal can't be caught at the
far end reliably, so **information crosses in physical capsules**, thrown and caught like any
payload: by the same rule, data crosses a light year in about 200 ms, a regular ship in tens of
seconds, a capital ship in minutes.

(*How the numbers are chosen*: each invented constant comes from a stated design target, see
`docs/world/README.md`; the calculations are in `tools/experiments/`.)

## 1. The medium

| Law | Formula | Constants |
|---|---|---|
| Speed limit near masses | `v_lim = K · d` (`d`: distance from the nearest surface) | K = 2 /s: at 1 AU from a star about 1,000 c |
| Interlock | nothing moves in the medium within 1 km of a body's highest ground | INTERLOCK = 1,000 m |
| Slack | `s = min(1, (K · d / v_open)²)` | v_open = 10⁶ c: slack sets in a few hundred AU out |
| Where a field forms | only where `s` < 0.01: inside systems | STIFF_SLACK = 0.01 |

- Deep inside a system the slack is about 0; between stars it's 1.
- Climbing out of a well is slow: near a world the limit is small (20,000 km/s at 10,000 km).

## 2. The field: hyperdrives

A ship's hyperdrive holds a field round it and pushes it through the medium. Its draw:

    P = m · s · (P_FLOOR + P_PUSH · (v / v*)³) / η

| Constant | Value | Meaning |
|---|---|---|
| P_FLOOR | 10 kW/kg | holding a field in open space, per kg in it |
| P_PUSH | 5 kW/kg | pushing it at the best speed; grows with the cube of speed |
| v* (V_BEST_C) | 1,000 c | the speed the push is reckoned at |
| η | the drive's (a product spec) | the share of its draw that holds and pushes the field; the rest is heat |

- **In a system** (s ≈ 0) power hardly matters: the medium's limit binds. Speed follows the room
  ahead and the throttle; dropping out keeps the exit velocity.
- **Between stars** (s = 1) power per kg is **the wall**: about 12.5 kW/kg with the best drive. No
  ship today reaches it; a future **explorer** (about 30 kW/kg) makes 5 ly in about 1.3 days and
  40 ly in about 10: an epic, not a trip.
- **The field collapses** when the power can't hold it (the capacitors run dry), and won't form
  where the medium is slack.
- *Simulated:* the drive draws from the capacitor banks, charged by the plant.

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
- The tube is entered under the ring's capture speed (300 m/s: faster and the capture fails),
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
    P_hold  = E_open,natural / TUBE_HOLD,      TUBE_HOLD = 100 months (1% a month)

| | Kind | Value | From the target |
|---|---|---|---|
| Diameter exponent | Invented constant | K = 3 | **opening a tube sized for one ship (100 m) just for one pass costs thousands of passes through a held gate; a held gate pays once about 9 ships a day use it** (K = 2 tried: holding always pays, at 0.3 ships a day) |
| ρ | Invented constant | 406 kg | **opening a typical gate (a 3 km ring, 5 ly) costs about a year of a 100 GW industry** |
| TUBE_HOLD | Invented constant | 2.59×10⁸ s | **holding costs about 1% of the opening a month: small next to opening, big in absolute terms; a lapse (re-opening) is ruinous** |

| Tube | Opening (natural pace) | Takes | Holding |
|---|---|---|---|
| Gate 3 km, 1 ly | 6.3×10¹⁷ J | 74 min | 2.4 GW |
| **Gate 3 km, 5 ly** | **3.2×10¹⁸ J** | 6 h | **12 GW** (about 1,500 S2 plants) |
| Gate 3 km, 10 ly | 6.3×10¹⁸ J | | 24 GW |
| One-ship tube 100 m, 5 ly | 1.2×10¹⁴ J (4,000 passes) | 12 min | 0.5 MW |
| Relay tube 1 cm | next to nothing | | microwatts |

(`tools/experiments/tube_holding.py`.) *Open for one pass* is a comparison, not a thing ships do:
no ship today can hold a tube between the stars (the wall).

### The flow settles: data's cadence

After a throw a tube's unstable flow takes **`TUBE_SETTLE` = 3 s** to settle before the next
throw can be caught: the cadence data is batched at, the same for every tube (target: **news
crosses a relay link in 1-2 s**). Data across a tube takes half a settle (the wait for the next
throw) plus the capsule's crossing: a relay hop in a system about 1.5 s (the crossing is
microseconds), a 5 ly gate about 2.5 s (+ its relay's handling).

## 4. Typical gates

A typical gate spans **up to about 5 ly** (40 ly is an outlier). *Today's galaxy doesn't fit
yet:* its stars are a median 41 ly apart and the seeded lanes run 16.5-90 ly; 5 ly gates need
the real-density galaxy on the roadmap (neighbours about 4-5 ly apart).

## Open questions

1. **Natural speed and the owner's power:** should more holding power buy a faster tube (premium
   lanes, a neglected gate slowing down)? Leaning yes.
2. **Capsules as products:** casing mass and hardened storage density per brand (today a gate's
   1 kg and a relay's 10 g are tuning values in the world sheet).
3. **Gate economics in play:** owners paying to open and hold, fees by mass and speed, ships
   choosing their crossing speed (today every crossing is at natural speed, unpaid).
4. **Today's lanes are long** (16-90 ly in the 41 ly galaxy): a 100 t ship's natural crossing
   there is 2-14 minutes. The real-density galaxy (5 ly gates) brings it to under a minute.

*In code (2026-10-02):* Dogma's Tube laws (`config/dogma.ron`), `hyper::tube_*`; gate transits
take the natural time for the ship's mass and the lane; the hypernet's relay hops and gate data
cross in capsules at the settle cadence; the Dogma checks test the targets above.

## Numbers at a glance

| | |
|---|---|
| Speed limit at 1 AU from a star (open medium) | about 1,000 c |
| The wall (best drive, between stars) | about 12.5 kW/kg |
| A future explorer, 40 ly | about 10 days |
| Data through a gate, natural | 200 ms per ly (1 s for 5 ly) |
| A 100 t ship through a 5 ly gate, natural | 46 s, an S2 plant-hour |
| A capital ship (100 kt) through a 5 ly gate | 7.7 min |
| Rushing 2× / 3× / 10× | 2.7× / 7× / 8,000× the cost |
| Opening / holding a 5 ly gate | 3.2×10¹⁸ J / 12 GW |
| A relay hop in a system | about 1.5 s |
