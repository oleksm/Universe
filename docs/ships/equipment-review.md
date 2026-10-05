# Ship equipment: a review of what is on file

*Registry session, 2026-10-05. The user: "next we will need to work our spaceship modules: engines,
miners, sensors, everything." This is the look before the work: the 57 equipment records
(`standards/SFO/metadata/equipment/`) read against physics, against the game's code, and against
the MC-07 they are fitted to. No record is changed by it.*

## What is on file

57 products of ten makers, every one marked `outdated`, every figure a first guess from the game's
early list.

| Kind | Records | Makers |
|---|---|---|
| Power plant | 6 (two makers, three sizes) | Hadley, Aurel |
| Main drive | 4 | Kestrel |
| Thrusters, belly lift | 3 and 3 | Kestrel |
| Tank | 5 | Hadley |
| Hyperdrive | 6 (two makers, three sizes) | Kestrel, Halcyon |
| Capacitor | 3 | Meridian |
| Cargo rack, cabin | 4 and 4 | Hadley |
| Life support | 2 | Hadley |
| Flight computer, nav computer, transponder, radar, comm | 2, 2, 1, 1, 2 | Orbital, Tallis |
| Gun, laser | 1 and 1 | Garrick |
| Mining rig | 1 | Cormorant |
| Ground and station comms, relays, throat coil | 6 | Tallis, Halcyon |

## What holds for all of them

1. **None can be built.** No record says what it is made of or which works makes it. Since
   yesterday the registry can make steel, aluminium, copper, titanium, chips, boards, motors, pumps
   and heat pumps; nothing joins those to a drive or a radar.
2. **A size class is all that says how big it is.** 1 to 4, with a mass and a volume. The MC-07
   carries class 1 equipment in class 3 slots.
3. **Nothing sheds heat.** There is no radiator. The game's sheet says so: "radiators come as
   modules"; until then 1% of a ship's power draw reaches its skin and the rest vanishes.

## Kind by kind

### Main drive, thrusters, lift

The figures (torch S1): 900 kN a nozzle, exhaust at 10,000 km/s (3% of light), 30% of the fuel's
energy into the jet, burning deuterium.

What that is, on the MC-07 (six main nozzles, 154 t by its parts, 4 t of deuterium):

| | |
|---|---|
| Thrust | 5.4 MN: 35 m/s2, 3.6 g |
| Power in the jets | 27 TW. All of humanity uses about 19 TW |
| Heat that is not jet, at 30% | 63 TW |
| What its skin can shed at its 1,500 K limit (500 m2) | 0.000115 TW (115 MW) |
| Fuel | 0.54 kg/s: the tank lasts two hours at full thrust |
| Change of speed on a tank | 256 km/s |

- **Energy adds up:** 0.54 kg/s of deuterium fully burnt gives 186 TW; 27 TW of jet at 30% asks
  for 90 TW. So the drive burns about half of what passes through it. That is consistent.
- **Heat does not.** If one part in half a million of the 63 TW reached the ship, it would be at
  its limit. A torch like this is only possible if nearly all the loss leaves with the plume, as
  light and fast particles, and never touches the ship. The game computes the figure
  (`engine_heat`) and does not use it.
- **The lift is too weak** (known): 6 x 220 kN is 1.32 MN, and 154 t weighs 1.51 MN at one g.
- **A drive draws 700 kW** of electric power on its record: nothing next to its jet. What it is for
  (magnets, pumps) is not said.

**This is the one decision that shapes everything else** (see the end).

### Power plant

