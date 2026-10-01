# Hyperdrive autopilot opens the throttle

The hyperdrive autopilot steered (and took the stick) but left the throttle where it was, and
engaging the drive zeroes it, so with the autopilot on the ship sat still unless the pilot
pushed the throttle up. It now runs the drive at full throttle itself: the drive's speed
follows the room ahead, and the destination counts in that room, so it slows for the arrival
on its own. The test no longer sets the throttle by hand, and checks the autopilot does.
