# ch.s2 chamber: the Blender review's R1-R5 answered

`equipment.engine.ch.s2`, from the modeller's review (blender, engine-ch-s2/refinement-v06/review/registry-rework-d318249b.yaml):

- **R1 size**: CHE2-01 is 1.96 m long (chamber 0.35 + bell 1.61, injector face to exit) and 1.31 m across the rim (exit
  1.21 m inside, the 50 mm torus outside it). `exit_manifold` is the torus tube's outer diameter. The engine stays 2.3 m:
  0.34 m of pumps, valves and injector ahead. The rim swings 0.14 m each way at full gimbal.
- **R2 feed**: two coolant passes (`coolant_passes: 2`): down every other tube to the exit torus, back up the others, so
  the torus is a turnaround and every line sits at the chamber end, behind the hull's closure.
- **R3 stock**: plate stays as a mass proxy, said so; tube stock and a brazing works are not in the registry yet.
- **R4 mount**: the engine owns its gimbal ring and thrust take-out (in CHE2-01's 40 kg); the mount's flange and struts are
  the hull's.
- **R5 margins**: coolant 7 MPa (invented); hoop stress 27 MPa at the throat, 246 MPa at an unspliced exit (over 304's
  207 MPa), so the tubes branch 180 to 360 at area ratio 20: 123 MPa at the exit, margin 1.7. `vacuum_only: true`: 3.4 kPa
  at the exit; sea-level air would push back 117 kN against 110 kN of thrust.

Schema: `coolant_passes`, `tubes_lower`, `splice_expansion`, `coolant_pressure`, `vacuum_only` on the engine's `chamber`.
`next_asset` briefs now carry each part's description and the record's basis notes.
