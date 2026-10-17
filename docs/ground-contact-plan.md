# Ground contact: closing the gap

Status: built, 2026-10-17 (see "What was built" at the end). Found in a test drive of the MC-07 draft: the ship hung 12.0 m over the
ground at pitch +64, roll +22, still turning, climb and speed 0. Nothing touches the ground; the
ship is welded to a point.

## How it works today

| Step | Where | What it does |
|---|---|---|
| Touch | `physics/src/collide.rs::detect` | the first hull sphere that meets a solid; one contact, normal = radial (not the terrain's slope) |
| Judge | `world/src/rules.rs::apply`, `Rule::Lock` | sink speed against `Legs::hardest` (one aggregate for all legs): wreck, hard landing, or lock |
| Lock | same, `Pose::Ground/Deck/Top` | **teleport**: centre to surface + `rest_height`, orientation `upright`, spin zeroed, `ShipState::Landed`, power off |
| Landed | `world/src/world.rs::landed_step` | position welded (`physics/src/ops.rs::Weld`); when powered, `Ship::drive` still turns it about its centre |
| Lift off | `rules::release` | thrust along the normal over gravity: velocity added, moved 2 m clear |
| Bump | `integrate.rs` / `ops::bounce` | linear only, restitution constants 0.3 / 0.4 in code; no friction, no spin |

Rotation is outside Dogma: `Ship::drive` → `flight_control::integrate_rates` (`ω += I⁻¹τ dt`, no
gyroscopic term). `RigidBody` has no mass or inertia; the integrator never changes `ω`. The inertia
tensor exists (`ClassSpec::inertia`, `trim::inertia`).

## The gaps

| # | Gap | Effect seen |
|---|---|---|
| G1 | Landing is a snap and a weld: no contact forces | can't tip, slide, rock on its legs or fall over |
| G2 | A landed, powered ship turns about its centre | the test drive's pinned spin |
| G3 | A draft's rest height falls back to `SHIP_RADIUS` 12 m: compiled drafts have `visual: None`, so `ClassSpec::rest_height` ignores their `gear_*` nodes; `fit.legs` is empty unless gear blocks are placed | hangs 12 m up |
| G4 | Contact shape is auto-fitted spheres on a 7 m grid; feet are not contact points; `detect` returns one contact | no support polygon |
| G5 | No mass, inertia or torque in Dogma's rigid body | contact can't turn the ship |
| G6 | Legs have no stiffness or damping; `Fit::landing` collapses them to one aggregate | no per-leg load |
| G7 | No friction or restitution laws in Dogma; constants in code | not registry-sourced |
| G8 | Damage by one aggregate sink speed | a single overloaded leg can't fail |
| G9 | Code that assumes landed = upright at `rest_height`: autopilot (`avionics/landing.rs`, `plan.rs` probe), ports (`port.rs`, `station.rs`), taxi, `resettle`, `ship_on`, devkit scenarios | must accept a real pose |
| G10 | Ground normal is radial, not the terrain's slope | slopes act flat |

## The design

Contact is a law of Dogma, not a rule of the structure. A ship touching ground stays a rigid body
under gravity, thrust and contact forces until it settles; only then does it sleep.

- **Contact points**: one per leg foot (from `LandingLeg.foot`), plus the hull's spheres. Every point
  that touches gives a contact, not just the first.
- **Legs are oleo struts**: gas spring plus damper over the leg's `stroke`, derived (tier
  `derived`) from the figures they already carry: preload so one g of the ship's weight compresses a
  leg about a third of its stroke; damping so `sink_rate` is absorbed within `stroke × efficiency` at
  a peak force no more than `holds`. At the end of its stroke a leg bottoms out and becomes a hard
  contact.
- **Hull contacts**: stiff, with restitution from Dogma (a soft constraint, so the step stays
  stable).
