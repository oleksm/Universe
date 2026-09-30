# 2026-09-30 — Architecture: physics kernel / world / avionics / orchestration

## Decision

The user asked for the physics engine to be completely independent of the navigation programs,
and "ruthlessly stable and fair regardless of what we do on top of it" — because stations,
spaceports, gates and ships will later be things players *construct*. Adopted a layered
architecture (reference: `docs/architecture.md`):

- **physics kernel** (`universe-physics`): rail bodies, rigid bodies, gravity, integration,
  colliders (surface, polytope with cut-outs, ring + trigger), facts (contacts, triggers),
  audited ops (weld, relocate), queries (incl. simulate-a-copy). No game concepts.
- **world** (`universe-world`): content (galaxy, systems, terrain), structures, ships as rigid
  body + devices, device and contact rules (docking port, landing gear, gate device, hyperdrive
  device, damage), traffic control service, events.
- **avionics** (`universe-avionics`): ship software — nav computer, guidance, planner,
  autopilots (dock/land/gate/hyper/route); sensors in, `ShipCommands` out; future modules.
- **sim** (`universe-sim`): orchestration of ticks, ships and traffic; save/load.

Why: the review found physics and navigation interleaved in one 2,600-line `universe.rs` —
autopilots running inside the integrator's substep loop, the hyperdrive half engine / half
navigation computer, physics decisions reading clearance state, physics clearing navigation
state, navigation fields on the ship's physical state, and the planner carrying its own second
integrator (the source of several past bugs).

## Plan and process

`docs/refactor-plan.md`: four implementation phases (kernel → world → avionics → orchestration &
game), each leaving everything compiling with all tests passing, then a parallel verification
phase (architecture audit, regression vs baseline, game smoke test, code review) and a fix pass.
Executed by a multi-agent workflow. No git in this repo, so each phase backs up the tree first.

Baseline recorded in the plan (29 tests + 10 long runs green; traffic 0 crashes; bench numbers).
Pre-refactor fix: the long landing ETA-smoothness test assumed a flight plan every frame; with
the autopilot's hyperjump there is none during hyperdrive, so the test now measures the
countdown after the jump.

## Phase log

### Phase 1 — Physics kernel crate (`crates/physics`, `universe-physics`)

**What moved where**

- New crate `crates/physics` (package `universe-physics`, depends on `glam` only), added to the
  workspace; `universe-sim` depends on it and re-exports it as `universe_sim::physics` (and
  `universe_sim::Orbit` as before). Modules:
  - `orbit` — `Orbit`, moved verbatim from `sim/orbit.rs` (file deleted).
  - `rails` — `RailBody { parent, orbit, mu, attracts, radius, tilt, day, collider }` with
    `rotation`/`angular_velocity`; the `OnRails` trait (`rail()` + optional `surface()`);
    `positions`, `velocity`, `Ephemeris` (moved from `system.rs`), `Frame` (a rail body's pose
    and motion at an instant: `of`, `local`, `velocity_at`).
  - `surface` — `Surface` trait (`height`, `surface`, `max_height`, `liquid`), and
    `surface_radius`, `surface_radius_at`, `max_radius` (moved from `Body`).
  - `body` — `RigidBody { position, velocity, orientation, angular_velocity, radius }`.
  - `collide` — `Collider::{None, Surface, Polytope, Ring}`; `Polytope` (symmetric faces,
    `cuboctahedron(scale)`, `CutOut` pockets) replaces `docking::contact`; `Ring` +
    `RingCrossing` replace `gate::crossing` (with the start-of-step ring-motion fix); facts
    `Fact::Contact(Contact)` / `Fact::Trigger`; `surface_contact` and `detect` replace
    `Universe::collision`/`gate_crossing`'s geometry.
  - `integrate` — the leapfrog + adaptive-substep loop from `Universe::flight_step`, as
    `integrate(bodies, ephemeris, positions, body, Span, driver)`; the `Driver` trait is the
    force callback (`applied` before each substep, `respond` to a fact → `Continue`/`Stop`/
    `Bounce`); `leapfrog` helper; `MAX_SUBSTEPS`, `FINE_RANGE`, `FINE_STEP`.
  - `ops` — `Weld` (capture/place), `Relative` + `relocate`, `bounce`.
  - `query` — `gravity`, `pull`, `dominant`, `segment_distance` (from `docking.rs`), `simulate`.
- `universe-sim` now uses the kernel throughout:
  - `system.rs`: `Body`'s motion fields (`parent, orbit, mu, radius, tilt, day`) moved into
    `Body.rail: RailBody`; `Body` implements `OnRails` (surface = its `Terrain`), keeps
    `rotation`/`angular_velocity`/`surface_radius*`/`max_radius` as thin delegates.
    `StarSystem::{positions, ephemeris, velocity, gravity, dominant}` delegate to the kernel.
    Colliders are chosen by kind (`BodyKind::collider`): station → `docking::hull()`, gate →
    `gate::RING`, everything else → surface; `attracts = kind.massive()`.
  - `terrain.rs`: `Terrain` implements `universe_physics::Surface`. The drawing
    classification enum `terrain::Surface` was renamed `Ground` (name clash with the trait;
    game updated).
  - `docking.rs`: `contact()`/`Contact` removed; `hull()` (cuboctahedron + slot cut-out,
    floor at the old docked depth), and the port/hull rules `docks(contact, roll_error)` and
    `bounces(contact)`. `StationFrame::new` builds on `Frame::of`.
  - `gate.rs`: `crossing()`/`Crossing` removed; `RING` constant; `GateFrame::frame()`.
  - `universe.rs`: `flight_step` = build a `RigidBody`, call `integrate` with a `Pilot` driver
    (engines + the dock/land/gate autopilot per substep, exactly as before, *outside* the
    kernel), then `react` to the stopping fact (`touch_down`, dock/crash, `enter_gate`, ring
    hit). Bumps are `Response::Bounce` (kernel `bounce` op). `landed_step` and `ship_at` use
    `Weld::place`; gate entry/arrival use `Relative` (the relocate op). `collision` and
    `gate_crossing` deleted.
  - `landing::predict` steps with the kernel `leapfrog` and `pull` (was its own two-body
    loop).
  - `ship.rs`: `Ship::rigid()` / `set_rigid()` convert to/from the kernel's `RigidBody`.
