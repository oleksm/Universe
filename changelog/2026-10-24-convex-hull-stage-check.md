# Convex hulls in a second, exactly; a stage can't fly; a stage check for candidate hulls

Blender `20261024T102646-blender-80af`, `20261024T103620-blender-d5a3`; the Explorer ground-stage V3 import never finished.

- `Mesh::convex_hull` was every plane through three points, O(n⁴): a model without `COL_*` parts (the Explorer's
  hull from 459,381 vertices) never finished importing. Now incremental, with exact integer side tests (points on a
  2⁻²⁰ m grid, finer than f32). Coplanar triangles join across edges by the same exact test, and each face keeps only
  its outline's corners. The V3 hull: 1.5 s, 164.687282 m³ (Qhull 164.687284). Blender's 340 exact vertices give the
  same. The exhaustive search stays for flat sets.
- The old search also fanned a point inside a face into its outline: a cube with a face-centre point came out at
  area 5.75. Blender's audit of its 340-point hull (175.76 m³, +6.7%) was that. Fixed by the outline step.
- `Ship::drive`: an uncommissioned stage makes no thrust whatever it carries (the flight contract said "can't be
  flown"; nothing enforced it).
- `examples/hull_stage_check.rs`: the real importer on a candidate export with given mechanisms and exits, before
  the registry has its record: binding, markers per state, gaps to the gear plane, exits, a walk down the walk exit,
  the climb. `import::hull_from_record` is public for it. V3 result: `docs/ships/explorer-v3-stage-check.txt`.

Tests: `mesh::tests::convex_hull_matches_exhaustive`; the battery test now also checks a stage doesn't fly.
