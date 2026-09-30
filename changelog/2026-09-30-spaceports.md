# 2026-09-30 — Planetary spaceports, navigation map, landing guidance and autoland

## Summary

You can now pick a spaceport on a navigation map, request landing clearance, and land on a planet
by hand (with guidance) or with the autopilot, from anywhere in orbit.

## Player-facing

- **Spaceports** on every rocky planet and about half the large moons (radius over 800 km). Each pad is
  a marked square with an "H", a light beam, and (within 150 km) a 1 km ground grid out to 10 km.
  The grid is the altitude cue on the way down.
- **Navigation map (M)**: an opaque full-screen chart. On the left, every station and spaceport in
  the system, sorted by distance; on the right, a top-down schematic (planets on evenly spaced
  rings at their true angles, moons, stations as squares, ports as triangles, "YOU").
  Up/Down (or W/S) to select, Enter to lock the nav target, Del to clear, M/Esc to close.
  Mute moved from M to F8.
- **Nav target marker**: the HUD diamond (or edge arrow) now points at the locked target with its
  name and range; without a target it still points at the nearest station.
- **R requests clearance** from the locked target, station or spaceport. Landing clearance is
  granted within 20 planet radii of the pad.
- **Landing guidance**: panel with range, altitude above the pad, vertical/horizontal speed,
  horizontal offset, tilt, and a recommended GO speed or SINK rate; hints like
  "IMPACT AHEAD - PULL UP", "LEVEL OUT - BELLY TO THE GROUND", "SINKING TOO FAST - SHIFT+E TO BRAKE".
  In 3D: a free-fall trajectory prediction in the planet's rotating frame with a red X at the
  impact point, the magenta guidance path, and the descent column from 3 km down to the pad.
  For large velocity changes the thrust hint says `BURN <dv>: FACE THE SQUARE, THEN W`.
- **K: autopilot** now docks *or* lands, depending on the clearance. Landing: main-engine flight
  in the planet's rotating frame to a point 3 km above the pad (following the curve of the planet
  when the direct line would cut through it), then belly-down vertical descent to a 1.5 m/s
  touchdown. From the home station's orbit it takes about 2.5 hours of game time (use warp).
- **Lift thrusters**: Shift+E now pushes up at 25 m/s² (enough to hover), other RCS axes 6 m/s².
  Shift+E also lifts off from the ground.
- Touching down within 250 m of a pad gives "TOUCHDOWN - WELCOME TO <PORT>" and the HUD shows
  "LANDED AT <PORT>".

## Simulation

- `system::Spaceport` generated from its own random stream, so the planets are unchanged.
- New `landing` module: `PadFrame`, `guidance`, `predict`, `status`, `autopilot`.
- Clearance generalized: `Ship::clearance: Option<Clearance { target: NavTarget, autopilot, phase }>`,
  `Ship::nav_target`; phases Approach/Align/Final (dock) and Approach/Descent (land). Docking
  now shares the attitude controller and RCS model. Events renamed: `ClearanceGranted/Denied/
  Cancelled`, `Autopilot { on }`; new `NavTargetSet`, `LandedAtPort`.
- Bug found by the tests: routing around the planet by chasing a point ahead at the same altitude
  spiralled into the ground (a chord always dips below the arc). Now it cruises along the great
  circle with an explicit altitude hold (at least 5% of the planet radius).
- Tests: autoland from orbit, from the far side of the planet, and from 50 km above the pad.
  All 12 sim tests pass.

## Dev

- Scenarios: `navmap`, `landing` (cleared, in orbit), `padview` (12 km over the pad),
  `autoland` (autopilot mid-descent), `touchdown`.

## Follow-up: hyperdrive throttle

- Playtest: engaging the hyperdrive at full throttle jumped straight to full hyperdrive speed and
  flung the ship far away. Toggling the hyperdrive on or off now resets the throttle to 0 (and
  dropping out still matches the nearest body's velocity). The hyperdrive test checks both.

## Follow-up: hyperdrive drops out at the nav target (manual by default)

- Playtest: hyperdrive dropped out ~18,000 km from a targeted port. It stopped at 3 planet radii
  whenever a body was dead ahead; for planets that is tens of thousands of km.
- Design direction from the user: full manual flight by default, autopilots optional (navigation
  aids will later be modules the pilot buys).
- With a nav target locked, you still steer. Hyperdrive speed also scales with the distance to
  the target (no overshoot), and it drops out within 20 km of a station or 120 km of a port,
  matching the target's motion (station velocity / the ground's rotation), then requests clearance.
  Heading for a targeted port doesn't trigger the "planet dead ahead" stop while the line to the
  pad is clear.
- Without a target: stars keep the 3-radii margin; planets and moons drop you out ~1,000 km up.
- K in hyperdrive (with a target) toggles the hyperdrive autopilot: it steers to 100 km above the
  pad (around the planet if needed) or to the station. HUD shows `HYPER AUTO`.
- Tests: manual jump to a port (pilot re-aiming at the marker), hyperdrive autopilot to a port,
  untargeted stop short of a planet. 15 sim tests pass.
- Bug in the first autopilot version, found by the tests: the aim point 100 km above the pad
  counted as "blocked by the planet", so the ship circled at ~8,000 km. Fixed.
