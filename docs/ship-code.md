# Ship code in Studio

Open **CODE** (Ctrl+E), paste a ship YAML document, then press **APPLY**
(Ctrl+Enter). Studio validates the whole document before replacing the current
editable ship. Invalid code stays in the editor with an error; the ship is unchanged.
The same installed equipment visuals appear in Design, Walk, Balance and Test Drive.

The code panel supports selection, undo/redo, copy/cut/paste and scrolling. PASTE
replaces the whole draft; Ctrl+V inserts at the selection. COPY copies the entire
source. EXPORT writes a new `.ship.yaml` under the application data directory's
`freefall/exports` folder and reports its path. CLOSE keeps an unapplied draft;
REVERT discards that draft and reloads the current visual design. Apply is one
whole-design undo step, including decks and view state. Visual edits made while a
code draft is closed are retained until Apply explicitly replaces the design.

Ctrl+S saves the editable ship as one YAML document, including all decks and display
layers. Open lists these documents alongside legacy designs. Existing `.json` and
`.decks.json` saves remain readable; their next save creates YAML without removing
the old files. A document opened by path saves back to that path. New documents
use the `freefall/interiors` application data directory and avoid existing names.
Unapplied drafts must be applied or reverted before saving the visual ship.

Saving writes and syncs a temporary sibling before replacing the destination.
Changes made externally since Open/Save cause a conflict error rather than being
overwritten. This is a pre-save content check, not a cross-process locking protocol.
Save-and-close remains open if saving fails, including deck-only changes.

Author IDs follow editor entities through moves, reordering, deletion and undo.
Unchanged source is byte-for-byte preserved. Simple scalar edits retain comments
in place. Structural changes use canonical YAML and retain comment-bearing source
lines in a labelled comment appendix. Unchanged metadata and numeric precision
survive the native editor projection; quaternion edits remain coupled rotations.

The adapter covers equipment, holds, points, paths, groups, hatches, path ends,
frame members, plates, landing pads, deck floors, curved walls and doors, stairs,
ladders, requirements, layers and camera/work plane. Hidden layers are saved too.
Hull-free decks use the shared deck builder for drawing, walking surfaces and
ladder climbing. Authored room/deck surfaces also appear in Balance and Test Drive
through the same placed-mesh renderer as installed equipment. Connections retain explicit destination-deck identities.

## Command-line use

Run from the repository root with a binary built from this branch:

```sh
cargo run --bin freefall-studio -- docs/proposals/ship-code-v1/all-layers.ship.yaml
cargo run --bin freefall-studio -- --validate-ship docs/proposals/ship-code-v1/all-layers.ship.yaml
cargo run --bin freefall-studio -- --export-ship design-3 --output /tmp/design-3.ship.yaml
```

Omit `--output` to print YAML. Explicit export never replaces an existing file.
`UNIVERSE_SHIP_CODE=1` opens the code panel at startup for review captures.

## Contract and limits

The [ship-as-code guide](proposals/ship-code-v1/README.md) remains the design guide.
`universe-ship-design` owns the schema, parser, source patching and validation;
Studio owns projection to the editor. The maintained schema is
`crates/core/ship-design/ship.schema.json`. Registry records remain authoritative
for mass, thrust and material properties; YAML describes placement and requirements.

Unavailable records, unverified dependency locks/model hashes and unsupported
named hull-anchor bindings produce errors rather than silently omitted geometry.
Neutral models retain the existing envelope fallback when no installed mesh exists.
Physical envelopes retain the draft's axis-aligned convention. Code validation
and a visible model do not imply structural or flight acceptance.

## Verification

191 workspace tests pass. After the final cross-mode surface change, all 33 game
unit tests pass again. The registry build and Studio binary build pass. Six CLI
checks cover source-exact export, legacy design/deck migration and no-clobber
behavior. Automated Vulkan captures cover CODE, Design, Walk, Balance and Test
Drive. Clipboard-to-Apply uses the real editor with an injected test clipboard;
manual desktop clicking was not part of this run. See
[verification record](diagnostics/ship-code-v2.json).

Installed engine orientation limitation (2026-10-12): the Studio module pose
uses local +Y for simulated thrust. CH-S1 and CH-S4 packages declare glTF +Z
exhaust (therefore -Z thrust), but the shared preview adapter currently applies
the saved quaternion directly to their raw mesh. Their nozzle direction is
90 degrees from the simulated force in every Studio view. This predates the
YAML integration; model loading/drawing is verified, nozzle-to-force alignment
is held. See [the basis review](diagnostics/studio-engine-basis-review.json).
A registry-backed model-to-design basis is required; changing the saved pose
cannot repair this relative mismatch. Physics and saved designs are unchanged.
