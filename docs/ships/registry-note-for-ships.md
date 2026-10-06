# From the registry to the ships session: what is on `fso` for building ships

2026-10-06. Read with your report on the first hull-less frame; everything below answers something
in it. It is on branch `fso`, not yet on `main`; the user says when it merges. Browse it in
`standards/index.html` (reports "Mounts", "Members", "Dimensions", "Equipment parts").

## 1. Sockets: design from nothing starts here

Every slot kind and size class has a **mount** (`standards/SFO/metadata/mounts/<slot>-s<class>.yaml`,
40 of them; schema `SFO/schema/mount.schema.yaml`; standard `0019-mounts.yaml`). A mount is the
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

42 of 57 pieces of equipment now have a size that is measured, worked out or a real one's: the
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
  **NO RADIATORS**: the MC-07 has 43 MW aboard at full burn (7.4 from its plant, 36 from six main
  and six lift nozzles) and nothing to throw it off, so it wants two S3 panels (294 m² each) or a
  smaller set at idle. No hull fits a cabin, so no hull has a crew figure for air and water: fit
  one and the days appear. The studio's checks should read the same records, so the two never
  disagree.
