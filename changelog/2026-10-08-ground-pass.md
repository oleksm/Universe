# A ground renderer can plug into the engine (branch ground-pass)

- `engine::GroundPass` (`ground.rs`): a ground renderer handed to the engine through
  `Config::ground`, set up once against the scene's bind group layouts and formats
  (`GroundSetup`), then called each frame: `prepare` before any pass (its own compute and uploads,
  with the camera), `draw_shadow` in each shadow cascade (cascade 3 the ground's own), `draw` in
  the scene pass after the meshes with groups 0–2 bound. The engine's air, sea, clouds, shadows
  and HUD go over and round it. For planet-unfold's bench, which runs on universe-engine with no sim,
  world or UI.
- Unused, nothing changes: the game passes no ground and draws its own as before (98 tests; deck and
  Heath's range in frames as before; start time as main's). 52 lines added, 5 removed, in three
  engine files, plus the new one.
