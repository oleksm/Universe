# The land pyramid never draws coarser than the base

Planets `20261023T174656-planets-4124` (the owner: features pop, change or vanish on zoom): at 2,835 km the base cube
(9.8 km texels) gave way to pyramid tiles of 14-29 km cells, coarser than the base, and the relief vanished at the swap.

- `Source::base_cell` (`Pyramid::set_base_cell`): the base's own resolution; `Ground` sets it to its cube's texel
  (πR/2 ÷ n: 9.8 km on forge_e124s43). A tile with coarser cells always splits, whatever its error, so the pyramid
  only refines below the base; such a parent is never drawn, so its children don't morph toward it.
- (The base under the pyramid is dropped at load, so the base can't stand in there: the pyramid refines to it.)

forge_e124s43-mesh from 6,000 km: 12 tiles of 14-58 km cells → 90 of 7.2 km and finer, the range crisper
(`out/review/tile-textures/bc6.png`); 3,500 km: 33 → 65 tiles; nearer, the same picture. Plains' relief a little softer
than when the coarse tiles took the cube's normals; a faint curved line at the left of that view may be the pyramid's
edge against the base, to look at. The workspace's library tests pass.
