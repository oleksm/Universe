# GPU residency: one frame, one allowance table, one memory report

`engine::residency` holds what the renderer keeps on the GPU, kind by kind (meshes, the mesh pool, PBR models,
globe maps, the near world's full maps):

- One frame counter: a frame is what `Renderer::render` draws, its views and all. Meshes' keep window, the globe
  layers' recency and the pool's coldness counted every view as a frame before (with views, a mesh unused for
  600 "frames" went in 600 / (views + 1)).
- One allowance a frame for each kind's uploads (PBR 8 MiB, globes 2 MiB, world maps 2 MiB; meshes and the pool
  made where they're drawn, counted), spent across the frame's views, not given again to each (the globes'
  was).
- A view shows the near world's full maps only if they're the set bound; it no longer uploads its own (two
  worlds in turn took the one bound set from each other, ~150 MiB a time).
- Each kind's bytes on the GPU, every frame: `Resources` adds the globe maps', the world maps' (in no count
  before) and the total (the observer's /resources). Hoar E's port scene: 1.29 GB (pool 1,024 MiB, globes 256,
  meshes 11).

Captures match (the planet app compared at the same moment); engine tests and the residency's own pass.
