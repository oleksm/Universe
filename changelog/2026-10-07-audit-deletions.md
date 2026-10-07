# The audit's deletions (phase 2)

- The ~36 hand-written "not made" stubs in `modules.rs`: the generated trait's defaults do it
  (one `not_made`), now that the records flag the kinds where the generator reads them.
- Dead Rust: `worldmaps::Slot` (folded into a doc on `SLOTS`), `App.net_seen` (written, never
  read), and `let _ = x;` leftovers (unused parameters are named `_x`).
- Doc comments on the wrong item moved to their own (Shadows, render, think_all, gunners,
  set_layout), doubled ones merged, and counts that had gone stale corrected (four cascades,
  four cloud levels; `look2.w` is the world's globe layer).
- With the lab: the dead cloud and noise functions in its shader files, on `planet`.
- Stale docs corrected: deck plans are kept with the hull's design (they said "not saved");
  `App` fields carried comments of fields long gone; content's module doc described the RON-pack
  era (most content is the registry's records now); error messages named `standards.ron`, which
  nothing reads. The world's seed is the registry's `seeding.galaxy`, required (no 1984 fallback).
- Eight generated RON files nothing reads (bodies, brands, celestial, galaxy, industry,
  rock_classes, settlements, standards): the Scientist is asked to stop writing them.
- RON-era serde derives dropped from types now built only from records (`Module`, `Does`,
  `Material`, `GoodsKind`, `HullDef`, `ThrusterDef`): nothing deserializes them any more.
- The eight unread RON files are gone (fso merged): content/base holds aliases, prices, shapes and
  the sheet; the registry build writes only its page and trackers.
