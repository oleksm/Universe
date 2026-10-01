# Universe — Architecture

Status: adopted 2026-09-30 and implemented by the refactor's Phases 1–4 (see
`changelog/2026-09-30-architecture.md`). This document describes the code as it is, and marks
what is still to come (**Not yet**). It is the reference; keep it in sync when the code's shape
changes.

**Next:** the core / services / clients re-architecture is planned in `docs/rearchitecture.md`
(proposed, under review).

## Principles

1. **The physics engine is ruthlessly stable and fair.** It is the same for every object in
   the world — the player's ship, a settler, a half-built station — and it never changes its
   rules because of what is built or run on top of it.
2. **Physics knows nothing about game concepts.** No "station", "spaceport", "gate", "dock",
   "land", "autopilot", "clearance", "route" or "player" inside the physics kernel. Those are
   objects and programs *in* the world, and later things players construct.
3. **Everything moves through devices.** Nothing but the kernel's integration and its few
   explicit, audited operations (spawn, weld/unweld, relocate) changes a body's position or
   velocity. Intent (a pilot's stick, an autopilot) becomes commands to a ship's devices
   (engine, thrusters, lift, hyperdrive…), and devices turn commands into forces within their
   physical limits.
4. **Software is software.** Navigation, guidance, autopilots and the flight planner are
   programs running on a ship: they read sensors and write device commands, nothing else. They
   can't cheat, so an autopilot is exactly as limited as a human pilot — and each can later be
   an installable (purchasable) module.
5. **Physics first, honest.** Real Newtonian mechanics and Kepler orbits; where something is
   simplified or fictional (hyperdrive, gates), it is an explicit device with explicit rules.
6. **Deterministic.** Same inputs → the same world, bit for bit: fixed update order, seeded
   randomness, no wall clock inside the simulation. This underpins multiplayer (server
   authority, client prediction), replays and reproducible traffic simulations.

## Layers (crates), dependencies pointing down only

```
 game (universe)        rendering, HUD, input, audio, dev scenarios        crates/game
   │
 sim (universe-sim)     orchestration: the tick loop, ships (player +      crates/sim
   │                    crafts), per-ship avionics, traffic (settlers),
   │                    save/load. The only place layers meet.
   ├──────────────┐
 avionics            │  ship software: nav computer, guidance, planner,   crates/avionics
 (universe-avionics) │  autopilots (dock/land/gate/hyper/route)
   │                 │  reads sensors → writes ShipCommands
   ├─────────────────┘
 world (universe-world) entities & devices: galaxy, star systems, terrain  crates/world
   │                    content, structures (station, spaceport, gate),
   │                    ships and their devices, contact rules (docking
   │                    port, landing gear, damage, gate transit), world
   │                    services (traffic control / clearance), events
   │
 physics                the kernel (below)                                 crates/physics
 (universe-physics)
                        engine (universe-engine): rendering only, no sim deps
```

## Threads: the world engine and the client

The world engine (`sim` and everything below it) and the client (`game` + `engine`) meet at one
boundary, `universe_sim::engine`:

- **Commands in.** Everything the client does to the world is a `Command` (ship settings,
  throttle changes, clearance, autopilot, follow, lock, route edits, trades…), applied in
  order before the next tick. Requests with an answer (a lock, a trade) answer with events.
- **Views out.** After each tick the engine publishes a `View`:
  - the player's ship, avionics and crew;
  - the other ships as drawn (`CraftView`);
  - weapons fire, kills and trades;
  - the events since the last view;
  - what the player's ship computers make of things (radar contacts, fire control, approach
    guidance, the flight plan, the collision warning, follow status, the nav marker);
  - the turrets and pad owners of the player's system.

  A view the client didn't take passes its events and hits on to the next.
- **Charts shared.** What's fixed about the galaxy (stars, gates, goods, the star systems as
  generated) is `world::charts::Charts`, shared read-only by both sides.

Threads:

