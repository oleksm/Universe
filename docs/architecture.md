# Universe — Architecture

Status: target architecture, adopted 2026-09-30 (see `changelog/2026-09-30-architecture.md`).
This document is the reference; keep it in sync when the code's shape changes.

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

Crate boundaries are enforced by Cargo: `universe-physics` cannot import world or avionics
types, `universe-world` cannot import avionics, and so on.

## Physics kernel (`universe-physics`)

Knows only:

- **Rail bodies**: celestial bodies moving on exact Kepler orbits around a parent (planets,
  moons, and anything placed "on rails"), with radius, gravitational parameter, spin
  (tilt + day), and an optional **surface** (a height function, e.g. terrain). Positions,
  velocities, rotations at any time; per-frame **ephemeris** snapshots (p + v·τ + ½·a·τ²) for
  cheap substeps; gravity sum; dominant body.
- **Rigid bodies**: position, velocity, orientation, angular velocity, mass. Moved only by
  integration of gravity + applied forces (leapfrog, adaptive substeps), or by kernel ops.
- **Colliders**: surface (sphere + height function), convex polytope with optional cut-outs
  (e.g. a slot), ring (torus) with an opening **trigger**. The kernel reports **facts**:
  contact (which body/collider, point, normal, relative velocity), trigger crossing — never
  what they *mean*.
- **Ops** (explicit, audited): weld a body to a rail body at a local pose (and unweld),
  relocate a body between two reference frames preserving relative motion, spawn/despawn.
- **Queries**: gravity at a point, dominant body, ray/segment clearance, and **simulate a copy**
  (the planner uses this, so there is exactly one physics implementation).

Fairness rules: no special cases by object kind or owner; the same integrator, tick and
contact rules for everything; all inputs are forces/impulses or audited ops.

Invariant tests live with the kernel: determinism (state hash after N ticks), orbit stability,
energy/momentum sanity, collider/trigger correctness.

### As built (Phase 1)

`crates/physics/src/`: `orbit`, `rails`, `surface`, `body`, `collide`, `integrate`, `ops`,
`query` (plus a test-only `testkit`). Concretely:

- `RailBody { parent, orbit, mu, attracts, radius, tilt, day, collider }` — the rail
  description also carries the body's **collider** and an `attracts` flag (bodies too light to
  matter neither pull nor dominate). The **surface** is not stored in it: owners implement
  `OnRails` (`rail()` + optional `surface() -> &dyn Surface`), so the world keeps its own
  terrain type and hands the kernel the height function. Kernel functions (`positions`,
  `velocity`, `Ephemeris::new`, `gravity`, `dominant`, `Frame::of`, …) take `&[B: OnRails]`.
- `Surface` trait: `height(dir)` (ground under any liquid), `surface(dir)` (what is touched),
  `max_height()`, `liquid(dir)`.
- `RigidBody { position, velocity, orientation, angular_velocity, radius }`. Mass stays with
  the owner: drivers return an **acceleration** (force / mass). Orientation is carried but not
  yet integrated by the kernel — the ship's attitude model still sets it (from the driver
  callback) until attitude becomes a device.
- `integrate(bodies, ephemeris, positions, body, Span { t, dt, max_h }, driver)`: leapfrog +
  adaptive substeps (1/100 of the dominant body's orbital time scale, 0.01–3600 s, ≤ 2000 per
  call), substeps ≤ 0.05 s within 30 km of any small collider (polytope/ring). The `Driver`
  trait is the force callback: `applied(body, t, h, positions) -> acceleration` before every
  substep, `respond(fact) -> Continue | Stop | Bounce{..}` after a substep that produced a fact.
