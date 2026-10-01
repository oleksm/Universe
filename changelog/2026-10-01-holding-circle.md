# 2026-10-01 — The holding circle over spaceports; the throttle kept on a false start

## Waiting for a pad: a circle, level

User: "When I wait for clearance on a planet, right now it nose up like a rocket trying to bind to
a single point… Can we rewrite wait program so it arranges more like a circle queue pilots fly
around the spaceport and so horizontally?… 100 ships could circle around safely… in radius of
sams for protection so maybe 10-20km from the surface"

- **Before:** each queued ship hovered at its own fixed point (6 km up, on a 4 km ring),
  pointing the engine against gravity: nose up.
- **Now (`landing::hold`):** queued ships fly a level circle round the port, 12 km up and 10 km
  out (`HOLD_ALTITUDE`, `HOLD_RADIUS`), at 120 m/s, belly to the ground and nose along the
  circle. The lift thrusters hold the height; the engine and thrusters keep the speed and turn.
  - There are 100 places, 630 m apart, and the whole circle turns, so each ship keeps to its
    own place. Moving up the queue means moving up a place.
  - Farther than 5 km from its place, a ship flies there first, as on an approach.
- **The port's SAMs now reach 20 km** (`PORT_TURRET_RANGE`; their rounds fly 30 km), so the
  circle is under their cover. Other turrets still reach 6 km. Turret reach is now per turret
  everywhere it's used: gunners, pirates keeping clear, shelter, and the HUD's range circles.
- **Test:** the full-pads hold now also checks the circle. After two minutes the waiting ship is
  12,041 m up and 9,977 m out, level (1.000), at 119 m/s; it then lands on the freed pad.
- **Dev scenario:** `holding`. Its parked ships still free a pad quickly, so it shows the
  approach more than the circle.

## Hyperdrive: no lost throttle on a false start

User: "If I go full throttle and try hyper and it does not turn like too close to a planet -my
throttle resets. this is a bit annoying"

A drive that drops out on its own within a second of being switched on (a planet in the way)
hadn't really run: the ship gets back the throttle and velocity it had (`Ship::hyper_engaged`,
`FALSE_START`). Drop-outs its pilot orders, and those after a real run, are as before.

**Test:** `a_drive_that_drops_straight_out_leaves_the_throttle_and_speed_alone`.

## Guidance while holding

- The plan (orange) ends where the ship joins its place on the circle
  (within 400 m and 30 m/s of it): no more line looping back over the ship,
  and no polygon from simulating laps of the circle. The HUD's ETA reads
  HOLDING.
- The circle itself is drawn exactly (180 segments, faint magenta), with a
  marker on our place while joining (hidden once on it).
- The `holding` dev scenario tells traffic control the pads are taken, so it
  really holds.
