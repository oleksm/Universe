# Planets, not tomatoes

- **Shaded smooth:** a globe's corners take the average of the faces meeting there (`WireModel.smooth`),
  so no more ribs along the meridians. Hulls and rocks keep their facets.
- **Continuous ground colour** (`terrain_view::ground_color`) instead of four flat classes:
  - seas darker the deeper, turquoise in the shallows;
  - sandy coasts; land green to brown with height; snow on peaks; ice at the poles;
  - other worlds in their colour, lighter high, darker in craters;
  - a little variation everywhere.
- **Finer globes:** detail 8 near, 2 far (was 4 and 1), so coasts are no longer stair-stepped.
- The `planet` scenario takes `UNIVERSE_DIST` (radii) and `UNIVERSE_YAW`.
