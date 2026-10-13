# Bounded asynchronous asset preparation

The engine supplies a shared keyed CPU cache with worker preparation, coalesced requests, lease-aware eviction and load/hit/unload/reload counters. PBR texture mip generation and compression move to workers. GPU payload transfer is incremental, with a neutral bounds placeholder until completion; cold resources retire after submitted work completes.

`PbrModel::with_lease` retains a CPU cache owner across render commands, texture jobs and pending uploads. `Frame::keep_alive` retains canvas-only owners through frame consumption. Attach PBR leases to temporary draw clones, never to a model inside its own owner.

Initial scheduling and memory values are development tuning: GPU payload residency 512 MiB, 8 MiB transfers per frame, 1 MiB chunks and a soft 2 ms scheduling target. CPU reservations are estimates, not allocator limits; payload accounting excludes driver overhead. Predictive movement scheduling and startup model migration remain integration work. Physics and authored assets are unchanged.

Validation: engine unit regressions cover coalescing, priority, cancellation, cache leases, failures, texture preparation and render-owner lifetime. A separate Vulkan test exercises chunked BC and RGBA mip transfers. The window-model example recorded one CPU load and one GPU upload across repeated requests and a short hidden interval.

Registry build and all 167 workspace tests pass on the isolated loader branch.