1. **World engine** (`EngineHandle::start`): ticks at `TICK_HZ` (60), applying commands,
   then publishing the view. Within a tick, after the player:
   - **crafts step side by side** on all cores (`rayon`), each against the world frozen at
     the tick's start (`World::freeze`: systems, body positions at both ends of the tick, the
     integrator's snapshot, turrets, read without locks) and the others as the frame's
     snapshot has them (`Universe::snaps`);
   - their traffic control requests (pads, corridors) are answered from the last tick's state
     and made after, in craft order, so the outcome doesn't depend on thread timing;
   - what came of each turn (events, hunts, crashes, trades) is applied in craft order;
   - then combat, collisions and traffic presence (presence worked out in parallel).

   Dev scenarios set the universe up before the thread starts. `UNIVERSE_ENGINE_THREAD=0` runs
   it in the client's thread, for debugging.
2. **Client main thread**: window and input, turning input into commands, taking the newest
   view and building the frame. Between ticks it carries motion forward (`App::ahead`,
   `turned`, `now`) so a fast display doesn't judder on 60 Hz ticks.
3. **Render thread** (`engine::render_thread`): owns the GPU. It takes finished frames (one
   may wait), uploads, submits and presents, and reports sizes and timings back. Meshes live on
   the GPU (`Mesh`: uploaded once, instanced by mesh, transformed and lit in the shader).

Crate boundaries are enforced by Cargo: `universe-physics` depends on `glam` only and cannot
import world or avionics types; `universe-world` depends on the kernel (+ `glam`, `serde`) and
cannot import avionics; `universe-avionics` depends on the kernel and the world; `universe-sim`
on all three; the game on `universe-sim` and `universe-engine`. `universe-sim` re-exports the
layers below it (`universe_sim::{physics, world, avionics}`, the old module paths
`universe_sim::{galaxy, names, rng, ship, system, terrain, units, docking, landing, gate, plan,
route}` and the common types at its root), so the game needs only the one dependency.

## Physics kernel (`universe-physics`)

`crates/physics/src/`: `orbit`, `rails`, `surface`, `body`, `collide`, `integrate`, `ops`,
`query` (plus a test-only `testkit`). It knows only:

- **Rail bodies**: celestial bodies on exact Kepler orbits around a parent (planets, moons,
  stations, gates — anything placed "on rails"). `RailBody { parent, orbit, mu, attracts, radius,
  tilt, day, collider }`: gravitational parameter, spin (tilt + day), its **collider**, and an
  `attracts` flag (bodies too light to matter neither pull nor dominate). The **surface** is not
  stored in it: owners implement `OnRails` (`rail()` + optional `surface() -> &dyn Surface`), so
  the world keeps its own terrain type and hands the kernel the height function. Positions,
  velocities, rotations at any time (`positions`, `velocity`, `Frame::of` / `local` /
  `velocity_at`); per-frame **ephemeris** snapshots (p + v·τ + ½·a·τ²) for cheap substeps;
  gravity sum; dominant body.
- **Surface** trait: `height(dir)` (ground under any liquid), `surface(dir)` (what is touched),
  `max_height()`, `liquid(dir)`; `surface_radius`, `surface_radius_at`, `max_radius`.
- **Rigid bodies**: `RigidBody { position, velocity, orientation, angular_velocity, radius }`,
  moved only by integration of gravity + applied acceleration, or by kernel ops. Mass stays
  with the owner: drivers return an **acceleration** (force / mass).
- **Integration**: `integrate(bodies, ephemeris, positions, body, Span { t, dt, max_h },
  driver)` — leapfrog + adaptive substeps (1/100 of the dominant body's orbital time scale,
  0.01–3600 s, ≤ 2000 per call), substeps ≤ 0.05 s (`FINE_STEP`) within 30 km of any small
  collider (polytope/ring). The `Driver` trait is the force callback: `applied(body, t, h,
  positions) -> acceleration` before every substep, `respond(fact) -> Continue | Stop |
  Bounce{..}` after a substep that produced a fact.
- **Colliders** `Collider::{None, Surface, Polytope, Ring}`: surface (sphere + height function,
  liquids as a flat surface); convex polytope `w·|x| ≤ limit` (+ `cuboctahedron(scale)`) with
  `CutOut` pockets open along local +Y, hull normals by least penetration; ring (torus) with an
  opening **trigger**, measuring the start of a step against where the ring was then. The
  kernel reports **facts**, never what they mean: `Fact::Contact(Contact { body, feature:
  Surface{liquid} | Hull | CutOut(k) | Ring, normal, local, surface_velocity, relative_velocity
  })`, `Fact::Trigger { body, relative_velocity }`.
