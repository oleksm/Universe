# Ship as code in Ship Studio

Ship Studio should let a designer describe a complete ship in human-readable YAML, open that code beside the visual editor, copy or export it, and paste it back to recreate the design. Code and visual tools must edit the same ship document, with every current design layer represented. The resulting design must use the same model resolution, analysis and flight-control paths as a visually authored ship.

This is the agreed direction and implementation guide following the owner's review on 10 October 2026. YAML is the authoring format. The schema and examples remain drafts; the importer and code editor are not implemented. The goal is a complete round trip: design visually, export YAML, paste into a fresh design, and retain equivalent geometry, references, layers, budgets and test-drive inputs.

The designer opens **Code** beside the visual view and sees the complete current design. The editor offers equipment and stock names, field descriptions and reference completion. Selecting an object in the viewport highlights its definition; selecting its definition highlights the object. Applying valid code updates the design as one undoable operation. An invalid draft keeps the last valid design visible and identifies errors by line, object and field. Pending text edits must be applied or discarded before visual edits can overwrite those same values.

**Copy ship** and **Export ship** include all authored content, including hidden layers and decks. Paste or Import opens a new design by default; replacing the current design is an explicit, undoable action. Copy selection produces an assembly fragment, with its necessary references included or reported. Pasting a fragment assigns fresh instance IDs and updates its internal references together. Renaming a design does not change the identities of its objects, and an imported display name is never used unchecked as a filesystem path.

YAML's mappings, sequences and scalars suit a document people will read and edit, and comments let a designer explain why a component is there. The format will use YAML 1.2, with a defined subset that maps cleanly to the typed design document. The YAML specification distinguishes its data model from presentation details such as comments; retaining comments therefore needs explicit editor support. [YAML specification](https://yaml.org/spec/1.2.2/)

The normal authoring and export extension is **`.ship.yaml`**. Strict JSON can remain an optional machine interchange format for the same data. JSON Schema remains the structural validation format: parse YAML into ordinary string-keyed mappings, arrays and scalar values, then validate those values. This does not require the designer to write JSON. The draft uses [JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12), with simple constructs suitable for editor tooling.

For the first version, a file contains one YAML document. Keys are strings; numbers must be finite; duplicate keys are errors. Custom tags, merge keys and aliases are outside the initial profile, keeping object identity explicit through IDs. A versioned profile must define scalar interpretation rather than inherit a parser's YAML 1.1 defaults. This prevents a name such as `on` from unexpectedly becoming a boolean. Parser selection remains an implementation decision, with comment-preserving edits and clear source locations required acceptance tests.

An equipment definition can look like this excerpt from the draft. Placement and envelope dimensions are illustrative, not an accepted engineering fit:

```yaml
modules:
  engine_left:
    kind: equipment
    record: equipment.engine.ch.s2
    pose:
      position_m: [-2, 1, 3]
      # Local +Y thrust points toward the ship's nose, -Z.
      rotation_xyzw: [-0.7071067811865475, 0, 0, 0.7071067811865476]
    envelope_m: [1.5, 1.5, 2.3]
```

The equipment key selects the registry specification and installed model. The instance ID, `engine_left`, identifies this placement independently of other engines of the same type. Mass, thrust and material properties remain registry facts. Editing the ship document changes the arrangement and requirements, not those equipment specifications. Missing visual packages retain the existing envelope fallback and a diagnostic; missing equipment records remain unresolved and cannot quietly produce invented budgets or flight behaviour.

The shared document is the centre of the implementation:

```text
YAML code editor <-> Shared ship document <-> Visual design tools
                              |
                              v
                Design / Walk / Balance / Test Drive
```

A source-preserving YAML representation retains comments, formatting and source locations. A single typed semantic document holds the ship's meaning. Visual changes update the affected definitions through that bridge; typing an incomplete YAML draft does not create a second authoritative ship. Derived render, analysis and test-drive data are rebuilt from the shared document. The production JSON Schema should be generated from its Rust types so schema and serialization cannot drift independently. Schemars supports schema generation from types and follows Serde serialization attributes. [Schemars documentation](https://docs.rs/schemars/latest/schemars/)

The document must cover the complete current design, not just placed modules:

