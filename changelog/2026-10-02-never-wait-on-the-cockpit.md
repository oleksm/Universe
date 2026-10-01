# The client never waits on the cockpit

A freeze reported (Application Not Responding) on locking a pirate near a starport.

- **What can freeze the game:** the cockpit (the player's pilot and ship computers) thinks on its
  own thread, holding its lock for the whole of each update: thinking, prediction, fire control,
  the flight plan, the impact warning. The main thread took that same lock every frame to build
  the view, and again on every command. Whenever the cockpit's update ran long, the game froze
  for that long, and orders stalled with it ("LATE" climbing).
- **Now the main thread never waits on it.**
  - **Commands** are held in order and handed over once the cockpit is free (`EngineHandle::flush`,
    every frame and on each send). The lock taken to check it's free is the one the command uses,
    so there's no window to block in.
  - **The view** takes the cockpit's displays when it can, and otherwise those it had last
    (`View::with_cockpit_of`).
  - Saves and loads still take the lock, since they must be exact.
- **The guide-frame builder** can't spin forever on a path length that isn't a number: a
  non-finite path clears the guide, and the rungs are capped at 4096.
- **Fixed, found while reproducing:** a locked rock's "drift" (our speed against its surface) took
  the rock's spin times the distance to *us*. From 1.74 AU off that read 1,020,041,750 m/s. It's
  now the spin at the surface under us, both in the HUD and in the follow guide.
- **Not yet found:** what made that particular cockpit update long. In a rebuilt scenario (Reat,
  117 km over a starport, combat mode, a pirate near, a rock locked) the cockpit costs well under
  a millisecond. Dev scenarios `hangrepro`, `hangrepro2`.
