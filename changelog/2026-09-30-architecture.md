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

### Phase 2 — World layer (`crates/world`, `universe-world`)

**What moved where**

- New crate `crates/world` (package `universe-world`; depends on `universe-physics`, `glam`,
  `serde`), in the workspace between physics and sim. `universe-sim` depends on it and
  re-exports it as `universe_sim::world`, plus the old paths `universe_sim::{galaxy, names, rng,
  ship, system, terrain, units}` and the common types at the crate root.
  - Content, moved verbatim: `units`, `rng`, `names`, `galaxy`, `terrain` (with its test),
    `system` (now with `on_pad`/`port_at`, the one pad-radius rule the landing gear, the route
    and the HUD share). `network`: `find_home`, `build` (the gate network) and `links_of`, from
    `Universe::{find_home, build_gate_network, gate_links_of}`.
  - `World { galaxy, time, home_system, gate_links }` + caches (systems, neighbouring stars per
    system, per-system ephemeris) — the clock and content that were fields of `Universe`.
  - Structures and their contact rules: `station` (`StationFrame`, `hull()`, `bounces`, `docks`,
    and the docking port's `contact` → `dock` or destroy, and `launch`, all from `docking.rs` /
    `universe.rs`), `spaceport` (`PAD_RADIUS`, `LAND_SPEED`, `pad_position`, the landing gear's
    `touch_down` and `lift_off`), `gate` (`GateFrame`, `RING`, `GATE_RADIUS`, `RING_TUBE`,
    `MAX_TRANSIT_SPEED`, `TRANSIT_TIME`, the gate device's `enter` and `emerge`).
  - `ship`: `Ship` (rigid-body state, device settings `throttle`/`rcs`/`hyperdrive`, `state`,
    fuel, cargo), `ShipState`, `Controls`, the constants, `facing`/`upright`; **`ShipCommands`**
    and `HyperdriveCommand`/`Destination`. Engine + thrusters (`thrust`) and attitude (`steer`,
    now crate-private) are the flight devices.
  - `hyperdrive`: the device (`switch`, `cruise`; `HYPER_RATE`, `PLANET_MARGIN`), from
    `Universe::{toggle_hyperdrive, hyperdrive_step}`' physical half.
  - `damage` (`destroy`, `RESPAWN_TIME`), `World::respawn`, `World::ship_at` (settler spawn).
  - `traffic`: traffic control — `Facility`, `request`, `lapsed`, `nearest_station`, ranges
    (`DOCK_RANGE`, `LAND_RANGE_RADII`, `TRANSIT_RANGE`, were `docking::CLEARANCE_RANGE`,
    `landing::CLEARANCE_RADII`, `gate::CLEARANCE_RANGE`), from `Universe::{request_clearance,
    check_clearance, clearance_range, target_position, target_name, nearest_station}`.
  - `events`: `ShipEvent` (physical) and `TrafficEvent` (service), `ClearanceKind`.
  - The ship step, `World::step_ship` (+ `World::command`), from `Universe::{step, landed_step,
    hyperdrive_step, flight_step, react, enter_gate, arrive_through_gate, dock, crash, touch_down,
    check_system_handover}`; the kernel `Driver` is now the world's `Devices`.
- `universe-sim` keeps the navigation (Phase 3 moves it) and the orchestration:
  - `avionics.rs` (was `ship.rs`): `Phase`, `Clearance`, `NavTarget` (= the world's `Facility`,
    re-exported under the old name; same serialized form), and **`Avionics { nav_target,
    clearance, hyper_autopilot }`** — the nav state that was on `Ship`. `Avionics::observe`
    applies the world's physical events (landed/docked/crashed end a clearance; a gate or a
    system hand-over also forgets the target; any hyperdrive switch hands steering back;
    respawn clears everything) — what `crash`/`dock`/`enter_gate`/… used to do to the ship.
  - `computer.rs` (new): the ship's `FlightComputer` — the dock/land/gate autopilot every
    substep (was the `Pilot` driver), and hyperdrive navigation every frame (`hyper_aim`, arrival
    drop-out, steering around bodies, exit velocity near the target; was in `hyperdrive_step` /
    `toggle_hyperdrive`).
  - `docking.rs`/`landing.rs`/`gate.rs`: guidance, status and autopilots only; the autopilots'
    `Command` converts to `ShipCommands`. `PadFrame` stays in `landing.rs` (it is the pad as
    guidance sees it: entry point, terrain clearance).
  - `universe.rs`: `Universe { world, ship, ship_system, avionics, route, events, crafts, … }`;
    `Craft` gains `avionics` and loses its `neighbours` cache (the world caches per system).
    Every change to the engine/thrusters from sim code (route departure and launch, hyperjump,
    autopilot off, clearance lost, …) goes through `Universe::command` / `set_controls`.
    `Event` = `Ship(ShipEvent)` | `Traffic(TrafficEvent)` | the avionics' own (hyperdrive
    arrived, route stop/complete/blocked, autopilot on/off, nav target set, `Refused`).
