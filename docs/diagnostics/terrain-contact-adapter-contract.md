# Actual resident terrain contact adapter contract

Active task 20261013T130727-engine-bd9a. Isolated branch ships-contact-adapter from engine CPU pilot 6f3e894f; main stays untouched. This contract is proposed, not an integrated or accepted runtime.

Snapshot only after Lod's actual fallback-selected `shown` list is complete. Each entry is the real selected `Key`, immutable `Mesh.id`, cheap shared `Mesh` clone, body-frame origin and partial flag. Source identity includes system/body, GlobeMap.id, immutable Surface Arc, selected-set generation, radius, eye and exact morph parameters. No `ideal()` neighbours or sampled reconstruction stand in for absent resident meshes. Refuse partial, absent or overlapping ancestor/descendant support at publication unless the worker explicitly models the actual overlap.

For the shared client worker (`universe-terrain-contact`, depending on world), avoid an engine dependency and avoid copying vertex arrays. The initial game bridge trait is:

```rust
pub trait RetainedMesh: Send + Sync {
    fn vertex_count(&self) -> usize;
    fn position(&self, index: usize) -> [f32; 3];
    fn morph(&self, index: usize) -> [f32; 4];
    fn faces(&self) -> &[[u32; 3]];
}
pub struct RetainedPatch {
    pub key: [u32; 4], // face, level, x, y
    pub mesh_id: u64,
    pub origin: [f64; 3],
    pub geometry: std::sync::Arc<dyn RetainedMesh>,
}
```

The game implements the trait with a retained engine Mesh clone. Accessors return actual positions/colors; faces() exposes only the 512 grid triangles, excluding skirts. It does not regenerate the grid or assume all shader offsets vanish. The snapshot array is bounded by the existing 3000 patch cap; shared geometry leases are existing resident data, not newly copied arrays. Worker still must meter metadata, temporary construction and returned geometry against the declared 16 MiB transient cap.

The shared worker returns a typed complete corrected group: exact source/selection generation, input patch keys and IDs, replacement meshes and origins, any complete retained support replacements, verified CPU topology/core/join evidence, and explicit GPU group ownership. Every affected incident patch is part of one transaction. Lock only boundary-crossed triangle vertices and the finite-owner endpoint/incident dependencies proven necessary by the worker. Publish every incident selected patch atomically. Unchanged exterior vertices retain their original morph; freezing whole support patches would open their other edges as the eye moves. Locked vertices must retain positive color.w footprint size while their delta xyz becomes zero; zero size makes the shader smoothstep undefined. New tiles also need a positive derived footprint size.

Stage all replacement meshes using Frame::prepare_mesh. Readiness requires every Mesh::gpu_resident receipt, the current selected/input IDs and unchanged source/generation. Only then choose the whole group for this frame. Pending, failed, stale, missing-owner, changed-selection, budget refusal or partially uploaded groups keep the actual existing terrain draw selection and explicitly report not walk-ready. Never hide only some input patches. Geometry readiness must not reset or replace canonical physics.

Engine owns Terrain::surface_source, source/group worker, MeshBudget::group and GPU retirement lifetime. Ships owns terrain_lod snapshot, generation checks, atomic draw substitution and readiness adapter. GPU experiments remain held until final actual-resident CPU join gates pass. The earlier seven-site CPU pilot used settled ideal selected grids; it does not certify these runtime snapshots.

## Delivered CPU foundation and remaining admission gap

The real drawing selector now feeds a retained snapshot without rebuilding neighbours. Four scoped terrain tests pass, including the external CPU export fixture. Six snapshots were produced: loading-parent and settled states for both canonical crests and the parent-face join. See [validation](terrain-contact-actual-snapshots.md).

Source/selection cancellation now excludes eye: the stamp binds system/body/map ID, radius, selected mesh IDs and retained immutable Surface Arc identity. Every snapshot keeps its own immutable recorded eye; jobs stay current across eye movement while unchanged selected geometry shares one Arc-backed metadata slice. Source/map replacement or remade/changed selected meshes cancels old jobs. Current selection alone is **not** publication admission: current-subject coverage and incident-lock/excision closure remain mandatory and unimplemented. Four scoped tests, including fresh six-output CPU fixtures, pass after this separation.

No corrected group is drawn, no native selection is suppressed, and no GPU walk-ready state is published. Shared engine hooks are available, but the worker's actual-input join and dependency closure gates remain prerequisites to the consumer.

Publication must also account for the full patch topology, including skirt-top aliases and skirt triangles in or crossing the replaced footprint. The 512 grid-face CPU contact bridge is insufficient evidence for suppression or for GPU geometry budgeting. Replacement must preserve aliases to locked grid vertices, excise all affected original triangles, and count actual full mesh faces rather than the grid-only lower bound. An overlay on the existing patches is not an accepted replacement.

## Shared bridge integration

The game now directly implements the committed shared client `RetainedMesh`; `Snapshot::worker_input` emits complete `Binding` metadata and nearby strongly retained `RetainedPatch` wrappers. No vertex arrays are copied and no helper implementation is duplicated. All six fresh CPU cases pass shared conversion. The full snapshot retains all actual selected meshes if a later closure traversal requires further incident owners. Only the nearby subset is currently passed to the contact conversion; its fixed export ring does not certify closure. Live worker admission/scheduling, mesh group staging and draw substitution remain held.

| Consumer step | State |
|---|---|
| Actual fallback-selected snapshot and complete selected identity | Implemented, CPU fixture tested |
| Retained physical source and source/selection cancellation | Implemented; movement preserves job eye and leases |
| Shared client retained-geometry bridge | Implemented; six actual conversions pass |
| Corrected typed worker flow with verified reverse closure | Pending engine closure handoff |
| Full exterior/skirt excision and alias coordination | Pending |
| Complete group GPU preparation and atomic draw substitution | Held pending CPU proof |
| Moving subject core coverage and per-frame ready trace | Pending; current traces are CPU-only/not-ready |

Dependencies on this branch: engine hooks `792f2bac`, actual-input reader `a4ae5ea9`, shared client worker `186e4be5`, and game snapshot/source-generation commits `104470d8` + `b0ec1e8b`. No runtime publication gate has been relaxed.
