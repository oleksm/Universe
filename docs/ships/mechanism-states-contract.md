# Mechanism states and exits: a record-driven contract (engine proposal to the registry, 2026-10-23)

Blender `20261023T183222-blender-b1b5`: agree a generic contract with the registry before the engine binds node names.
The case is crew-hatch-01's 42 mechanism nodes (`functional-mechanism-frames.json`): two ramp pivots, the gear legs'
hinges, oleos and covers, two hatch slides, two ladder pivots. This is the minimum functional level from
`explorer-functional-plan.md`. Mechanisms take authored poses as discrete states; motion between them comes later
without changing the contract.

## In the hull record (registry schema)

```yaml
mechanisms:
  - key: rear_ramp
    kind: ramp            # ramp | hatch | ladder | gear | door
    nodes: [RearRamp_Pivot, RearRamp_ExtensionPivot]   # exact export node names, parent first
    states:
      closed: { RearRamp_Pivot: rest, RearRamp_ExtensionPivot: rest }        # rest: the export's own pose
      deployed: { RearRamp_Pivot: { rotate_deg: [0, -119.8, 0] }, RearRamp_ExtensionPivot: { rotate_deg: [0, 180, 0] } }
    default: closed
    deploys_when: landed  # landed | always | pilot
    contact: { node: RearRamp_Toe, ground: required }   # deployed only where this meets the ground
exits:
  - key: rear
    mechanism: rear_ramp
    open_in: deployed
    kind: walk            # walk (its posed leaves are floor) | climb (a ladder segment) | step (teleport, as now)
  - key: belly
    mechanisms: [crew_hatch, crew_ladder]
    open_in: deployed
    kind: climb
    top: CrewLadder_Top   # empties at the climb's ends, in the deployed pose
    foot: CrewLadder_Foot
```

- A state is a transform per node, relative to the node's own exported (rest) pose: `rest`, `rotate_deg` about its
  local axes, `translate_m`, or both. Nodes and their children move rigidly. Unnamed nodes don't move.
- `deploys_when`: `landed` (opens when landed and its `contact` holds), `pilot` (a control), `always`.
- Exits: several per hull. The crew uses the nearest open one. A hull with none boards as now (the `hatch` node).
- Gear: `kind: gear` with `deployed`/`stowed` states. Contact only through `gear_*` feet whose mechanism is deployed.
- Fitted equipment with motion (CHS2 and the like) keeps the fit entry's `pose: rest` until its package gives states.

## In the engine (after the schema lands)

Each mechanism is posed by its current state's transforms; walk meshes are posed with it; exits open by their
mechanisms' states; ground `contact` checked before deploying (refused with the reason). State changes are instant at
this level. Motion later interpolates the same transforms, with sweep checks.

## Questions for the registry

1. The angles above are read from the packet's open matrices. Should states carry angles, or the export's matrices
   verbatim?
2. The toe and ladder ends need named empties: the export doesn't have them. Blender's to add.
3. Mechanisms as hull-record fields, or as their own records the hull lists?