- **Ops** (explicit, audited): `Weld` (capture a body's pose in a rail body's frame, place it
  there each step), `Relative` + `relocate` (between two reference frames, preserving relative
  motion), `bounce` (the contact response a driver asks for).
- **Queries**: `gravity`, `pull`, `dominant`, `segment_distance`, and `simulate` — run a *copy*
  of a body forward under a driver (optionally from an ephemeris). The planner uses it, so
  there is exactly one physics implementation.

Fairness rules: no special cases by object kind or owner; the same integrator, tick and
contact rules for everything (substep length comes from physical state — proximity and whether a
device pushes — never from who or what is flying); all inputs are accelerations or audited ops. The kernel contains
none of the game words (checked by grep).

Invariant tests live with the kernel: determinism (`same_inputs_same_world`: a state hash after
N steps), orbit stability and energy (`orbit_closes_and_keeps_its_energy`,
`a_circular_orbit_stays_circular`), the orbit's numeric derivative, exact integration of an
applied acceleration, colliders and triggers (polytope hull / cut-out / open pocket / spin,
ring opening / tube / outside / a fast sweep, surface liquids, detection), ops (weld rides the
spin, relocation keeps relative motion), stop/bounce, and `simulate` matching the real thing.

**Not yet**

- Rotation: orientation and angular velocity are carried but not integrated by the kernel; the
  ship's attitude device sets the orientation (see World). Integrate rotation (torques) once
  attitude is a force-producing device.
- Ops: there is no explicit unweld, launch-impulse, spawn or despawn op; the world's device
  rules write those poses directly (see World, *Not yet*).
- Mass and forces: drivers give accelerations; a kernel-side mass (and momentum bookkeeping
  across bodies) comes with constructed, non-rail bodies.
- Polytope faces are not normalised (the station's octahedron faces get radius/√3 of true
  distance, copied from the old test) — a behaviour change for when that is allowed.

## World (`universe-world`)

`crates/world/src/`, the entities and their rules:

