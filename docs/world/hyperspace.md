# Hyperspace

*A world article: how the hyper layer works in the universe, all in one place. The laws are
Dogma's (`config/dogma.ron`, every constant marked Invented); the reasons and the rules of the
invented are in the charter (`docs/physics.md`). Kept current as the hyper layer changes.*

## In one breath

Space carries a **hyper-medium**. Near masses it's stiff and holds anything moving in it to a
speed limit that grows with distance from the nearest surface; far from them, between the
stars, it goes slack. A **field** (a ship's hyperdrive) moves through the stiff medium at up to
that limit, cheaply; in slack space holding a field at all costs power per kilogram, a wall no
ship today climbs. **Gates** cheat the distance: a pair of rings is one wormhole **throat**, held
open at a power that grows with the cube of its span. **Hyper-signals** carry the hypernet's news
through the medium between relays, in seconds where light takes hours.

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

## 3. The throat: gates

A gate pair is one wormhole: what enters one ring leaves the other.

| Law | Formula / value | Status |
|---|---|---|
| Holding the throat open | `P = P₀ · (S / S₀)³`, P₀ = 1 GW, S₀ = 10 ly: 10 ly 1 GW, 25 ly 16 GW, 50 ly 125 GW, 100 ly 1 TW | on paper |
| A transit's energy | `E = τ · m · S`, τ = 2.6×10⁻⁹ J/(kg·m): 1,000 t across 40 ly ≈ 10¹⁵ J (the gate's, not the traveller's) | on paper |
| Entry speed | at most 300 m/s, or the transit wrecks what goes in | simulated |
| Time through | 10 s, ring to ring, for anything (TRANSIT_TIME) | simulated |

- **Ring classes** (world content, not Dogma): a ring's class sets the longest throat it holds:
  I 10 ly, II 25 ly, III 50 ly.
- Why gates matter: across 40 ly a gate costs about 20 times less energy per kg than an
  explorer's crossing, any ship can use it, in seconds instead of days, without stranding.
- **Laying a lane:** the pair is built together, then one ring is hauled to the far star by an
  explorer's power: an expedition, a faction's or corporation's project.

## 4. Hyper-signals: the hypernet's carrier

- A relay's signal travels the medium **at its limit along the line**: its time is `∫ ds / v_lim`,
  slow climbing away from each world, fast across the open middle. A 1 AU hop: about 10-15 s
  (light: 499 s). Each relay adds its handling (0.1 s).
- *Simplified:* a signal is reckoned at least 10,000 km from any surface (a site sits in orbit,
  not on its world).
- Through a gate a signal takes the throat's 10 s, as matter does (plus the gate relay's 1 s).
- See `docs/hypernet.md`: transceivers (the towers ships connect to, 0.5 AU) and hyper relays (the
  links between sites, the shortest network all told, planned daily).

## Open questions

1. **Why 10 s through a throat?** Asserted, not derived. It could follow from a throat length and
   the 300 m/s limit (300 m/s × 10 s ≈ a 3 km throat), perhaps longer for a bigger ring.
2. **Should signals cross a throat almost at once?** A signal isn't bound by the 300 m/s matter
   limit; through a 3 km throat at light speed it would take microseconds. Gate crossings are
   today the larger part of news between systems.
3. **Should a relay's energy matter?** The charter says a signal in open space goes "as fast as its
   energy buys"; today only the geometry counts. In a system that's consistent with ships (the
   limit binds); energy would only bite on links through slack space between stars.
4. **Gates' power and transit energy aren't simulated:** no powerplant, no fuel, no fees; lanes
   don't go dark. Natural with factions owning gates.

## Numbers at a glance

| | |
|---|---|
| Speed limit at 1 AU from a star | about 1,000 c |
| The wall (best drive, between stars) | about 12.5 kW/kg |
| A future explorer, 40 ly | about 10 days |
| A gate, 10 / 25 / 50 ly held open | 1 / 16 / 125 GW |
| 1,000 t through a 40 ly gate | about 10¹⁵ J |
| Through a throat | 10 s, entering under 300 m/s |
| A hyper-signal across 1 AU | about 10-15 s (light 8.3 min) |
