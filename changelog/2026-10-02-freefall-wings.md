# Freefall; wings; round drives; a new pirate ship

- **The game is called Freefall** (by Plurence). The window title and README say so, and the
  executable is `freefall`. Saves go to `~/.local/share/freefall/`, and an older save is still
  read from the old place. The code's crates keep their working names.
- **Shapes can have several convex parts** (`parts` in `shapes.ron`): wings, fins, pods,
  joined to the body. Their mass properties are counted together, each part gets its own fitted
  contact spheres, and the faces of each hide what's behind them.
- **The Drover has wings:** thin swept wings with a little dihedral, 38 m tip to tip, and a
  slimmer nose. It looks like a winged shuttle from above, not a box. Its tail thrusters sit on
  the wingtips, for more roll authority, and stay balanced about its centre of mass. Its numbers
  are the same (60 t, 30 m/s² main drive, 25 m/s² lift); turning is 1.4 / 1.5 / 3.2 rad/s²
  (pitch / yaw / roll).
- **Round main drives:** every hull's main nozzles are drawn as circles on its rear plate,
  instead of squares.
- **Pirates fly a new interceptor:** a needle-nosed fighter on a thin delta wing, with a tail
  fin, instead of a wide flat wedge. It turns hard (3.1 / 5.1 / 6.6 rad/s²).
- **The courier** is a slim octagonal tube with two canted tail fins.
- **The flight planner** looks ahead in substeps of up to 1 s far out, down from 2 s. At 2 s,
  the winged Drover's planned landing from orbit came down short of the pad while the real
  flight landed. A landing plan from orbit now takes about 100 ms to build, on the cockpit's own
  thread, rebuilt about once a second.
- Checked in frames: each hull three-quarters on (dev scenario `look_<hull>`, for example
  `look_drover`), and all of them in a row (`fleet`).
