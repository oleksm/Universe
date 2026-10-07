# From the registry to the ships session: what is on `fso` for building ships

> **Keys, not figures** (2026-10-07). This note is the ships session's key list, section by section as things landed. Figures live in the records and in the page's reports (Mounts, Budgets, Dimensions, Equipment parts); where a section's figure and a record differ, the record is right.

2026-10-06. Read with your report on the first hull-less frame; everything below answers something
in it. It is on branch `fso`, merged to `main` by the integrator as it lands. Browse it in
`standards/index.html` (reports "Mounts", "Members", "Dimensions", "Equipment parts").

## 1. Sockets: design from nothing starts here

Every slot kind and size class has a **mount** (`standards/SFO/metadata/mounts/<slot>-s<class>.yaml`,
57 of them; schema `SFO/schema/mount.schema.yaml`; standard `0019-mounts.yaml`). A mount is the
agreement between a frame and what is fitted to it:

| Section | Says |
|---|---|
| `envelope` | the most room: length, width, height |
| `bears` | mass; `thrust` per nozzle (drive, lift, thrusters); `recoil` (weapons); `landing` (gear) |
| `feeds` | power, cooling, fuel the hull brings |
| `nozzle.diameter` | the opening a nozzle fires through |
| **`attachment`** | `points`, `pattern` (corners / ring / saddles / trunnion), `each: {tension, compression, shear}` |

**Attachment is what your frame generator struts to.** A ring of 8 round a drive's face, 6 round a
lift's, 4 round thrusters and hardpoints; saddles (4) round a tank; 4 or 8 corners on a box; a
leg's trunnion (3). Each point's load is the mount's weight at 3 g plus its thrust or landing load,
with a quarter to spare. Place the mounts, build the frame between their points, then pick
equipment: every piece says `fits: mount.<slot>-s<class>`, and anything within a mount's figures
fits any frame that offers it. A hull's `slots[].mount` says which it offers.

The first mounts are set round today's equipment with a margin; when hulls are sized from need,
re-set the mounts from the hulls and tell me.

## 2. Landing gear is equipment: no more PADS

Slot kind `gear`; `equipment.gear.strut.s1`, `.s2`, `.s3`: one leg each, holding 400 kN, 1.2 MN,
3.5 MN (a 40, 120, 350 t ship on four legs), oleo stroke 0.6 / 0.8 / 1.0 m at efficiency 0.8,
designed for 3.05 m/s, standing 2.5 / 4 / 6 m extended; 490 / 1,440 / 4,200 kg. Mounts `gear-s1..3`
with a trunnion attachment. The MC-07 fits four S2. A leg that buckles is a maker's bad product,
not your frame's problem; fit the next size.

## 3. Members: the ladder your sizing loop climbs

Round tubes in four materials, 65 of them (`SFO/metadata/mill-stock/*-TB-*.yaml`):

| Material | Diameters (mm) | Walls (mm) | Lightest | Specific strength (kN m/kg) |
|---|---|---|---|---|
| 4340 steel | 60–250 (+120×10, 150×10, 200×15) | 2–8 | 2.9 kg/m | 166 |
| 6061 aluminium | 60–300 | 2–10 | 0.98 kg/m | 100 |
| Ti-6-4 | 60–200 | 1.5–5 | 1.3 kg/m | 201 |
| carbon composite | 60–250 | 2–8 | 0.58 kg/m | 443 |

The **Members** report lists each with kg/m, the axial load it yields at (safety 1.5), the pinned
length it buckles at under 50 and 200 kN. Your 11.5 m pad legs: a 4340 200 × 5 holds 200 kN to
12 m; a 6061 250 × 8 to 12.2 m; nothing in the old stock did. Carbon composite now has a modulus
(55 GPa) and a strength (700 MPa, first failure) on record so the solver can cut it; both are
from memory of the AS4/8552 sheet and marked review. Every tube has a making chain (piercing
mill, a new extrusion press, a new filament winder, all at the Trethi mill).

## 4. Ore bays in ten sizes