- Colliders: `Collider::{None, Surface, Polytope, Ring}`. `Polytope` = symmetric faces
  `w·|x| ≤ limit` (+ `cuboctahedron(scale)`) with `CutOut` pockets open along local +Y; hull
  normals by least penetration. `Ring` = torus + opening trigger, including the "measure the
  start of the step against where the ring was then" fix. Facts: `Fact::Contact(Contact {
  body, feature: Surface{liquid} | Hull | CutOut(k) | Ring, normal, local, surface_velocity,
  relative_velocity })`, `Fact::Trigger { body, relative_velocity }`.
- Ops: `Weld` (capture/place), `Relative` + `relocate`, `bounce` (the contact response a driver
  asks for). Queries: `gravity`, `pull`, `dominant`, `segment_distance`, `simulate` (a copy).

## World (`universe-world`)

- **Content**: galaxy, star systems, bodies, terrain (implements the kernel's surface trait),
  the gate network, spaceports.
- **Structures**: a station = rail body (circular orbit) + polytope collider with a slot
  cut-out + docking port; a spaceport = a pad on a body's surface + landing zone; a gate = rail
  body + ring collider + opening trigger + link to its paired gate.
- **Ships**: rigid body + parts/devices + fuel/cargo mass. **Devices** turn `ShipCommands` into
  forces or ops within limits:
  - engine (throttle → thrust ≤ max; fuel later), thrusters and lift, attitude (turn rates),
  - **hyperdrive** (a fictional device with explicit rules): moves along a commanded heading at a
    speed set by throttle and clearance from obstacles (and an optional commanded stop
    distance), relative to a commanded reference frame; interlock: never enters a body;
    disengaging leaves the ship co-moving with that frame,
  - **docking port** (contact with a slot cut-out, slow and aligned → weld; else damage),
  - **landing gear** (surface contact slow enough → weld; oceans and fast impacts → damage),
  - **gate device** (opening trigger crossed below the speed limit → relocate to the paired gate;
    ring contact → damage).
- **Damage**: from contact facts (impact speed); destruction and respawn rules.
- **Services**: traffic control grants/denies clearance (a world rule, not physics).
- **Events**: physical facts (contact, docked, landed, destroyed, transit) separate from
  service/UI events.

### As built (Phase 2)

`crates/world/src/` (package `universe-world`, depends on `universe-physics`, `glam`, `serde`):

- Content: `units`, `rng`, `names`, `galaxy`, `terrain`, `system` (moved from sim), `network`
  (home system choice, gate network, links). `World { galaxy, time, home_system, gate_links }`
  owns the clock and caches: generated systems, each system's neighbouring stars, and the
  per-system ephemeris shared by every ship stepping from the same moment.
- Structures with their contact rules: `station` (`StationFrame`, `hull()`, the docking port:
  `docks`/`bounces`, `contact` → dock or destroy, `launch`), `spaceport` (`PAD_RADIUS`,
  `pad_position`, the landing gear: `touch_down`, `lift_off`), `gate` (`GateFrame`, `RING`, the
  gate device: `enter` → transit state or too fast, `emerge` via the kernel's `Relative` op).
  (`PadFrame` — the pad as guidance sees it, with its entry point — stays with the landing
  guidance.)
- `ship`: `Ship` = rigid-body state + device settings (`throttle`, `rcs`, `hyperdrive`), `state`,
  fuel, cargo — **no navigation state**. `ShipCommands { throttle, rcs, turn: Option<Controls>,
  hyperdrive: Option<HyperdriveCommand> }`; `HyperdriveCommand { engage, heading, steering,
  frame_velocity, exit_velocity, destination }`. Engine + thrusters (`Ship::thrust`) and attitude
  (`steer`, crate-private) are the flight devices; `Ship::holding()` gives commands that change
  nothing, as a base for new ones.
- `hyperdrive`: the device. Speed = `HYPER_RATE` × room (nearest surface, or the commanded
  destination if nearer: the stop distance) × throttle, relative to the commanded frame (else the
  dominant body's); along the commanded heading or the nose; drops out by itself short of an
  obstacle dead ahead unless the heading is being steered; interlock: never into a body;
  dropping out leaves the commanded exit velocity (else co-moving with the dominant body).
- `damage` (destroy, `RESPAWN_TIME`), `World::respawn`, `World::ship_at` (spawn docked/landed).
- `traffic`: traffic control — `Facility` (station/spaceport/gate), `request` (not in flight,
  in hyperdrive, no target, not in this system, out of range → refused with a reason),
  `lapsed` (beyond twice the range, or gone), `nearest_station`.
- `events`: `ShipEvent` (physical: landed/docked, landed at a port, took off, launched, bumped,
  crashed, respawned, entered system, hyperdrive on/off, gate entered/arrived/too fast) and
  `TrafficEvent` (clearance granted/denied/cancelled).
- The ship step: `World::command(ship, system, &ShipCommands)` (no time passes: settings, then
  the hyperdrive switch) and `World::step_ship(ship, system, &ShipCommands, computer, real_dt,
  warp)` — destroyed (respawn countdown), landed (weld; turn; launch / lift-off), transit
  (countdown; emerge), hyperdrive, or free flight through the kernel, then the system hand-over.
  It reads only the ship, the commands and kernel facts. A **flight computer** takes part
  through the `FlightComputer` trait, again only with commands: `substep()` gives commands for
  every integration substep (the autopilots are feedback controllers and have always run at
  substep rate), `hyperdrive()` gives the frame's hyperdrive orders once the clock has moved on,
  and `interval()` is the longest substep it can fly with.
- Fine substeps come from proximity to structures (kernel: within 30 km of a polytope or
  ring), the thrusters firing (device state), or the flight computer's interval — never from
  clearance.

## Avionics (`universe-avionics`)

Per-ship software, pure functions of (sensors, own state) → `ShipCommands`:

- nav computer (nav target), clearance requests (via the traffic-control service),
- guidance (docking corridor, landing profile, gate run) and the flight **planner** (simulates a
  copy through the kernel with the same autopilot),
- autopilots: dock, land, gate, hyperdrive (steer to target, route around bodies, decide when to
  drop out), route (multi-stop, gates, dwell),
- later: each is a module a ship has installed or not.

## Orchestration (`universe-sim`)

Each tick, deterministically and in a fixed order for every ship:
avionics (or the player's input) → `ShipCommands` → devices → kernel step → contact/trigger
facts → device/world rules react → events. Owns the player's ship and the crafts (settlers),
traffic statistics, save/load. The world time scale (default 2×) is ticks per real second.

As of Phase 2 the navigation code still lives in `universe-sim` (moving to avionics in Phase 3):
`avionics.rs` (per-ship `Avionics { nav_target, clearance, hyper_autopilot }`, which learns what
happened from the world's `ShipEvent`s), `computer.rs` (the `FlightComputer`: the dock/land/gate
autopilot per substep, hyperdrive navigation per frame), guidance in `docking`/`landing`/`gate`,
the planner and routes. `Universe { world, ship, ship_system, avionics, route, crafts, … }`; its
`Event` is the pilot's feed: `Ship(ShipEvent)`, `Traffic(TrafficEvent)` and the avionics' own.

## Where today's code maps (refactor starting point)

| Before | After |
|---|---|
| `sim/orbit.rs`, ephemeris, gravity, `positions`/`velocity`/`dominant` in `system.rs` | physics: rails (done, Phase 1) |
| integration in `Universe::flight_step` | physics: rigid-body step (done, Phase 1) |
| terrain height function | world content, via physics `Surface` trait (done, Phase 1) |
| `docking::contact` | physics polytope collider + world docking port (done, Phases 1–2) |
| `gate::crossing` | physics ring collider/trigger + world gate device (relocate op) (done, Phases 1–2) |
| `touch_down`, `crash`, `dock` | world landing gear / docking port / damage (done, Phase 2) |
| `hyperdrive_step` | world hyperdrive device (done, Phase 2) + avionics hyperdrive autopilot (sim `computer.rs` for now) |
| galaxy, names, rng, units, system generation, terrain, gate network | world content (done, Phase 2) |
| `docking.rs`/`landing.rs`/`gate.rs` guidance + autopilots, `plan.rs`, `route.rs`, `route_step` | avionics |
| clearance granting | world traffic-control service (done, Phase 2) |
| `Ship.{clearance, nav_target, hyper_autopilot}` | avionics state per ship (off the ship: sim `Avionics`, Phase 2) |
| `Universe.route` | avionics state (per ship) |
| `Universe`, crafts, settlers, save/load | sim orchestration |
