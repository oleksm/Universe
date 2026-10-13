# v8 crease runtime preflight — bounded refusal

The compiled local geometry passes, but the **frozen 128 × 128 m footprint cannot yet be published against the unchanged surrounding grid**. Its outer boundary crosses canonical crests where the existing settled/morphed grid misses the source by 0.209–0.226 m. The unchanged sampled contact target is 0.10 m. This is a CPU publication refusal, **not a runtime or GPU pass**.

Source remains `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`. The lab retains Treistun E radius 6,281,370 m and canonical directions/heights; source radius is 6,371,000 m. No physics, registry, default LOD, installed binary or main change is made by this work.

## Compiled worker

`pgs_crease_worker` uses the strict Rust reader and the new immutable `Surface::face_geometry` copy API. Search starts at `Surface::query(centre).face`; fixture face IDs do not seed the clipping traversal. Only selection of the existing large-native-face control uses its frozen adjacent face IDs. The port retains the 4,096-face search limit, 32,768 vertices/triangles, 8 m clipping cells and 2 MiB serialized-output refusal. A generation token is checked during traversal, between face emissions and before returning. A stale-token control refuses. This is an isolated example, not a scheduler integration or a mid-flight cancellation test.

A first compiled shore build refused its 10 micrometre seam gate at **10.3005 micrometres**. Using the selected owner's mean corner height as the local origin was too distant on the large native face. Origins now use the canonical bed height at the queried centre. Geometry still evaluates each vertex's own face; wet faces emit body-level visible water, while the API retains bed heights separately. The unchanged seam gate passes after this correction. The old refusal is archived.

The eight compiled outputs repeat the independent canonical reader audit and saved topology check: **127,374 samples**, maximum **4.520 micrometres**, **49 frozen queries** within **2.252 micrometres**, zero centroid-owner differences and zero unexpected topology edges. Compiled worker costs are **0.100–0.641 ms/pair** in this run; strict decode/index takes **2.758 s**. Maximum retained output-vector capacity is **84,609 bytes** and maximum expanded GPU estimate is **310,712 bytes/pair**. Peak process RSS is **1,682,564 KiB** including strict source decoding/indexing. See the bound JSON report for exact costs, sample counts, owner checks and errors. Worker timing excludes source decoding, JSON serialization, GPU work and scheduling; this short warm in-process observation is not a frame-time guarantee. The original Python output is retained separately.

## Outer interface failure

The interface script extracts every outer mesh edge and samples its endpoints and fractions 0.1, 0.25, 0.5, 0.75 and 0.9. It queries the existing settled cube-grid/morph reconstruction with one fixed centre eye at visible height +1.7 m per feature. It does **not** move the eye to each boundary sample. All seven feature boundaries are checked; the large-face control is covered by the local geometry audit only.

| Feature | Maximum outer-grid contact error | Failing boundary samples |
|---|---:|---:|
| Shore | 0.001905 m | 0 |
| Parent join | 0.000469 m | 0 |
| Sampling-window join | 0.000001 m | 0 |
| Relief/native join | 0.000192 m | 0 |
| Crest 1 | 0.208885 m | 12 |
| Crest 2 | 0.204132 m | 4 |
| Crest 3 | 0.225633 m | 33 |

Worst witness: crest 3 chart position `(6.71982683, -64)` m, level 19, canonical bed/visible height **5492.599793570798 m**, existing grid **5492.374160711654 m**, dry. Crests 1 and 2 also fail at level 19, so this is not merely selecting a coarser outer LOD.

A transition strip conforming to that unchanged outer grid inherits the failed outer endpoint. Ending the accurate mesh there instead leaves a discontinuity. Overlapping both sheets is prohibited. This refusal applies to the chosen fixed footprint and unchanged outer grid, not to every possible crease-preserving renderer. A subsequent design needs a compatible replacement of the adjoining grid cells/ring, with a new declared interface and checks; silently increasing the footprint, uniform level or tolerance would invalidate this experiment.

These are finite CPU samples of settled ideal grid selection and f32 morph arithmetic. They do not certify asynchronous submitted geometry, GPU shader equivalence, normals/materials or an actual moving camera. They are sufficient to reject this boundary before implementing/uploading it.

## Budget and unfinished runtime gates

The preimplementation plan freezes **16 MiB additional GPU geometry across active, uploading and retired-but-referenced allocations**, plus the existing two-worker / 48-pending / 3,000-patch limits. Actual `GpuMesh` storage is unindexed: three **56-byte** attribute vertices per triangle, no index buffer. Each of the two empty edge buffers still occupies **40 bytes**. An initial accounting assumption of a 4-byte empty buffer was corrected before any upload, adding 72 bytes per pair; the ceiling stayed unchanged. Current output expansion sizes are reported, not GPU measurements.

Reservations must eventually follow the renderer allocation through destruction, including cached retired meshes and submitted frames; attaching them only to CPU mesh ownership is insufficient. That lifetime hook has **not** been implemented here because publication failed first. Retained CPU vector-capacity bytes and serialized bytes are reported separately; they are not a measured peak transient allocation. The 16 MiB per-pair transient working-memory gate remains unverified for runtime use. Strict-reader process peak RSS includes the resident source/tree and is recorded separately.

No prototype GPU buffers, upload jobs or completion queue were created: additional GPU allocation and queued output are zero. Total renderer GPU bytes are **not measured** because no renderer was launched. Fixed GPU captures, full measured camera transforms, atomic replacement, moving/walker contact, queue priority, cancellation during execution, eviction/revisit and presentation checks remain unrun. No fallback is labelled walk-ready. Prior loader/physics behavior is unchanged.

Artifacts: `out/review/v8-crease-runtime/` (plans, compiled geometry, independent audit, topology, interface probes/results, process resource measurement, source/binary hashes and check logs). `docs/diagnostics/v8-crease-runtime.json` binds the compact evidence. No install or merge.

Validation: **68 workspace tests pass**, registry build exits0, final compiled examples and all artifact checks pass. No new permanent unit tests were added.
