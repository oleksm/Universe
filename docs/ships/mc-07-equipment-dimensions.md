# The MC-07's equipment: its size on record against a real one's

*Registry session, 2026-10-05. An audit, asked for by the user. No record is changed. The real
figures are from `standards/sources/research_ship_equipment.json` and `research_people_needs.json`
unless said otherwise.*

## What the records hold

- **A mass and a volume, and no dimensions.** No piece of equipment says its length, width or
  height: none can be placed in a hull, and nothing checks that it fits its slot.
- **Every one is 375 to 515 kg for each m3** (the tank apart). That is a rule of thumb applied to a
  weight, not a size anybody worked out.
- **All twelve together take 67 m3 of the hull's 23,181 m3:** 0.3%.

## Piece by piece

| Equipment | On record | As a box | A real one | Apart by |
|---|---|---|---|---|
| Torch drive S1 | 3.5 t, 8 m3 | 2.0 m cube | 27 TW of jet at the 14 kW a kg of NASA's Discovery II engine: 1.9 million t. That engine's nozzle alone is 12 m long, for 4.8 GW | 550,000 times the weight |
| Belly lift S1 | 1.1 t, 2.5 m3 | 1.4 m cube | 6.6 TW the same way: 465,000 t | 420,000 times |
| Thruster quads S1 | 0.7 t, 1.5 m3 | 1.1 m cube | 0.6 TW the same way: 42,000 t | 60,000 times |
| Fusion plant S1 | 1.8 t, 3.5 m3 | 1.5 m cube | The turbine set and radiators alone for 4 MW: 4.2 t (Discovery II's is 30 t for 28.6 MW), with about 70 m2 of radiator at 1,000 K. No fusion reactor this small has been designed | over 2 times, before any reactor |
| Hyperdrive S1 | 2.5 t, 6 m3 | 1.8 m cube | Nothing real | not to be judged |
| Fuel tank 4 t | 0.35 t, 40 m3 | 3.4 m cube | 4 t of liquid deuterium is 24.6 m3: a ball 3.6 m across inside. A real tank is 10 to 11% of its fuel's weight | agrees (9%; 40 m3 leaves room for insulation) |
| Life support S1 | 0.8 t, 4 m3 | 1.6 m cube | The space station's water and oxygen plant: 2.06 t in three racks (4.7 m3) for a crew of six | agrees, for a crew of two or three |
| Radar S1 | 0.3 t, 0.8 m3 | 0.9 m cube | A fighter's radar is 98 to 135 kg in 0.1 m3, for 150 km. A radar satellite's antenna is 880 kg, 12.3 m by 0.8 m. A radar is an area, not a box: 0.8 m3 is 8 m2 of array 10 cm thick | weight agrees; its shape is wrong |
| Comm S1 | 40 kg, 0.1 m3 | 0.5 m cube | The Mars Reconnaissance Orbiter's radio: 94 kg with a 3 m dish, 360 W. It reaches 400 million km to a 34 m dish on the ground | half the weight; a dish does not fit 0.1 m3; it draws 2 kW against 360 W |
| Flight computer S1 | 200 kg, 0.4 m3 | 0.7 m cube | A spacecraft's computer board draws 10 W; a star tracker weighs 3 kg, an inertial unit 4 kg. A whole set in threes: perhaps 30 kg and 200 W | 7 times the weight, 250 times the power (50 kW) |
| Nav computer | 100 kg, 0.2 m3 | 0.6 m cube | The same kind of thing | 3 times the weight, 50 times the power (10 kW) |
| Transponder | 50 kg, 0.1 m3 | 0.5 m cube | A deep-space transponder: 3 kg, 16 W | 17 times the weight, 60 times the power (1 kW) |

(The mining rig, which the MC-07's fit does not carry though it is a mining ship: 2.5 t and 6 m3
on record. A real cutting machine with a 300 kW head, the Sandvik MT720, is 130 t and 19.4 m long.)

## What it comes to

**Three kinds of finding.**

1. **The drive, lift and thrusters are the invented technology.** By the only fusion engine ever
   designed in detail they are tens of thousands to half a million times too light. This is known
   and kept (the review's option A); the figure says by how much.
2. **The plant is too small even before its reactor,** and has nowhere to put 70 m2 of radiator.
3. **The electronics are the other way about:** computers, transponder and comm are several times
   too heavy and tens to hundreds of times too hungry. Together they draw 63 kW where real ones
   would draw about half a kilowatt.

**Two agree with real ones:** the tank and the life support.

**And shape is missing everywhere.** A radar is a flat array, a comm is a dish, a tank is a ball,
a drive is long. One volume says none of that.

## If the records were to follow real ones

| Equipment | Would be | Why |
|---|---|---|
| Flight computer | about 30 kg, 0.05 m3, 200 W | three computers, three inertial units, three star trackers |
| Nav computer | about 15 kg, 0.03 m3, 100 W | one computer and its sensors |
| Transponder | about 3 kg, 0.005 m3, 20 W | a deep-space transponder |
| Comm S1 | about 90 kg; a 3 m dish (7 m2, 0.5 m deep stowed) and a 0.05 m3 box; 360 W | the Mars orbiter's |
| Radar S1 | 300 kg kept; an array of 8 m2 by 0.1 m | by its area |
| Life support S1 | kept; say it keeps three | the station's, a third of it |
| Tank 4 t | kept; a ball 3.8 m across outside | by its fuel |
| Fusion plant S1 | at least 4.2 t and 70 m2 of radiator, or said to be invented, as the drive is | the turbine set alone |
| Drive, lift, thrusters | kept as invented; but given a length and a nozzle's width, since they are what a ship is shaped round | |

That would take about 300 kg and 62 kW off the ship, and add 2.4 t to the plant.

## What is needed to do it

- **Dimensions on equipment:** length, width, height (the common `physical` group has them; no
  equipment record uses them). And for what is not a box, what it is: an area, a dish's width.
- **The user's word:** the MC-07 is deferred until it is flown, and these are its equipment.
  Nothing here is done until then.
