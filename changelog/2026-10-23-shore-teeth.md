# Planet LOD: no black teeth along the coasts

Planets `20261023T194005-planets-79ce` (the owner: black triangular teeth all along the coast, forge_e124s43-mesh-v4,
371 km, 58.78°S 41.22°W).

- The cause was holes, not shading: the black was the background. A base-package face was dropped whenever its centre
  lay on the pyramid's land mask. The pyramid draws land cells only, so the sea side of every face across the coast
  was drawn by nothing. That left one triangular tooth per such face.
- Now a base face (and edge) is dropped only where the land mask covers all of it: its corners, the middles of its
  sides and its centre. Faces across the coast, or round an inlet, stay under the pyramid's coast, so the base still
  draws the sea there.
- Captures in `out/review/shore-teeth/`: `now.png` (before, 9,859 black pixels), `fixed2.png` (after, 0). The same
  coast at 60 km and 1,500 km has 0 too. `dbg_zoom.png` is the diagnostic: the teeth were neither shaded (NaN) nor
  base faces.
- Not this fix: the soft grey band where the base's coarse coast meets the pyramid's edge (the coast seam already
  noted).
