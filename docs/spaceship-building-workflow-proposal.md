# Spaceship generation workflow specification

**Spaceship generation workflow specification — draft proposal, revision 0.1, 2026-10-20.**

We should build complex, inhabitable spaceships through a persistent design specification, coherent visual studies, and small, independently testable geometry changes. The proposed system combines custom Blender hull authoring with reusable registry equipment and the existing FREEFALL ship-as-code document. It preserves the owner's chosen silhouette while resolving interior space, mechanisms and construction together. AI acts as an interpreter, modeller and reviewer of bounded tasks; the authoritative design lives in versioned files, not the latest prompt. A ship can be visually accepted while engineering or runtime features remain explicitly unqualified. This document proposes the process and its authoring tools; **it authorizes no modelling, installation, registry edits, or resumption of held work**.

Contents: [1. Scope and existing systems](#1-preserve-identity-while-integrating-existing-systems) · [2. Persistent specification](#2-make-the-specification-persistent-and-traceable) · [3. Shape and inhabitation](#3-resolve-shape-and-inhabitation-before-detail) · [4. Mechanisms and representations](#4-treat-mechanisms-interfaces-and-representations-as-contracts) · [5. Execution and gates](#5-execute-bounded-changes-through-evidence-gates) · [6. Verification and release](#6-verify-the-exact-artifact-and-its-limits) · [7. Pilot and implementation](#7-prove-one-bounded-pilot-before-expanding) · [8. Sources](#8-sources-and-research-provenance)

## 1. Preserve identity while integrating existing systems

### 1.1 Scope and intended outcome

The target is a distinctive ship with the visual ambition associated with Star Citizen: recognizable proportions, a convincing cockpit, inhabitable spaces, readable mechanisms and consistent construction detail. This is not a reproduction of CIG's current private pipeline or a request to copy a particular vessel. The workflow must support original small ships first and larger, multi-zone vessels later, without turning each into an unrelated procedural script or a single unmanageable prompt.

The deliverable is a coordinated design: custom hull geometry where identity requires it; referenced equipment where the registry already supplies it; an assembly document defining placement; explicit access and motion data; and evidence showing what has actually been checked. An attractive exterior alone is insufficient. A populated box with a vaguely pointed nose is also insufficient, even when its dimensions pass.

The existing short guidelines (external repository `blender:docs/spaceship-building-guidelines.md`) remain the quick working agreement. This detailed proposal adds requirements, artifacts, authority, state transitions and failure handling. “Must” below describes the proposed acceptance contract, not a claim that the relevant tool already exists.

### 1.2 Findings that shape the proposal

Historical CIG accounts support resolving mission and identity before expensive surface work. Ryan Church's 2012 Constellation discussion connects crew, purpose, emotional character, silhouette exploration and functional resolution in 3D. The useful lesson is to preserve an explicit design choice through subsequent construction, rather than expecting detail to rescue weak proportions. This is evidence from one historical project. ([RSI interview, 2012](https://robertsspaceindustries.com/en/comm-link/transmission/12783-Introducing-The-RSI-Constellation-Mk3))

The 2015 Cutlass account describes revisions spanning gameplay role, thruster placement, interior organization and concept paintovers. Its relevance is the coupling of exterior, behavior and occupied space: a change that improves one can invalidate another. It does not establish a universal set of spacecraft dimensions. ([CIG Cutlass design history, 2015](https://robertsspaceindustries.com/en/comm-link/engineering/15056-Design-Notes-Sharpening-The-Cutlass))

CIG's historical pipeline separated concept, hangar and flight readiness, while its May 2015 report discussed a common manufacturer language for details. We adopt separate readiness claims and a reusable visual grammar; we do not adopt undocumented present-day staffing, schedules or polygon budgets. ([CIG pipeline, 2014](https://robertsspaceindustries.com/en/comm-link/transmission/14244-How-Ideas-Take-Flight-The-Star-Citizen-Ship-Pipeline), [CIG studio report, 2015](https://robertsspaceindustries.com/en/comm-link/transmission/14758-Monthly-Studio-Report-May-2015))

Local experience makes interface discipline particularly important. Cradle v1 had correct port positions but incorrect facing directions; a raw-GLB test exposed what a position-only check missed. Bridge studies achieved algebraic reaction closure yet lacked feasible actual frame allocations. These are different failures and require different checks: representation correctness, assembly feasibility and engineering qualification cannot substitute for one another. (Cradle diagnostic (external repository `blender:physical-mounts/cradle-v1-port-diagnostic.json`), bridge allocation review (private historical review; not a required registry dependency))

### 1.3 What exists and what this proposal adds

| Capability | Evidence at research snapshot | Treatment in this proposal |
|---|---|---|
| `.ship.yaml` parser, validation and Studio projection | Implemented according to current documentation and inspected CLI source | Reuse; do not create a competing runtime schema |
| Registry models, family templates and installation tools | Existing local contracts and packages | Reference and pin their versions |
| Profile extrusion and matching-profile loft helpers | Present in MC-07 generator | Reuse selectively, after separating its coordinate assumptions |
| Static GLB plus `freefall-motion/1` | Existing schema and validation tools | Use only supported mechanisms; test consumer behavior separately |
| Delivery states and hash-bound evidence runner | Existing asset-delivery workflow | Extend with ship-specific checks rather than replace |
| Reference atlas and visual-grammar sidecar | Proposed | Implement as authoring files first |
| Region dependency compiler and safe partial regeneration | Proposed | Prove on a bounded pilot before generalizing |
| Generic continuous clearance certification | Not established | Never infer from sampled poses |
| Qualified automatic structural mounting | Blocked by incomplete data/contracts in relevant cases | Preserve explicit qualification limits |
| Full ship GPU, motion and walkability harness | Not audited by this research | Require feature-specific runtime evidence |

The local research inspected registry revision `c5d3861f886c8df5e6d78206f0a8c04ee7cbe7ec` and ships revision `24152fd12bb42aa6a3f54cd5f795ccbd37c9c629`. These are worktree snapshots, not claims of merger to main. No compilation, GPU session or engineering qualification was performed for this document. Current implementation documentation takes precedence over older roadmap prose. (Ship-code implementation (external repository `universe-ships:docs/ship-code.md`), CLI source (external repository `universe-ships:crates/game/src/interior/ship_code.rs:1204`))

## 2. Make the specification persistent and traceable

### 2.1 Separate family, type and instance

A **family** defines shared visual grammar and compatible module practices: characteristic section shapes, canopy treatment, edge vocabulary, materials and service-panel conventions. It does not determine every hull. A **type** defines one ship design: mission, capacity, arrangement, hull regions, equipment allocation and supported operating states. An **instance** selects a type revision and permitted options, placements, finish or damage state without silently rewriting the type.

A generation seed is an instance input, not design authority. Randomization may vary approved secondary details within declared bounds. It must not move the pilot seat, alter a ramp mechanism or change the ship's recognized silhouette unless that variation is an explicitly approved type option. Different seed streams for finish and layout prevent a paint adjustment from changing geometry.

### 2.2 Keep three complementary specification layers

| Layer | Defines | Typical acceptance |
|---|---|---|
| Functional requirements | Crew tasks, cargo, access, equipment, operating modes, service routes | Measured fit, task demonstration and consumer tests |
| Visual grammar | Proportions, silhouette, landmarks, curvature, glazing and detail hierarchy | Registered views and owner decision |
| Construction interpretation | Regions, seams, structures, joints, openings and source ownership | Geometry, connection and assembly checks, with qualification labels |

A requirement record has a stable ID, statement, rationale, owner, source, severity, status and verification method. Hard functional constraints, identity constraints and preferences remain distinguishable. Passing a beautiful render cannot waive a blocked escape path. Conversely, fitting a pilot volume cannot accept a cockpit that lost the selected character.

Assumptions use their own IDs and statuses: proposed, approved-for-prototype, superseded or resolved. Each assumption names the decision that would replace it and the artifacts it affects. “Small utility ship” is not enough to infer crew anthropometry, gravity, cargo dimensions or endurance. Those remain named inputs or visible unknowns.

### 2.3 Establish authority before generating geometry

| Decision or artifact | Authority | Modeller's responsibility |
|---|---|---|
| Purpose, identity and major aesthetic trade-offs | Owner, with existing delegation recorded | Present bounded alternatives and preserve the decision |
| Hull/type design and ship assembly semantics | Ships | Coordinate region and assembly changes |
| Registry facts, interfaces, ratings and reusable asset requirements | Registry | Use pinned records; report contradictions |
| Blender source, custom surfaces and geometry evidence | Blender | Build, inspect and retain editable provenance |
| Runtime placement, rendering, interaction and playback | Engine/integrator | Supply reproducible package and test cases |
| Release tooling and package manifests | Existing installer/workflow | Use the tooling; do not hand-edit its outputs |

An agent must not resolve a cross-owner contradiction by silently changing a record or inventing a rating. It records the exact conflict and proposes an option. Work independent of that conflict can continue. Existing authorization carries forward; the workflow does not require another “continue?” between routine regional tasks.

### 2.4 Add an authoring sidecar, not another runtime format

The maintained ship schema accepts a defined set of fields and rejects unknown ones. Proposed references, region generators and review decisions therefore belong in an authoring sidecar. A future adapter projects only supported assembly information into `.ship.yaml`; unsupported requirements remain explicit in the sidecar. Registry equipment facts stay upstream. (Maintained ship schema (external repository `universe-ships:crates/core/ship-design/ship.schema.json`))

The following YAML is an **illustrative proposed authoring format**, not an implemented parser contract. IDs, file paths and seed values are examples; the pilot design is not approved by this document.

```yaml
format: freefall-ship-workflow/0-proposal
id: ship.pilot-01
revision: 1
status: proposed
family: family.example-utility
runtime_document: assembly/pilot-01.ship.yaml
frames:
  authoring:
    units: m
    handedness: right
    right: '+X'
    up: '+Z'
    forward: '+Y'
  runtime:
    units: m
    handedness: right
    right: '+X'
    up: '+Y'
    forward: '-Z'
  authoring_to_runtime: [[1, 0, 0], [0, 0, 1], [0, -1, 0]]
authorities:
  identity: owner
  hull_design: ships
  geometry: blender
  equipment_facts: registry
  consumer: engine
intent:
  role: small_single_pilot_utility
  identity_requirements: [REQ-COCKPIT-01, REQ-NECK-01, REQ-RAMP-01]
requirements: requirements.yaml
assumptions: assumptions.yaml
references: references/index.yaml
regions: [regions/cockpit.yaml, regions/neck.yaml, regions/belly.yaml]
interfaces: interfaces.yaml
build:
  entry: build.py
  toolchain_lock: toolchain.lock.yaml
  dependency_lock: dependencies.lock.yaml
  seed_streams: {layout: 41, finish: 92}
  manual_inputs: [source/canopy-approved.blend]
review:
  cameras: review/cameras.yaml
  decisions: review/decisions.yaml
  active_candidate: candidate-004
release: candidate-004/delivery.yaml
```

The sidecar owns design intent and input references. Existing `delivery.yaml` owns package workflow and evidence state; the installer owns release manifests. The adapter should fail on unsupported required output rather than dropping it. Its proposed roundtrip test preserves stable author IDs, intended placements, access and decks through the current Studio representation.

### 2.5 Use IDs to connect prose, geometry and evidence

IDs remain stable when filenames, object ordering or labels change. Suggested namespaces are `REQ-`, `ASM-`, `REF-`, `REG-`, `IF-`, `MECH-`, `CHECK-` and `DEC-`. Runtime instance IDs follow the existing runtime contract; the sidecar records the mapping instead of renaming them casually. Blender collection names are convenient labels, not sufficient identity on their own.

```yaml
id: REQ-RAMP-01
statement: Belly ramp opens forward toward the cockpit.
kind: identity_and_function
owner: owner
status: proposed
source: illustrative_pilot_brief
verify: [CHECK-RAMP-DIRECTION, CHECK-ENTRY-ROUTE, CHECK-RAMP-VISUAL]
related: [REG-BELLY, MECH-RAMP]
assumptions: [ASM-LANDING-STANCE]
```

An identity requirement such as this needs a directional diagram and owner interpretation. It should be decomposed into separate checks for hinge support, passage, ground contact and playback. Otherwise one attractive image could incorrectly satisfy several unrelated claims.

## 3. Resolve shape and inhabitation before detail

### 3.1 Build a reference atlas with confidence labels

Collect features before commissioning a whole-ship sketch. Intake is a conversation in small rounds, usually one to three related questions, not a demand for a complete specification in one prompt. Carry previous answers and corrections forward; ask only about missing information or genuine contradictions.

Cover six areas: purpose (mission, crew, endurance, cargo and environment); shape (cockpit, nose, proportions, belly, wings and engines); occupied spaces (seating, aisle, rooms, storage and equipment); access and motion (ramps, doors, gear and deployables); visual character (reference features, finish and colours); and exclusions (features the owner does not want). Mark each area answered, explicitly undecided or not applicable. Do not force dimensions or equipment choices before their implications are understood.

Give each feature a stable ID, required/preferred/undecided classification, source, interpretation and decision status. A proposed default is not a user answer. Separate the priority of a feature from whether its interpretation is confirmed. Surface conflicting features with the consequence and available trade-off; retain unresolved issues rather than hiding them in the prompt.

Summarize the consolidated feature brief for owner correction and confirmation, or apply an explicit existing delegation. G0 is ready when all six areas have been considered, the required features are coherent, and remaining unknowns are explicitly bounded as permitted exploration or blockers. Silence is not confirmation. Small labelled clarification diagrams may resolve a hinge direction or other ambiguity during intake; they are not a selected whole-ship design.

After the brief is confirmed and reference interpretation is coherent, derive a versioned sketch prompt from that exact brief. Include mission/context, every required feature by ID, preferred features, exclusions, reference priorities, requested views, permitted variation and explicitly undecided details. Do not silently resolve undecided features. Check prompt coverage before sketching; text must not introduce unsupported dimensions or technical claims. This is a reusable instruction for a human or image tool, not a requirement to use a particular generator.

Review each sketch against the same feature IDs, recording met, missed, ambiguous or intentionally varied features and the limits of what the view can show. Sketch approval does not prove interior fit or engineering. A changed feature invalidates the affected prompt and sketch evidence; preserve unaffected answers and earlier candidates. A new material design choice returns to brief confirmation, while wording corrections within confirmed intent need no repeated approval.

For a rehearsal, freeze the actual owner answers as a separate hashed input. Label each added answer or interpretation as actual-owner evidence, simulated trial input or a proposed interpretation; priority is independent of provenance. Permission to run a rehearsal is not permission to confirm the real brief or select a production design. A simulated confirmation names the exact trial brief hash, the simulator and rehearsal-only scope. It cannot satisfy a real-owner gate. Check inherited answers by stable ID and meaning, not merely by counting features.

Prompt coverage requires a witness in the exact prompt text for every required feature and exclusion, not just the presence of its ID in a checklist. Include retained interpretations, permitted variation and bounded unknowns. An independent reviewer checks that the witness expresses the requirement without weakening it. Record absent references explicitly; do not invent an atlas image. Before whole-ship sketching, reject missing confirmation, an unbounded blocker, a dropped required feature or an unresolved contradiction.

Bind the brief, confirmation, prompt, generated candidate and review through exact input hashes and the workflow revision. A changed brief invalidates its dependent coverage and prompt; a changed prompt invalidates claims that an existing image was generated from that prompt. Keep earlier inputs and candidates as history. Reconfirm material design choices only; an intent-preserving wording repair does not invent a new approval requirement. Tests must demonstrate rejection of a missing prompt feature, a forged real-owner confirmation using simulated authority and a stale dependent input.

Visual review records met, missed, ambiguous, intentionally varied or not observable for each feature, with a view/region witness and a separate claim scope. A requirement mentioned in the prompt is not proof it appears in the image. An exterior picture cannot verify a hidden aisle, berth, pressure boundary, mechanism clearance or equipment fit; carry those as not observable and identify the later check. Keep prompt coverage, visual identity, occupancy and engineering results separate. Compare component counts, fore/aft positions, camera projection and mechanism poses across labelled views and the rough occupancy diagram; a caption cannot turn perspective into an orthographic witness or a lowered ramp into a closed one. Iterate with specific misses and retain prior candidates; close a rehearsal only after independent review of the current evidence, with remaining limitations explicit.

Each reference entry records its source URL or file hash, provenance, crop, visible camera, scale cues, controlled regions, borrowed features and excluded features. Label statements as **observed**, **inferred**, **owner-selected** or **unresolved**. A visible canopy profile is observed; a hidden rear bulkhead reconstructed from it is inferred. Record contradictions rather than averaging them invisibly.

An atlas contains whole-ship silhouettes, cockpit treatment, neck/shoulder transitions, central sections, belly access, stern and equipment attachment. A source can inform one feature without dictating the whole ship. Rights and attribution must be retained for references used in public deliverables; “inspired by” does not establish permission to redistribute source artwork.

Generated images can explore mood or annotate a proposed paintover. Independently generated top, side and front pictures do not establish one consistent object. Once a blockout exists, its own rendered views become the geometric baseline. Further image studies use those views so a new concept does not quietly invent different engine spacing or internal volume.

### 3.2 Compare a small set of coherent silhouettes

The proposed default is three inexpensive directions, adjustable when the design problem warrants fewer or more. Use identical camera positions, orthographic framing, lighting and flat materials. Show top, side, front, underside and two three-quarter views. Include one scale fixture and a minimal cockpit/interior occupancy outline; omit panel lines and small hardware.

Evaluate first-read character, volume distribution, important negative spaces, cockpit-to-body relation and recognizability at thumbnail size. Select one coherent candidate rather than mixing every appealing feature across alternatives. Record the selected proportions and distinctive landmarks. Numeric image differences are review triggers, not a substitute for aesthetic judgment.

The owner packet states one decision, a recommended direction and its main trade-off. It includes the chosen candidate's input hash and camera set. After approval, a geometry repair that changes the beak, canopy slope, neck constriction or maximum beam location returns to this visual gate if it exceeds agreed guardrails. A normal correction does not.

### 3.3 Define named regions and shared boundaries

Regions follow meaningful shape and construction changes: cockpit pod, canopy, neck, shoulder, belly, stern, nacelles or fins. They are not arbitrary equal-length slices. Each has one authoritative source: procedural generator, editable manual surface, referenced component or derived output. A generator may replace only its owned output collections.

```yaml
id: REG-COCKPIT
source_kind: editable_sections
source: source/cockpit-profiles.yaml
requirements: [REQ-COCKPIT-01, REQ-PILOT-VIEW-01]
references: [REF-COCKPIT-SIDE-02]
owned_collections: [Hull_Cockpit, Canopy_Frame]
interfaces: [IF-COCKPIT-NECK-01]
read_only_neighbors: [REG-NECK]
acceptance_views: [CAM-TOP, CAM-SIDE, CAM-FRONT, CAM-PILOT]
checks: [CHECK-PILOT-CLEARANCE, CHECK-CANOPY-PROFILE]
allowed_auto_changes: [repair_normals, remove_duplicate_faces]
```

The region record also carries editable parameters, permitted ranges, keep-in/out volumes, mass ownership, protected landmarks, material roles and downstream dependencies. Boundary curves are owned once and consumed by both sides. An intentional crease, tangent-continuous join and curvature-continuous join are different contracts; coincident vertices alone do not satisfy all three.

### 3.4 Author stations, profiles and guide curves

Place stations at shape events: nose, pilot eye, canopy apex, canopy rear, neck minimum, shoulder onset, maximum beam, ramp corners, equipment bulkheads and stern interface. Each station specifies its local plane, centerline offset, profile control points and semantic correspondence. Longitudinal dorsal, keel and shoulder curves guide transitions between sections. Port/starboard symmetry is explicit and can be intentionally broken.

Station order and section orientation must be checked before lofting. A profile's vertex correspondence cannot change accidentally between sections: an upper chine must not connect to a lower shoulder because an array was reordered. Changes in topology—window openings, branching nacelles, a split stern—require an explicit transition region or manual patch with its own boundary contract. They are not ordinary parameter edits.

A local MC-07 helper already lofts X-ordered sections with matching vertex counts; Blender also documents bridging loops with twist and interpolation controls. Neither provides an automatic general-purpose ship designer. The proposed tool should retain profile editing and purpose-built surfaces instead of forcing every cockpit into one fixed polygon ring. (MC-07 helper (external repository `blender:build_mining_ship.py:361`), [Blender Bridge Edge Loops](https://docs.blender.org/manual/en/4.4/modeling/meshes/editing/edge/bridge_edge_loops.html))

Review section overlays on evaluated geometry, not only control curves. For each edited station, compare local interior clearance, skin/service allowance, neighboring tangents and registered silhouette. Choose per-region tolerances from the actual design scale and output purpose; there is no universal station spacing or millimetre tolerance in this proposal.

### 3.5 Co-design cockpit, routes and hull

Maintain a spatial adjacency graph and a separate task-route graph. The first states which spaces touch; the second demonstrates that a person or cargo can actually move between them. Maintain service-removal routes as well: a fitted engine or battery is not serviceable if its removal path passes through permanent structure.

The volume model separates useful occupied space, structural depth, pressure skin where applicable, services/insulation and external armour. Accepted aisle space cannot be consumed silently by a later rib or hose. Fixtures include standing, seated, turning, reaching, carrying and ingress poses appropriate to the intended crew and worn equipment.

NASA's human-system guidance emphasizes task-specific fit, reach, view and operation across the intended population rather than one nominal body. We borrow that evaluation approach, not a claim of NASA compliance. The project must select its own character and suit fixtures and state their limitations. ([NASA physical characteristics guidance](https://www.nasa.gov/reference/4-0-physical-characteristics-and-capabilities-vol-2/), [NASA human integration process](https://ntrs.nasa.gov/api/citations/20140009559/downloads/20140009559.pdf))

The cockpit fixture includes seat travel, eye points, head/helmet rotation, knee and pedal space, control reaches and the transition into the seat. Render actual pilot views. Report direct visibility separately from sensor-assisted visibility; a camera feed does not improve a physical window's field of view. Control availability should be evaluated during the intended task, not from a third-person beauty view. ([NASA crew-interface guidance](https://www.nasa.gov/reference/10-0-crew-interfaces-vol-2/))

When packaging conflicts arise, compare local alternatives: move equipment, adjust a bulkhead, change the seat approach or modify one profile. Each alternative reports silhouette impact, useful-space change and access consequences. Global enlargement is a major design decision because it changes mass assumptions, equipment fit, mechanisms and the selected visual proportions.

### 3.6 Establish readable construction without false engineering claims

Primary structure and openings precede decorative detail. Each substantial external part should have an intelligible attachment path, and each service opening should correspond to access or an explicitly aesthetic purpose. “Looks manufactured” is a visual criterion; actual strength, pressure, thermal and propulsion performance require their own evidence.

Use three detail levels: primary masses/negative spaces, secondary armour/openings/attachments, and tertiary seams/fasteners/markings. No fixed percentage is prescribed. Leave restrained areas deliberately clean. A detail that changes the visible outline at the target distance belongs to silhouette review regardless of its nominal category.

## 4. Treat mechanisms, interfaces and representations as contracts

### 4.1 Use explicit local and global frames

The proposed pilot authoring profile uses metres, +X right, +Y forward and +Z up; its runtime mapping is `(x,z,-y)`. This is a named profile, not a universal assumption about every existing Blender file. MC-07 has a rotated root, and equipment has its own mount frames. A region's local frame, ship frame, module frame, hinge frame and port frame must be distinguishable.

A port/interface record includes stable ID, kind, owning rigid part, parent frame, origin, full orientation including roll, mating role, registry key where applicable, permitted transform, occupied envelope and access envelope. Surface boundaries, service connections and structural joints must not share an undifferentiated “socket” type. Exported names are required to be unique under this workflow even though glTF itself does not guarantee name uniqueness. ([Khronos glTF specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html))

Existing equipment mounting uses `hull_placement * hull_mount * inverse(primary_model_mount)`, without arbitrary bounds centering or scale. The workflow must preserve that consumer-specific behavior. A generic centering convenience must not move a physical attachment datum. (Equipment mounting contract (external repository `universe-ships:docs/equipment-mounting.md`))

Use an asymmetric calibration fixture with labelled axes, known distances, an off-axis port and a rotated parent. Verify source coordinates, accumulated raw-GLB transforms and actual consumer placement. Check orientation and handedness as well as positions. The independent checker should not reuse the same transformation function as the exporter it is testing.

### 4.2 Resolve the complete ramp and landing mechanism

For a belly ramp, specify the opening boundary, hinge line, moving leaf, deployment direction relative to cockpit, landing stance, operating limits, latch state, actuator anchors and passenger/cargo route. “Forward-opening” is insufficient without a side-section trajectory diagram. Define closed, unlocked, first-open, intermediate, fully open and entry states.

The hinge must attach to structure; an actuator needs both fixed and moving anchors, closed length, usable stroke and overlap. Hoses or cables need endpoint frames, length, bend criteria and an approximation model. Landing gear needs a support/contact interpretation and declared ground conditions. These checks precede detailed skins, seals and decorative joints.

Combined-state checks cover gear down with ramp moving, ramp open with a person entering, hatch open with service access, and all other permitted interactions. Forbidden configurations belong in an interlock contract; a geometry test that avoids them does not prove runtime enforcement. Sample endpoints, intermediate states and near-contact intervals, retaining the worst-case pose and object pair. **Sampled clearance is a screen, not proof of continuous collision freedom.** Stronger claims require conservative bounds or continuous methods.

FREEFALL motion uses its own static-model-plus-description contract. Blender drivers remain authoring aids unless an actual consumer supports them. The existing motion checker and runtime playback have distinct scopes; a plain-axis check can pass while game consumption remains pending. (Motion proposal and schema evolution (registry repository `docs/proposals/plant-motion.md`), motion schema (registry repository `docs/formats/freefall-motion-1.schema.yaml`))

### 4.3 Keep physical attachment and visual adjacency separate

A reusable module should appear once in the assembly/BOM and retain its own model reference. Shared holders count once. A mesh touching a frame does not establish load transfer. Structural attachment requires compatible interfaces and a complete path through equipment, holder and frame, with the qualification status of every link visible.

The parked clamp/bridge studies remain parked. This proposal does not authorize new holder geometry or treat provisional friction, equal shares or bolt capacities as qualification. Missing actual frame allocation, stiffness or joint data blocks a qualified-load claim without necessarily blocking a clearly labelled visual prototype. (Physical mount proposal (registry repository `docs/proposals/physical-mounts.md`))

### 4.4 Maintain separate representations

| Representation | Purpose | Must preserve |
|---|---|---|
| Editable master | Profiles, custom surfaces and design decisions | Semantic controls and manual intent |
| Evaluated construction | Actual modifiers, solids/sheets and attachments | Source design and declared construction |
| Render/export meshes | Materials, hierarchy, visibility and animation ownership | Identity, required nodes and interface frames |
| Collision/interaction | Exterior physics, walking, selection and use | Open passages and operational states |
| Mass/engineering | Registry parts, measured solids and explicit allowances | Ownership and accounting provenance |
| Envelopes | Placement, service and motion exclusions | Conservative coverage and meaningful open space |

Evaluated Blender geometry includes modifier results; checks against an unevaluated cage can miss what is exported. Export can also change triangulation and vertex counts at normal or UV discontinuities, so source totals are not final runtime totals. ([Blender dependency graph API](https://docs.blender.org/api/5.2/bpy.types.Depsgraph.html), [Blender glTF exporter](https://docs.blender.org/manual/en/3.6/addons/import_export/scene_gltf2.html))

Collision is not automatically the visible hull. A convex approximation around the whole vessel can close its ramp and aisle. Interior walking, moving passengers and ship motion require explicit engine support. External engine documentation illustrates why visual and physics representations differ, but its restrictions must not be imported as FREEFALL rules. ([Godot collision representation example](https://docs.godotengine.org/en/4.4/tutorials/physics/collision_shapes_3d.html))

### 4.5 Set budgets against a named workload

Record exported triangles/vertices, unique meshes and instances, material primitives, texture residency, transparency, lights, shadow casters, collision shapes, motion channels, loading cost and file size. Allocate budgets by exterior, cockpit, interior zone and equipment. Numeric limits require an engine benchmark with hardware, resolution, graphics settings, ship count and camera path; this proposal supplies no universal AAA budget.

LOD must retain silhouette, cockpit proportions and visible mechanism state. Reduce small hardware before primary form. Do not shrink collision or interaction volumes merely because a render LOD changes. Inspect transitions and transparent canopy/interior behavior in the target renderer. Frame-time and visibility evidence matter alongside triangle counts; performance advice from other engines is methodological evidence, not an assertion of FREEFALL features. ([Godot LOD example](https://docs.godotengine.org/en/4.7/tutorials/3d/mesh_lod.html), [performance guide](https://docs.godotengine.org/en/stable/tutorials/performance/optimizing_3d_performance.html))

### 4.6 Build surfaces from material roles and readable scale

The proposed surface pipeline assigns material roles to each region before making individual shaders: outer hull, armour, structural metal, canopy, interior lining, floor, seals and markings as applicable. A shared PBR baseline defines the approved palette and roughness/metallic behavior, while deliberate regional exceptions remain recorded. Start with simple neutral materials during form reviews; use final lighting and finish only after primary form is stable. Material variation must not conceal unresolved seams or make every region look like a different manufacturer.

Choose the representation of each surface feature deliberately. Openings, deep recesses, silhouette breaks and moving joints require geometry where their function or viewing distance demands it. Shallow panel markings, labels and wear can use textures, trim sheets or decals if the actual exporter and engine support the selected method. UV density and trim reuse should follow viewing distance and material scale, with an agreed reference object for comparison. Avoid baking misleading shadows into a finish to imitate unsupported construction. Decal and texture support are compatibility questions to test, not capabilities asserted by this proposal.

Inspect materials in the source scene, the exported/reimported package and the target renderer. Record color-space handling, normals, transparency, texture dependencies and any substituted shader behavior. Optional wear, fasteners and microdetail must earn their geometry, texture and draw cost through visible improvement at the target camera distances. If a feature disappears in the intended view or obscures a readable construction boundary, omit it or move it to a cheaper supported representation.

## 5. Execute bounded changes through evidence gates

### 5.1 Use the existing release state machine

Retain the existing outer progression: **Proposed → Approved → Building → Optimizing → QC passed → Accepted → Installed**. Inside it, the ship has design gates and region/check states. `blocked` is an overlay with a reason, owner and recovery action; it does not erase the last good candidate. Installation is distinct from consumer validation. (Asset-delivery workflow (external installed workflow reference: `freefall-asset-delivery/SKILL.md`; not bundled))

| Gate | Entry condition | Exit evidence and decision | Failure/rollback |
|---|---|---|---|
| G0 brief | Authorized design study | Role, identity, scope, unknowns and success tests recorded | Resolve missing essential input; no detailed build |
| G1 references | Brief accepted or delegated | Annotated atlas; conflicts and inferred views declared | Return disputed feature to owner interpretation |
| G2 silhouette | Coherent reference interpretation | Comparable blockouts; selected candidate and landmarks | Retain alternatives; discard no source |
| G3 architecture | Selected silhouette | Sections, cockpit fixtures, task routes, equipment allocations | Reopen affected shape/arrangement only |
| G4 mechanisms | Openings and interfaces defined | State storyboard, supports, pose checks and limitations | Roll back mechanism candidate; preserve shell baseline |
| G5 design lock | Functional blockout accepted | Primary/secondary form reviewed in full context | Reopen visual gate if identity changes |
| G6 production | Stable form and component ownership | Evaluated geometry, materials, budgets, collision and export | Repair the owning region; invalidate dependents |
| G7 release | Current source/export evidence | Exact-candidate acceptance and immutable package | New candidate required after content changes |
| G8 integration | Installed package and compatible consumer | Actual placement, walking, playback and GPU evidence | Keep package status separate from runtime failure |

The owner normally chooses identity, silhouette and significant arrangement/mechanism trade-offs. Explicit delegation can cover other gates or predefine acceptable outcomes. Routine repairs proceed autonomously inside approved constraints. A gate is not an excuse to ask permission for every bevel, nor to continue through a newly discovered fundamental design fork.

### 5.2 Compile a dependency DAG for local regeneration

Design exploration can alternate hull and interior decisions, but an individual build requires an acyclic dependency graph. Freeze a coherent arrangement revision, then derive geometry from that revision. The proposed graph runs from brief/reference selections through profiles and shared boundaries to region geometry, mechanisms, detail, collision/LOD, export and evidence.

A change identifies its owned region and affected dependencies before mutation. Build into a candidate collection or clean file, validate, compare with accepted neighbors, then promote the candidate. Unaffected equipment and manually authored sources retain their hashes. Shared boundary changes notify both consumers; the system must not silently repair the neighbor by regenerating it.

Manual geometry is first-class source. Its file, object IDs, revision and input hash are pinned. Editing generated output is either temporary and discarded or explicitly promoted into a manual input with ownership changed. Never rely on a hidden live-session adjustment that disappears during a clean build.

Pin Blender, exporter settings, scripts, registry dependency versions, source files and seed streams. A clean background build must not depend on selection or active-object state. Blender provides command-line execution mechanisms; determinism across arbitrary versions is not guaranteed. Compare canonical geometry/attribute/transform signatures, documenting nondeterministic metadata rather than demanding unexplained byte identity. ([Blender command-line documentation](https://docs.blender.org/manual/en/3.6/advanced/command_line/arguments.html))

### 5.3 Define bounded AI task packets

A task packet contains candidate ID, target region, requirement IDs, reference crops, permitted edits, fixed neighbors/interfaces, tests and required output views. It specifies the intended result, not merely a mesh operation. The agent first reports affected dependencies, then performs the smallest coherent edit and records the result.

```text
Candidate: candidate-004; region: REG-COCKPIT
Goal: recover the accepted hawk-like canopy profile in REF-COCKPIT-SIDE-02.
Preserve: one pilot fixture, IF-COCKPIT-NECK-01, accepted cockpit rear station,
          REG-NECK geometry and all equipment instances.
May edit: canopy roof guide and two cockpit profile stations within recorded bounds.
Must not: widen the cabin to fit an unresolved fixture; regenerate the whole hull.
Evidence: same-camera side/front/top overlays, pilot-view mask, section clearance,
          interface position/tangent check, source hashes for untouched regions.
If infeasible: retain candidate and show two local alternatives with trade-offs.
```

A mechanism packet similarly names pivots, anchors, allowed states, required source/export comparisons and runtime limitations. A review packet requests diagnosis before alteration. Prompt templates are proposed helpers, not an autonomous authority to reinterpret the owner's brief.

### 5.4 Invalidate only evidence that depends on the change

| Change | Invalidate | Normally retain |
|---|---|---|
| Hull profile or global scale | Affected form, space, interfaces, collision, mass inputs, export | Unchanged reusable component sources |
| Module placement | Mount alignment, routes, access, assembly collision and mass distribution | Module model package |
| Registry mass only | Accounting and dependent load analyses | Geometry if no geometry requirement changed |
| Port datum | Mating, routing, required marker and placement checks | Body mesh unless an actual conflict exists |
| Axis/hierarchy | Bind transforms, motion, connections, swept envelopes and runtime poses | Unrelated static regions |
| Material/shader | Appearance, export materials and performance | Geometry checks with identical geometry inputs |
| Exporter/toolchain | Raw GLB, materials, transforms, package compatibility | Source design decision, unless output drift changes it |
| Reference/identity decision | Related visual gates and downstream form evidence | Independent functional results where inputs are unchanged |

An accepted design decision reopens only when its premises changed. Every evidence item contains input hashes, not merely a timestamp. The existing runner already supports stale evidence and protected accepted packages; use that mechanism rather than create a second approval database. (Execution-loop contract (external installed workflow reference: `freefall-asset-delivery/references/execution-loop.md`; not bundled))

### 5.5 Record defects and recover without endless regeneration

A defect record names failing requirement/check, affected objects, geometric witness or pose, expected and observed result, severity, suspected cause and repair scope. Keep the last passing candidate. After two unsuccessful attempts at the same defect, change the approach materially or report the precise missing decision; do not spend repeated cycles reproducing the same failure.

Before opening or rebuilding a live scene, preserve unsaved work. Keep one active candidate visible in Blender. Background builds must not reset the live session. On crash, recover from pinned inputs and the last complete candidate, verify package/index state before resuming and avoid overwriting a release to “finish” an interrupted operation.

## 6. Verify the exact artifact and its limits

### 6.1 Use a complete evidence checklist

| Check group | Required evidence |
|---|---|
| Requirements | Every must requirement has current evidence or explicit blocker; assumptions have owners |
| References/identity | Atlas provenance, coherent views, landmark comparison and recorded visual decision |
| Hull geometry | Station ordering, explicit loop correspondence, seam intent, evaluated sections and protected regions |
| Mesh integrity | Degenerates, normals, unexpected intersections, Boolean results, intended solids/sheets and small features |
| Inhabitation | Pilot fit/views, entry, movement, cargo path, service removal and declared emergency route scope |
| Construction | Supported parts, real contacts, interfaces, duplicated ownership and qualified/unqualified distinctions |
| Motion | Rest/end/intermediate/combined states; anchors, stroke, overlap, hose reach and coverage limitations |
| Materials | Source/export agreement, texture paths, transparency, normals and supported shading |
| Export | Required IDs, hierarchy, raw accumulated port frames, scale, bounds and actual triangulated cost |
| Assembly | Existing `.ship.yaml` validation, stable IDs, placements, referenced modules and dependency locks |
| Runtime | Loader, placement, interaction/collision, walking, playback/interlocks and GPU checks independently |
| Release | Exact accepted hashes, provenance, previews, limitations and reproducible rebuild instructions |

Boolean checks classify intentional sheets and assembly overlaps rather than demanding every render object be a closed solid. Blender's solver guarantees depend on input conditions; a successful operator call alone is not proof of a valid result. Inspect evaluated geometry and retain before/after witnesses for important cuts. ([Blender Boolean documentation](https://docs.blender.org/manual/en/dev/modeling/modifiers/generate/booleans.html))

A format validator is also necessary but narrow. Khronos glTF-Validator checks structural/data validity; it cannot establish a usable cockpit or load-bearing hinge. A Blender reimport adds visual comparison but is not an independent proof of loader behavior. ([Khronos validator](https://github.com/KhronosGroup/glTF-Validator/blob/main/README.md))

### 6.2 Store enough evidence to reproduce a result

Each check record contains check and requirement IDs, candidate ID, input/spec/script/export hashes, frame profile, command and arguments, timestamp, reviewer, method, tolerance with authority, expected/observed values, worst-case witness, images and limitations. States are pending, pass, fail, blocked, stale or not-applicable-with-reason. A waiver names its owner and expires when its responsible input changes.

Maintain separate statuses for `geometry_ready`, `package_valid`, `placement_verified`, `runtime_verified` and `engineering_qualified`. “Reader compiles” is not “GPU renders”; “axes validate” is not “game enforces sequencing”; “mesh volume measured” is not “mass qualified.” Final summaries state exactly which representations and consumers were tested.

### 6.3 Reuse verified command entry points carefully

The following are existing local entry points documented or inspected during research. They were **not run for this proposal**. Substitute actual candidate paths and inspect the current CLI help before execution; this document does not authorize installation.

```bash
# Run from external repository universe-ships:
cargo run --bin freefall-studio -- --validate-ship /absolute/design.ship.yaml

# Run from registry repository:
python tools/standards/next_asset.py --key template.run-clip --json
python tools/standards/precheck_model.py /absolute/candidate/delivery.yaml

# Existing independent registry-port checker (its registry-record scope matters):
python external repository blender/physical-mounts/check_ports.py   /absolute/model.glb /absolute/reference-record.yaml
```

The first command validates the existing ship document, not the proposed sidecar. The port checker provides an established independent test pattern, not a generic parser for proposed ship interfaces. A new sidecar validator, region-build command, camera-regression command and whole-ship runtime harness remain implementation tasks. Their flags must be designed and tested rather than invented here as working commands.

### 6.4 Freeze releases and retain their provenance

Each candidate preserves source, inputs, generator revision, toolchain, export, motion/collision definitions where supported, reports and previews. Exact acceptance freezes those contents. Further changes create a new candidate and release version. The existing installer owns immutable manifests, hashes and registry pointers; the authoring coordinator supplies inputs and evidence, not hand-edited manifests. (Installation contract (registry repository `docs/asset-contract.md`))

A release's compatibility entry names tested consumer revision and feature set. An unsupported feature stays pending or blocks the relevant promised use. If only a visual prototype is accepted, label it that way rather than weakening tests until it resembles a production release. Reopening an old release for a new renderer creates new compatibility evidence without silently changing its original files.

## 7. Prove one bounded pilot before expanding

### 7.1 Proposed pilot scope

The illustrative pilot is a small single-pilot utility hull with a hawk-like cockpit, a narrow neck widening into a central body, and a belly ramp deploying forward toward the cockpit. These traits derive from the earlier discussion but are **not a binding ship design or authorization to start modelling**. The owner would first confirm the mission, scale, crew fixture, cargo and landing assumptions.

The pilot includes three actively authored regions—cockpit, neck and widening belly—within a deliberately simple complete exterior. It includes one pilot fixture, one access/cargo fixture, one demonstrated ramp mechanism and reused static equipment placeholders. It excludes new propulsion design, new clamp qualification, procedural fleet generation, elaborate habitation, damage systems and complete thermal/structural certification.

Its purpose is to demonstrate that an approved silhouette survives useful interior space, local regeneration and an exported moving opening. A decorative finished ship that cannot rebuild cleanly fails this pilot. A perfectly reproducible shape that loses the cockpit character also fails.

### 7.2 Pilot acceptance tests

| ID | Test | Passing evidence |
|---|---|---|
| P01 | Coherent direction | Owner selects a single blockout from identical registered views; choices and assumptions recorded |
| P02 | Clean rebuild | Same pinned inputs yield stable IDs, hierarchy, bounds and canonical geometry signatures |
| P03 | Local change isolation | Belly edit leaves cockpit source/output unchanged unless an explicit shared boundary changes |
| P04 | Boundary failure | Deliberately incompatible seam produces a specific blocker, not silent neighboring regeneration |
| P05 | Identity retention | Before/after top, side, front and underside views preserve accepted landmarks and cockpit character |
| P06 | Occupied-space fit | Actual pilot ingress, seated reach/view and route fixtures fit at declared scale |
| P07 | Ramp usability | Documented landing stance, supported hinge/actuator, directional trajectory and usable passage |
| P08 | Motion screening | Sample schedule, worst clearance witness and exclusions recorded; no continuous-proof claim |
| P09 | Coordinate negative test | Intentionally rotated port fails raw-frame test even when its position is correct |
| P10 | Reuse accounting | Equipment referenced once; stable instance IDs survive reorder and assembly roundtrip |
| P11 | Dependency failure | Missing or changed locked dependency blocks without silently substituting an asset |
| P12 | Evidence invalidation | Source edit marks dependent checks stale; unrelated evidence remains valid |
| P13 | Manual preservation | Regeneration retains the pinned manually authored canopy input |
| P14 | Recovery | Interrupted build preserves last good candidate; accepted files cannot be overwritten |
| P15 | Consumer separation | Document validation, loader, walking, playback and GPU statuses are individually reported |
| P16 | Release traceability | Every accepted claim points to exact candidate, tool and evidence hashes |

Tests should include intentional failures, not only a happy-path demonstration. P09 is particularly valuable because the earlier cradle defect passed positions while failing all orientations. For P08, a near-contact interval should trigger additional samples or conservative treatment rather than a universal fixed sampling count.

### 7.3 Implement in phases with measurable exits

| Phase | Build or formalize | Exit condition |
|---|---|---|
| A: authoring discipline | Sidecar, IDs, reference atlas, camera set and decision records | One manually orchestrated pilot has traceable design gates |
| B: regional generation | Owned collections, profile inputs, shared boundaries, dependency cache and manual-source preservation | P02–P04, P12–P14 pass |
| C: representation checks | Raw-frame fixture, evaluated/source/export comparisons and mechanism evidence adapter | P07–P09 pass with explicit limitations |
| D: assembly projection | Supported sidecar-to-`.ship.yaml` mapping and locked component references | P10–P11 and roundtrip validation pass |
| E: runtime proof | Engine-owned walk, placement, playback and performance test scenes | Claimed runtime features have actual consumer evidence |
| F: controlled expansion | Additional hull families and larger interior zones | Second type demonstrates reuse without erasing distinctive identity |

Measure elapsed effort, repeated defects, avoidable regeneration, owner decisions and stale-evidence incidents during the pilot. These observations should determine further automation. Do not promise a production schedule from a conceptual specification. A full fleet generator is justified only after a second distinct type shows that the workflow generalizes beyond one script.

### 7.4 Decisions still needed and proposed defaults

| Open decision | Proposed default for review | What it affects |
|---|---|---|
| First mission and capacity | Small single-pilot utility study | All space and equipment requirements |
| Visual direction | Three coarse alternatives, then one selected | Reference atlas and protected landmarks |
| Crew/cargo fixtures | Existing game assets where available; otherwise labelled provisional fixtures | Passage, reach, view and loading tests |
| Environment and landing stance | One declared nominal condition initially | Ramp/gear scope and contact tests |
| Sidecar ownership/version | Draft authoring-only format under ships/Blender review | Compiler and compatibility responsibilities |
| Global coordinate profile | Explicit +Y-forward/+Z-up authoring profile for new pilot only | Source/export adapter and fixtures |
| Runtime feature target | Minimal supported motion and walk prototype | Engine evidence and release claims |
| Numeric budgets/tolerances | Set from pilot workload and approved scale | QC thresholds and optimization |
| Engineering depth | Visual prototype with separately listed unqualified systems | What release may truthfully claim |
| Autonomous scope | Routine construction/checks within accepted gates | When owner decisions are reopened |

The most important early investment is a reliable boundary between design intent and implementation. Once the chosen cockpit, spatial requirements and interface contracts can survive a local rebuild, adding detail becomes controlled production. Until then, generating more geometry mostly makes changes more expensive.

## 8. Sources and research provenance

External sources below supplied specific methods or historical examples. The workflow rules, schema examples and pilot are proposed synthesis. No source establishes a universal spacecraft budget, safe mechanism or qualified FREEFALL ship.

| Source | Scope used |
|---|---|
| [RSI Constellation interview, 2012](https://robertsspaceindustries.com/en/comm-link/transmission/12783-Introducing-The-RSI-Constellation-Mk3) | Historical artist account of identity and functional design |
| [CIG ship pipeline, 2014](https://robertsspaceindustries.com/en/comm-link/transmission/14244-How-Ideas-Take-Flight-The-Star-Citizen-Ship-Pipeline) | Historical staged readiness and reuse |
| [CIG Cutlass history, 2015](https://robertsspaceindustries.com/en/comm-link/engineering/15056-Design-Notes-Sharpening-The-Cutlass) | Coupled role, arrangement and visual revision |
| [CIG May 2015 report](https://robertsspaceindustries.com/en/comm-link/transmission/14758-Monthly-Studio-Report-May-2015) | Historical manufacturer design language |
| [NASA physical characteristics](https://www.nasa.gov/reference/4-0-physical-characteristics-and-capabilities-vol-2/) and [crew interfaces](https://www.nasa.gov/reference/10-0-crew-interfaces-vol-2/) | Tailored fit/reach/view/task evaluation, no certification claim |
| [NASA/TP-2014-218556](https://ntrs.nasa.gov/api/citations/20140009559/downloads/20140009559.pdf) | Iterative human integration evaluation |
| [Blender Bridge Edge Loops 4.4](https://docs.blender.org/manual/en/4.4/modeling/meshes/editing/edge/bridge_edge_loops.html), [Depsgraph 5.2](https://docs.blender.org/api/5.2/bpy.types.Depsgraph.html) | Profile construction and evaluated geometry |
| [Blender CLI 3.6](https://docs.blender.org/manual/en/3.6/advanced/command_line/arguments.html), [Boolean documentation](https://docs.blender.org/manual/en/dev/modeling/modifiers/generate/booleans.html), [glTF exporter 3.6](https://docs.blender.org/manual/en/3.6/addons/import_export/scene_gltf2.html) | Build/export foundations; installed-version behavior must be rechecked |
| [Khronos glTF 2.0](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html), [validator](https://github.com/KhronosGroup/glTF-Validator/blob/main/README.md) | Interchange frames, hierarchy and format validation |
| [Godot collision](https://docs.godotengine.org/en/4.4/tutorials/physics/collision_shapes_3d.html), [LOD](https://docs.godotengine.org/en/4.7/tutorials/3d/mesh_lod.html), [performance](https://docs.godotengine.org/en/stable/tutorials/performance/optimizing_3d_performance.html) | Illustrative methods only; not the FREEFALL engine contract |

Local sources were read through research notes dated 2026-10-20. Registry and ships snapshot revisions are recorded in section 1.3; Blender sources and skills are working-tree files with no revision asserted here. Earlier proposals may lag implementation and must not override maintained schemas.

| Exact local path | Role |
|---|---|
| `external repository universe-ships/docs/ship-code.md` | Current ship-as-code implementation description |
| `external repository universe-ships/crates/core/ship-design/ship.schema.json` | Maintained runtime document schema; draft identifier remains a caveat |
| `external repository universe-ships/crates/game/src/interior/ship_code.rs` | Inspected CLI implementation |
| `external repository universe-ships/docs/ship-import.md` | Hull import frame and artifact contract |
| `external repository universe-ships/docs/equipment-mounting.md` | Primary-mount placement semantics |
| `registry repository/docs/asset-contract.md` | Registry assets, templates, rework and installation |
| `registry repository/docs/proposals/physical-mounts.md` | Attachment semantics and provisional qualification |
| `registry repository/docs/proposals/plant-motion.md` | Motion capability evolution and limitations |
| `registry repository/docs/formats/freefall-motion-1.schema.yaml` | Motion interchange schema |
| `external repository blender/build_mining_ship.py` | Existing profile/loft helpers, not a generic compiler |
| `external repository blender/physical-mounts/check_ports.py` | Raw-GLB port-check pattern |
| `external repository blender/physical-mounts/cradle-v1-port-diagnostic.json` | Concrete orientation regression evidence |
| `private historical bridge-contract review (2026-10-19; not bundled)` | Actual-frame allocation limitations |
| `external installed workflow: shipbuilding/SKILL.md` | Live Blender inspection and localized repair practices |
| `external installed workflow: freefall-asset-delivery/SKILL.md` | Existing asset state machine and acceptance |
| `external installed workflow: freefall-asset-delivery/references/execution-loop.md` | Evidence invalidation and recovery practices |
| `external repository blender/docs/spaceship-building-guidelines.md` | Short owner-facing workflow agreement |

The research notes are retained at `external repository blender/research_notes/Spaceship generation workflow specification/design.md`, `production.md` and `local-contracts.md`. The canonical proposal is `external repository blender/docs/spaceship-generation-workflow-spec.md`; the complete report copy is `external repository blender/reports/Spaceship generation workflow specification.md`. Both copies should remain identical until one is explicitly retired or replaced by a versioned publication process.
