# Hoar drawn from its lines, as the anchor model

- Hoar is drawn from the earth_s13 graph root (`worlds/TRE3/ground/earth_s13-graph-20261009b` in the
  worlds store, checked by its manifest's hash) as the planet lab's anchor model draws it: its level
  lines (land tan, paler above 2,000 m; sea floor blue), shores white, rivers pale blue, dry traces
  brown, peaks red and lows blue, as lines on the globe (`worlds::lines_draw`, a mesh of edges in
  their own colours, drawn unlit over the globe from afar). No painted texture: under the lines, flat
  ground, land grey and water near black from the shores (`worlds::lines_colour`, in place of the
  bake's `globe_color.jpg`, from afar and in the near maps).
- Its five highest peaks and five lowest lows (from the lines' anchors) pinned to the ground, from
  any distance, in every view, on top as the HUD's marks are: a ring on the spot, a stem straight up
  the screen with a head, and a label on a dark box (`PEAK 1  8,566 M  3,760 KM`: which, its height,
  how far). Round the back of the world, dimmed.
- Near, down to the ground: the lines within the horizon laid on the ground the patches show
  (`lines_near`: each edge cut finer toward the eye, each point at the ground's height), laid again
  on a thread of its own when the eye has moved on (14 to 18 ms low down, about 75 ms from 1,250 km),
  the last laid drawn till then.
- Hoar's ground from its lines (`worlds::LineHeights`, given to its terrain in place of its bake's
  heights): the nearest level line (loop or shore) and which side of it, the nearest line beyond it
  that way (a higher level or a peak on the high side, a lower one or a low on the low), the straight
  blend between by distance. Exact on every line and anchor (held by a test); the sea flat at 0 over
  it; about 31 µs a height, from a grid of the segments by 1° cell. The globe, the near patches,
  landing and collisions all read it. Its bake's runtime detail, surface fields and near maps (ground, normals, climate,
  rock, shine) are not used: near, only the lines' flat ground. From afar the lines sit on a globe
  just over its highest ground. Low down few lines are in view: the root has a level line every
  200 m from samples ~160 km apart.
- SHIFT+F4 stretches the lines' heights for seeing their relief, x1, x5, x10 (`worlds::set_stretch`;
  `UNIVERSE_HEIGHTS_STRETCH` from the start): the ground itself, so drawing, landing and collisions
  all see it; the globes, patches and lines made again. The pins keep the true heights.
- The package is named in `worlds/lines.rs` until Hoar's record names its lines.
