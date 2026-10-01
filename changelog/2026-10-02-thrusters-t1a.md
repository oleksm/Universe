# Thrusters as data (T1a)

The first step of real thrusters, with no change in flight yet.

- **Thrusters are nozzles on the hull's shape** (`shapes.ron`), each listed on the hull with a
  role and a thrust (`hulls.ron`). The Cobra has 18:

  | Group | Nozzles | Each |
  |---|---|---|
  | main drive | 2 on the rear plate | 1.35 MN |
  | thruster quads | 4 corners: nose and tail, left and right; each fires up, outward, and ahead (nose) or astern (tail) | 135 kN up; 270 kN side and fore/aft |
  | belly lift | 4 | 562.5 kN |

- **The hull's numbers are derived, not typed in:** main thrust (the drive along the nose), lift
  (the belly upward), and translation thrust (the weakest of its five directions) are worked
  out from the layout at load. A hull needs a main drive; a nozzle that isn't on the shape is
  refused.
- **Mass properties:** a ship's centre of mass and inertia tensor come from its shape as a solid,
  scaled to its mass as loaded. The Cobra (90 t): centre of mass 4 m aft of the shape's origin;
  inertia about (6.9, 13.9, 7.8) × 10⁶ kg·m² in pitch, yaw and roll.
- **Measured for what comes next:** its thrusters can turn it at up to about 2–3 rad/s² in
  pitch, 1 in yaw and 2 in roll, against today's fixed-rate turning (about 6).
- Tests: the Cobra's thrusters add up to exactly the old envelope (2.7 MN main, 540 kN every
  translation direction, 2.25 MN lift); inertia follows the shape (yaw hardest) and the load.
