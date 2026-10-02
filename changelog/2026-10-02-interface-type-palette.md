# The interface: a typeface, a palette

- **Noto Sans Mono** (open font licence; notice in `crates/engine/assets`) replaces the 8-pixel
  bitmap font.
  - Its printable ASCII rasterised once into an atlas at three times the layout's pixels, drawn
    as textured quads on the same monospaced grid (every table's columns hold).
  - Fills and text share one queue: plain fills sample the atlas's solid patch, so layering
    keeps its order.
- **The HUD layer at full resolution:** its layout still 960×540 units, drawn crisp at the
  screen's pixels; hairline borders.
- **The palette leaves green:**
  - information soft white;
  - secondary text slate;
  - "better" cyan;
  - amber and red as before;
  - panel backgrounds a neutral dark blue-grey instead of green-black.
- **Dust:** a mote when still (not a streak); streaks only once moving.
