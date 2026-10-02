# Gates lined up, a white hull, no cemetery

- **Gate approaches from the right side:**
  - the hyperdrive drops a ship out well back along a gate's entry side (24 km behind the ring), not
    on whatever side it came from;
  - from the far side, the run goes round the outside of the ring (out wide, past its plane, in), no
    longer back through the empty ring and round again;
  - the route autopilot counts a gate as near within that drop-out distance. Before, it would
    hyperdrive again, forever.
- **Snow-white hull** (ours), and **the eye adapts its white** to the starlight most of the way:
  a white hull looks white under an orange star. The star's own disc and glare keep its colour.
  Planetshine is half washed out (ground of many colours, and air, between).
- **No boiling up close:** colour detail only from octaves at least about 12 pixels across (its
  patches are cut by thresholds), slopes from about 6.
- **The station's roof:** masts are slim poles on footings with a platform partway up and a thinner
  tip; no cross-arms. Plant of different sizes is spread about, not a row of identical boxes.
- **A round radar scope:** its rings, ticks and sector lines are drawn where they fall (`hud_line_smooth`),
  not snapped to whole HUD units (two real pixels each), which made the curves step and wobble.
  Rings have 128–160 segments. Every HUD ellipse is smooth now.
- **No "queued" on a gate's final run:** a ship asks for the corridor only while approaching or lining
  up. Traffic control frees it as the ship nears the ring, and asking again on the run put it back in
  line, behind the others. Every ship waiting there also counted one more ahead than there was.
