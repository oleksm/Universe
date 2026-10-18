# Pellet plant for airless ground (proposal)

Status: proposed by registry, 2026-10-15; records made the same day in `tools/standards/plants.py` as `layout.pellet-plant-airless`. This replaces the open question "sealed grate-kiln or fluidised-bed fines".

## What changed when the records were made

- **The loop runs at 4 bar, not 1.2.** At 1.6 m/s2 the shaft's bed weighs about 3.4 kPa a metre. At 1.2 bar the gas's drag through it would be about 5.4 kPa a metre, so the bed would lift and hang. At 4 bar the gas is denser and slower and its drag about 1.6 (Ergun, 12 mm pellets). This settles "Shaft gravity" below. The shaft is round, 4.7 m across, and its shell is a pressure vessel.
- **The hall is built of sections.** It is three 30 m sections (`unit.pressure-hall-30m`) and two heads (`unit.pressure-hall-30m-end`), so it fits the plant bays. It lies at grade on a sintered bedding with loose regolith packed against it, not sunk, and nothing covers it. Without a cover it never carries a load from outside, so buckling is not a case.
- **The green balls travel in a sealed gallery.** They go to the shafts in a 3 m tube at the hall's 20 kPa. In vacuum their water would boil off and freeze in their skins. The shafts stand in a 10 m pit so the gallery climbs less.
- **The heat leaves in the filtrate.** The mill's power ends as warm slurry, so a cooler on the presses' filtrate takes it to the radiator loop at 330 K. Fan coils in the sections take the rest.
- **The water is 3.3 kg/s, not 4.4.** That is the green balls' 9%. All of it is condensed in the loops and returned to the presses.
- **The ore comes in through its own lock.** `unit.ore-lock-hopper` stands outside the west head.

The site is airless, at about a moon's gravity (1.6 to 2 m/s2). The rate is the air plant's: 125 t of pellets an hour, for `layout.shaft-furnace-airless`.

## What vacuum breaks in the air plant

| Step in air | Why it fails in vacuum | Airless answer |
|---|---|---|
| Wet ball mill, slurry, open troughs | Water boils off below 2.3 kPa at 20 deg C and freezes as it does | The whole wet section stands in a closed hall at 20 kPa of nitrogen. That keeps water liquid to about 60 deg C (vapour pressure 20.0 kPa at 60 deg C). |
| Vacuum disc filters | They pull against the hall's 20 kPa, not 101: the drive falls to a fifth, so the area roughly doubles | Pressure filters (plate presses at about 6 bar), which do not care what is outside |
| Balling discs | Need the cake's 9% water to stay in it | Unchanged, inside the 20 kPa hall |
| Straight grate: an open hood, fans pulling outside air down through a moving chain | There is no outside air, and a moving chain of cars cannot be sealed | Two vertical shaft furnaces for pellets, sealed, with lock hoppers top and bottom, on a closed gas loop |
| Stack and baghouse | Nothing to vent into, and the water and oxygen are worth keeping | The gas loop is cleaned by a cyclone and candle filter; its water is condensed out and kept |
| Cooling tower | No air | Radiator fields, as the other airless layouts use |

## The wet hall

The wet hall is a steel tube 30 m across and about 90 m long, lying on its side and half sunk in a regolith cut. Its floor is a deck inside the tube.

- **Shell.** Hoop stress p r / t at 20 kPa and r 15 m needs only 2 mm against 150 MPa. Buckling, handling and the regolith cover set the wall instead: 6 mm, with ring stiffeners every 3 m. These figures are to be checked.
- **Why a full tube.** A vault on a slab would lift off: 20 kPa over 30 x 90 m is 54 MN of uplift. A full tube closes its hoop through its own bottom, so the foundation carries only its weight.
- **Access.** Airlocks at both ends for crews in suits, and a vehicle lock at one end.
- **Contents.** The mill, the plate presses and the four balling discs. The 30 t crane runs on rails hung from the stiffeners.
- **Pipes and belts in and out.** The ore comes in by a belt through a lock at the west end. The green balls leave by a lock hopper at the east end, toward the shaft furnaces.
- **Heat.** The motors' heat goes to the loop water through the motors' water jackets, because air at 20 kPa cools a fifth as well.

## The pellet shaft furnaces

There are two shafts of about 63 t an hour each, 1.1 Mt a year between them. Pellet shaft furnaces are the oldest induration method, and they work.

- **Process.** Green balls go in at the top through a lock hopper. Each shaft dries, preheats, fires at about 1,300 deg C and cools on the way down, counterflow to a closed gas loop. The gas is oxygen-enriched nitrogen at about 1.2 bar; the recipe's 4.5% oxygen comes from the hydrogen plant's oxygen. The fired pellets leave at the bottom through a lock.
- **Heat.** The gas that cools the pellets is the firing gas, so the heat comes back. The new heat is the module's 0.5 GJ a tonne, made by plasma torches heating the loop gas: about 19 MW, as in air.
- **Gas loop.** Top gas goes through a cyclone, then candle filters, then a condenser that keeps the 4.4 kg/s of water (it goes back to the mill), then a fan, and back to the bottom.
- **Heat rejection.** The condenser and the torches' cooling water reject to radiator fields. The radiator temperature still has to be chosen against the condenser's dew point.

## Units to add

1. `pressure-hall-30m`: the tube with its locks, deck and crane rails. It is a unit, because the layout's frame is a kit-strut lattice and this is a pressure shell.
2. `filter-press-2`: two plate presses, sized from the 33.2 kg/s of ore.
3. `pellet-shaft-63th`: one sealed shaft with its locks and torch ports.
4. `pellet-gas-loop`: the cyclone, the candle filter, the condenser and the fan.
5. The mill, the balling discs and the crane are reused, with `atmosphere: 20 kPa nitrogen` noted.
6. The radiator fields, the transformer, a crew post and the conveyors are reused as in the other airless layouts.

## Open

- **Shell gauge.** The wet hall's shell against its cover and its buckling has not been checked.
- **Shaft gravity.** It is not yet shown that a pellet shaft runs at 1.6 m/s2. The bed's weight against the gas's drag is the same question as in the airless shaft furnace (`needs.gravity` there).
- **Fine shaft numbers.** The shaft's diameter, heights and gas rate are from memory; to be derived as for `shaft-furnace-200th-hp`.
