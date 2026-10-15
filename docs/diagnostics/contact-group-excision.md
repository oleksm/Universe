# Contact group: excision, lock closure and the rim

Task `20261013T151045-planets-5f73` (engine-claude). Branch `engine-claude-walk` from main b8e70369. CPU only: no
game wiring, GPU run, install or main merge yet. Source, physics and caps unchanged.

`universe_terrain_contact::worker::group` builds one complete group from the actual resident selection:
the band (`sparse_band`, now on the runtime's host radius), the corrected retained triangles
(`ownership::correct`), and the new `replacement::excise`:

- **Excision.** Every resident grid triangle inside the chart square `[-64, 64]²` goes; every crossed one keeps
  only its parts outside, cut along the band's own outer chain (`sparse_band::crossing`, shared bit for bit) and
  stitched to the band's points between. Skirt segments along an edge inside the square go, crossing ones are cut.
- **The rim.** The kept parts of crossed triangles are drawn as one rim mesh in the band's frame. In a coarse
  patch's frame (hundreds of metres) f32 cannot hold the band's points to 10 µm: the parent snapshots measured
  19–32 µm when they were drawn there. In the band's frame the band–rim seam is exact; the rim meets each patch at
  that patch's own f32 precision, as patches already meet each other.
- **Locks and their closure.** Every footprint vertex stops geomorphing (delta zero, size kept), at its corrected
  place. A seam relation (the same lattice point on two patches, or a fine point on a coarser neighbour's
  segment) holding a locked vertex locks its every moving member; vertices with zero delta never move and stop the
  closure. A dependency on a patch not supplied, or across a cube face, refuses the group.
- **Refusals.** Unsupplied/stale/partial input, resident coverage of the square other than 16,384 m², f32 collapse of
  two distinct points more than 10 µm apart, a band seam over 10 µm, or any patch seam the replacement makes wider
  than the original renderer's (at the job eye or that eye) at seven eyes (job eye, ±30 m east/north, +20 m up,
  90 m east). Two distinct boundary points that are one f32 point in the band's frame and under 10 µm apart are
  drawn as one and counted.

## Six actual resident snapshots

`cargo run --release -p universe-terrain-contact --example contact_group <surface.pgs> <out.json> <snapshots…>`
on the ships-contact-adapter exports (`out/review/contact-adapter/shared-client-snapshots`, source
`4b30e7ed…`). Review: `out/review/contact-group/v1.json` (local, not in Git). Skirt faces are not in the export:
the example lays them out as the patch maker does over the exported skirt vertices (labelled in the output).

| Snapshot | Patches changed | Band + rim triangles | Closure locks | Peak MB | GPU MB | ms | Band seam m | Rim–patch m | Seam widening m | Holes / double | Core contact m | Band vs canonical m | Exterior vs resident m | Ownership correction m |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|
| crest 1 parent | 39 | 4481 + 1527 | 94 | 5.59 | 3.82 | 38 | 0 | 4.0e-6 | 0 | 0 / 0 | 5.1e-6 | 9.38 | 2.75 | 10.2 |
| crest 2 parent | 21 | 4238 + 1039 | 47 | 3.13 | 2.39 | 29 | 0 | 7.1e-6 | 0 | 0 / 0 | 4.1e-6 | 10.7 | 0.282 | 0.51 |
| face join parent | 1 | 1154 + 148 | 0 | 0.72 | 0.35 | 7 | 0 | 3.2e-6 | 0 | 0 / 0 | 5.2e-6 | 0.027 | 3.2e-6 | 0 |
| crest 1 settled | 77 | 5097 + 2850 | 24 | 11.95 | 3.93 | 83 | 0 | 4.0e-6 | 0 | 0 / 0 | 5.1e-6 | 0.189 | 0.0010 | 0.0015 |
| crest 2 settled | 78 | 5104 + 2885 | 26 | 11.97 | 4.01 | 83 | 0 | 4.1e-6 | 0 | 0 / 0 | 4.1e-6 | 0.182 | 1.1e-5 | 0.00075 |
| face join settled | 77 | 2318 + 2849 | 14 | 11.83 | 3.31 | 65 | 1.6e-6 | 4.0e-6 | 0 | 0 / 0 | 5.2e-6 | 0.00047 | 2.1e-6 | 1.3e-6 |

All six build; each also refuses when its generation changes. Peak is the worker's allocator delta (resident
meshes and the source are leased, not counted), under the 16 MiB per-worker cap. GPU MB is the group's expanded
size (band, rim, replaced patches), additional to the resident patches kept for fallback.

"Submitted" columns sample the ground as it would be drawn at the job eye (band, rim, replaced and untouched
patches; skirts excluded) with rays from the world's centre on a 1 m lattice over ±128 m: 65,536 rays each, no
hole and no double cover. Core contact is against the canonical visible surface. The transition band
(32–64 m) is not a walking claim; its parent maxima (9.4, 10.7 m) are the known parent-band errors. Outside the
square the drawn ground differs from the resident only where `ownership::correct` moved a fine boundary vertex
onto its coarser owner: in the parent snapshots that closes metre-wide T-junction cracks the skirts used to hide.
This is the coordinated renderer correction, not unchanged exterior geometry.

Not shown here: the group drawn, GPU upload and atomic publication, moving-eye validity (the seven-eye relation
check is CPU), shore sites, and the rim's appearance (its normals are its own; shading across the rim–patch edge
needs a look).
