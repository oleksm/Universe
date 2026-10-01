# SAM missiles

The defence turrets had only their guns (6 km; 20 km at ports), so aggressors almost always
stayed out of their reach and the SAMs sat idle. Now every turret also has a missile launcher.

- **Missiles** (`world::missiles`): the turret's gunner names a target (the nearest fair game
  in 80 km on a clear line: `TurretCommand::launch`). Each launcher keeps two in the air, with
  a 10 s reload. A missile's motor pushes 6 g for 60 s (3.6 km/s all told), then it coasts.
- **Guidance:** zero-effort-miss guidance, the form of proportional navigation that allows for
  the missile's own motor. Time to go is reckoned with the motor's push, and the missile pushes
  across the line of sight by N × predicted miss / t_go².
- **Fuse and blast:** a 40 m proximity fuse, checked at closest approach over the frame. The
  blast is 15 MJ (¾ of a hull, and it jams the hyperdrive). A missile that loses its target
  (to hyperdrive, a gate, docking or destruction) destroys itself.
- **Fixes found on the way:**
  - A ground site's missile was counted as hitting the planet at launch: the body check used
    the highest peak's radius, and now uses the ground under the missile.
  - Missiles launched in a frame no longer also fly that frame. This matches how slugs work.
  - Guidance compared the missile at the frame's start with its target at the frame's end.
    At orbital speed that is ~530 m, and every missile homed exactly on a point that far from
    the ship. It now steers on where the target was at the missile's own moment.
- **Seen:** missiles are drawn as darts with a flickering plume while the motor burns. Those
  after you get a red diamond (the nearest with its range) and a blinking "MISSILE LOCK - N
  INBOUND, NEAREST …, IMPACT IN …".
- Test: an aggressor 40 km out (beyond every gun) is brought down by missile.
- Dev scenario `samfar`.