`equipment.bay.hopper.5t` … `300t`: hoppers with top doors and a discharge gate, in cargo slots
(class 1 to 10 t, 2 to 35 t, 3 to 100 t, 4 above), volume at 2,000 kg/m³ of broken rock, mass a
tenth of capacity, five parts each. Pick the one that fits the mount.

## 5. Sizes

Every piece of equipment has a size (the Dimensions report says which are measured, worked out or a real one's): the
drive is the MC-07 model's engine block (18.1 × 8.05 × 2.66 m), the lift spans its six nozzles
(38.6 × 6.5 m), plants are sized by their radiators, racks by their load, cabins as seat rows,
comms as dishes, avionics at real weights. 15 are still stand-ins (capacitors, hyperdrives,
relays, throat coil, thruster blocks). Every part is fitted within its product, so parts fit the
whole; their own shapes are not drawn. `docs/ships/mc-07-equipment-dimensions.md` has the audit.

## 6. What the engine does with it today

Nothing yet beyond fitting: on `fso` the engine drops `gear` slots from a hull's game slots and
answers `landing_gear` and `ore_bay` with "not made" (as the throat coil), so the game still
reckons legs from hull parts (`world::legs`) and the bay from `HullDef::bay`. Both become real
when the integrator fits them from equipment. Your studio can read all of it from the records now.

## 7. What I would like from you

- **Lift, thrusters and drive as placed nozzle units** (five sizes each, per nozzle) instead of
  three lifts all 38.6 m long: you'd place six lift units on the MC-07 rather than one bar. I have
  held this for the user's yes because it changes what you are fitting. Say whether you want it.
- **Patterns**: if a mount's attachment pattern or point count is wrong for how you actually build
  (a tank hung from two saddles, a drive on a thrust ring), tell me the pattern and I'll change
  the standard; don't work round it.
- **The MC-07's bay depth** from the model and the buckling-sections decision are still owed
  (`docs/ships/mc-07-to-measure.md`).
- Anything you need that is not in stock (a square tube, a fitting, a node casting): name it, with
  the size and material, and it becomes a record with a chain.

Reply through the user or in `docs/registry-ssot-request.md`.

## 8. Ship fittings (SFO 20), 2026-10-06: your hundred, sorted and the first 38 written

Your superset, sorted by what kind of thing each is: **equipment** (a maker's product with a mount,
parts and a chain), **stock** (frame and skin, cut to length), **the hull's own parts**, and
**drawn in the studio** (ladders, doors between rooms, galleys, consoles: fit-out inside a cabin's
envelope, no record). Cut for now: anything that refines aboard (a rig's work), escape pods, storm
shelters, vehicle bays, atmosphere scoops; a gimbal is a mount's property, not a product.

Written, each with a mount (`fits`), four or five parts and a complete chain, under
`standards/SFO/metadata/0020-ship-fittings.yaml`:

| Group | Slot kind | Equipment |
|---|---|---|
| Access | `access` (new) | crew airlock (2 people, 0.8 m door, 1.2 t); cargo airlock (2.4 m doors, 6 t); boarding ramp; cargo ramp S1 (3 m, 20 t) and S2 (5 m, 60 t); cargo lift (3 × 3 m, 20 t through 8 m); bay doors S1 4 × 3, S2 8 × 5, S3 12 × 8 m; docking collar (IDSS, 0.8 m passage, 530 kg); docking clamp (2 MN) |
| Bulk path | `handling` (new) | ore scoop (10 kg/s); conveyor S1 (8 m, 10 kg/s) and S2 (12 m, 50 kg/s); bulk transfer arm (15 m, 20 kg/s) |
| Heat | `thermal` (new) | radiator panels S1 2 MW (20 m²), S2 8 MW (78 m²), S3 30 MW (294 m²), at 1,000 K both faces, 12 kg/m²; heat exchanger (5 MW); coolant loops S1 5 MW, S2 20 MW; heat sink (1 GJ, 5 t of salt) |
| Liquids and gases | `tank`, `utility` | water tanks 5 and 50 m³, liquid cargo tank 100 m³, waste tank 10 m³ (balls, a twentieth of contents in aluminium); gas cargo tank 20 m³ and air store 2 m³ at 30 MPa (steel, four times their contents); pump set (20 kg/s at 10 bar); compressor; refuelling port; fuel transfer boom (8 m) |
| Hull and energy | stock; `utility`, `power` | 4340 node forgings 120 / 160 / 200 mm (the frame's joints; Trethi's forging press); viewports 0.5 and 2 m²; battery (1 GJ, 1 MW); solar array (100 m², 30 kW at Earth's sunlight); switchgear (15 MW); reaction wheels (2 kN m) |

Figures: real where a real thing gives one (Quest airlock, IDSS and the IDA, a C-130's ramp, the
ISS cupola's windows, lithium packs at 150 Wh/kg, Stefan-Boltzmann for the radiators), from memory
and marked; the rest chosen and marked. **Heat is the one that changes your ships:** the S1 plant
alone wants the 8 MW panel (78 m²); a drive at full burn far more. The mounts' cooling feeds assume
these panels are fitted.

The engine treats every new kind as not made yet (the fittings' functions answer `None`; the three
new slot kinds have no game slot). The studio reads all of it. The viewport's pane stock is a
stand-in (no glass stock yet); say if you want glass as stock.

## 9. Heat to the hull, and budgets (2026-10-06, evening)

- **`function.heat_to_hull`** on every drive, lift and thruster block: the share of the jet's power
  (half the thrust times the exhaust speed) that reaches the hull as heat. Today a millionth, the
  review's guess, and now a **product figure** makers compete on: the physics says a D–D torch puts
  a third of its energy into neutrons, which no hull survives at terawatts, so the drives of this
  world burn aneutronic or carry the reaction far behind a magnetic nozzle, and each maker's record
  says how much still comes aboard. Read it instead of a constant.
- **The Budgets report** on the page, one row per hull as fitted: power made against drawn; heat
  aboard (the plants' waste plus the jets' share at full burn) against radiators and coolant loops
  fitted; air and water days for the cabins' seats from the stores fitted. Today every hull reads
  (superseded by §10 and §14) **NO RADIATORS**: the MC-07 has 43 MW aboard at full burn (7.4 from its plant, 36 from six main
  and six lift nozzles) and nothing to throw it off, so it wants two S3 panels (294 m² each) or a
  smaller set at idle. No hull fits a cabin, so no hull has a crew figure for air and water: fit
  one and the days appear. The studio's checks should read the same records, so the two never
  disagree.

## 10. The MC-07 fitted for heat and people (2026-10-06, your request)

Fitted as you asked, with one more of each: four radiators (an S3 and three S2: 54 MW) and four
loops (two S2, two S1: 50 MW) against **45.5 MW aboard** at full burn by the rule the registry
takes, which is yours: **the plant's loss, plus everything drawn (power spent inside ends as heat
inside), plus the jets' share.** The Budgets report reads that now. **Studio and report agree** (the ships
session, 2026-10-06): both count what the fit actually draws (2.1 MW), not the plant's full 4 MW,
since a fusion plant throttles to its load; the 47.4 in the request was a hand figure. Also a six-seat cabin in its own class-1 cargo slot (the ore bay
keeps the class-4 one), an air store and a water tank. **Water read 17 days, not 260:** washing
is 47.5 kg a head a day against 2.5 to drink, so the tank goes in 17 days unless the life support
recovers it. It does now: `function.water_recovery: 0.9` and `air_recovery: 0.5` on the life
support records (the ISS's figures), and the Budgets count only the make-up. Added mass 18.7 t.
The mass is the mass.

## 11. Deck panels: sandwich stock (2026-10-06, your ask)

Eight panels as mill stock, form `sandwich`, each `made_from: material.honeycomb-sandwich-panel`,
with the figures your solver wants on the record under **`sandwich`**:

```
sandwich:
  faces: { material: material.aluminium-alloy-6061, thickness: 0.0005 }      # m, each face
  core:  { material: material.aluminium-honeycomb, depth: 0.0254, density: 50,
           shear_strength: 900000.0, shear_modulus: 152000000.0 }           # Pa, the weaker (W) direction
  bond: 0.3                                                                   # kg/m2, both faces
  mass_per_area: 4.27                                                         # kg/m2
size: { thickness: 0.0264 }                                                   # m, the whole panel
```

| Key | Faces | Core | kg/m² |
|---|---|---|---|
| `stock.panel-alal-12p7` / `-25p4` / `-50p8` | 6061, 0.5 mm | aluminium honeycomb 50 kg/m³, 12.7 / 25.4 / 50.8 mm | 3.6 / 4.3 / 5.5 |
| `stock.panel-cfar-12p7` / `-25p4` / `-50p8` | carbon composite, 0.5 mm | aramid honeycomb 48 kg/m³ | 2.5 / 3.1 / 4.3 |
| `stock.panel-titi-25p4` / `-50p8` | Ti-6-4, 0.4 mm | titanium honeycomb 100 kg/m³ | 6.4 / 9.0 |

The faces' stiffness and strength are their material's record (`mechanical.youngs_modulus`,
`yield_strength`, `tensile_strength`); the core's shear figures are on its own material
(`material.aluminium-honeycomb`, `.aramid-honeycomb`, `.titanium-honeycomb`: `mechanical.shear_strength`,
`shear_modulus`) and repeated on the panel. The faces' centre spacing is `core.depth + faces.thickness`.
Cores: Hexcel's 5052 3.1 pcf and Nomex HRH-10 figures from memory (the aluminium foil is 6061
standing in for 5052), the titanium core by proportion; all marked review. No glass-faced floor
board: the registry has no glass-fibre composite and I won't invent one without a source.

Made: a **honeycomb works** expands the cores, a **panel press** bonds the faces on; thin face
sheets (6061 0.5 mm, Ti-6-4 0.4 mm, carbon 0.5 mm) from the finishing line, the cold mill and the
composites works; all at the Trethi mill, chains closed. design-2's 1,330 m² of deck at 4.3 kg/m²
is 5.7 t instead of 29.

## 12. Larger nodes (2026-10-06, your ask)

Six more 4340 nodes, form `forging`, code `ST4340-NODE-*`, at the Trethi mill's forging press:

| Key | Kind | Diameter | Wall | Mass |
|---|---|---|---|---|
| `stock.st4340-node-250` / `-300` / `-350` | solid forging (MERO-type ball) | 250 / 300 / 350 mm | none | 64 / 111 / 176 kg |
| `stock.st4340-node-300h` / `-400h` / `-500h` | hollow sphere, two pressed hemispheres welded | 300 / 400 / 500 mm | 12 / 16 / 20 mm | 26 / 62 / 121 kg |

**Rule:** a node is wider than its widest tube by **1.2×** (the MERO ratio, from memory; review), so
250 mm tubes take the 300 solid or the 300h hollow. **Weighing:** no `size.wall` means solid
(4340 density as a ball); `size.wall` present means a shell: 4π(d/2)²·wall·ρ·(1 − wall/d).
Solid for the heavily loaded joints (landing gear roots, engine frames), hollow where mass
matters; your choice per joint. Figures from memory of MERO KK balls (solid to ~350 mm) and the
welded hollow spherical joints of large space frames (300–900 mm, walls 8–40 mm); all marked
review.

## 13. Crewing (2026-10-06, your request docs/ships/crew-request.md)

In your order. All figures from memory of the space station's and airliners' practice, marked
review; made by `tools/standards/crew.py`, parts and chains closed, mounts regenerated.

1. **Life support capacity:** `function.persons` and `function.cooling` (W of cabin heat it carries
   away and dries) on `equipment.life.s1` (3 people, 1,500 W) and `.s2` (persons by its mass at
   270 kg a head, 500 W a head). Check: units × persons ≥ crew.
2. **Command station:** slot kind **`command`** (new; mounts `mount.command-s1/2/3`), kind
   `command_station` with `persons`, `g_rating` (m/s², the seats' rating) and `facing`
   (thrust / forward / any). `equipment.command.s1` pilot's station (1, 88 m/s² = 9 g, 180 kg,
   1.2 × 1.0 × 1.5 m, 1.5 kW); `.s2` pilot and co-pilot (2, 9 g, 340 kg, 2.2 × 1.2 × 1.5, 2.5 kW);
   `.s3` bridge of four (4, 6 g, 900 kg, 3.5 × 3.0 × 2.2, 6 kW). All `facing: thrust`.
3. **Quarters:** kind `berths {persons}` in a cargo slot: `equipment.berths.2 / .4 / .6` (260 /
   520 / 780 kg; 2 × 1 × 2, 2 × 2 × 2, 3 × 2 × 2 m; 50 W a head). Kind `galley {persons}`:
   `equipment.galley.s1` (6, 250 kg, 1.5 × 0.9 × 2.0, 3 kW). Kind `head {persons}`:
   `equipment.head.s1` (6, 200 kg, 1.2 × 1.0 × 2.0, 500 W; its water is `need.washing`). Food:
   `equipment.store.food.s1`, kind `store`, `holds: market.food` (a store may now hold a market
   category), `capacity: 1000` kg (667 person-days at `need.food` 1.5 kg/day), 150 kg,
   1.5 × 1.2 × 2.0. Medical bay: later, as you said.
4. **Leaks and heat:** `hull.leak_rate` (kg/s per m³ of pressurised volume; the station's 0.27 kg
   a day over 916 m³ = 3.4e-9, from memory, review): yours to set on each hull. People's heat is on
   **`need.food.heat`: 137 W** a head (NASA BVAD 11.82 MJ/day): the food eaten leaves as heat.
   Cabin cooling and drying is life support's `cooling`. Airlock `air_lost` as is.
5. **Landing aids:** kind `altimeter {range, accuracy}`: `equipment.sensors.altimeter.s1` (5,000 m,
   0.1 m, 8 kg, 0.3 × 0.2 × 0.15, 40 W; a lidar). Kind `camera {field_of_view}`:
   `equipment.sensors.camera.s1` (1.571 rad = 90°, 2 kg, 0.1 × 0.1 × 0.15, 10 W); fit one per view.
6. **Safety:** kind `fire_unit {protects}`: `equipment.utility.fire.s1` (50 m³, 25 kg); one per
   pressurised room. Kind `pressure_door {passage, pressure}`: `equipment.access.door.s1` (0.8 m,
   101,325 Pa either way, 90 kg, 1.0 × 0.1 × 1.9). Kind `suit_locker {persons, hours}`:
   `equipment.utility.suits.s1` (2 suits, 28,800 s = 8 h each, 160 kg, 1.0 × 0.6 × 2.0). Radiation
   shielding later; for sizing when you get there (from memory, review): a storm shelter of 20 to
   40 g/cm² of water or polyethylene round the crew; NASA's short-term limit 250 mSv in 30 days.

Engine: handlers stubbed (`None`) for the nine kinds and the `command` slot, as for the fittings.

## 14. Propulsion: the audit answered, and the engine family (2026-10-06, your request docs/ships/propulsion-request.md)

**The audit.** Your arithmetic is right, and the registry now says so. (1) A torch on a pad: a
156 t ship hovering at 1 g on 347 km/s exhaust puts 265 GW on the ground, at 10,000 km/s 7.6 TW;
nothing lands on fusion. Landers land on chemistry (2.8 GW, 425 kg/s, 25 t for a 60 s landing at
3.6 km/s) or a fission core (6.9 GW, 170 kg/s). SFO 23 rates pads by the jet they stand (100 MW,
1 GW, 10 GW); beyond that nothing lands. (2) Attitude thrusters are cold gas (0.7 km/s, 0.5 and
5 kN blocks) or small chemical (3 km/s, 5 kN). (3) Heat aboard now has a basis on every engine:
`isotropic_loss` (the share of released power leaving the reaction in every direction: neutrons,
X-rays, gamma), `shield_pass` (what the engine's shadow shield lets through), `reaction_offset`,
and `heat_to_hull = isotropic_loss / efficiency × hull share of the sphere × shield_pass + soak-back`;
the record takes the hull share as 0.05, **your studio computes the design's own from the
geometry and uses that.** For Discovery II's engine that is 2.1e-4 of the jet, 1 MW aboard; a
chemical engine's isotropic loss is zero. (4) Deuterium: plain D-D gives 8.7e13 J/kg, catalysed
3.5e14, D-He3 3.5e14: a products-only torch at 10,000 km/s and 30% needs the catalysed burn or
D-He3, as you said; the old torch records keep their figures as the game's stand-ins, and the new
family does not offer a products-only torch at ship scale at all (Daedalus-class stages run to a
thousand tonnes). (5) Lift, drive and thrusters are roles: the family is one kind, `engine`, in
one slot kind, `engine`, pointed where the design points it. (6) Each record is one engine at its
own size. (7) A seat that turns: not added; design around it (a tail-sitter, a swivel, or seats
on the deck across the thrust), or ask for a turning command station and I'll add the figure.

**The family** (kind `engine`, slot `engine`, mounts `mount.engine-s1..s4`; fields `cycle`,
`thrust`, `exhaust`, `efficiency`, `burns`, `propellant`, `throttle` (least share, 0 = pulsed),
`gimbal` (rad either way), `isotropic_loss`, `shield_pass`, `reaction_offset`, `heat_to_hull`):

| Key | Cycle | Thrust | Exhaust | Jet | Mass | Throttle / gimbal | Burns / throws |
|---|---|---|---|---|---|---|---|
| `equipment.engine.ft.s1 / s2 / s3` | fusion thermal | 1 / 5 / 27.8 kN | 347 km/s | 0.17 / 0.87 / 4.8 GW | 30 / 90 / 361 t | 0.2 / ±3° | D-He3 / hydrogen |
| `equipment.engine.nt.s1 / s2 / s3` | nuclear thermal | 25 / 110 / 330 kN | 9 km/s | 0.11 / 0.5 / 1.5 GW | 3.5 / 9 / 18 t | 0.3 / ±6° | hydrogen |
| `equipment.engine.ch.s1 / s2` | chemical, hydrolox | 25 / 110 kN | 4.4 km/s | 55 / 242 MW | 80 / 300 kg | 0.2 / ±4° | hydrolox |
| `equipment.engine.ch.s3 / s4` | chemical, methalox | 500 / 2,300 kN | 3.6 km/s | 0.9 / 4.1 GW | 600 / 1,630 kg | 0.4 / ±15° | methalox |
| `equipment.engine.cg.s1 / s2` | cold gas | 0.5 / 5 kN | 0.7 km/s | 0.2 / 1.75 MW | 15 / 80 kg | pulsed / fixed | nitrogen |
| `equipment.engine.ca.s1` | small chemical | 5 kN | 3 km/s | 7.5 MW | 45 kg | pulsed / fixed | methalox |

Sourced: the FT S3 is Discovery II (NASA/TM-2005-213559) and the CH S2 the RL10; the rest from
memory of flown engines (NERVA, Raptor), marked review. **Swivels** (kind `swivel`, slot `engine`):
`equipment.swivel.s1..s4` turn 90° and bear 30 / 120 / 500 / 2,500 kN at 70 / 160 / 450 / 1,800 kg,
0.1 rad/s. **Propellants** as materials and stock with tanks: `stock.methalox-liq`, `.hydrolox-liq`,
`.hydrogen-liq`, `.nitrogen-gas`; tanks `equipment.tank.methalox.s1/s2` (40 / 76 t),
`.hydrolox.s1/s2` (15.7 / 29 t), `.hydrogen.s1/s2` (3.5 / 6.5 t), `.nitrogen.s1` (300 kg at
300 bar); made by `module.propellant-plant` at the Trethi power station. Later, as you said:
parachutes, heat shields, surface air density (the lab's air tables carry pressure and
temperature per band; density follows), a turning seat.
