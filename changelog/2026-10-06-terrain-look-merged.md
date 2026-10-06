# terrain-look merged; `cargo run` is the game; a bind-group fix

- **Merged `terrain-look`** (2026-10-04, the user's word): the design `docs/world/planets.md`
  (worlds grown, baked once and shared; ports placed on the baked ground) and worlds seen through
  their air (the ground fading toward the sunlit air's light by the air between it and the eye).
  The air there is a placeholder: the lab's scattering (Rayleigh, Mie and ozone from each world's
  `atmosphere.json`) replaces it (`docs/planet-studio-plan.md`). In the mesh shader its air fields
  sit beside `up` (locations 13 to 15).
- **`cargo run` runs the game:** the workspace's default member is `crates/game` (the world crate's
  `registry-figures` tool made a bare `cargo run` ask which). `--workspace` for all of it, as the
  tests are run.
- **Fixed:** a textured model (a ship) drawn before the meshes' edges left its material in bind
  group 2, where the world's maps belong: the game stopped on a validation error with a ship in
  view. The world's maps are bound again after the textured models.
