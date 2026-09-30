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

### Phase 3 — Avionics (`crates/avionics`, `universe-avionics`)

**What moved where**

- New crate `crates/avionics` (package `universe-avionics`; depends on `universe-physics`,
  `universe-world`, `glam`, `serde` only), in the workspace between world and sim. `universe-sim`
  depends on it and re-exports it as `universe_sim::avionics`, the old module paths
  `universe_sim::{docking, landing, gate, plan, route}`, and the common types at the crate root
  (`Avionics`, `Approach`, `Event`, `NavTarget`, `Phase`, `Clearance`, `Plan`, `Route`, `Stop`, the
  status types…). Modules:
  - `nav` (from sim `avionics.rs`): `NavTarget`, `Phase`, `Clearance`.
  - `avionics`: `Avionics { nav_target, clearance, hyper_autopilot, route, debug_way }` and its
    `observe` (from sim `avionics.rs`), plus the per-frame programs and pilot requests that were
    `Universe` methods: `prepare` (= `route_step` + `autopilot_hyperjump`), `conclude` (hyperdrive
    arrival event + `check_clearance`), `set_nav_target`, `request_clearance`, `toggle_autopilot`,
    `toggle_hyperdrive`, `toggle_route`, `command`/`record`; `approach()` and `plan()` for the HUD;
    `Approach` (from `universe.rs`).
  - `bus`: the new `Bus` trait — sensors (`ship()`, `system()`, `star_system()`, `time()`,
    `gate_links()`, `positions()`), all read-only, and `command(&ShipCommands) -> Vec<ShipEvent>`.
  - `computer` (from sim `computer.rs`): `Computer` (the `FlightComputer`) and
    `computer::autopilot` (the dock/land/gate dispatch, shared with the planner).
  - `hyperdrive` (from sim `computer.rs`): `aim`, `exit_velocity`, `navigate` (the hyperdrive
    autopilot), `HYPER_ARRIVE_STATION`/`HYPER_ARRIVE_PORT`.
  - `docking`, `landing`, `gate`: guidance, status types and autopilots (from sim, unchanged
    apart from `h`, below).
  - `route` (from sim `route.rs` + `Universe::{route_step, route_fly, leave, landed_at,
    ground_altitude, hyperjump_limit, stop_name}`): `Route`, `Stop`, `DWELL`, `gate_path`,
    `stop_name`, the route autopilot.
  - `plan`: the flight planner, rewritten (below).
  - `events`: `Event` (from `universe.rs`).
