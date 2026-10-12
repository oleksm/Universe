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
