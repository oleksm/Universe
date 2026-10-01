# Shapes about their centre of mass; landing gear (T1d)

- **Shapes are centred on their centre of mass as loaded:** mesh, detail lines and nodes are
  moved to match. A ship's physics point is its centre of mass, so it turns about it (the
  Drover's is 4 m aft of where its points were drawn from).
- **The landing legs come from the shape's `gear_*` nodes:** two aft, one under the nose. They
  were offsets in the renderer.

## Contact by the real shape

(This went in with commit 90249d5 by mistake, so it's written up here.)

- **The kernel takes compound bodies:** a rigid body may carry `parts`, spheres fixed in its
  frame (`physics::Sphere`). Against solids (station hulls, the ground, rocks) each sphere is
  tested where it is on the turned body and as it moves with the body's spin, after one check
  over the whole shape's bounding sphere. Passing a ring (a gate's opening) is still decided by
  the body's centre path, so a transit registers once.
- **Spheres come from the shape:** a shape gives its contact spheres, or they're fitted. On a
  grid 7 m apart across its plan, each column through the hull gets a sphere covering its cell
  from top to bottom (the hull's height taken at the cell's centre and corners, plus 0.3 m).
- **The Drover:** 38 spheres; every sampled point of its surface (corners, edge middles, points
  across each face) lies at least 0.67 m inside one (tested). It touches things by its 52 m
  span, not a 12 m sphere.
- **Still spheres:** ship against ship, and weapons' hitboxes. Docking slots are 300 × 80 m, so the
  Drover fits whichever way it's rolled.
- **Verified:** landing, autoland, a gate's final run and a docking approach look right in
  frames; all tests pass. One test now allows for an oblique touch: a rock is met by the sphere
  nearest it, a little aslant (38 m/s closing for a 40 m/s drop).
