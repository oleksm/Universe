# Planet LOD at 5,000 km: the relief's light kept whole to 2 km pixels; the lab's unsharp mask

Planets `20261023T183352-planets-e909` (the owner: blurry from 5,000 km, forge_e124s43-mesh-v2, 13°S 17°W).

- `TAPER_FAR` 19.5 km → 2 km: x13 kept whole down to 2 km pixels (5,000 km up had about x6.7, half the approved
  look), x1 at 100 m, log-linear between.
- `Frame::sharpen`: an unsharp mask on the screen's colours, after the curve (amount over a one-pixel 3×3 binomial
  blur), the scene only (not the HUD or front layer); `Globals::post.x`. The planet LOD sets it from `sharp=`
  (default 0.25, the lab's; `sharp=0` off). Other apps: 0.
- Captures (5,000 km, nadir, real view): `out/review/sharpen-5000km/` (before, taper only, taper + sharpen).
