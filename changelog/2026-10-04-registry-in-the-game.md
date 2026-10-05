# The registry built into the game

- **A registry crate** (`crates/registry`, `universe-registry`): the registry's records (`standards/`)
  as typed Rust. Each record's kind is the first part of its key. Unknown fields are refused, and
  every reference the game follows must name a record of the right kind.
- **Read when the game is built, decoded at start.** The world crate's build reads every record,
  checks them, and encodes what the game uses into the binary (6.9 KB today). At start the game
  decodes it once (16 µs). Nothing is read from disk while the game runs. A problem in the records
  stops the build and names each one. The registry is part of the content's hash, so a save or a
  peer made from other records is told apart.
- **The galaxy's settings are the registry's** (`seeding.galaxy`): the seed, the charted region's
  side, the density of stars, the sector size and the mix of star classes. `galaxy::REGION`,
  `STAR_DENSITY` and `SECTOR` and the class cut-offs in code are gone (`galaxy::charted()`). The
  galaxy comes out the same; the celestial guard holds it to the records.
- **Rock classes are read from the registry**, not from `content/base/rock_classes.ron`. The game's
  four classes are still its own code, held to their records by the guard. Next: made from the
  records.
- The game no longer loads `galaxy.ron` or `rock_classes.ron`.
