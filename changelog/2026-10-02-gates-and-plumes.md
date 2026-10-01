# Gate flashes, longer transits, drive plumes you can see

- **Other ships going through a gate are visible:**
  - A ship crossing the ring and vanishing leaves a white starburst, an expanding ring in the
    gate's plane and a streak out through it.
  - A ship about to come out shows the light gathering to the point where it will emerge.
  - Each flash lasts 2.5 s, drawn from the ship's transit (where it crossed, where it's going).
  - Dev scenario `gateflash`; checked over consecutive frames.
- **Gate transits take twice as long:** 10 s instead of 5.
- **Thruster plumes are worked out from their thrust:**
  - Each firing nozzle's plume is as long as √ of the force it's giving: a full Drover drive
    nozzle (1.35 MN) about 40 m, a thruster quad about 8 m.
  - The drive and lift nozzles' mouths glow, sized from their rated thrust, so a drive seen from
    behind is visibly lit.
  - The numbers come from the hull and fit (cached per fit), not the load.
- **Your own drive was hidden in the chase view:** your hull is drawn in a front layer over
  everything, and the plume behind it was covered. The front layer now takes lines too, so your
  jets are drawn with your hull. Dev scenarios `burn` and `burnside`.
