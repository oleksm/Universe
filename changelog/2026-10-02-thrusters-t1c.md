# Real thrusters: the flight computer, turning by torque (T1c)

Ships now fly on their thrusters as placed.

- **The flight computer** (a base block every ship carries, `Ship::drive`). Each step:
  1. the push the throttle and thruster controls ask for, and the turn the stick or an autopilot
     asks for (rates, brought about within `TURN_RESPONSE`, 0.12 s, as far as the thrusters
     allow), are **allocated** to the hull's nozzles (`world::thrusters`): a bounded
     least-squares problem in accelerations, with the push weighted over the turn, solved by
     projected coordinate descent warm-started from the last settings;
  2. what the nozzles give, a force and a torque about the centre of mass, moves the ship: the
     force through the integrator, the torque against the shape's inertia;
  3. fuel is the sum over the nozzles, each by its own thrust.

  Nozzles do what their places allow: the main engines throttle apart to yaw, the belly lift
  pitches, couples turn without pushing. A hull without side thrusters couldn't strafe.
- **Turning envelope** per hull, derived at load: the Cobra turns at about 2.0 rad/s² in pitch,
  1.3 in yaw and 2.0 in roll (a lighter ship faster). A 180° flip takes about 3.9 s (it was
  3.1 s with the old fixed-rate turning) and settles cleanly, with the main engine on or off.
- **Autopilot attitude control** (`docking::attitude`) asks, on each axis, for no faster a turn
  than it can still stop from in the angle left (√(2αθ), with a margin).
- **The Cobra's layout:** the thruster quads and belly lift are placed about its centre of mass
  (4 m aft: its wings are at the back). The first layout had the lift 16 m ahead of it and 8 m
  behind, which cost a quarter of the usable lift (the nose lifts had to throttle back to keep
  the ship from pitching) and crashed the landing autopilot. Every translation direction now
  gets 100% of its thrust without turning the ship (tested).
- **Simplification:** the spin's own coupling (ω × Iω) is left out. The flight computer holds
  against it anyway, and a ship tumbling free keeps its body-frame spin (near enough for a
  near-symmetric hull, and steady over the planner's long steps, where it blew up).
- **The flight planner** steps at most 2 s far out (was 10): a held command over 10 s with real
  turning no longer matched flight, and the landing plan failed. Plans take ~70 ms to build in
  the background (were ~30).
- Cost: 1,001 ships still 0.67 ms a tick.
- Tests: full throttle is both engines with no turn; a pure turn pushes nowhere; strafing doesn't
  yaw; every direction gets full thrust; the envelope; a 180° flip in under 5 s that settles.
  The docking, landing, gate and route autopilot tests pass on real thrusters.
