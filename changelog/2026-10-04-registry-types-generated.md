# The registry's types generated from its schemas

- **The registry crate's Rust types are generated from the registry's JSON schemas**
  (`crates/registry/build.rs`) every time a schema changes: 30 schemas, 241 types. A record's
  struct is its schema, field for field. Unknown fields are refused, enums are enums, a `oneOf`
  tagged by `kind` is a tagged enum, and an angle is `Degrees` with `.rad()`. The 2,600 lines of
  hand-written types are gone. A change in the registry is a rebuild: nothing to port by hand.
- **Every record of every kind is read**, the kinds the game doesn't use yet included (elements,
  parts, processes, the vocabulary): 1,022 records. **Every reference is checked through the
  schemas' `x-ref`**, not by hand per kind. The registry in the binary is about 1 MB, decoded once
  at start in a few milliseconds.
- **The generator refuses what it can't type**, and says where. Its first catch: `part.schema.yaml`
  had `revision` twice (a design letter and the lifecycle stage). YAML kept the second; the first
  is removed.
- Nothing changes in play.
- **Handler traits, generated:** for every kind-tagged choice in a schema (a device's `function`),
  a trait with one method a kind, `kind()` (its registry name) and `handle()`. The engine turns
  equipment into its devices by implementing it. A kind added in the registry is a method the build
  asks for. A kind marked `x-in-game: not made` defaults to `not_made`, so the build passes until
  there's behaviour for it.
- **Every generated enum has `as_str()`,** its registry text, so a new zone use or facility kind is
  named as the registry writes it.