- **Content**: `units`, `rng`, `names`, `galaxy`, `terrain` (implements the kernel's `Surface`),
  `system` (star systems: bodies with a `rail`, kinds, colours, spaceports; `on_pad`,
  `port_at`), `network` (home system choice, the gate network, links). `World { galaxy, time,
  home_system, gate_links }` owns the clock and caches: generated systems (the gate network's
  kept when evicting), each system's neighbouring stars, and the per-system ephemeris shared by
  every ship stepping from the same moment.
- **Structures** with their contact rules:
  - `station`: a rail body (circular orbit) + polytope collider with a slot cut-out
    (`StationFrame`, `hull()`); the **docking port** (`docks`/`bounces`, `contact` → dock (weld)
    or destroy, `launch`).
  - `spaceport`: a pad on a body's surface (`PAD_RADIUS`, `pad_position`); the **landing gear**
    (`touch_down`: slow enough → weld, oceans and fast impacts → destroyed; `lift_off`).
  - `gate`: a rail body + ring collider + opening trigger + link to its paired gate
    (`GateFrame`, `RING`); the **gate device** (`enter`: trigger crossed below the speed limit →
    transit, else destroyed; `emerge` via the kernel's `Relative` op; ring contact → destroyed).
- **Ships** (`ship`): `Ship` = rigid-body state + device settings (`throttle`, `rcs`,
  `hyperdrive`), `state` (flying, landed/docked, in transit, destroyed), fuel, cargo — **no
  navigation state**. Commands: `ShipCommands { throttle, rcs, turn: Option<Controls>,
  hyperdrive: Option<HyperdriveCommand> }`, `HyperdriveCommand { engage, heading, steering,
  frame_velocity, exit_velocity, destination }`; `Ship::holding()` gives commands that change
  nothing, as a base for new ones. Devices: engine + thrusters (`Ship::thrust`), attitude (turn
  rates; `steer`, crate-private), and:
- **Hyperdrive** (`hyperdrive`, a fictional device with explicit rules): speed = `HYPER_RATE` ×
  room (nearest surface, or the commanded destination if nearer: the stop distance) × throttle,
  relative to the commanded frame (else the dominant body's); along the commanded heading or the
  nose; drops out by itself short of an obstacle dead ahead unless the heading is being steered;
  interlock: never into a body; dropping out leaves the commanded exit velocity (else co-moving
  with the dominant body).
- **Damage** (`damage`): destroy, `RESPAWN_TIME`; `World::respawn`, `World::ship_at` (spawn
  docked/landed).
- **Services** (`traffic`): traffic control grants or refuses clearance — `Facility`
  (station/spaceport/gate), `request` (not in flight, in hyperdrive, no target, not in this
  system, out of range → refused with a reason), `lapsed` (beyond twice the range, or gone),
  `nearest_station`.
- **Events** (`events`): `ShipEvent` (physical: landed/docked, landed at a port, took off,
  launched, bumped, crashed, respawned, entered system, hyperdrive on/off, gate entered /
  arrived / too fast) separate from `TrafficEvent` (clearance granted / denied / cancelled).
- **The ship step**: `World::command(ship, system, &ShipCommands)` (no time passes: settings,
  then the hyperdrive switch) and `World::step_ship(ship, system, &ShipCommands, computer,
  real_dt, warp)` — destroyed (respawn countdown), landed (weld; turn; launch / lift-off),
  transit (countdown; emerge), hyperdrive, or free flight through the kernel with `Devices` as
  the driver, then the reaction to the fact the kernel stopped on, then the system hand-over (a
  floating origin at interstellar scale). It reads only the ship, the commands and kernel facts.
  **Nothing runs inside the step but physics:** whatever flies the ship (a pilot, a program)
  has had its say before it, as device settings that hold through it (hyperdrive orders too,
  `Ship::hyper_orders`). Turning is physics, over the game time the step covers (in hyperdrive,
  over real time). Fine substeps come from proximity to structures (kernel) or any device
  pushing — engine or thrusters (device state), whoever flies — never from who is flying.
  `step_ship_at` steps from an explicit clock, so ships can step side by side.
- `Devices` (public) is the kernel `Driver` for a ship's devices: they push with the thrust
  they're set to, station bounces are judged by the world. The same driver flies a *copy* of a
  ship when a flight is simulated ahead (the planner).

**Not yet**

- Direct pose writes remain in world device rules, outside kernel ops: the hyperdrive's motion
  and drop-out velocity, the docking port's launch, the landing gear's lift-off nudge, the dock
  and touch-down pose, respawn and `ship_at`, and the hand-over (a change of coordinates, not
  physics). Candidates for explicit kernel ops (unweld, launch impulse, spawn).
- `Ship`'s fields are `pub`, so "only through commands" is kept by review, not by the compiler.
- Fuel is carried but not consumed; cargo has no mass effect yet; parts/modules are not
  modelled (the devices are fixed per ship).

## Avionics (`universe-avionics`)

`crates/avionics/src/`, per-ship software. It reads sensors and its own state and writes
`ShipCommands`, nothing else:

- `nav`: `NavTarget` (the world's `Facility`), `Phase`, `Clearance`.
- `avionics`: **`Avionics { nav_target, clearance, hyper_autopilot, route, debug_way }`** — all
  per-ship navigation state. It learns what happened from the world's `ShipEvent`s (`observe`:
  arriving, crashing or leaving through a gate ends a clearance; leaving the system forgets the
  target; a respawn starts afresh but keeps the route). Its programs run once a frame through a
  `Bus`: `prepare` (the route autopilot, then the dock/land/gate autopilot's hyperjump),
  `conclude` (hyperdrive arrival, clearance lapse), and the pilot's requests (`set_nav_target`,
  `request_clearance` via the traffic-control service, `toggle_autopilot`, `toggle_hyperdrive`,
  `toggle_route`). `approach()` / `plan()` give the HUD its guidance and flight plan; `Approach`
  (Dock/Land/Transit status) lives here.
- `bus`: the `Bus` trait — the avionics' only link to their ship: read-only sensors (`ship()`,
  `system()`, `star_system()`, `time()`, `gate_links()`, `positions()`) and
  `command(&ShipCommands)`, which returns the physical events that followed. The orchestrator
  implements it over the world.
- `computer`: the dock/land/gate autopilot (`computer::autopilot`) — what it commands for the
  next tick from where the ship is. `Avionics::fly` runs it once a tick, before the world steps
  the ship (and, in hyperdrive, the navigation: `hyperdrive::navigate`).
- `hyperdrive`: the hyperdrive autopilot (`aim`, `exit_velocity`, `navigate`: steer to the target
  and around bodies, drop out on arrival, choose the frame/destination/exit velocity).
- `docking`, `landing`, `gate`: guidance (docking corridor, landing profile and `PadFrame`, gate
  run), HUD status (`DockingStatus`, `LandingStatus`, `GateStatus`, `Guidance`) and the
  autopilots. The autopilots take `h`, how long their command holds: feedback gains are capped
  at `1/h` (`docking::gain`) so a long substep can't overshoot; at the real 0.05 s this changes
  nothing.
- `route`: `Route` (with `pop`, keeping its progress valid), `Stop`, `DWELL`, `gate_path`,
  `stop_name`, and the route autopilot (leave, cross systems through gates, hyperdrive, dock or
  land, dwell, repeat).
- `plan`: the flight **planner**. It clones the ship and flies the copy with the kernel's
  `simulate` and the world's `Devices`, under the same `Computer`/autopilot, point by point; it
  arrives when the world's rules (docking port, landing gear on the target pad, gate device)
  would take the copy in. Within the autopilot's own range (the hyperjump limit: 200 km from a
  port, 30 km from a station or gate) the copy reacts every 0.05 s exactly like the ship, so the
  plan *is* the flight; farther out it uses substeps up to 2 s.
- `events`: `Event`, the pilot's feed — `Ship(ShipEvent)`, `Traffic(TrafficEvent)` and the
  avionics' own (hyperdrive arrived, route stop/complete/blocked, autopilot on/off, nav target
  set, refused).

