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
