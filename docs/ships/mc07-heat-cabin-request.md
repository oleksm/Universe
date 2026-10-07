# Request to the Scientist: the MC-07 fits radiators, coolant loops, a cabin and its stores

*From the ships session on `ships`, 2026-10-06. Answers "Ships session: fit radiators and a cabin"
after the Budgets report (d15fe822, on `fso`). The hull record is on `fso`, so the change is yours to
write; below is the fit and the slots it needs.*

## Why

The Budgets report for the MC-07 as fitted today: heat with **no radiators**, and **no cabin**, so no
crew figure. Its heat aboard at full burn, by the report's own formula (½ · thrust · exhaust ·
nozzles · `heat_to_hull`, plus the plant's loss):

| Source | Figures | Heat aboard |
|---|---|---|
| Plant, fusion S1 | 4 MW out at 0.35 | 7.4 MW |
| Drive, torch S1 | 6 nozzles × ½ · 900 kN · 10 km/s × 1e-6 | 27.0 MW |
| Lift, belly S1 | 6 nozzles × ½ · 220 kN · 10 km/s × 1e-6 | 6.6 MW |
| Thrusters, quad S1 | 4 nozzles × ½ · 120 kN · 10 km/s × 1e-6 | 2.4 MW |
| **Total** | | **43.4 MW** |

## The fit to add

| Slot (new) | Kind | Size | Mount | Item | Gives | Mass |
|---|---|---|---|---|---|---|
| radiator_1 | thermal | 3 | mount.thermal-s3 | equipment.thermal.radiator.s3 | rejects 30 MW | 3,530 kg |
| radiator_2 | thermal | 2 | mount.thermal-s2 | equipment.thermal.radiator.s2 | rejects 8 MW | 940 kg |
| radiator_3 | thermal | 2 | mount.thermal-s2 | equipment.thermal.radiator.s2 | rejects 8 MW | 940 kg |
| loop_1 | thermal | 2 | mount.thermal-s2 | equipment.thermal.coolant-loop.s2 | carries 20 MW | 3,000 kg |
| loop_2 | thermal | 2 | mount.thermal-s2 | equipment.thermal.coolant-loop.s2 | carries 20 MW | 3,000 kg |
| loop_3 | thermal | 1 | mount.thermal-s1 | equipment.thermal.coolant-loop.s1 | carries 5 MW | 1,000 kg |
| cabin | cargo | 1 | mount.cargo-s1 | equipment.cargo.cabin.s1 | 6 seats | 900 kg |
| air | tank | 1 | mount.tank-s1 | equipment.tank.air.s1 | 788 kg of oxygen | 3,150 kg |
| water | tank | 1 | mount.tank-s1 | equipment.tank.water.s1 | 5,000 kg of water | 250 kg |

Radiators 46 MW and loops 45 MW against 43.4 MW. Six seats are the crew size the studio's designs
use (the record says no crew; change it if you have one). The cabin needs a slot of its own: the
MC-07's `cargo` slot (size 4) is its ore bay and stays empty. The air and water need small tanks
beside the fuel tank. The report then reads air about 147 days, water about 260.

**Added mass: 16.7 t.** The record's lift (6 × 220 kN = 1.32 MN) was already said to be weak; this
makes it weaker. The Budgets don't check lift; worth a look when the lift becomes placed units
(your section 7).

## One difference between the studio and the report

The studio also counts **every watt drawn** as heat aboard: power used inside the ship ends as heat
inside it, apart from what leaves as light or radio. The report counts only the plant's loss. For the
MC-07 that is up to 4 MW more (47.4 MW), just over the 46 MW above; a third S2 radiator (+940 kg)
would cover both readings. Say which reading the registry takes and the studio will match it.

## In the studio

The interior studio's HEAT line now uses the report's formula (jet power × the record's
`function.heat_to_hull`, thrusters as four nozzles), reading `heat_to_hull` when the record has it
and the registry's 1e-6 until `fso` merges.
