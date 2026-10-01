# 2026-10-01 — Guide frames set in space; NPC pilots apart only once the world runs

User: "Those frames are jitterring back and forth, and also it keeps coming at me even if I stay
still… If I stay still frames should stay in their positiions. Remember those frames are
guidance set in space, so shit is going through arch, not the vise versa"

## Before

Frames sat at fixed moments of time along the plan (every 5 s). The plan starts at the ship and
is rebuilt several times a second, so:
- holding still, the same moments slid along toward the ship;
- each rebuild put the same moments at slightly different places, and the blend between plans
  swam them back and forth.

## Now (`scene::Guide`)

- **Fixed in the target's own frame:** they ride and turn with the station, gate or planet.
- **Placed by distance back from the goal along the path,** on a ladder: 60 m from the goal,
  then each a quarter further, 60 m to 20 km apart.
- **Where the ship is doesn't move them.** Only those still ahead of it are drawn.
- **Sticky:** a frame stays while each rebuilt plan's path still goes through it (within a
  quarter of its spacing). A plan short of the goal leaves them as they are.

Measured in the target's frame, consecutive frames:
- **Station queue, gate run, pad from 12 km:** no frame moved at all (0 m) over 200–360 frames,
  the ship flying in through them.
- **Landing from 12,000 km:** two far frames were re-placed in 320 frames, where the path itself
  had moved by more than 5 km.

## Also

**Dev scenarios that fast-forward the world** ran with the NPC pilots already apart, unpaced, so
the pilots fell far behind ("LATE 102959") and flew blind. They now go apart only when the
engine thread starts, at real-time pace (`EngineHandle::start`); setting up, they keep in step.

## Seen, not fixed yet

Every ship hyperjumping to a port aims at the same point 100 km above its pad. With 1,000
settlers they sometimes collide there (the autoland scenario: a settler hits the player at
201 m/s).
