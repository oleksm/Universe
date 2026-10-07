# The asteroid figures from their record only (audit phase 4, #9)

- `belts.rs` and `small_bodies.rs` each carried the Sun's belt figures as fallbacks
  (`unwrap_or(3.08e11)` and the like, 17 of them); they read `seeding.asteroids` alone now, and a
  missing figure is an error that names it (`belts::need`). One `resonance` (it was in both),
  which refuses a resonance that isn't `p:q` (it turned one into 1).
- `belt::SMALLEST` (15 m) is gone: the field's draw reads `sizes.smallest`. The record cited the
  code's constant as its source; the Scientist is asked to cite the record's own basis.
