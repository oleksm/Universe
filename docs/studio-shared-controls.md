# Shared Studio flight controls

Studio Test Drive and normal flight use `flight_input::sample` for keyboard/mouse
commands, `manual::{held_for,labels_for}` for spatial numpad bindings, and
`world::flight_control` for rate demand, bounded actuator allocation, manual
firing and angular integration. The normal pilot retains its existing autopilot,
cursor and threaded-input gates.

- W/S adjusts forward throttle at the normal rate.
- Shift+WASDQE commands translation; Shift+E is upward lift.
- Arrows/A/D/Q/E and grabbed mouse command body turn rates.
- G switches assisted/manual control; manual uses the same numpad layout and W
  for main engines, with no hidden braking.
- Assisted rates come from the placed flight computer. Without one, Studio uses
  manual mode and explains what is missing.
- Releasing a turn commands zero rate (braking), not a level attitude. There is
  no separate Studio attitude target or tilt cap.
- Saved design axes, chase view and radar use -Z forward, +X right, +Y up.

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
