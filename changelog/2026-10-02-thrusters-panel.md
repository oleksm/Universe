# The thrusters panel (F7)

In the pilot's seat, **F7** opens a screen of the ship's thrusters:

- **Every thruster:** its level (a bar), the thrust it's giving (kN) and the fuel it's burning
  (kg/h). The drive is amber, the lift blue, the translation thrusters white; idle ones dim.
- **The ship's plan from above and from the side:**
  - its hull outline, with each thruster where it sits;
  - firing ones plume out along their exhaust, as long as they're firing (seen end-on, a ring);
  - the centre of mass (+), where it is now.
- **The fuel:**
  - what's in the tank;
  - what's burning now (the drives' thrust over the exhaust velocity, and the hyperdrive's draw
    when it's engaged);
  - how long the tank lasts at this burn ("EMPTY IN 29.6 H AT THIS BURN", red under ten
    minutes).
- **The balance:** the centre of mass's offset, what the drive, the lift and the thrusters give
  without turning the ship (see the balance changelog), and their rated thrust.
- Dev scenario `thrusters`.
- The F-key help lists F7, and the throttle's help no longer says W launches (you lift off a
  deck now).

**Findable:** the mode bar has an **F7 THRUST** cell, lit while the panel is open. Before, it was
only in the F1 help.
The NAV instruments grid has an **F7 THRUSTERS** cell too, lit while the panel is open.
