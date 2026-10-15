# Four-region lab

- The engine's `Surface` reads PGS1 revision-4 packages (`Surface::open_package`): mapped files, lazy regions, block
  hashes on touch, composed ids and cross-file neighbours. The dev lab loads a package folder.
- Dev: keyframe camera flights with captures (`UNIVERSE_PGS1_FLIGHT`), package read/residency log, a haze-free
  diagnostic switch (`UNIVERSE_PGS1_NO_AIR`); `pgs_walk_audit` reads packages and makes line corridors.
- Results on the four-region S15 package: docs/diagnostics/four-region-lab.md.
