# Ships collide by their shapes

Ship against ship used a 12 m ball for every hull, whatever its size: an 80 m hauler and a
winged Drover alike. Now:

- **Broad phase:** each ship's reach is the sphere round its own shape (the kernel's grid,
  swept over the frame, as before).
- **Exact check:** for pairs that come that close, their hulls are compared at moments through
  the frame (so a fast pass can't slip through). The test is a corner or edge middle of one
  inside a convex part (body, wings, fins) of the other, each ship where it was then and turned
  as it is now.
  - Contact is where they really meet: a wingtip clips another's; one passes over another's back
    untouched.
  - The normal is out of the face the point came in by (judged by how it was moving against the
    other hull), so wings, which are thin, don't turn a sideways clip into a vertical one.
  - The bounce, the damage and the push apart (by how deep they overlap at the end) follow from
    there, as before.
- **Shapes** keep their convex parts as solids (`Shape::solids`: the face planes, `inside`,
  `probes`).
- **Cost:** in the `crowd` scenario (1,001 ships), collisions take 0.02 ms a tick on average,
  0.25 ms at most.
- Tests: two Drovers, one 16 m over the other's back, pass clear (the old balls would have
  met); side by side 34 m apart their wingtips clip; 44 m apart they're clear. The bounce and
  hard-hit test holds.