- `universe-sim` is now only orchestration: `lib.rs` + `universe.rs` (`Universe`, `Craft`,
  `TrafficStats`, `CrashReport`, `UniverseSave`, settlers, save/load, and a private `Link`: the
  avionics' `Bus` over `World` + the ship being stepped). `Universe.route` and `Universe.debug_way`
  moved into `Avionics`; `Craft.route` into `Craft.avionics.route`.
- Kernel: `simulate` takes an optional `Ephemeris` (like `integrate`); its test also checks the
  ephemeris path. World: `Devices` (the ship's devices as a kernel `Driver`) is public with
  `Devices::new`, so a copy of a ship can be flown by the same devices.
- Game: `u.route` → `u.avionics.route`, `craft.route` → `craft.avionics.route`; nothing else.

**The planner**

- The private integrator is deleted. `plan::plan` clones the ship and an `Avionics` with only
  the clearance (autopilot on), and flies the copy chunk by chunk with
  `universe_physics::simulate(&sys.bodies, ephemeris, &rigid, Span, &mut Devices::new(sys, &mut
  copy, &mut avionics.computer(), …))` — the kernel's integrator and contacts, the world's
  devices, the same `Computer` and autopilots as the real ship. A plan point (time, position and
  attitude in the target's frame, the autopilot's aim, action, phase) is recorded at every chunk
  start, spaced as before (≤ 0.5 s within 20 km of the goal, ≤ 60 s far out, short at first).
- Arrival: when the kernel stops on a fact, the world's own rules judge the copy — the docking
  port (`station::contact` → docked), the landing gear on the target's pad
  (`spaceport::touch_down` + `port_at`), the gate device (`gate::enter` on the target gate's
  trigger). Anything else (a hard hull hit, the ring, the ground elsewhere) ends the plan
  without arriving, as before. Gentle station bumps bounce, as in flight. (Was: within 15 m of
  a goal point, or the ring-crossing test.)
- Substeps: within the autopilot's own range (`route::hyperjump_limit`: 200 km of a port, 30 km
  of a station or gate — beyond it the real autopilot jumps by hyperdrive first) the copy
  reacts every 0.05 s exactly like the ship, so the plan *is* the flight. Farther out it ramps
  to 2 s substeps after the first few seconds; rail bodies come from an ephemeris snapshot per
  5 s window (each point interval is flown in pieces of at most 5 s), as in flight. The hyperjump itself is still not planned (as before).
- To keep long substeps stable, the autopilots take `h` (how long their command holds) and
  cap their feedback gains at `1/h` (`docking::gain`: attitude 1.5/s, docking and gate velocity
  1.2/s, landing 0.8/s). At the real autopilot's ≤ 0.05 s the cap never binds, so real flights
  are bit-identical (fingerprint below).

**Decisions / deviations**

- **Route into `Avionics`** (per the plan's "per-ship avionics state: … route"). It is
  `#[serde(skip)]` there and saved in `UniverseSave.route` as before, so the save format is
  unchanged. A respawn resets the avionics but keeps the route (as before, when the route was
  on `Universe`).
- **The `Bus`**: the route autopilot and the pilot's requests issue several commands in a row
  and read the ship between them (e.g. the hyperdrive switch zeroes the throttle, then the
  route sets full throttle; `observe` must see the switch event before the autopilot flag is
  set). So the avionics don't return one `ShipCommands` per frame; they send commands through
  the bus, which applies each at once via `World::command` and hands back the physical events,
  which the avionics observe and report in order. The avionics can only read the ship
  (`&Ship`), never write it.
- `Event` lives in avionics (the pilot's feed is what the ship's computer reports, including
  the world and traffic-control events it heard); sim re-exports it.
- `debug_way` (where the hyperdrive autopilot steers; debugging only) is per ship now, in
  `Avionics`; before, the last craft stepped overwrote the player's.
- Bench regression, explained: the planner now flies the real autopilot at its real rate
  through the kernel, so it costs about 1 µs per 0.05 s of flight near the goal. See gates.
- Tests: the avionics' observe test and `gate_paths` moved with their code; new
  `plan::tests::the_plan_is_the_flight` (in the avionics crate, driving `World::step_ship` with
  the avionics' `Computer` directly): docking from spawn planned 150.0 s / flown 150.0 s,
  landing from 150 km planned 305.2 s / flown 305.3 s. The sim's integration tests stay in sim
  (they test the orchestration); only paths changed (`u.route` → `u.avionics.route`,
  `computer::HYPER_ARRIVE_PORT` → `hyperdrive::HYPER_ARRIVE_PORT`, `u.debug_way` →
  `u.avionics.debug_way`). No assertion changed.
- `docs/architecture.md`: "As built (Phase 3)" under Avionics, the orchestration paragraph,
  kernel `simulate`, world `Devices`, mapping rows.

**Gates**

- `cargo clippy --workspace --all-targets`: 0 warnings, 0 errors.
- `cargo test --workspace --release`: physics 18 passed; world 15 passed; avionics 3 passed (2
  moved, 1 new); sim 17 passed, 10 ignored (19 before: the observe and `gate_paths` tests moved).
- `cargo test --workspace --release -- --ignored`: 10/10 passed. Docking from spawn: planned
  150 s, actual 150 s (was 149/150; every sample along the way also says 150 s); landing from
  orbit 384 s (after the hyperjump the plan says 384 s throughout); gate from 30 km: planned
  259 s, actual 259 s; 10-stop settler route complete in 3.5 game h; all 13 moon ports autoland
  (264–285 s); 100 settlers × 10 game h: 1,868 stops, 1,999 transits, 156 routes, **0 crashes**,
  0.27 ms/frame; 1,000 settlers: **0 crashes**, 2.27 ms/frame. ETA smoothness, biggest jump:
  docking 0.04 s (baseline 0.41), gate 0.05 s (0.23), landing 0.02 s (0.08). Bench (µs/frame):
  coasting 2.08, warp 1000 4.55, near station 2.06, docking autopilot 5.20, landing autopilot
  3.47 (warp 100: 6.50), hyperdrive 1.95, near ground 2.27 — unchanged. **Planner: 11.3 ms
  (landing from orbit, a 2.5 h plan; was 1.97) and 2.19 ms (docking; was 1.09)** — the
  explained change: it now flies the real autopilot at its real rate through the kernel
  (~1 µs per 0.05 s of flight within the autopilot's range). The game rebuilds the plan 10×/s,
  so a held landing clearance from orbit costs ~11% of a core; Phase 4 may rebuild less often
  (the countdown is now exact, so it would stay smooth) or off the frame.
- `cargo build --release`: ok.
- Bit-identity of real flights: the Phase 2 fingerprint (20 settlers × 1 game h, autodock,
  autoland, 1000× coast) gives the same hashes, times and states after the move and after the
  `h`-aware gains (`FP traffic 89dcbbf0e04ee5b0`, `dock 4e3bafb857af7993`, `land a10b2ac334b30908`).
  Only plans changed.

### Phase 4 — Orchestration and game (`crates/sim`, `crates/game`)

**What moved where**

- `universe-sim` is split by job (it was one `universe.rs` of 1,343 lines, three quarters tests):
  - `universe.rs`: `Universe { world, ship, ship_system, avionics, events, crafts, traffic,
    crash_log }` — the tick order (`step`, `step_world`), the pilot's requests (`command`,
    `respawn`, `toggle_hyperdrive`, `set_nav_target`, `request_clearance`, `toggle_autopilot`,
    `toggle_route`) and what the HUD reads (`approach`, `plan`, `target_position`, `stop_name`,
    …). Public API unchanged.
  - `vessel.rs` (new): **one ship's turn**, the same code for the player's ship and every craft.
    `Vessel { ship, system, avionics, events }` borrows a ship's per-ship state; `Vessel::tick`
    = avionics `prepare` → the pilot's stick as the frame's `ShipCommands` (unless a computer
    flies) → `World::step_ship` with the avionics' `Computer` (devices → kernel → facts →
    device/world rules) → `Avionics::record` (observe, then the pilot's feed) → avionics
    `conclude`. `Vessel::run` connects the avionics to the ship through `Link`, the `Bus` over the
    world (moved here from `universe.rs`).
  - `traffic.rs` (new): `Craft`, `TrafficStats`, `CrashReport`, `spawn_settlers`,
    `settler_route`, and a craft's turn (`fly_craft`: its tick, the crash report, the tally and a
    new route when one is done), with the settler tests.
  - `save.rs` (new): `UniverseSave`, `save`/`load`, with the save round-trip test.
  - Tests that fly whole flights through the orchestrator's public API moved to integration
    tests: `crates/sim/tests/flights.rs` (autodock, autoland, hyperdrive, plans, gate, route) and
    `crates/sim/tests/probe.rs` (bench, ETA accuracy/smoothness, debug printouts). No assertion
    changed; only imports (`universe_sim::…` instead of `super::*`/`crate::…`).
- **No more swapping.** `step_world` used to swap each craft's ship, system, avionics and event
  list into the player's fields, step "the player", and swap back. Now every ship is stepped in
  place by `Vessel::tick`; a craft's events are a local list, tallied and dropped (`Craft` lost
  its private `events` field). Same order (player, then crafts in order), same moment (the clock
  is reset to the frame's start for each craft, and set once to where the player's ship got).
- Avionics: `Route::pop` (take the last stop off, keeping `next` in range) — the nav map's
  route editing used to do that bookkeeping itself.
- Game:
  - The nav map's Backspace uses `Route::pop`.
  - **Flight plan cadence.** Phase 3 made the planner fly the real autopilot (a landing plan
    from orbit costs ~11 ms). The game rebuilt it 10×/s regardless, so holding a landing
    clearance from orbit spent ~11% of a core, in 11 ms bursts inside a frame. Now it measures each
    build and waits `20 × cost` (0.1–1 s) before the next: still 10×/s for docking and gate
    plans (~2 ms) and near the pad, about 4×/s for a landing from orbit (keeping the planner
    to ~5% of the time). It still rebuilds at once when the clearance, target or autopilot
    phase changes, and the ETA countdown eases between plans as before. (The wall-clock
    measurement is in the game, not the simulation.)
  - Nothing else needed changing: the game already used only the public API. The fields it
    reads (`u.ship`, `u.avionics`, `u.events`, `u.crafts`, `u.world`, …) stay put.
- `docs/architecture.md` rewritten to describe the implemented state: per layer what exists and
  what is **Not yet** (rotation integration, kernel ops for the remaining direct pose writes,
  modules, planning the hyperjump, multiplayer, saving settlers…), a new Game section, and the
  "Where today's code maps" table replaced by **"Where things live"** (concept → crate/module).
  The per-phase "As built" sections are folded into the layer sections.

**Decisions / deviations**

- **Player fields stay flat on `Universe`** (`ship`, `ship_system`, `avionics`, `events`) rather
  than becoming a `player: Craft`-like struct. The game and the tests use them in ~250 places;
  the borrowed `Vessel` gives the player's ship and each craft the very same turn without that
  churn.
- The clock rule ("each craft steps from the frame's start; the clock ends where the player's
  ship got") is kept as it was, and is now documented, including its one oddity: a craft whose
  warp is limited more than the player's simulates less than the clock moves (never at 2×).
- The flight tests became integration tests because they only use the public API; being
  outside the crate keeps them honest about it.
- Save format unchanged (`UniverseSave` is the same struct, moved).

**Dev scenarios** (release build, `UNIVERSE_SCENARIO=<name> UNIVERSE_SCREENSHOT=…`), before and
after this phase: cleared, autodock, landing, autoland, gate, transit, route, traffic all run,
exit cleanly and render the same pictures (HUD text, phases, guidance, traffic counts; the only
differences are live-frame jitter such as a 1 s ETA). Each scenario's logged set-up state
(pending events, clearance) is identical before and after. ~6 ms/frame over the 120-frame
capture, unchanged.

**Gates**

- `cargo clippy --workspace --all-targets`: 0 warnings, 0 errors.
- `cargo test --workspace --release`: all pass — physics 18, world 15, avionics 3; sim 3 unit
  tests (1 ignored), `tests/flights.rs` 13 (2 ignored), `tests/probe.rs` 1 (7 ignored). Sim total
  17 passed + 10 ignored, as before.
- `cargo test --workspace --release -- --ignored`: 10/10 passed, the same numbers as Phase 3:
  docking from spawn planned 150 s, actual 150 s; landing from orbit 384 s; gate from 30 km
  planned 259 s, actual 259 s; 10-stop settler route complete in 3.5 game h; all 13 moon ports
  autoland (264–285 s); 100 settlers × 10 game h: 1,868 stops, 1,999 transits, 156 routes,
  **0 crashes**, 0.27 ms/frame; 1,000 settlers: **0 crashes**, 2.27 ms/frame; landing ETA
  biggest jump 0.02 s. Bench (run alone, µs/frame): coasting 2.09, warp 1000 4.51, near station
  2.04, docking autopilot 5.20, landing autopilot 3.45 (warp 100: 6.42), hyperdrive 1.93, near
  ground 2.22; body positions 1.35 µs; planner 11.2 ms (landing from orbit) / 2.2 ms (docking)
  — unchanged from Phase 3.
- `cargo build --release`: ok.
- Bit-identity: the Phase 2/3 fingerprint (20 settlers × 1 game h, autodock, autoland, 1000×
  coast) gives the same hashes, times and states before and after (`FP traffic
  89dcbbf0e04ee5b0`, `dock 4e3bafb857af7993`, `land a10b2ac334b30908`).

## Verification (after Phase 4)

Four read-only verifiers ran in parallel. The user then stopped the workflow before the automated
fix pass ("it takes lots of time and burns huge amount of tokens"). The findings are open, to be
picked by hand.

- **Regression**: 53 tests + 10 long runs all pass. Traffic is identical to the baseline
  (100 settlers: 1868 stops / 1999 transits / 156 routes / 0 crashes; 1000 settlers: 0 crashes).
  Docking is 150/150 s and landing 384 s, and frame costs are within ±4%. The one regression is
  the planner: 11.2 ms from orbit (was 1.97) and 2.2 ms for docking (was 1.09), because it now
  flies the real autopilot through the kernel.
- **Architecture audit**: the dependency graph matches this document, and the kernel has no game
  words. World code never reads avionics state.
- **Game smoke test**: all 32 dev scenarios run cleanly. Their setup event sequences match the
  pre-refactor build.
- **Code review**: tick order, substeps and frame math are equivalent to the old code.

Open findings:
1. (medium) Planner cost, plus an 11 ms spike on the game's frame thread. Options: larger
   substeps when far from the target, reusing the previous plan, or building on a worker thread.
2. (medium) Fairness: substep length depends on `computer.interval()`, so an autopilot gets finer
   substeps than a human in the same state. Either record it as an explicit exception or derive
   it from device state.
3. (low) A manual landing with clearance at warp above 3× now gets coarse substeps (the old rule
   used clearance). A possible fix: the avionics' `interval()` goes fine while clearance is held.
4. (low) Legacy saves made mid-route in hyperdrive lose `hyper_autopilot`/`nav_target`, and the
   ship cruises with no destination. Fix: migrate the old ship fields on load.
5. (low) `Driver::applied` gets `&mut RigidBody`. Narrow it so a driver can't write position or
   velocity.
6. (low) Doc precision: the audited-ops list (spawn/unweld aren't implemented; bounce is missing),
   the `step_ship` signature, and the "within 30 km" rule (it is evaluated once per call).
7. (low) Rename `HyperdriveCommand.steering` to a device term (e.g. `lookahead_dropout`).
