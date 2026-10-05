# The MC-07: what its drive, tank and lift must do

*Registry session, 2026-10-05. The first sizing from need: not "what does the list offer" but
"what must this ship do, and what does that take". No record is changed: the fit is the user's to
settle (it changes how the ship flies).*

## What a mining ship must do

Leave a port, reach the belt, fill its hold, come back, and set the ore down. For the MC-07 of
Port Trethi (on Treistun f, 1.13 AU from the star, no air, 4.06 m/s2 at the ground), with the main
belt 0.8 AU from the star: the belt is 0.35 AU away at the nearest and about 2 AU on the far side.

The ship by its record: 154 t of structure and equipment, a hold of 1,200 t of ore, a tank of 4 t
of deuterium, six main nozzles and six lift nozzles, slots of size 3 for drive, lift and plant and
of size 4 for its tank. **It is fitted with size 1 equipment in every one of them,** the cheapest of
each, as the game's first list had it.

## 1. The lift: it cannot land what it mines

To hold a ship against a world's pull takes its weight; to land and take off with a margin, half
as much again.

| Where | Pull at the ground | Lift needed, empty | Lift needed, hold full |
|---|---|---|---|
| The moons with ports (Aipika, Fabindum, Seiwiti, Sirnendis, Weisonum) | 1.1 to 2.2 m/s2 | 0.5 MN | 4.4 MN |
| Port Trethi (Treistun f) | 4.06 | 1.0 MN | 8.3 MN |
| Port Zaudalein, Port Nacaubun, Port Eikir (b, e, d) | 9.4 to 10.5 | 2.2 to 2.5 MN | 19 to 21 MN |
| Port Lisaur (Treistun c) | 16.4 | 3.9 MN | 33 MN |

**Fitted: 1.32 MN** (six nozzles of 220 kN). It lifts the empty ship from the moons and, barely,
from Port Trethi. It cannot bring a full hold down anywhere but a small moon, nor land empty on
any of the four large worlds.

The size 3 lift its slot takes gives 9.6 MN: a full hold onto Port Trethi and every moon, and the
empty ship onto any world but Treistun c.

## 2. The tank: a full hold goes nowhere

With exhaust at 10,000 km/s, the change of speed a tank gives is 10,000 km/s times the logarithm
of (mass with fuel over mass without).

| | Change of speed | The near belt and back to rest (0.5 AU) | The far belt (2 AU) |
|---|---|---|---|
| 4 t tank, empty hold | 256 km/s | 7 days | 27 days |
| 4 t tank, **full hold** | **29 km/s** | **59 days** | **235 days** |
| 80 t tank, empty hold | 4,180 km/s | 1.4 days | 2.3 days |
| 80 t tank, full hold | 574 km/s | 3.8 days | 13 days |

(Each is one way on a whole tank: half to get up to speed, half to stop.)

**The 4 t tank carries the empty ship out in a week or a month and brings the full one home in
two to eight months.** A hold of 1,200 t wants a tank to match. The size 4 tank its slot takes
holds 80 t: 493 m3, against the 669 m3 of the hold and the 23,000 m3 of the hull.

To bring a full hold 2 AU in ten days takes 97 t of fuel for the one leg: no tank on the list.

## 3. The drive: enough, and far too much

5.4 MN on 158 t is 3.5 g empty; on 1,358 t with a full hold, 0.4 g. Either is plenty: the tank
runs out long before thrust matters (a full burn empties the 4 t tank in two hours). The drive
does not need to grow. What it needs is the honest account of its heat (the review).

## 4. The plant: enough

What the fitted equipment draws when all of it works: drive 0.7 MW, hyperdrive 0.9, mining rig
0.5, lift 0.15, thrusters 0.1, life support 0.1, radar 0.1, computers 0.06: **2.6 MW of the plant's
4 MW.** Its own waste heat, 7.4 MW, takes about 70 m2 of radiator at 1,000 K (a carbon panel at
1.5 kg a m2: 110 kg); the 1,800 kg plant's parts list allows 46 kg for it. Small, and to be put
right when radiators are a thing of their own.

## What follows

| | As fitted | By need | On the list |
|---|---|---|---|
| Lift | 1.32 MN | 8.3 MN to land a full hold at Port Trethi | Belly lift S3: 9.6 MN, 7 t |
| Tank | 4 t | 80 t or more to bring a full hold home in under two weeks | Fuel tank S4: 80 t, 5 t |
| Drive | 5.4 MN | 0.4 g with a full hold is enough | Torch S1, as fitted |
| Plant | 4 MW | 2.6 MW | Fusion plant S1, as fitted |

With the S3 lift and the S4 tank the ship weighs 165 t empty and 1,445 t full and fuelled; the
lift it then needs at Port Trethi is 8.8 MN, inside 9.6.

**Or the hold is wrong.** Its 1,200 t is the registry's own figure: the bay under its doors, taken
as 8 m deep and never measured, full of stony ore (`docs/ships/mc-07-to-measure.md`). At 5 m deep
it is 750 t. And the game flies the ship at 88 t, not 154. The three figures (hold, tank, lift)
have to be settled together.

## Deferred (the user, 2026-10-05)

"Defer MC to once we start fitting it, keep draft. I want to hit the issue from the actual try-fly
side, then we will rework." So: nothing here is acted on. The MC-07's record stays a draft and its
fit stays as it is. The three questions that were asked (does it land with its ore, how long may a
run take, fit it to its slots) wait for the flying.
