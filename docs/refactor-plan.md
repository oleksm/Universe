# Refactor plan: physics kernel / world / avionics / orchestration

Target: `docs/architecture.md`. Executed by phase agents in order; each phase leaves the
workspace compiling, clippy-clean and with **every test passing**.

## Ground rules (all phases)

- **Behaviour-preserving.** This is a restructuring, not a redesign of gameplay. Existing tests
  move with the code but keep their meaning; none are deleted to make a phase pass. If a test
  must change, say why in the phase's changelog entry.
- **Gates for "done"** (run all of these at the end of a phase):
  - `cargo clippy --workspace --all-targets` → no warnings, no errors
  - `cargo test --workspace --release` → all pass
  - `cargo test --workspace --release -- --ignored` → all pass (long runs: settlers, routes,
    moon landings, ETA smoothness, bench). Compare with the baseline below: 0 crashes in traffic
    runs; landing/docking times and bench numbers within ~20% unless the change is explained.
  - `cargo build --release` for the game.
- **Git**: the repo is on GitHub (`oleksm/Universe`, `main`). The orchestrator commits and pushes
  after each phase passes its gates; phase agents don't run git commands themselves (a scratchpad
  tar backup is optional). Never run destructive commands on the tree beyond your own edits.
- **Determinism**: no wall-clock or unseeded randomness in simulation code.
- Keep comments/code style like the surrounding code. Keep dev scenarios
  (`crates/game/src/dev.rs`, `UNIVERSE_SCENARIO=... UNIVERSE_SCREENSHOT=...`) working.
- Record what you did in `changelog/2026-09-30-architecture.md` (append a section for your
  phase: what moved where, decisions, deviations from the plan, test/bench numbers).
- If the plan is wrong somewhere, deviate deliberately and document it in the changelog and in
  `docs/architecture.md`.

## Phase 1 — Physics kernel crate (`crates/core/physics`, `universe-physics`)

Create the kernel and move the generic physics into it; `universe-sim` depends on it.

- **Rails**: `Orbit` (from `sim/orbit.rs`), a rail-body description (parent, orbit, mu, radius,
  tilt, day, optional surface), positions/velocity/rotation/angular velocity at time,
  `Ephemeris`, gravity sum, dominant body. `StarSystem` keeps its content (names, kinds,
  colours, spaceports) but delegates motion/gravity to kernel rails.
- **Surface trait** (`height(dir)`, `max_height()`, `surface(dir)` incl. oceans as a flat
  surface); `Terrain` (stays in sim/world content) implements it.
- **Rigid body + integrator**: the leapfrog + adaptive substep logic from `flight_step`, taking
  a force callback (so devices/autopilots are *outside*), with the ephemeris shared per system
  and moment.
- **Colliders & facts**: surface collider (terrain contact), convex polytope with cut-outs (the
  station's cuboctahedron: cube ∩ octahedron, slot as a cut-out box), ring + opening trigger
  (from `gate::crossing`, including the "where was the gate at the start of the step" fix).
  Return facts (contact/trigger), not decisions.
- **Ops**: weld (landed/docked pose in a rail body's frame, updated each step), relocate between
  frames (gate transit).
- **Queries**: gravity, dominant, segment clearance, simulate-copy primitive for the planner.
- **Kernel tests**: move `orbit_state_matches_numeric_derivative`, orbit stability; add a
  determinism test (hash of a multi-body state after N steps is reproducible) and collider tests
  (polytope + cut-out, ring hit vs opening, surface contact).
- The kernel must not contain game words: station, spaceport, gate, dock, land, autopilot,
  clearance, route, nav, player, settler (generic names only: polytope, cut-out, ring, trigger,
  weld, relocate…).

## Phase 2 — World layer (`crates/core/world`, `universe-world`)

Split world content and device/contact rules out of `sim`:

- Move galaxy, names, rng, units, system generation, terrain content, gate network data into
  `universe-world`.
- **Ship = rigid body + devices**. Introduce `ShipCommands { throttle, rcs, turn (pitch/yaw/roll),
  hyperdrive: { engage, heading, reference-frame velocity, stop distance } }`. The world's ship
  step reads **only** commands (+ physics facts) — never nav target, clearance or route.
- Devices: engine, thrusters/lift, attitude, **hyperdrive device** (speed law from clearance and
  the commanded stop distance; interlock never entering a body; on disengage keep the commanded
  frame velocity), **docking port**, **landing gear**, **gate device** (trigger → relocate op),
  damage (impact speed → destroyed; ocean → destroyed), respawn rules.
- Traffic control service: clearance request/grant/deny rules (ranges, "not in flight", etc.).
- Events split: physical facts vs service/UI.
- Physics decisions must not read navigation state (e.g. fine substeps near structures should
  come from proximity to structures, not from "has clearance").

## Phase 3 — Avionics (`crates/core/avionics`, `universe-avionics`)

- Move guidance, the dock/land/gate autopilots, the hyperdrive autopilot (steering to the target,
  going around bodies, deciding when to drop out and which frame/stop distance to command), the
  route autopilot (and `gate_path`), and the flight planner.
- Per-ship avionics state: nav target, clearance (as granted by traffic control), autopilot
  flags/phases, route. `Ship` no longer carries these.
- Avionics produces `ShipCommands` from a read-only sensor view of the world.
- The planner simulates a copy using the kernel's simulate primitive + the same autopilot
  (delete the planner's private integrator).
- Status types for the HUD (DockingStatus, LandingStatus, GateStatus, Approach, Plan) live here.

## Phase 4 — Orchestration and game (`crates/core/sim`, `crates/game`)

- `universe-sim` becomes the orchestrator: `Universe` owns world + player ship + crafts, each ship
  with its avionics; `step_world` = for each ship in fixed order: avionics/player input →
  commands → world/physics step → facts → rules → events. Settlers, traffic stats, crash log,
  save/load (keep save compatibility where reasonable; document if not).
- Update the game crate to the new APIs; no gameplay/HUD regressions. Check a few dev scenarios
  render (cleared, autodock, landing, autoland, gate, transit, route, traffic).
- Update `docs/architecture.md` to describe the final state (what's implemented vs future).

## Phase 5 — Verification (parallel, read-only)

- Architecture audit: dependency graph and forbidden concepts per crate; any physics code that
  reads avionics/world-rule state; any position/velocity writes outside kernel ops/integration.
- Regression: all tests + ignored long runs + bench vs baseline.
- Game smoke: build and capture scenarios; inspect.
- Code review: correctness risks introduced by the move.
Findings are fixed in a final pass.

## Baseline (before the refactor)

29 tests + 10 long-running (ignored) tests pass. Long runs and bench:

- docking from spawn: planned 149 s, actual 150 s; landing from orbit (with hyperjump):
  actual ~384–690 s; gate from 30 km: 259 s; 10-stop settler route: complete in 3.5 game h.
- all 13 moon ports: autoland succeeds (264–285 s).
- 100 settlers × 10 game h: 1,868 stops, 1,999 transits, 156 routes done, **0 crashes**,
  0.26 ms/frame. 1,000 settlers flying: **0 crashes**, 2.25 ms/frame.
- bench (µs/frame, one ship): coasting 2.0, warp 1000 4.65, docking autopilot 5.0, landing
  autopilot 3.4 (warp 100: 6.5), hyperdrive 1.8, near ground 2.1; body positions 1.37 µs,
  gravity 0.06 µs, terrain lookup 0.13 µs; planner 1.97 ms (landing) / 1.09 ms (docking).
- ETA smoothness: docking biggest jump 0.41 s, gate 0.23 s, landing 0.08 s.