| Content | Stored authoring data |
| --- | --- |
| Hull and fixed points | Hull reference, anchor bindings, authored points and names, including entry, dash, windows, service, mining and mounting points |
| Modules | Equipment record, stable instance ID, optional hull slot, placement, orientation, physical envelope and hull-hold placement |
| Access | Points, paths, cross-sections, groups, walls, side doors, end closures and hatches |
| Frame | Members, stock references, endpoints and pinned joints |
| Structural decks | Plate bounds, heights and stock |
| 2D decks | Levels, floor outlines, curved walls, railings, doors, ladders and stairs |
| Landing supports | Authored landing-pad positions |
| Presentation | Named layer visibility, work plane and optional camera |
| Design requirements | Gravity, cargo, crew, trip duration and delta-v targets |

The [coverage map](layer-coverage.json) accounts for all 26 current layer entries. Some entries expose authored objects; others expose calculated results. Clashes, pressure, reach, hollow maps and structural results are recalculated from the ship and its dependencies. Their display settings are saved, while cached answers are not authoritative content. Hidden layers remain in full exports and continue to participate in analysis and physics.

Every referencable object needs a stable ID. Paths refer to named point IDs instead of positions in an array. Stairs and ladders identify their destination deck explicitly. Reordering code cannot reconnect a corridor, change a stair's destination or select a different module. IDs are scoped to their object collections, and cross-collection references have defined types.

Coordinates are metres in the existing right-handed design frame: +X right, +Y up and -Z forward. Quaternion order is x, y, z, w, and rotation maps local coordinates into design coordinates. Placement origin and asset-centering conventions must be defined once in the shared adapter. The current schema draft preserves the existing design-axis-aligned physical envelope separately from model orientation; that envelope must never stretch the installed mesh. General module rotation is a consumer integration task because today's saved block stores position, envelope size and an optional thrust direction, rather than a complete orientation. Unsupported poses must not be silently discarded.

The document has an explicit format version. Exports record their registry revision and referenced model package versions or hashes. Compatible resolution reports changed dependencies; locked resolution requires the recorded dependencies. A schema-valid document is not evidence that the necessary assets are installed, that the structure is sound or that the ship can fly. Import produces ordinary design diagnostics and retains existing runtime acceptance gates.

The current implementation provides useful foundations but needs consolidation. [Plan](../../../crates/game/src/interior.rs) stores modules, structure, access and requirements, while [DeckPlan](../../../crates/core/world/src/deckplan.rs) is saved separately. Layer visibility is session state, and several references are array indexes. Migration must combine the old design and deck files, assign stable IDs and preserve their relationships. Hull anchor snapshots need explicit dependency checks instead of silently moving authored content when a hull changes. Saving the unified document must be atomic, and undo must cover the whole document, including decks.

Delivery should proceed in three stages:

1. **Document foundation.** Introduce the shared typed document, stable IDs, combined design and deck storage, legacy migration, placement conventions, YAML import/export and atomic saves. Complete semantic validation and the adapters used by current Studio consumers.
2. **Code workflow.** Add the editor and clipboard integration, completion, diagnostics, Apply, shared undo, linked selection and comment-preserving visual edits. Ship complete coverage of current layers as part of this stage.
3. **Reusable design code.** Add assembly snippets, parameters, repetition and symmetry. Any procedural authoring resolves into the same shared document and uses the same consumers.

The feature is complete when these acceptance checks pass:

- Visual design to YAML to a fresh design preserves all authored layers, geometry, references, requirements, budgets and test-drive inputs.
- Hidden content, separate legacy deck plans and hull bindings survive migration and export.
- Reordered definitions retain their meaning; duplicate IDs or keys and dangling references produce precise diagnostics.
- Code Apply, visual edits and undo work across both the main design and decks without partial updates.
- Visual edits preserve unrelated comments; invalid text leaves the last valid design intact.
- Missing or changed records and model packages are reported without silently dropping objects or inventing specifications.
- Placement tests cover coordinate conventions, module orientation, centering and physical envelopes across Design, Walk, Balance and Test Drive.
- Parser and import limits are measured against representative large designs before declaring performance acceptable.

The working artifacts are the [draft schema](ship.schema.json), [YAML example](all-layers.ship.yaml), equivalent [JSON interchange example](all-layers.ship.json), [layer coverage map](layer-coverage.json) and [validation record](validation.json). Structural example validation is a preliminary check. Runtime round trips, YAML comment preservation, placement consumers and dependency locking remain implementation acceptance work.
