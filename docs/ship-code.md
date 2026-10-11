# Ship code: first implementation

Ship Studio can validate a `.ship.yaml` document, open it as a preview using the
normal Studio views, and export an existing saved design to YAML. This is the
first implementation of the [ship-as-code guide](proposals/ship-code-v1/README.md),
not the complete code editor or replacement save format.

From the repository root, using a binary built from this branch:

```sh
cargo run --bin freefall-studio -- --validate-ship docs/proposals/ship-code-v1/all-layers.ship.yaml
cargo run --bin freefall-studio -- docs/proposals/ship-code-v1/all-layers.ship.yaml
cargo run --bin freefall-studio -- --export-ship design-3 --output /tmp/design-3.ship.yaml
```

Omit `--output` to print YAML to stdout. An existing output file is never replaced.
Export writes and syncs a temporary sibling, then publishes the complete file.
Saved designs and their separate deck files are read without resizing equipment,
snapping anchors or changing the source saves. A missing or unrepresentable record
is an error, not an excuse to omit that part of the ship.

The shared `universe-ship-design` crate owns the document types, generated schema,
YAML parser, canonical writer and structural/reference validation. Studio owns the
adapter to its current editor and simulation. Registry equipment and stock retain
authority over mass, thrust and material properties. YAML contains placement and
authored requirements, not overrides of those facts.

The adapter covers equipment, holds, points, paths, groups, hatches, path ends,
frame members, plates, landing pads, deck floors, curved walls and doors, stairs,
ladders, requirements, display layers and camera/work plane. Hidden collections
are exported too. Legacy saves do not contain visibility or camera state, so their
headless export uses Studio defaults. Trip hours in legacy saves become seconds.
Legacy entities receive deterministic IDs for this migration export; these are not
yet persistent identities through subsequent visual editing.

Full quaternion orientation, including roll, now follows the existing shared
placement path through Design, catalogue, Walk, Balance and Test Drive. Physical
envelopes remain aligned with the design axes as specified by the draft. Models
are centered once and are not stretched to the envelope. Legacy saves without a
quaternion retain their previous thrust-direction behavior.

## Preview boundary

A YAML preview retains the source document and displays a persistent preview
label. Save cannot overwrite either the YAML source or an existing native design.
Visual experiments are temporary. Parsing and compatibility checks finish before
replacing the open design; a rejected document leaves it unchanged.

This boundary is deliberate: the old editor still stores many identities as array
indexes and has separate deck undo/save state. Enabling YAML save through that
representation would lose author IDs, comments and metadata after some edits.
The next step is stable editor identities and one document/undo/save transaction,
then the in-app code panel, paste, copy/export and source-preserving Apply.

The preview currently rejects unresolved equipment/stock, unavailable hulls,
unverified dependency locks, explicit hull model hashes, named hull-anchor
bindings, and stairs/ladders that do not connect to the next higher deck. Validating
for preview includes these compatibility checks; the shared library's validation
only checks the document itself. A successful check is not structural or flight
acceptance. Missing installed neutral meshes retain Studio's existing envelope
fallback. Source-only decks in a hull-free design retain the current Studio
limitation: deck geometry that requires hull clipping is not drawn.

The maintained schema is `crates/core/ship-design/ship.schema.json`, generated from
the typed document. The proposal schema remains the original design reference.
The library retains original source text and locations, but its canonical emitter
is not a comment-preserving editor.

## Verification

164 workspace tests pass, including the five Studio conversion/placement tests
and 13 shared-document tests. The registry build and Studio binary build pass.
Headless CLI checks cover all-layer example validation, legacy design plus deck
export, validation of that export, unchanged source hashes, and existing-file
rejection. A Vulkan preview capture was inspected and shows the installed CH-S2
and flight computer meshes. See [verification record](diagnostics/ship-code-v1.json).
