# v8 contact core and transition band — CPU refusal

This experiment explicitly changes readiness to a **64 × 64 m contact core inside the same 128 × 128 m footprint**. The transition band is never walk-ready. It does not reverse `f94d8f26`'s failed whole-footprint contact gate.

The implementation is still **not publishable**. Five of seven features exceed the unchanged 32,768-triangle limit. The shore and parent-join outputs fit the size limits and pass their sampled core-height checks, but independent edge incidence fails and physical f32 triangles include degenerate slivers. No GPU slice, movement or loading demonstration follows these failed prerequisites.

## Construction and scope

Frozen plan: `out/review/v8-contact-core/plan.json`. Source is unchanged `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`; host radius stays 6,281,370 m versus source 6,371,000 m. Physics and water ownership are unchanged.

The compiled example uses the canonical owner's adjacency search and reconstructs the retained cube-grid triangles from the existing terrain mesh rules: 16 cells per patch, settled terrain-aware selection through level 19, f32 local vertices and f32 parent morph. A fixed eye is placed at each feature centre's visible height +1.7 m. This is the retained **triangle geometry and morph arithmetic**, not a new analytic terrain approximation. It remains a CPU reconstruction of ideal settled selection, not a snapshot of the asynchronous renderer's actual residency. Broad cube nodes are culled in cube coordinates before local gnomonic clipping to avoid projecting irrelevant back-hemisphere corners.

The band intersects canonical face edges with retained triangle edges. At chart position `(x,y)`, define `wx=smoothstep(clamp((abs(x)-32)/32,0,1))`, similarly `wy`, and `t=1-(1-wx)*(1-wy)`. The radial position interpolates canonical visible height and the ray intersection with the retained triangle plane using `t`. This tensor-product taper is C1 as a function of chart coordinates; the piecewise height fields and their creases do not acquire a global smooth-normal guarantee.

The core uses canonical faces clipped to 8 m cells. Band split vertices on the inner boundary are inserted into core boundary edges, with interior-centroid triangulation to retain those splits. This avoids carrying the dense retained grid throughout the core. The earlier full-overlay shore attempt hit the triangle cap too; its result is retained as a superseded implementation diagnostic.

Water-bearing source faces use visible body level; dry faces use canonical bed height. The source API still exposes both separately. The independent audit classifies wet core samples separately; those samples never imply dry walkability.

## Results

| Feature | Result |
|---|---|
| Shore | 25,563 triangles, 14,812 vertices; size fits, topology/degenerate-triangle gates fail |
| Parent join | 26,444 triangles, 15,296 vertices; size fits, topology/degenerate-triangle gates fail |
| Sampling-window join | Triangle cap reached while adding the core |
| Relief/native join | Triangle cap reached in the band |
| Crest 1 | Triangle cap reached in the band |
| Crest 2 | Triangle cap reached in the band |
| Crest 3 | Triangle cap reached in the band |

All cap refusals occur at 32,768 emitted triangles. At the three crest refusals, only approximately 10,594 / 10,738 / 11,013 square metres have been emitted out of the 12,288-square-metre band; core geometry is still absent. These partial surfaces are not saved or published as complete meshes. No limits were expanded.

For the two completed CPU outputs, independent audits sample vertices, edge midpoints and triangle centroids, with each feature below the existing 300,000-query cap. The shore has 178,941 samples and the parent join 185,108. Core visible-height maximum errors are **2.037 and 2.199 micrometres** respectively. Their transition-band maximum canonical errors are **0.001906 and 0.000509 m**. These height results do not override topology or normal failures.

Outer emitted vertices differ from their supporting retained triangle positions by at most **4.131 and 4.251 micrometres**. This is a vertex-position result, not certification of a complete stitched outer interface. Edge incidence reports **2,931 and 3,036 unmatched/unexpected interior edges**. A geometric welding/splitting solution has not been implemented; small coordinate differences cannot simply be declared watertight. The audit also records **9 and 11 collapsed physical f32 triangles**, producing **63 and 77 nonfinite sampled normals**, explicitly; statistics separate finite and invalid samples instead of hiding them in a null maximum. No normal-quality acceptance follows.

The previous **0.209–0.226 m crest outer errors** remain recorded as distant-grid LOD errors. The new experiment permits such errors in the band, but cannot yet construct those crest bands within its chosen overlay complexity bound. This failure is not the old core-height requirement being silently applied to the band.

## Budget, validation and remaining work

The 16 MiB additional GPU lifetime ceiling is unchanged. Current completed-output expansion estimates are **4,294,624 and 4,442,632 bytes**, including three 56-byte GPU vertices per triangle and the empty 40-byte edge buffer. They are not GPU allocation measurements. Accounting would have to include active, uploading and retired-but-referenced allocations; no renderer reservation hook or upload was added here.

No renderer was launched: additional prototype GPU allocation and queued runtime output are zero; total renderer geometry memory is unmeasured. Process peak RSS is **1,680,676 KiB**, including strict reader/index construction. Peak transient per-worker allocation remains unverified against the 16 MiB CPU working-memory gate, another prerequisite before runtime use.

The next implementation needs both a less expansive band triangulation that preserves the required interfaces and robust shared-edge topology after f32 conversion. Neither increasing the triangle limit nor overlooking the failed edge audit is authorized. The geometry must pass those gates before the planned fixed GPU views and the 16 m anchor-trigger movement experiment. No feet/core-coverage, delayed-build, refused-build runtime display, cancellation, eviction/revisit or camera-transform claim is made here.

Source files: `crates/core/world/examples/pgs_contact_core.rs`, `contact_core/`, and `pgs_contact_audit.rs`. Artifacts: `out/review/v8-contact-core/`. Compact measurements and hashes: `docs/diagnostics/v8-contact-core.json`. No main merge, installation, default renderer change or new permanent unit tests.

Validation: **68 workspace tests pass**; registry build exits0; compiled fixture/audit examples run successfully. Geometry gate failures remain explicit measured results, not test-suite acceptance.
