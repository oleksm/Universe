# Hoar drawn from its lines, as the anchor model

- Hoar is drawn from the earth_s13 graph root (`worlds/TRE3/ground/earth_s13-graph-20261009b` in the
  worlds store, checked by its manifest's hash) as the planet lab's anchor model draws it: its level
  lines (land tan, paler above 2,000 m; sea floor blue), shores white, rivers pale blue, dry traces
  brown, peaks red and lows blue, as lines on the globe (`worlds::lines_draw`, a mesh of edges in
  their own colours, drawn unlit over the globe from afar). No painted texture: under the lines, flat
  ground, land grey and water near black from the shores (`worlds::lines_colour`, in place of the
  bake's `globe_color.jpg`, from afar and in the near maps).
- Its five highest peaks and five lowest lows pinned (from the lines' anchors): a pin stuck in each
  dot, straight up the screen with a head and a label on a dark box, `PEAK 1  8,566 M` to `LOW 5`,
  on the side facing the eye, while the globe is big in view.
- Near, down to the ground: the lines within the horizon laid on the ground the patches show
  (`lines_near`: each edge cut finer toward the eye, each point at the ground's height), laid again
  on a thread of its own when the eye has moved on (14 to 18 ms low down, about 75 ms from 1,250 km),
  the last laid drawn till then.
- Only the look: Hoar's heights and ground are still its earth_s3 bake (TRE1), so the lines seen from
  orbit are not the ground you land on until TRE3 comes to main. From afar the lines sit on a globe
  just over its highest ground. Low down few lines are in view: the root has a level line every
  200 m from samples ~160 km apart.
- The package is named in `worlds/lines.rs` until Hoar's record names its lines.
