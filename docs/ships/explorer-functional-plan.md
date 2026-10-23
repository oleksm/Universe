# Explorer: what makes it functional (engine dependencies, 2026-10-23)

Blender `20261023T182845-blender-aa66` (the owner: define what's missing so the explorer works). The export is
crew-hatch-01 (packet `install-20261023`: open 1399990f, closed 50a33c16, source b29b2470). Two levels for each part:

- **Minimum functional:** fly it, land it, board it and leave it, on main's physics. Mechanisms take their authored
  end poses (stowed / deployed) as states. Nothing replaced by a stand-in.
- **Full:** the authored mechanisms move between their poses, with their sweeps checked.

No authored mechanism is replaced silently. Where the game can't move it yet, it shows the authored pose that the
current state calls for, and says so in `ClassSpec::limits`. Owners: **R** registry (records), **S** ships (export,
fit, test drive), **E** engine (this session), **B** Blender (geometry, nodes).

## 1. Commissioning (prerequisite for everything)

| # | What | Owner | Minimum | Notes |
| --- | --- | --- | --- | --- |
| C1 | Hull record `hull.explorer-study`: key, `model` = the exact exported path, slots and fit per instance (engines, cold-gas blocks, tanks, power, gear legs, command, base blocks) | R | yes | `goods::hull_mass` must be Recorded or Parts: partial/none is refused (ed70fa59b) |
| C2 | Engine slots bound to real nozzle empties (`thrusters`: nozzle, slot, share; a block's shares sum to 1), the empties at the real exits, −Z along the exhaust | R, B | yes | Engine route e4a742e1c, cold gas 97921ca0e; nozzle orientation note in `explorer-propulsion-route.md` |
| C3 | An attitude-control fit (cold-gas blocks with nitrogen tanks, or another RCS) | R, S | yes | No gimbal simulated; the base blocks need RCS |
| C4 | `COL_*` collision parts, `gear_*` foot nodes at the deployed contact points, `cockpit`, mount nodes per slot | B | yes | The hull-only export, not the assembled review |
| C5 | Fitted motion packages (CHS2 v2 and the like) marked `pose: rest` per fit entry | R | yes | 7b5659851; drawn still |
| C6 | Import diagnostic clean (`hull_import_review`), then `UNIVERSE_HULL=…` flight | S | yes | Currently exit 2: no record, no binding, no mass |

## 2. Landing gear (edited folding gear)

- **Minimum (no engine work):** the gear stands deployed. The `gear_*` nodes are the feet, and the leg products in
  the fit (inert modules, mass counted, ed70fa59b) carry the landing limits. The model shows the authored deployed
  pose. Stowed can't be shown, and it's stated as a limit. Flight works.
- **Full (E):**
  - E-G1: a gear state (deployed or stowed) on the ship, switched by the pilot.
  - E-G2: legs' contact only when deployed (`ground::struts` filters).
  - E-G3: drawing each leg's motion package at its state's pose (a `pose` per state, not only `rest`).
  - E-G4: the swing animated between poses, checked against the gear bays, as Blender's sweep says.
  - Needs from B: per-leg nodes with their hinge axes and the two end poses in the export, a motion contract per
    package. From R: the gear product's states in its record.

## 3. Rear ramp, two leaves

- **Minimum (E, small):**
  - E-R1: the importer reads `RearRamp_Pivot` and `RearRamp_ExtensionPivot` (two rigid groups, panel and extension)
    and their two authored end poses: closed (panel 0°, extension folded −180°) and deployed (the open export's
    matrices).
  - E-R2: the ramp is a two-state part. Landed where the hatch opens, it takes the deployed pose exactly as authored,
    and the walk mesh is the posed leaves. Otherwise closed. No "swing to the gear's height", and no 90° limit:
    the authored pose stands.
  - E-R3: the toe's ground check. Deployed only where the toe meets the terrain within a tolerance under the
    authored pose. Otherwise the ramp stays shut and the hatch is the way out (§4), with the reason shown.
  - Needs from B: the two pivots exported as named nodes (already in the review export), the deployed pose's
    matrices (the packet's), and the toe's contact point.
- **Full (E):**
  - E-R4: the chain of hinges posed by angle.
  - E-R5: the deploy and stow sequence (panel, then extension).
  - E-R6: the sweep checked against hull, gear and ground, refused where it hits.
  - E-R7: the deploy solved against uneven terrain (`explorer-rear-ramp-audit.md`).

## 4. Pocket hatch and ground ladder (multiple exits)

- **Minimum (E):**
  - E-H1: several exits on a ship (`exit_*` nodes: kind ramp or ladder, its top and foot). Interacting picks the
    nearest exit that's open.
  - E-H2: a ladder exit is climbable from its top to its foot. The walker already climbs inside ladders, so it needs
    a climbable segment in the ship's frame from the model, and its foot on the terrain (refused where too high or
    steep).
  - E-H3: the hatch as a two-state part (`CrewHatch_FloorSlide`, `CrewHatch_BellySlide` at their authored closed and
    open poses), open when landed and the ladder is down.
  - Needs from B: `exit_*` nodes with ladder top and foot, the slides' end poses (already in the packet), the
    ladder's deployed geometry.
- **Full (E):**
  - E-H4: the slides and the ladder fold animated between poses, with the sweep checks Blender already runs (61
    sampled states) reproduced in the game.
  - E-H5: human, suit and carry envelopes on the ladder.

## Order (minimum first)

1. C1–C6 (R, B, S): the explorer flies, lands on its deployed gear and is boarded the existing way. That is one
   exit, by the hatch teleport, until §3 and §4.
2. E-R1–E-R3 and E-H1–E-H3 (E): walk out by the authored ramp or the ladder, whichever is open.
3. E-G1–E-G3 (E): gear up and down as states.
4. Full animation: E-R4–E-R7, E-H4, E-G4.

Engine items in step 2 are a few days' work and can start once B exports the `exit_*` nodes and the toe point.
Everything in step 1 is records and export, and needs no engine change.