4 MW from 1.8 t (Hadley S1): 2.2 kW a kg. The registry's own fusion power station is 400 MW from
18,040 t of parts: 22 W a kg. **The ship's plant is a hundred times lighter for its power.** "Big
ships must be justified by physics": so must small plants. Either the station is a guess too heavy
(ITER's vessel, scaled) or miniaturising is a technology with a price that should be said.

Its waste heat (7.4 MW at 35%) is within what the hull can shed.

### Tank

4 t of liquid deuterium in a 350 kg tank (S0). Liquid deuterium is 162 kg a m3, so 25 m3, at 20 K.
A tank at 9% of its fuel's weight is light but not absurd. Its volume on the record should follow
from what it holds; it is not checked.

### Capacitor

5 GJ in 500 kg: 10 MJ a kg. Real capacitors hold 0.03, the best batteries 0.9, TNT 4.6. **An
invented technology,** and should be labelled so, or brought down.

### Hyperdrive

By Dogma's two invented laws of the field. Nothing to check against nature; it is what it says.

### Mining rig

No figures on the record. The game's code has them: the excavator works at 300 kW and carries off
10 kg/s at most; the anchor reaches 30 m and holds below 0.5 m/s. (The record draws 500 kW.) With
each rock class's cutting energy from the registry:

| Rock | Energy to cut | Rate | To fill the MC-07's 1,200 t |
|---|---|---|---|
| Rubble, any | 2 kJ/kg | 10 kg/s (the limit) | 33 hours |
| Ice | 20 kJ/kg | 10 kg/s | 33 hours |
| Stony | 60 kJ/kg | 5 kg/s | 67 hours |
| Nickel-iron | 400 kJ/kg | 0.75 kg/s | 18 days |

One product, one size. A miner's whole trade is here and it has one tool.

### Sensors

A radar: 100 kW, 500 km. The range is a constant in the game's code, the same for every ship. The
belt survey (new in the game) resolves a rock at ten million times its size out to 20 million km:
also a constant, invented, waiting for a sensor's record.

### Gun and laser

Figures in the game's code, none on the record.

- **Gun:** a 0.5 kg slug at 3 km/s, ten a second, 500 in the magazine. That is 2.25 MJ a shot and
  **22.5 MW at the muzzle; the record draws 50 kW.** Off by 450 times, unless it fires from a
  store of energy, and none is said. Recoil is 15 kN, small against the thrusters.
- **Laser:** 2 MW in the beam from a 2.5 MW draw: 80% efficient, where real ones reach 30 to 50%.
  It overheats in 8 s and cools in 4: its heat is the only heat in the game that is played.

### Life support, cabin

Life support S1: 800 kg and 100 kW, for a crew of no stated number. The registry now knows what a
person takes (0.9 kg of oxygen, 2.5 kg of water, 1.5 kg of food a day) and gives off; a life
support should say how many it keeps, and what it uses up or cleans.

A cabin: 150 kg and 2 kW a seat.

### Computers and comms

A flight computer of 200 kg drawing 50 kW; a nav computer of 100 kg and 10 kW; a transponder of
50 kg and 1 kW. Heavy and hungry for what they are (a rack of servers draws 5 to 10 kW), but
harmless. Comms reach 7.5 million km (basic) to 75 million (long): the figures are the hypernet's
and were worked through with it.

## What is missing, if "everything" is meant

| Missing | Why it matters |
|---|---|
| Radiators | The game's own note waits for them; heat is physics the ship should fly by |
| A survey sensor | Asked for by the engine |
| Sizes of mining rig; an ore processor aboard; a hopper | A miner's choices |
| Docking gear, landing gear | The MC-07 has both as parts, not as equipment |
| Batteries (real ones) | Against the invented capacitor |
| Repair, medical, crew quarters | People aboard |
| Tug gear, cargo handling | The roadmap's towing and hauling |
| Drone bays | The roadmap |

## How I would do the work

1. **Settle the drive** (the decision below). Its figures set the size of tank, plant, radiator
   and hull for every ship.
2. **Figures out of the code and onto the records:** gun, laser, mining rig, survey. A new figure
   on a kind of device changes the engine's generated handler for that kind, so the schema and
   the engine's code have to change in one step: the list is with the integrator
   (`docs/registry-ssot-response.md`).
3. **One kind at a time, each made real:** what it does by physics, sized from a need (as the
   MC-07's hull was sized from its loads), what it is built of, which works builds it, how long it
   lasts. In the order a ship is: drive and tank, power and radiator, mining rig and sensors, life
   support, the rest.
4. **Then sizes and makers:** a range of each kind, and the second maker where there is one, as
   real differences and not a letter.

## The decision: what a main drive is

| | A. The torch stays | B. A drive the ship can cool |
|---|---|---|
| What it is | Fusion plasma straight out the back at 3% of light; the loss leaves as light with the plume | Exhaust slow enough that its power is megawatts to gigawatts, and the ship sheds the loss itself |
| Flying | As now: 3 g for hours, anywhere in a system in days | Either low thrust for long burns, or high thrust for seconds |
| Physics | Honest only with one invented rule: how little of the plume's loss the ship takes. A ship near a running torch is in 63 TW of light: a weapon, and a hazard at any port | All real |
| Work | Small: a law, a figure on the drive, radiators for the rest | Large: flight, travel times and every hull change |

My recommendation is A, said honestly: one figure on a drive for the share of its loss the ship
absorbs (it must be under a millionth), a Dogma note that this is the invented part, and the plume
as something other ships must keep clear of. It keeps the hands-on flying, and it makes heat a
thing to play with instead of a thing left out.
