# Land pyramid: a density cap on splitting

Mountains drew more triangles than pixels (the owner's session, forge_e104-mesh3, 24.8 km: 13.1 M triangles on a
959 × 526 surface, ~26 a pixel). A tile now splits only while both rules say so: its error over its distance
(as before) and its children's triangles averaging at least `UNIVERSE_TERRAIN_TRI_PX` pixels (default 2; 0 off) on
the actual render surface: its footprint's projected area (its side over its distance and a pixel's angle,
foreshortened by the angle the eye sees the ground at, no flatter than 0.05) over its children's triangles
(`mesh_nt`, else the grid's). The same 0.7 hysteresis; the morph follows either rule, so children appear on their
parent's surface where either only just splits it. Prefetch asks for children only near both. `Ground::draw`
records the surface's height for the next selection.

mesh3, -33.75° 97.4°, 24.8 km: 13.2 M triangles → 1.1 M on a 479 × 526 surface; simulating a 1080-high one
(0.474 on the small window), 3.6 M (1.7 a pixel), the ridges kept, the farthest a little softer; at 2 km 2.8 % of
pixels change. Test: the density cases in `pyramid_branch_four_bilinear_morph_and_hysteresis`.