**Not yet**

- Modules: every ship has every program; "installed or not" (and buying them) is future work.
- The hyperjump itself is not planned: a plan starts where the hyperdrive drops out (no plan,
  and no ETA, while the autopilot is in hyperdrive).
- Sensors are perfect (the bus reads the true world); sensor ranges and noise come later.

## Orchestration (`universe-sim`)

`crates/sim/src/`: the only place the layers meet.

- `universe`: **`Universe { world, ship, ship_system, avionics, events, crafts, traffic,
  crash_log }`** — the world, the player's ship with its avionics and event feed, the crafts.
  `step` gives the player's ship its turn; `step_world` gives every ship its turn in a fixed
  order — the player's first, then each craft in order, all from the same moment — and moves
  the clock once (as far as the player's ship went). The pilot's requests (`command`,
  `toggle_hyperdrive`, `set_nav_target`, `request_clearance`, `toggle_autopilot`,
  `toggle_route`, `respawn`) go to the player's avionics (or straight to the devices, for
  `command`) and take effect at once; `approach`, `plan`, `target_position`, `stop_name`… are
  what the HUD shows.
- `vessel`: **one ship's turn**, the same code for the player's ship and every craft (a
  `Vessel` borrows a ship, its system, its avionics and its event feed):
  1. avionics `prepare` (route autopilot, hyperjump) → commands over the bus;
  2. the pilot's stick → the frame's `ShipCommands` (unless a computer is flying the ship);
  3. `World::step_ship` with the avionics' `Computer`: devices → kernel step → contact/trigger
     facts → device/world rules;
  4. the physical events → `Avionics::record` (observe, then the pilot's feed);
  5. avionics `conclude` (hyperdrive arrival, clearance lapse).

  A private `Link` is the avionics' `Bus` over the world and the ship being stepped.
- `traffic`: `Craft { name, ship, system, avionics, route_seed }` — the settlers, each on its
  own reproducible route (`spawn_settlers`, `settler_route`; a new route when one is done),
  `TrafficStats` (stops, transits, crashes, routes completed) and the `CrashReport` log (last 50).
- `save`: `UniverseSave { seed, time, ship, ship_system, route, avionics }` — `save`/`load`
  (star systems regenerate from the seed; saves from before the refactor still load, without a
  nav target or clearance; settlers aren't saved).

The world time scale (default 1×) is game seconds per real second, chosen by the game; single
player warp multiplies it (not in hyperdrive).

Tests: unit tests next to the code (settlers, save/load); whole flights through the
orchestrator in `crates/sim/tests/flights.rs` (autodock, autoland from orbit / the far side /
high above, hyperdrive to a port and a moving gate, plans reaching the pad and slot and staying
put, gate transit keeping motion, route autopilot), and benches, ETA accuracy and debugging
printouts in `crates/sim/tests/probe.rs`. The long runs are `#[ignore]`d.

**Not yet**

- Multiplayer: one fixed world time scale, server authority and client prediction are what the
  determinism is for; there is no networking.
- The settlers aren't saved with the game.
- Every ship steps from the same moment, but a ship whose warp is limited more than the
  player's (fine substeps near a structure) simulates less than the clock advances that frame;
  at the default 1× this never binds.

## Game (`universe`, `crates/game`)

Rendering, HUD, input, audio and dev scenarios over `universe-sim` and `universe-engine`. The
pilot's keys become `Universe` requests (throttle and thrusters through `Universe::command`,
the stick as the frame's `Controls`); the HUD reads the ship, `u.avionics`, `approach()` and
`plan()`. The flight plan is rebuilt 10×/s, less often when building it is dear (keeping it to
~5% of the time, at most 1 s apart), and at once when the clearance, target or autopilot phase
changes; the ETA countdown eases between plans. Dev scenarios (`crates/game/src/dev.rs`,
`UNIVERSE_SCENARIO=<name> UNIVERSE_SCREENSHOT=<png>`) set a situation up headless — writing the
ship's pose directly, like tests do — then render.

## Where things live

| Concept | Crate: module |
|---|---|
| Kepler orbits, rail bodies, positions/velocities/rotations, ephemeris, frames | physics: `orbit`, `rails` |
| Surface (height function) trait, surface radius | physics: `surface` |
| Rigid body; leapfrog + adaptive substeps, `Driver`, `Span` | physics: `body`, `integrate` |
| Colliders (surface, polytope + cut-outs, ring + trigger), facts | physics: `collide` |
| Weld, relocate, bounce | physics: `ops` |
| Gravity, dominant body, segment distance, simulate a copy | physics: `query` |
| Galaxy, star names, seeded rng, units | world: `galaxy`, `names`, `rng`, `units` |
| Star systems, bodies, spaceports; terrain (implements `Surface`) | world: `system`, `terrain` |
| Home system, gate network and links | world: `network` |
| The clock, system caches, `command`/`step_ship`(`_at`), `Devices`, respawn/spawn, hand-over | world: `world` |
| Ship, `ShipCommands`, `HyperdriveCommand`, engine/thrusters/attitude | world: `ship` |
| Station structure + docking port (dock, bump, launch) | world: `station` |
| Spaceport pad + landing gear (touch down, lift off) | world: `spaceport` |
| Gate structure + gate device (enter, emerge) | world: `gate` |
| Hyperdrive device (speed law, interlock, drop-out) | world: `hyperdrive` |
| Damage, respawn time | world: `damage` |
| Traffic control (clearance rules) | world: `traffic` |
| Physical and service events | world: `events` |
| Nav target, clearance, phases | avionics: `nav` |
| Per-ship avionics state, observe, per-frame programs, pilot requests, HUD approach/plan | avionics: `avionics` |
| Sensors + command bus | avionics: `bus` |
| The autopilots' per-tick say (dock/land/gate autopilot, hyperdrive navigation): `Avionics::fly` | avionics: `computer`, `avionics` |
| Ticks of at most 1/60 game s (warp: more ticks, up to a budget); crafts' command delay (tests: 2 ticks) | sim: `universe::step_world`, `vessel::Inbox` |
| Hyperdrive autopilot | avionics: `hyperdrive` |
| Docking / landing / gate guidance, status, autopilots | avionics: `docking`, `landing`, `gate` |
| Routes and the route autopilot | avionics: `route` |
| Flight planner | avionics: `plan` |
| Pilot's event feed | avionics: `events` |
| `Universe`: player ship + avionics, the tick order (every ship's turn, then the combat phase), pilot requests | sim: `universe` |
| One ship's turn; the avionics' bus over the world | sim: `vessel` |
| Settlers (crafts), traffic stats, crash log | sim: `traffic` |
| Save/load | sim: `save` |
| Whole-flight tests; benches and ETA accuracy | sim: `tests/flights.rs`, `tests/probe.rs` |
| Projectiles (slugs) swept against moving spheres; rays | physics: `projectile` |
| Crew on foot: seat / aboard (ship frame, magnetic boots) / outside (body frame, real gravity), interior layout, hatch rules | world: `crew` |
| Goods catalog (1,000 items from the seed) | world: `goods` |
| Markets per station/spaceport (produces, wants, bans; prices by stock/demand, recovery), the hold | world: `market`, `World::{market, quotes, trade}` |
| Traffic control: pads (booking, queue, occupancy) and corridors (one at a time, queued), explicit claim/release (`TrafficControl`, `Presence`) | world: `pads` |
| Atmospheres (exponential air on Terran worlds, turning with them), exact quadratic drag in the integrator, Sutton–Graves heating | physics: `atmosphere`, `integrate` |
| Hull skin temperature: re-entry heating vs radiation, burning past the limit | world: `heat` |
| Planetshine: the nearest planet's day side lights the shade (`Reflector`, view factor) | engine: `frame` |
| Ship-to-ship collisions (kernel `pairs` sweep + world bounce/damage rules) | physics: `pairs`, world: `collisions` |
| Flight recorder: every ship's last 15 s, incidents with traces | sim: `recorder` |
| Radar (sweep, blips) | world: `radar` |
| Gun, laser, hull damage (a hit jams the hyperdrive 15 s); the combat phase (`World::combat`, `Armed`) | world: `weapons`, `damage` |
| SAM turrets (seeded per station/gate/spaceport, 6 km reach, fire on aggressors with a clear line), coverage | world: `turrets` |
| Shelter: pirates keep out of turret reach, prey under fire runs for a defended place | avionics: `hunter`, sim: `combat::flee`, `traffic::sightings` |
| Fight or flight: lawful ships judge aggressors (`hunter::judge`) and gang up on them; frame-start ship snapshot for what ships see (`Universe::snaps`) | avionics: `hunter`, sim: `traffic` |
| Fire control: track on a contact, gun lead | avionics: `fire_control` |
| Follow: keep at range / orbit a ship, station or gate (thrusters + engine, anchor-acceleration feedforward) | avionics: `follow`, sim: `follow` |
| Collision warning: predicted path through the kernel, first impact (bodies, stations, gates, ships) | avionics: `collision` |
| Radar contacts + transponders, the lock | sim: `contacts` |
| Combat phase in the tick (ship ids: player 0, craft i → i+1), fire control for the player | sim: `combat` |
| Frame profiler: named scopes per frame (per-thread tallies), mean/worst over 120 frames (F3 panel, `UNIVERSE_PROFILE=1`) | prof |
| The world engine's boundary: `Command`, `View`, `Engine`, `EngineHandle` (thread, mailbox) | sim: `engine` |
| Crafts side by side: `FrameLink` (bus over a shared world), deferred traffic `Request`s | sim: `vessel`, `traffic::fly_crafts` |
| Shared charts: galaxy, gates, goods, star systems | world: `charts` |
| Render thread; GPU meshes and the mesh shader | engine: `render_thread`, `renderer`, `model::Mesh` |
| Rendering, windowing, input, audio, frame timing (`Perf`) | engine |
| HUD, scene, nav map, observer, sounds, save file, dev scenarios | game |


## How changes are verified

- **Unit tests** in each crate (kernel invariants, device rules, traffic control, markets…): the
  whole suite runs in a few seconds.
- **Interaction tests** (`crates/sim/tests/interactions.rs`): 2–10 ships placed in one situation
  (two ships at a gate, full pads with one holding, a launch and a docking sharing a corridor, a
  pirate and its prey, a head-on collision), run for a few game minutes at 1×, and checked for
  the outcome. Together they take under a second. This is where traffic behaviour is developed.
- **The flight recorder** (`sim::recorder`): every ship's last 15 s. Any wreck files an incident
  with its trace, and the other ship's for a collision or kill (separation, closing speed, both
  clearances), printed by the interaction tests on failure. Causes are read, not guessed.
- **No long whole-traffic simulations** in the development loop: they're minutes each and, at
  warp, unrepresentative. The ignored `settlers_fly` / `settlers_trade` runs remain only as
  occasional benchmarks, on request.
- **Rendering** is checked with dev scenarios and screenshots (`UNIVERSE_SCENARIO`,
  `UNIVERSE_SCREENSHOT`).
