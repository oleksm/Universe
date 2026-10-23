# Planet LOD colour and light: package colours as authored, the lab views' shade

Planets `20261023T121702-planets-a68a` (`planet-forge/docs/lod-colour-finding-2026-10-23.md`): the base planet tiles
decoded the package's sRGB to linear, and nothing encoded it back (the swapchain is not sRGB: colours pass as
authored), so the LOD was darker with its low channels crushed; the pyramid's tiles passed it raw.

- One colour path: `terrain::package_colour` (sRGB bytes as authored) for the base tiles, the land layers, the
  pyramid's (as before) and the objects' legend; `srgb_to_linear` gone.
- `light_exaggeration` read from the package's `info.json` too (`BuildData::light`): the base tiles' normals from
  their corners lifted by land or sea (`lit_normal`; `WireModel::normals`, new, per vertex, used by `pack_mesh`).
- The planet LOD app lights as the lab's views: frame ambient 0.3 and a white sun at the brightness that makes the
  rest 0.95 (`0.3 + 0.95 max(0, n·l)`). Not matched: the sea floor's 0.6 of its departure, the unsharp mask, the
  shadow map's shade.

lod_e124s43_v3 from 22,635 km, nadir, the reference sun: mean of the disc 8.5, 10.5, 9.1 → 20.5, 27.1, 31.8. The
workspace's library tests pass.
