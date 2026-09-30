# 2026-09-30 — Combat, step 1: weapons

The combat plan was agreed as: weapons first (3), then settlers as targets and the consequences
(4), then combat mode (1: master arm, lawless and safe zones, arena), then turrets (2: stations,
surfaces, asteroids). The principle: nothing combat-specific in the physics; weapons are devices,
projectiles are bodies, and ranges emerge from the physics.

## What's in

- **physics `projectile`**: `Projectile` (a slug under gravity, semi-implicit Euler in ≤ 0.05 s
  substeps), swept against moving spherical `Target`s (relative motion, so a 3 km/s slug can't
  step over a 12 m ship) and against rail bodies (surfaces with terrain; polytopes by their
  bounding sphere). `ray()` for beams: the nearest target or body, with a coarse march plus
  bisection into surfaces. Hits are facts: `Hit::Target { id, point, relative_velocity }` or
  `Hit::Body`.
- **world `weapons`**: the ship gets a gun, a laser, hull integrity, ammunition, laser heat and
  triggers (`ShipCommands::weapons: Option<Triggers>`; None leaves them as they are, so the
  autopilots don't clear them). After every ship's turn, `World::combat(ships, dt)` runs:
  - **Gun**: 10 rounds/s, 0.5 kg slugs at 3 km/s from the muzzle plus the ship's own velocity,
    500 rounds. Every shot recoils (m·u/M), and a hit pushes the target (m·v_rel/M).
  - **Damage**: kinetic energy ½·m·v_rel². The hull holds 20 MJ, so about 9 hits at 3 km/s.
  - **Laser**: 2 MW on target within 2 km, falling as (2 km / r)² beyond, reaching 30 km. It
    heats over 8 s of fire, is then locked out until it cools to 30% (4 s from full heat to
    cold), so about 12.8 s to burn through a hull at close range.
  - Ships in hyperdrive, docked or landed can't fire. Wrecks and ships in transit can't be hit.
  - `damage::hit` produces `ShipEvent::Hit { by, damage, hull }` and destroys the ship at zero
    ("GUNFIRE" / "LASER FIRE").
- **avionics `fire_control`**: a `Track` on the locked contact, built from radar returns
  (velocity measured, acceleration estimated with smoothing), which gives a solution after
  `TRACK_TIME` = 1.5 s. `lead()` solves the intercept in the shooter's frame (shared gravity
  cancels over a short flight) against the muzzle speed.
- **sim `combat`**: the combat phase in the tick (player = id 0, craft i = i + 1).
  `Universe::fire_control(contacts)` and `ship_name(id)`. `TrafficStats::shot_down`.
- **game**:
  - SPACE fires the gun and V the laser, while held. Commands go only when a key changes, so
    they work under the autopilot too.
  - HUD: `HULL [..] GUN n LASER [..]` (flashing red when hit, and HOT when locked out); fire
    control reads `TRACKING n%`, then `LEAD READY SLUG FLIGHT t S`, or out of gun range.
  - The lead circle is drawn on the locked contact, with a faint line to it. Slugs show as
    tracers and points, laser beams in red.
  - Sounds: gunshot, laser hum (pitch rising with heat), and hit thuds.
  - New dev scenario: `gunnery`.

## Bug found on the way

In orbit, both ships move at about 7.8 km/s, and the slugs missed by hundreds of metres along
that motion. There were two errors, both shifting the slug by the absolute velocity. The spawn
point added the ship's whole velocity for the fraction of a frame since the shot, although the
ship was already at the end of the frame. And a new round was stepped a full extra frame. Fixed:
a round is placed `muzzle × late` ahead of the ship and starts flying the next frame. The test
`slugs_hit_when_both_ships_move_fast` covers it. End to end
(`the_player_shoots_down_a_settler_on_the_lead`): 18 rounds fired on the lead at a crossing
settler 3 km off, 9 hits, shot down.

## Cost

With nobody shooting, the combat phase only checks the triggers. 1,000 settlers ran at
2.29 ms/frame (was 2.27), with traffic results identical.

## Next (step 4)

Settlers as targets: they don't react yet (no evasion, no fleeing, no return fire), and there are
no consequences. Radar also can't see a target's hull (honest); hit feedback on the target is to
come.