- Game: `u.{galaxy, time, gate_links, home_system}` → `u.world.*`; `ship.{clearance, nav_target,
  hyper_autopilot}` → `avionics.*`; events matched as `Event::Ship(ShipEvent::…)` /
  `Event::Traffic(TrafficEvent::…)`; world constants imported from `universe_sim::world::…`;
  the pilot's throttle/thruster keys go through `Universe::command` (a `ShipCommands`).

**Decisions**

- **The world step reads only commands and kernel facts.** `World::step_ship(ship, system,
  &ShipCommands, computer, real_dt, warp)` never sees a nav target, clearance or route (the
  types aren't even in the crate). Navigation code takes part through the `FlightComputer`
  trait, again only by returning `ShipCommands`: `substep()` per integration substep (the
  autopilots are feedback controllers that have always run at substep rate — moving them to
  frame rate would change every trajectory), `hyperdrive()` per frame once the clock has moved
  on (the hyperdrive's navigation needs the bodies where they will be), and `interval()`, the
  longest substep the computer can fly with. The world hands the computer the star system and
  rail positions (its sensors).
- **Fine substeps without clearance**: the substep limit is now the kernel's proximity rule
  (within 30 km of a polytope or ring), the thrusters firing (device state), or the flight
  computer's `interval()` (0.05 s while an autopilot flies). `clearance.is_some()` is no longer
  read.
- **Hyperdrive split**: the device (world) holds the speed law (room to the nearest surface or
  the commanded destination × throttle), the reference frame, the look-ahead drop-out (only
  when flying along the nose, i.e. not `steering`), the interlock and drop-out with the exit
  velocity; the navigation (sim) decides the destination, frame, exit velocity (the target's
  within 1,000 km), the heading around bodies, and when it has arrived.
- **Nav state off the ship now** (planned for Phase 3): a world `Ship` with navigation fields
  would have meant world types holding avionics state. The sim's `Avionics` is per ship and is
  swapped with the crafts like the rest of the per-ship state.
- `World` owns the clock and content now (planned for Phase 4), since the ship step needs
  them; `Universe` keeps thin delegates (`system`, `ship_system`, `gate_links_of`,
  `distance_ly`).
- Everything kept **bit-identical**: the Phase 1 fingerprint (20 settlers × 1 game h, autodock,
  autoland, 1000× coast) gives the same hashes, times and states before and after this phase,
  and the full sim test output (docking/landing/gate times, hyperdrive drop-out distances,
  plan lengths) is unchanged apart from the event wrapper in printed event lists.

**Deviations / notes for later phases**

- Behaviour change (no test or run affected): a *manually* flown ship holding a clearance,
  thrusters off, far from any station or gate, now takes coarse substeps. The frame is only
  0.033 s at the default 2× time scale, below the 0.05 s fine step, so this shows only with
  extra warp; the pilot's commands change per frame anyway.
- Events: the hyperdrive autopilot's "lock a nav target first" refusal is now `Event::Refused`
  (the avionics refusing), not a traffic-control denial; the game shows the same text and tone.
  Event order within one frame can differ where a physical event and an avionics event happen
  together (world events are recorded first).
- Save format: `UniverseSave` gains `avionics` (`#[serde(default)]`); `Ship` no longer
  serializes `clearance`/`nav_target`/`hyper_autopilot`. Older saves still load (unknown fields
  are ignored) but come back without nav target or clearance.
- Direct position/velocity writes now live only in world device rules: hyperdrive motion and
  drop-out, the docking port's launch, the landing gear's lift-off, docking/landing (pose set
  by the port/gear), respawn and `ship_at` (spawn). Gate transit uses the kernel's `Relative`
  op, landed/docked ships the `Weld` op. The kernel has no explicit "unweld"/"launch impulse"
  op yet; Phase 5's audit may want these expressed as kernel ops.
- The attitude device (`Ship::steer`) still sets orientation itself; rotation is not yet
  integrated by the kernel. `ShipCommands::turn: None` means no turn is commanded that span.
- World caches: neighbouring stars are cached per system (was per ship); when more than 64
  systems are cached, the gate network's systems are kept (was: the current ship's system).
  Both only affect speed, not results.
- The planner (`plan.rs`) still has its private integrator (Phase 3).
- Tests moved to the world crate (unchanged assertions, driven through `World::step_ship` by a
  test `Probe`): `home_system_has_station_and_planets`, `ship_keeps_orbit_under_warp`,
  `heavier_ship_accelerates_less`, `gate_network_links_five_systems_with_one_to_three_gates_each`,
  `fast_hull_contact_crashes_and_slow_bumps`, `launch_from_docked_leaves_along_axis`,
  `clipping_the_ring_or_going_too_fast_is_fatal`, `hyperdrive_reaches_neighbour_and_drops_out_safely`,
  `untargeted_hyperdrive_stops_well_short_of_a_planet`, and the terrain test. New: traffic
  control grants/refuses/lapses; a flight computer's substep commands fly exactly like the
  pilot's; the landing gear lands gently and breaks on a hard touchdown; the hyperdrive
  interlock never enters a body; the speed law and the commanded exit velocity; the avionics'
  reaction to physical events (sim); the save test also round-trips the nav target.
- `docs/architecture.md`: "As built (Phase 2)" under World, an orchestration note, mapping rows.

**Gates**

- `cargo clippy --workspace --all-targets`: 0 warnings, 0 errors.
- `cargo test --workspace --release`: physics 18 passed; world 15 passed (10 moved, 5 new);
  sim 19 passed, 10 ignored (28 before: 9 moved to the world crate, 1 new; the terrain test moved too).
- `cargo test --workspace --release -- --ignored`: 10/10 passed, identical to the baseline and
  Phase 1: docking from spawn planned 149 s / actual 150 s; landing from orbit 384 s; gate from
  30 km 259 s; 10-stop settler route complete in 3.5 game h; all 13 moon ports autoland
  (264–285 s); 100 settlers × 10 game h: 1,868 stops, 1,999 transits, 156 routes, **0 crashes**,
  0.26 ms/frame; 1,000 settlers: **0 crashes**, 2.22 ms/frame; landing ETA biggest jump 0.08 s.
  Bench (µs/frame, in that run): coasting 2.12, warp 1000 4.57, near station 2.06, docking
  autopilot 5.19, landing autopilot 3.43 (warp 100: 6.50), hyperdrive 1.93, near ground 2.25;
  body positions 1.38 µs, gravity 0.06 µs, terrain 0.13 µs; planner 1.98 ms (landing) / 1.12 ms
  (docking). Run alone, back to back with the pre-phase tree on the same machine, frame costs
  are 2–6% higher and the targeted hyperdrive ~8% (1.88 vs 1.69–1.76 µs): an extra call layer
  (commands, the computer callback, a per-system neighbour lookup). Two trims went in for it:
  the world hands the computer its star system (no second system lookup per step), and the
  hyperdrive navigation no longer clones the target's name every frame.
- `cargo build --release`: ok. (Dev scenarios compile against the new API; not rendered in this
  phase.)
