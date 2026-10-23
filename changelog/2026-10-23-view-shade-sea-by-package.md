# View shade: sea floor is what the package calls sea (the "bubbles")

Planets `20261023T175055-planets-baa3`: light round discs (30-80 km) on base land in Mesh view, forge_e124s43-mesh at
3,826 km, 40.9° S 173.2° W. They were closed basins under 0 m: the view shade's sea rule (sea floor where a pixel lies
under the sea level's radius) shaded them as sea floor, pulled toward 1, lighter than the land round them; the same
rule as the pyramid's honeycomb, on the base.

- Each base vertex is sea if it's a corner of one of the package's sea tiles (`BuildData::sea`); its `data.w` says so
  (`sea_mark`: the cube marker, doubled for sea), in the Real view (with its cube direction) and the Mesh view (no
  direction: the cube isn't sampled there). The shade's sea rule takes that; an unmarked vertex still by its depth.

The Mesh view at 3,826 km: the discs gone (`out/review/tile-textures/bub_cmp.png`); the Real view from 10,000 km the same
(0.09 % of pixels; the sea's shade unchanged). The workspace's library tests pass.