- **Friction**: Coulomb, `μ` from Dogma (footpad on rock, hull on rock), smoothed near zero speed
  so a ship can stand still on a slope below `atan μ`.
- **Normal**: the surface's gradient at the point (`surface_radius_at` sampled around it), so slopes
  are slopes. Terrain mesh contact (`crates/terrain-contact`) can replace the sampling later.
- **Forces → motion**: the sum of the contact forces and their torques about the centre of mass
  goes through the ship's inertia tensor in Dogma's step. While touching: a contact substep of
  10 ms. A leg's natural period is about 1 s for the MC-07 (k ≈ holds/stroke), so that is ample.
- **Sleep**: once it's settled (|v| < 5 cm/s and |ω| < 0.01 rad/s for 1 s, and every touching point
  holding), the ship enters `Landed` with **the pose it settled in**, not upright. That is the
  existing weld, so a parked ship costs what it does today. It **wakes** on any thrust or turn
  command, a hit, or ground under it moving. A sleeping ship never turns in place (G2).
- **Breaking**: a leg whose force passes `holds` gives way. The ship drops onto that corner, and
  tips if its centre of mass is now outside its support. A hull contact over its impact limit is
  `damage::hit`. `HardLanding` comes from each foot's jolt. This replaces the aggregate
  `Legs::hardest` check.

## Phases

| Phase | What | Closes | Owner |
|---|---|---|---|
| P0 | Drafts stand on their feet: rest height from `gear_*` nodes whether or not there is a `visual`. The test drive says so when no gear block is placed instead of standing at 12 m. A landed ship doesn't turn in place (drive input wakes it instead) | G3, G2 now | ships (studio) + engine (`ship.rs` one line) |
| P1 | Dogma laws: `dogma.contact` with friction (footpad and hull on rock, sourced) and hull restitution. The leg's spring and damper derived from `LandingLeg` | G6, G7 | registry (request doc) + engine |
| P2 | `RigidBody` gets mass and inertia. Torque flows through Dogma's step, which integrates `ω` with `ω × Iω`. `Ship::drive` hands torque to it, so flight is unchanged (same-output test against today's rates) | G5 | engine |
| P3 | `physics/src/contact.rs`: points (feet and spheres), every touching one, surface normal from the gradient, leg struts, hull soft contact, friction, force and torque. Driven by a new `Response::Contact` | G1, G4, G10 | engine |
| P4 | World: `Pose::Ground` answers with contact, not a lock. Sleep into `Landed` with the settled pose, wake on command. Lift-off is physics (thrust beats weight), so `release` stays only for decks | G1, G2 | engine |
| P5 | Breaking per leg and per hull point; `HardLanding` per foot | G8 | engine |
| P6 | Downstream accepts a real pose: autopilot touchdown judged by the solver's outcome, ATC `landed` from sleep, `port::settle`/taxi only from sleep on a pad, devkit scenarios. Station decks keep `Pose::Deck` for now; they move to the same solver after ground works | G9 | engine |
| P7 | Studio: draw the contact points and each leg's compression in the test drive; the frame's landing load case takes the solver's foot forces | — | ships |

P0 can ship on its own today and fixes what the test drive shows. P2 to P4 are one change
behind a single switch, so the old snap is kept until the new path passes.

## Tests (fast, scripted, no long sims)

1. Four legs dropped at 1 m/s: settles at the static compression, sleeps within 3 s.
2. On a 10° slope with `μ` 0.5: stands. At 35°: slides.
3. Centre of mass outside the support polygon: tips over onto the hull and comes to rest.
4. One leg over `holds`: that leg gives way, the ship lands on that corner.
5. No legs: rests on its hull spheres.
6. Above the hardest landing: destroyed.
7. Energy never grows while it rocks; a sleeper costs one weld per tick.
8. Flight with no contact: the same position and spin as today's step.

## Cost

Sleepers cost what landed ships cost now. A ship is awake only while it's landing or taking off:
for a few seconds, at 10 ms substeps and a handful of points. That is small next to the NPC traffic
(about 100 ships).

