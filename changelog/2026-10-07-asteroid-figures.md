# The asteroid figures from their record only (audit phase 4, #9)

- `belts.rs` and `small_bodies.rs` each carried the Sun's belt figures as fallbacks
  (`unwrap_or(3.08e11)` and the like, 17 of them); they read `seeding.asteroids` alone now, and a
  missing figure is an error that names it (`belts::need`). One `resonance` (it was in both),
  which refuses a resonance that isn't `p:q` (it turned one into 1).
- `belt::SMALLEST` (15 m) is gone: the field's draw reads `sizes.smallest`. The record cited the
  code's constant as its source; the Scientist is asked to cite the record's own basis.
- `goods::Ore` was an enum of five ore keys written in Rust, repeating the rock classes'
  `mining.yields` and `rich_yields`; it's the stock item one of those names now
  (`Ore::all()` from the records, `from_key`, `of_item`), so a new rock class or ore needs no code.
  The content check reads the yields from the records too.
- Three enums that copied generated ones variant for variant are the generated ones now:
  `modules::Feature` (a nav computer's), `materials::Process` (a fuel's release) and
  `terrain::TerrainKind` (a world's ground), each with its hand-written mapping gone. Generated
  enums derive `PartialOrd`, `Ord` now (features are sorted). `modules::SlotKind` stays the game's
  own: its slots are a subset with their own meaning (a relay slot, not the registry's gate).
- Records found by key, not by name: a place's settlement record (the economy matched its people
  and stocking law by the settlement's name), and a works' facility (the economy and the zoning
  panel matched its blueprint's name each time). `settlements::Settlement` and the economy's
  `Place` carry the settlement's key; a land office works resolves its blueprint to the facility's
  key once, when it's recorded. (A seeded port still finds its record by name and body: that's
  all a seeded port has. The law by its administration's name waits on a record field: asked.)
