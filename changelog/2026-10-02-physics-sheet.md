# The physics sheet: the dogma, traceable

- **`config/physics.ron`:** every constant the world's physics runs on, in sections (nature, the
  medium, the hyperdrive, gates, drives, heat). Each has its value, unit, kind (real, grounded,
  simplified, invented, tuning, planned) and why.
- **The build makes the code's constants from it** (`universe_world::sheet`, via `build.rs`). The old
  hard-coded ones (the hyperdrive's rate, interlock and margin; the gates' size, limit and transit;
  the heat model's areas, limits and constants; the exhaust velocity; the hyperdrive's fuel draw;
  c, σ, air's heat capacity) now come from the sheet.
- **The dogma's claims are tests** (`crates/world/tests/dogma.rs`):
  - no ship today can cross between stars, not even one that's all reactor;
  - a future explorer makes 40 ly an epic (5 ly in about 1.3 days, 40 ly in about 10);
  - within a system the medium is stiff, and between stars it's slack;
  - gates are justified and limited.
- **The report:** `docs/physics-sheet.md`, generated (`cargo run -p universe-world --example
  physics_sheet`): the sheet's tables and what they add up to, ship by ship.
- The charter (`docs/physics.md`) points at it; the explorer's figures were corrected for the field's
  efficiency.
