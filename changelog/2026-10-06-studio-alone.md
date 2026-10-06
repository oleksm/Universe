# The planet studio shows worlds that aren't the game's

- A world in the store with no body in the game (Grown Earth, E4) is shown alone: a system of its
  own for the view only (`App::studio_world`; the sim never sees it), the star of the system in
  view and the world round it, its radius, day and tilt as the store gives them (`world.json`'s
  `day_hours`, `tilt_deg` where a package has them), its ground, colour, sea, air tables and
  clouds from its release, checked against the store's index (`worlds::Bake::release`,
  `Heights::of_release`). What the store doesn't give (its orbit, its air's density) is the
  system's first baked world with air's: the observer's ORBIT line is that world's.
- The view's globes, full maps and ground patches are kept under `View::cache` (the origin, or
  `STUDIO_CACHE` for the studio's world), so the two can't be mixed. Closing the studio drops its
  world and turns the observer to the star.
