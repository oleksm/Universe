# Shared ship design foundation

Pure crate `universe-ship-design`: no game, filesystem, renderer or registry dependency.
`Document` and all component types are exported from the crate root. Stable object
IDs are BTreeMap keys, references are IDs, poses use metre arrays and local-to-design
xyzw quaternions. Coordinates and placement convention match the Studio draft.

- `parse(&str) -> Result<Document, Error>` checks YAML, structure and local semantics.
- `parse_source` / `parse_with_limits` also retain the original source and a JSON
  Pointer -> one-based line/column map. Invalid drafts are returned as diagnostics;
  the caller retains its last valid document.
- `emit(&Document) -> Result<String, Error>` validates then produces deterministic
  YAML 1.2. **Canonical emission discards comments/formatting.** Original source is
  retained separately, not rewritten. A comment-preserving visual-edit bridge is
  still an editor integration requirement, not implemented by canonical emit.
- `validate(&Document) -> Vec<Diagnostic>` has no dependency resolution side effects.
- `schema()` generates Draft 2020-12 from Rust types plus the explicit v1 field
  constraints. Regenerate `ship.schema.json` with `cargo run -p universe-ship-design
  --example schema`. The draft schema in tests is only an independent fixture.
- `Document::new(id, name)` supplies the required empty collections and conventions.

The profile follows YAML 1.2 core scalar resolution (on/off/yes/no are strings),
finite numbers, string mapping keys, one document, and no duplicate keys. Explicit
tags, tag directives, anchors, aliases and merge keys are rejected. Only an optional
YAML1.2 directive is accepted. Integer scalars must fit i64/u64; typed physical
values are f64. Limits default to16MiB text,64 nested values,1million value/key
nodes and1MiB per scalar. Callers can lower them through `Limits`.
Normative scalar table: https://yaml.org/spec/1.2.2/#103-core-schema

Validation includes shape/unknown fields, IDs, finite/ranged values, a unit quaternion
norm tolerance1e-6 (no silent normalization), local references, hull/slot prerequisites,
nonzero spans, ordered plate bounds, wall bulge counts, basic floor area/edges,
destination deck elevations, closure consistency and required locked registry revision.
It does not resolve registry records/assets, enforce full lock coverage against installed
models, verify hull anchor snapshots, prove polygon simplicity/collision/structure,
check flight readiness or migrate old saves. Those remain consumer responsibilities.
A schema-valid or locally valid document is not an accepted ship.

## Physical wiring (authoring foundation)

The optional `wiring` map stores `Wire` values under stable wire IDs. Each end
references an equipment instance in `modules`, its registry port ID and a connector
product. `cable` names a run product; `pins` maps conductor IDs to contact IDs at
the two ends. `waypoints_m` contains only intermediate route points in ship
coordinates. The endpoints follow the ports when their modules move or rotate.
The registry defines channel, role/polarity, flow, mating geometry and ratings;
the document does not override those facts. `from`/`to` order the route, not the
permitted direction of current or signal flow.

Omission denotes a legacy design without authored connectivity; an empty map
denotes explicit disconnection. The parser rejects dangling equipment references,
multiple wires occupying one socket, duplicate pin assignments, and invalid
waypoints. Consumers still need to resolve ports and check all electrical and
mechanical compatibility. Crossings and proximity create no connections.

Measured foundation check (2026-10-11, shared2CPU/8GiB, dev build): a1000-module
230306-byte document emitted in2.91ms and parsed+validated in18.00ms. This is a
bounded synthetic parser probe, not an editor/renderer performance claim.
Thirteen focused tests cover the full draft fixture, scalar spelling traps,
malformed input, schema parity/snapshot, source paths, deck references and limits.

## Source patching

`patch(&SourceDocument, &Document) -> Result<String>` validates the new document
and checks that the original source still matches its parsed document. Unchanged
input is returned byte-for-byte, including comments and newline style. Single-line
scalar edits (including flow-array numbers) preserve surrounding source bytes and
comment positions. Locations are reparsed rather than trusting mutable metadata.

Structural edits and unsupported multiline scalar edits use canonical YAML plus
an explicitly labelled comment appendix. Full-line comments are retained; inline
comments retain their original source line as context. To avoid losing comments,
this fallback conservatively retains every source line containing `#`, including
some quoted/block-scalar hashes that are not comments. It does not preserve comment
attachment or formatting in the fallback, and repeated fallback does not recursively
prefix archived source. This is a bounded bridge, not a full YAML syntax editor.

Every result reparses to the requested typed document. The normal parser limits
apply; an oversized comment-preserving result returns an error instead of dropping
comments. Callers must retain the original draft on errors and update their
SourceDocument by parsing the successfully saved result.
