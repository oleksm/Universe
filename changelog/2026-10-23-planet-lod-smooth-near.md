# Planet LOD base tiles: smooth close in, faceted relief far; hud=0

Planets `20261023T123944-planets-6233`: per-triangle normals showed the 15 km base triangles as a mosaic at 600 km.

- Each package vertex gets its lit normal once (`vertex_normals`: its finest triangles round it, lifted by the
  light exaggeration, area-weighted; again when the height x changes), and every drawn triangle takes its corners'.
- Each drawn triangle also carries its own flat lit normal and mean edge in its corners' `data`: the view shade uses
  the flat normal while the face spans under 2 px and blends to the smooth one by 6 px. Far off, each face keeps its
  own relief (the owner's "perfecto"); close in, no facets.
- Corner colours per vertex, not one per face: the colour mosaic close in gone too.
- `hud=0` in `UNIVERSE_PLANETLOD_AT` hides the panel (for parity shots).

lod_e124s43_v3, nadir, the reference sun: far (22,635 km) detail 3.72 (flat 3.6; smooth everywhere was 2.18); 600 km
smooth (`out/review/lod-parity/v7_600_cmp.png`). The workspace's library tests pass.