## Open questions for the owner

1. Ground first and station decks after, or both at once?
2. Wheels and rolling taxi: later, as their own module?
3. Who builds P1–P6: engine session, or ships-claude with the engine's review?

## What was built (2026-10-17)

| Piece | Where |
|---|---|
| Contact forces: struts (spring `holds`/`stroke`, damper 0.7 `holds`/`sink_rate`, the registry's rule), hull points, friction, slope normals, turn about the centre of mass | `crates/core/physics/src/contact.rs` |
| Laws `FOOT_FRICTION` 0.6, `HULL_FRICTION` 0.4, `HULL_RESTITUTION` 0.3, `HULL_REST_SPEED` 0.5 m/s, `HULL_CRUMPLE_SPEED` 1.0 m/s (simplified, invented, to review) | `standards/Dogma/metadata/contact/` |
| A ship's legs and hull points (fitted legs; a modelled hull's gear; unmodelled stock hulls' four legs drawn 12 m down; a draft with no gear lies on its hull); the contact substep (10 ms); sleep after 1 s still; per-leg breaking; hard landing per foot; hull strikes crumple the hull | `crates/core/world/src/ground.rs` |
| Flight step runs contact near landing ground; landed ships never turn in place and wake on real thrust; rest welds the pose it lies in | `crates/core/world/src/world.rs`, `rules::rest` |
| The landing autopilot cuts its engines once down | `crates/core/avionics/src/landing.rs` |
| Drafts stand on their gear or their lowest hull corner, not 12 m | `ClassSpec::rest_height` |
| Test drive: ground line (feet, hull points, each leg's compression), no-gear note; dev hook `UNIVERSE_DRIVE_DROP=<m>` (+`UNIVERSE_DRIVE_TILT=<deg>`) | `crates/game/src/test_drive.rs` |

Hull points are the corners of the shape's convex parts (240 for the MC-07 draft; 15 µs a substep).
Differences from the plan: rotation from contact is integrated in the world's driver with the
inertia tensor rather than moving all flight rotation into Dogma (P2); station decks and rigs keep
their lock (P6). Still open: decks on the same contact, wheels and taxi, terrain mesh contact
(`crates/terrain-contact`), friction per ground.

## The ship's ground profile (2026-10-17)

What a ship is on the ground is worked out once for its design and legs (`ground::Profile`, kept on
the ship as `Ship::contact`, made again when the design or legs change), so the world step only
looks things up:

| Figure | How | Used by |
|---|---|---|
| Legs (foot, spring, damper, holds), hull points, reach | from the fit, gear nodes or hull corners | the contact step |
| Rest pose | the tilt within 30 deg of upright that brings the centre of mass lowest (1 deg grid, then 0.1 deg) | test drive; matches the flight (MC-07 draft: 10 deg) |
| Tip slope | the centre's distance to the nearest edge of what it rests on, over its height | landing checks, test drive |
| Slide slope | atan of the feet's (or hull's) friction | landing checks, test drive |
| Hardest landing | each leg's spring and damper stopping its standing share of the weight (by where the centre is), by halving; on the safe side (the flight holds up to about a tenth more) | the planner's landing prediction (`rules::apply`), the landing autopilot (sinks at no more than 0.7 of it), test drive |

The step itself culls: the whole ship clear of the ground under it (taken as a plane), nothing to
do; a point clear of it by more than the ground could rise across its distance (one in two), not
looked at. Answers are memoised per load (mass within a thousandth, centre within 10 cm).

Leg springs and dampers: a `LandingLeg` takes `spring` (N/m) and `damper` (N s/m) where its record
gives them; until the registry's landing-gear schema has them (asked 2026-10-17) they are worked
out as above. Sleep thresholds, the contact step and the push-out rate stay in code: they are the
simulation's numerics, not laws.
