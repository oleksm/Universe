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
- **Relays** are the same physics with thin tubes (capsules only, no ship fits): cheap to hold,
  carrying the hypernet between a system's sites.

### Holding a tube open

*Targets still to set* (the law's form and numbers): opening should cost colossal energy once,
holding it a great continuous power, enough that a faction is motivated to keep a gate up;
thin relay tubes far less. Today's Dogma has `P = 1 GW × (S/10 ly)³` for the old throat model;
it will be re-derived from those targets.

## 4. Typical gates

A typical gate spans **up to about 5 ly** (40 ly is an outlier). *Today's galaxy doesn't fit
yet:* its stars are a median 41 ly apart and the seeded lanes run 16.5-90 ly; 5 ly gates need
the real-density galaxy on the roadmap (neighbours about 4-5 ly apart).

## Open questions

1. **Holding a tube open:** the law's form and its targets (above).
2. **Natural speed and the owner's power:** should more holding power buy a faster tube (premium
   lanes, a neglected gate slowing down)? Leaning yes.
3. **Capsules as products:** casing mass and hardened storage density per brand.
4. **In code:** Dogma, the hypernet and gate transits still run the earlier model (a flat 10 s
   throat); they follow once these are settled.

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
