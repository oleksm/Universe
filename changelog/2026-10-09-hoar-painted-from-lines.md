# Hoar's colour painted from its lines

- Hoar's globe, from afar and in its near maps, takes its colour from the earth_s13 graph root
  (`worlds/TRE3/ground/earth_s13-graph-20261009b` in the worlds store, checked by its manifest's hash)
  in place of its bake's `globe_color.jpg`: `worlds::lines_colour`, an 8192 by 4096 image painted in
  about a tenth of a second. Each closed level line and shore is filled on its inside; a pixel takes the
  200 m band of the innermost line round it, water inside the shores by depth, rivers that carry water
  drawn over it.
- Only the colour: Hoar's heights and ground are still its earth_s3 bake (TRE1), so the continents seen
  from orbit are not the ground you land on until TRE3 comes to main.
- The package is named in `worlds/lines.rs` until Hoar's record names its lines.
