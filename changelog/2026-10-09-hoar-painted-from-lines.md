# Hoar drawn from its lines, as the anchor model

- Hoar is drawn from the earth_s13 graph root (`worlds/TRE3/ground/earth_s13-graph-20261009b` in the
  worlds store, checked by its manifest's hash) as the planet lab's anchor model draws it: its level
  lines (land tan, paler above 2,000 m; sea floor blue), shores white, rivers pale blue, dry traces
  brown, peaks red and lows blue, as lines on the globe (`worlds::lines_draw`, a mesh of edges in
  their own colours, drawn unlit over the globe from afar). No painted texture: under the lines, flat
  ground, land grey and water near black from the shores (`worlds::lines_colour`, in place of the
  bake's `globe_color.jpg`, from afar and in the near maps).
- Only the look: Hoar's heights and ground are still its earth_s3 bake (TRE1), so the lines seen from
  orbit are not the ground you land on until TRE3 comes to main. The lines sit just over its highest
  ground and are drawn from afar only, not over the near ground.
- The package is named in `worlds/lines.rs` until Hoar's record names its lines.
