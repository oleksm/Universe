# The workspace in layers: core, engine, game, devkit

- The runtime's crates under `crates/core/` (physics, prof, protocol, registry, world, services, avionics,
  sim): the same crates, moved; paths in docs, records and tools follow.
- The engine depends on the profiler alone; the world only for its agreement test (`detail_agree`).
- The game is a library with two binaries: `freefall` (the game) and `freefall-studio` (the studio alone:
  its window, `--report`, `--fit`, `--compare`). `freefall` still forwards the studio's flags.
- The developer's kit apart (`crates/game/src/devkit`: scenarios, the observer port), built with the `dev`
  feature, on by default; a player build (`cargo build --release --no-default-features`) leaves it out.
- `crates/game/tests/layers.rs` holds the layers.