- Game crate: `b.radius`/`b.parent`/`b.orbit`/`b.tilt` → `b.rail.*`; `Surface` → `Ground`.

**Decisions**

- `OnRails` trait instead of storing the surface in `RailBody`: the world keeps its concrete
  `Terrain` (game draws craters, classifies ground), the kernel only sees `&dyn Surface`.
  Kernel functions are generic over `&[B: OnRails]`, so `StarSystem` needs no parallel array.
- The collider and an `attracts` flag are part of the rail body's description (stations and
  gates don't pull, exactly as `BodyKind::massive` did).
- Drivers return an *acceleration* (force / mass), not a force: mass stays with the ship for
  now, and it keeps the arithmetic bit-identical (`thrust / mass × throttle`).
- The integrator already takes fine substeps within 30 km of any small collider (polytope or
  ring) — the kernel-generic form of the old "near a station" rule. The navigation-state part
  (`clearance.is_some() || rcs != 0`) stays in the sim and is passed as `Span::max_h`; Phase 2
  should remove the clearance read.
- Everything was kept **bit-identical**. Verified with a scratch fingerprint (state hash after
  20 settlers × 1 game h, autodock, autoland, coasting at 1000×) on the original tree and the
  refactored one: identical hashes, times and states. The long runs below reproduce the
  baseline counts exactly.

**Deviations / notes for later phases**

- Hull bounce normal: the kernel picks the face of least penetration; the old code used an
  ad-hoc rule (octahedron face if |x|+|y|+|z| > 1.9, else the largest axis). They differ only
  near edges between faces; no test or run changed.
- Polytope margin: the touching sphere's radius is added to each face limit as written (so the
  octahedron faces get radius/√3 of true distance), preserving the old station test exactly.
  Worth making honest (normalised faces) when behaviour changes are allowed.
- The kernel carries orientation/angular velocity but does not integrate rotation yet; the
  ship's attitude model (`Ship::steer`) still sets it, from inside the driver callback. Phase 2
  (attitude device) should move rotation into the kernel step.
- Still-direct position/velocity writes in sim (Phase 2 devices): hyperdrive motion and drop-out
  velocity, station launch (shot out of the slot), lift-off nudge, respawn/spawn. The system
  hand-over (`position -= offset`) is a floating-origin change of coordinates, not physics.
- `ship_at` (settler spawn) now also gives the ship the ground's spin velocity (weld op); it
  was overwritten by the first landed step before, so nothing observable changes. A ring hit
  now also writes back the ship's velocity at the crash (it was left stale); a destroyed ship's
  velocity isn't used.
- **The planner's private integrator (`plan.rs`) remains** — Phase 3 replaces it with
  `universe_physics::simulate` + the same autopilot (noted in `plan.rs`'s module doc).
- `docs/architecture.md`: added "As built (Phase 1)" under the kernel section and marked the
  moved rows in the mapping table.

**Kernel tests** (18): orbit numeric derivative (moved), orbit closes/keeps energy, rails
parent chains + ephemeris accuracy, determinism hash (5 rail bodies, 3 driven rigid bodies,
600 frames at 20× with shared ephemerides, reproduced bit for bit), circular orbit stability
(10 orbits: radius drift 4.7e-5, energy drift 1e-13), exact integration of applied
acceleration, stop vs bounce on contact, simulate-a-copy = integrate, polytope hull/cut-out/
open pocket, polytope spin velocity, ring opening/tube/outside, fast ring sweeping over a body,
surface contact incl. liquid, `detect` ordering, weld riding the spin, relocation keeps
relative motion, gravity/dominant (non-attracting bodies ignored), segment distances.

**Gates**

- `cargo clippy --workspace --all-targets`: 0 warnings, 0 errors.
- `cargo test --workspace --release`: physics 18 passed; sim 28 passed, 10 ignored (29 before,
  minus `orbit_state_matches_numeric_derivative`, which moved to the kernel).
- `cargo test --workspace --release -- --ignored`: 10/10 passed, identical to the baseline:
  docking from spawn planned 149 s / actual 150 s; landing from orbit 384 s; gate from 30 km
  259 s; 10-stop settler route complete in 3.5 game h; all 13 moon ports autoland (264–285 s);
  100 settlers × 10 game h: 1,868 stops, 1,999 transits, 156 routes, **0 crashes**, 0.27
  ms/frame; 1,000 settlers: **0 crashes**, 2.26 ms/frame; landing ETA biggest jump 0.08 s.
  Bench (µs/frame): coasting 1.98, warp 1000 4.58, near station 1.98, docking autopilot 5.05,
  landing autopilot 3.38 (warp 100: 6.37), hyperdrive 1.75, near ground 2.18; body positions
  1.40 µs, gravity 0.06 µs, terrain 0.13 µs; planner 1.95 ms (landing) / 1.08 ms (docking) —
  all within noise of the baseline.
- `cargo build --release`: ok (game builds against the new APIs).
