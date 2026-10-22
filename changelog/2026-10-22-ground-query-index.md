# Ground queries through a triangle index

Over Shasta (forge_e104-mesh2, 10 km over 37.25° S 100.47° E) the planet LOD app ran at 41 fps: each frame's
25 drawn-ground samples (`flight::average`) fell through the pyramid's hole to a scan of every triangle of the
3 × 3 chunks round each (Shasta's hold about 10⁵) and of the tile groups.

`terrain::tri_index::TriIndex`: a patch's triangles bucketed by their bounds on its own gnomonic plane (a ray
from the centre crosses a flat triangle exactly where its projection holds the point), about four a bucket.
`Ground` keeps one for each chunk's finest triangles (`ground_at`) and one for each tile group (both by
direction, so any height x), made when first asked; and one for each chunk's drawn surface, its triangles as
drawn (degenerate and repeated ones after the stand-ins dropped), made again with each new surface.

Same view, before and after: 41 fps to 120 (the cap); the same ground (917 m) and clearance (10,317 m). Test:
`tri_index_finds_every_triangle_a_ray_passes`.
