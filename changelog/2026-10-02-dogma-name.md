# The core engine is called Dogma

- The core engine (the `universe-physics` crate and its laws) is **Dogma**: the laws every world runs
  on, the same whatever is built in it. On it stands **the world**: a seeded instance, its initial
  conditions (matter, makers, products, structures, production, economy).
- The laws' sheet is now `config/dogma.ron` (still generating `universe_physics::laws`).
- "Kernel" (used since the re-architecture for the physics crate) and the short-lived "kernel and
  distro" are renamed across the docs, the sheets, the code's comments and the dogma check
  `dogma_names_no_material`. Dated history (changelogs, the finished re-architecture plans) keeps its
  words.
