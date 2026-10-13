# Actual resident terrain contact adapter contract

Active task 20261013T130727-engine-bd9a. Isolated branch ships-contact-adapter from engine CPU pilot 6f3e894f; main stays untouched. This contract is proposed, not an integrated or accepted runtime.

Snapshot only after Lod's actual fallback-selected `shown` list is complete. Each entry is the real selected `Key`, immutable `Mesh.id`, cheap shared `Mesh` clone, body-frame origin and partial flag. Source identity includes system/body, GlobeMap.id, immutable Surface Arc, selected-set generation, radius, eye and exact morph parameters. No `ideal()` neighbours or sampled reconstruction stand in for absent resident meshes. Refuse partial, absent or overlapping ancestor/descendant support at publication unless the worker explicitly models the actual overlap.

For the world-owned worker, avoid an engine dependency and avoid copying vertex arrays. Proposed world trait:

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

The shared worker returns a typed complete corrected group: exact source/selection generation, input patch keys and IDs, replacement meshes and origins, any complete retained support replacements, verified CPU topology/core/join evidence, and explicit GPU group ownership. Every affected incident patch is part of one transaction. Corrected positions and morph must be frozen together; unchanged mesh IDs alone are insufficient if boundary morph is still changing.

Stage all replacement meshes using Frame::prepare_mesh. Readiness requires every Mesh::gpu_resident receipt, the current selected/input IDs and unchanged source/generation. Only then choose the whole group for this frame. Pending, failed, stale, missing-owner, changed-selection, budget refusal or partially uploaded groups keep the actual existing terrain draw selection and explicitly report not walk-ready. Never hide only some input patches. Geometry readiness must not reset or replace canonical physics.

Engine owns Terrain::surface_source, source/group worker, MeshBudget::group and GPU retirement lifetime. Ships owns terrain_lod snapshot, generation checks, atomic draw substitution and readiness adapter. GPU experiments remain held until final actual-resident CPU join gates pass. The earlier seven-site CPU pilot used settled ideal selected grids; it does not certify these runtime snapshots.
