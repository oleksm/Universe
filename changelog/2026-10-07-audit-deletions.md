# The audit's deletions (phase 2)

- The ~36 hand-written "not made" stubs in `modules.rs`: the generated trait's defaults do it
  (one `not_made`), now that the records flag the kinds where the generator reads them.
- Dead Rust: `worldmaps::Slot` (folded into a doc on `SLOTS`), `App.net_seen` (written, never
  read), and `let _ = x;` leftovers (unused parameters are named `_x`).
- Doc comments on the wrong item moved to their own (Shadows, render, think_all, gunners,
  set_layout), doubled ones merged, and counts that had gone stale corrected (four cascades,
  four cloud levels; `look2.w` is the world's globe layer).
- With the lab: the dead cloud and noise functions in its shader files, on `planet`.
