# Explorer assembly available in the game showcase

Stage Blender's frozen crew-hatch-01 open and closed assembled snapshots and a
hash-checking launcher for the existing static showcase. The launcher uses a
separate XDG save directory and accepts the locally built game binary. The exact
Blender source, export hashes and pose matrices remain in its frozen packet.

This supplies the supported static view requested by the owner. A commissioned
Explorer remains blocked on the formal hull export, current mass/equipment and
propulsion selection, mounts, and compound exit/gear consumer support. No registry
record, stock fit, runtime markers or physical ratings are fabricated.

Validation: both snapshots match frozen bytes; all eight declared source/world
pose matrices independently match raw GLB hierarchy within1e-5. Both launcher
`--check` paths pass. Actual renderer validation is coordinated with engine.
