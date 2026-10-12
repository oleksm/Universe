# ch.s4 design gate: answers (registry, 2026-10-11)

To blender `engine-ch-s4/blockout-v01/design-questions.yaml`. Record: `equipment.engine.ch.s4` on fso (140fa5b9); its
`function.chamber` and basis are the authority. Axes as the questions state: mount face at Blender Y0, exhaust -Y.

## R1 form, scope, mass

- Not the CH-S2 design: full-flow staged combustion, 30 MPa, mixture 3.6, copper channel liner in a steel jacket.
- Throat 0.232 m (not 0.26), exit 1.2 m inside (not 0.96), area ratio 26.8, 80% bell 1.45 m; chamber 0.4 m across,
  0.5 m injector face to throat. From the mount face: injector face -1.15, throat -1.65 (your figure holds), exit -3.1.
  Rim 1.3 m outside (50 mm turnaround manifold).
- Your 30 MPa screen needed a thrust coefficient of 1.44 because the record's 3.6 km/s was wrong; the exhaust is now
  3,290 m/s vacuum, 3,120 at sea level, 699 kg/s. A thrust coefficient of 1.81 in vacuum.
- Landed atmosphere: sea-level capable up to about Earth's air (exit 105 kPa at full thrust, 42 kPa at the 40%
  throttle). `vacuum_only: false`.
- 734 kg on 3 mm 304 plate is a mass proxy only (no copper liner stock or channel works yet). Draw the form, not the
  plate.

## R2 feeds (derived; inlet conditions invented, review)

| Feed | Flow | Inlet state | Bore at 7 m/s |
|---|---|---|---|
| Oxygen | 547 kg/s | liquid, 90 K, 1,141 kg/m3, about 0.5 MPa | 0.30 m |
| Methane | 152 kg/s | liquid, 112 K, 422 kg/m3, about 0.5 MPa | 0.26 m |

Methalox's 830 kg/m3 bulk density is this mixture of these two liquids. Do not reuse the CH-S2 hydrogen and oxygen
ducts or their angles as ratings.

## R3 mount and gimbal

- The whole engine swings, turbopumps included. So the moving feeds are the two low-pressure inlet ducts at the mount,
  upstream of the pumps: each with an articulated joint near the pivot plane (Y -0.35), not high-pressure lines.
- The engine owns its gimbal ring, actuators and thrust take-out (CHE4-04 and CHE4-01's share). The hull owns the
  mount's flange and struts.
- 0.26 rad is a radial limit: a 15-degree cone in any direction, the record's `function.gimbal`.
- The neutral static box stays 3.1 x 1.3 x 1.3. The full sweep is the hull's room to leave: the rim swings 0.71 m each
  way, about 2.7 m across.

## R4 motion

`freefall-motion/1` takes any record key; its gimbal `limit` must equal the record's 0.26. The installer checks it. The
game does not read motion data yet (engine's sequence). Ship a static neutral `model.glb` with `motion.json` beside it,
in the same frame and basis as the accepted CH-S2 package. Runtime visualisation stays held.

## Not changed

CH-S3 has the same unreachable 3.6 km/s; it gets the same treatment when its turn comes.

## Feeds through the trunnions (blender's proposal, 20261011T235512): accepted, provisional

The proposal routes oxygen and methane through opposite hollow pitch trunnions, then pitch-owned curved ducts to opposite
hollow yaw trunnions, then the moving pump inlets. Each axis has a coaxial cryogenic rotary union; the bores are 0.30
and 0.26 m.

This is physically sound as exterior scope. Each union only turns ±0.26 rad about its own axis, so no hose bends and
the motion is pure nested rotation (`freefall-motion/1` nodes, no hose entries). The pressure load acts along each
union's axis and is held inside the union. Seal qualification stays open (invented, review). Conditions for the
blockout:

- **Axes:** pitch and yaw axes intersect at the pivot (Y -0.35). The two unions on an axis are coaxial with it.
- **Trunnion pins carry the thrust:** 2.3 MN over two pins is 1.15 MN each.
  - Ti-6Al-4V hollow pins are about 0.36 m outside for the 0.30 m oxygen bore and 0.33 m for the 0.26 m methane bore.
  - Each pin bears on a lined journal about 70 mm long, at about 50 MPa (derived).
  - The bearing sits outboard of the seal. The union housing is about 0.5 m across: draw it at the trunnion ends.
- **Oxygen side:** no hydrocarbon lubricant (metal and PTFE seals only). Methane at 112 K and oxygen at 90 K: draw the
  ring ducts and union housings insulated or jacketed. The two propellants stay on opposite sides, each in its own
  trunnion.
- **Ownership:** the engine owns the pins, unions, gimbal ring and ring ducts, and the frame that holds the pitch
  bearings. That frame bolts to the hull's flange, which stays the hull's (R3 unchanged).
- **Clearance:** the ring ducts ride with pitch. The yaw unions and the pump inlets ride with the engine. Clear the full
  15° cone at every pose.
- **Flown practice:** engines of this class mostly use gimballed bellows ducts. Rotary unions are this design's
  choice; record them as such. The record's figures do not change.

## Pivot depth (blender v04 packing, 20261012T000433): Y -0.60 accepted, once measured clear

The pivot is not one of the record's figures. Only the basis note's geometry follows it, and that note now says 0.6 m.
Injector -1.15, throat -1.65, exit -3.1 and the 1.3 m neutral width stay.

- **Aft:** the rim's lever shortens to 2.5 m. It swings 0.64 m each way, about 2.54 m across at the rim (was 2.7): less
  room for the hull to leave.
- **Forward:** parts ahead of the pivot swing the other way and up toward the mount face. A moving part d ahead of the
  pivot at radius r stays aft of Y0 at full gimbal while `d cos 0.26 + r sin 0.26 <= 0.60`. Example: a moving edge
  0.5 m ahead (Y -0.10) may reach r 0.45 m, not 0.65. Taper the forward pump and union package to that cone, or the
  hull must leave room ahead of the face, and the record would have to say so.
- **Thin front plate:** the fixed frame that holds the pitch bearings reaches 0.6 m aft of the flange. It still carries
  2.3 MN to the flange, so it must stay engine-owned structure of real section, not just a plate. Size it for 2.3 MN
  plus the side load at 15 degrees (about 0.6 MN).
- **Pass condition:** no contact between fixed and moving groups at any pose of the full 0.26 cone (intended bearing
  seats separated out). Nothing crosses Y0.

## Swept bounds (blender v13, 20261012T001821): recorded

The swept envelope from blockout v13 (513 poses, pivot -0.60) is now in the record's basis: ±1.271 m in X and Z,
aft to Y -3.183, nothing at Y > 0. That matches the geometry: rim radius 0.65, 2.5 m behind the pivot, at 0.26 rad.
Hull bays leave a swept box 2.542 x 2.542 x 3.183 m. The four contacts left are each actuator against its own fixed
bracket, which are intended seats. Still open: the final seats, containment and export QC.

## Mass screen (blender candidate v01, 20261012T085544): recorded, flagged

The candidate's measured screen is 1,485 kg before the pump and turbine internals, against 1,630 kg in total. The nozzle
is counted as solid copper, 556 kg. That leaves 145 kg for the internals. The shaft power is about 55 MW (derived), and
turbopumps need roughly 270 to 550 kg for it (invented range). A hollowed channel liner, about 430 kg, frees about 120 kg.
The record keeps 1,630 kg, with a review note that it may be 0 to 400 kg short. It is not thinned to fit. The
geometry and detail scope are unchanged.
