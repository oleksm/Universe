# The test suite cut to 77 fast tests

- **From 164 tests (~19 s of test time) to 77 (the slowest binary 0.6 s).** All pass; clippy clean.
- **Every test over ~0.5 s is gone:** the route autopilot run (9.5 s), pads full, the heaviest and
  nimblest hulls, the miner's whole round, the hangar stay, gate fly-through, closing on a rock,
  the shared corridor, keeping at range, settlers ganging up, autoland from orbit, lockstep pilots.
  What they guarded is mostly held by fast unit tests (ATC pads and corridor, the anchor and
  excavator, the docking computer from spawn, the plan reaching the pads, replay determinism).
- **Redundant fast tests pruned:** duplicates of a unit test by an interaction test (law, SAMs,
  prices), trivial ones (a cube is a cube, names are sayable, profiler scopes), and per-module
  extras where one test covers the module's promise (thrusters, mining, hyperdrive, design,
  shape, content, weapons, save compatibility). The dogma checks merged to 5.
- Dead test helpers and now-empty test modules removed.
