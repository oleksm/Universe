# Shared Studio flight controls

The shipyard's Test Drive flies a ship exactly as the game does: the same keys,
the same rules, one implementation.

- **Hands:** `flight_input::sample` (stick, translation, throttle delta) and
  `manual::held_for` (numpad jets) read the keys, in the game crate.
- **Rules:** `universe_avionics::helm` (client-side pilot software, beside the
  autopilots: the core never holds a pilot's intentions) turns them into orders
  for the ship as it is: J powers up or down only while set down; G (manual
  thrusters, flight computer off) only in flight, outside the hyperdrive, and
  going manual lets the autopilots go; an autopilot taken up or a landing puts
  the flight computer back on; jets are ordered only when the held set changes;
  while an autopilot flies the hands are off; otherwise W/S move the throttle,
  Shift+WASDQE the thrusters, arrows/A/D/Q/E and the grabbed mouse the stick. With
  thrust set and none given, it says the flight computer can't keep the ship
  straight (no thrusters to turn it).
- **Sending:** the game sends the orders through its cockpit
  (`App::pilot_input`); the test drive applies them to its sandbox ship
  (`LocalFlight::order`). The keys shown for the ship's state come from
  `flight_input::keys` in both (the HUD's manual cells, the test drive's line).
- The test drive's own keys are the sandbox's only: Z air/reset, R reset, Esc.

The Balance stand uses the same bounded allocator for trimmed lift. Its
untrimmed collective and single-actuator diagnostic remain explicit stand tools.
It reports delivered lift and residual torque instead of the old mixer's trim
percentage.

Studio evaluates available thrust, applies local power/fuel limits, then calls
the shared integrator with **realized** torque once. The last available rocket
fuel is shared proportionately across active nozzles. Ground/contact torque is
added separately. The design's arbitrary actuator layout supplies its authority
through the world's balanced-thrust helpers; saved placements are unchanged.

This consolidates pilot control, not the entire world simulation. Studio still
owns its local atmosphere, gravity, battery/thermal/propellant accounting,
spring legs and contact boxes. Its chase camera and practice-field HUD remain
Studio views; the radar renderer is shared. Autopilot, world navigation and
commissioning a design into the live world are outside this change.

Validation: game tests compare rotated-body and manual steps with the shared
controller, steering beyond the former cap, braking without levelling, empty and
partially exhausted fuel, shared input mapping, and a four-fan lift craft.
The core extraction additionally freezes legacy live-flight parity across 384
spans. Full verification results are recorded with the change.

Final verification: **146 workspace tests passed (18 game tests)**, registry build
passed, and both release launchers built. Vulkan captures of assisted Test Drive,
manual Test Drive and Balance were inspected. Evidence:
`out/review/studio-shared-controls/validation.json`, `drive.png`, `manual.png`,
`balance.png`, and build/test logs beside them. The isolated review design has
two real CH-S2 meshes, a flight computer, tank, battery and four gear struts.
It is a rendering fixture, not an accepted ship design. A frozen tested Studio
binary is beside the evidence; existing running windows need restarting.

Core dependencies: engine commits `15664110` (shared controller) and `9f114785`
(zero turning-bound convergence for balanced lift). Their ships cherry-picks
are `3512c02a` and `a61f1c1c`.
