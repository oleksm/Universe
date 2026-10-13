# Optional mesh-group ownership

`MeshBudget::group(Vec<WireModel>)` reserves the complete expanded buffer estimate or returns all input models. The estimate matches unindexed 56-byte face/edge vertices and both 40-byte empty-buffer placeholders. The group reservation remains until its last CPU, frame, GPU cache or queued-retirement owner is gone. `live_bytes()` is a conservative reservation total, not actual resident memory; partial group retirement deliberately retains the whole reservation. `peak_bytes()` records successful reservation totals at their atomic admission point.

`Frame::prepare_mesh(&Mesh)` stages without drawing. `Mesh::gpu_resident()` reports presence in a renderer cache after buffer construction, not GPU fence completion or geometric validity. Multiple renderer caches retain separate receipts. A caller must stage the group in its own renderer and recheck source, selected mesh identities and generation before publication. This API neither certifies terrain seams nor freezes geomorphs.

Dropping a cache entry clears its receipt and holds its reservation in `Queue::on_submitted_work_done`; old submissions remain budgeted. CPU/group leases can hold it longer. `Resources::mesh_gpu_bytes` reports this renderer's cached untextured buffers; it does not include retired buffers. Use the reservation for the additional lifetime cap.

CPU reservation regression passes. GPU upload/publication/retirement validation remains pending the actual-resident terrain input gates. No default behavior, install or main merge is included.
