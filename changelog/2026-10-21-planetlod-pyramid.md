# Planet LOD streamed land pyramid

Read planet-forge's revised vertex-aligned pyramid beside a view. Select tiles by geometric
screen-space error, frustum and horizon; stream row ranges through a bounded worker, retain
resident parents until whole visible replacement subtrees are ready, and prefetch near splits.
Morph vertices onto actual parent triangles through one GPU weight per tile. Add exposed-edge
skirts; preserve mask holes and Shasta's existing pattern. Filter covered static triangles and
compact unused vertices before pattern snapshots when no legacy index overlays depend on them.

Pool draws now support an explicit morph weight and streamed eviction. Renderer residency
receipts no longer keep removed CPU tiles alive. The viewer reports land selection, cache,
streaming and fallback counts, and samples cached ground height under the camera.

Validation and captures: docs/diagnostics/planetlod-pyramid-20261021.json.
