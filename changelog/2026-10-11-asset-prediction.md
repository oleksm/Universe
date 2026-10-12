# Predictive asset demand and bounded decoding

World visual requests now anticipate swept relative motion in wall-clock time, include nearby/rear coverage and widen with camera turns. Scene changes discard queued speculation. Scheduling refreshes priorities, learns preparation latency, prefers cheaper work at similar urgency, and can use remaining memory while a larger request waits on pinned assets.

The profiler adds a bounded 512-event preparation/eviction history and separate GPU ready hits. GPU transfer order considers current visibility, projected importance, transfer cost and starvation. Registry and native models share one cache; native decode options form part of the key and the resulting source hash is retained for mount overlays.

Package reads are bounded before allocation. Embedded GLB decoding now checks image dimensions, aggregate geometry/image expansion, external references and hierarchy depth/cycles before decoding. Development limits: 64 MiB GLB input, 8192 pixels per image dimension, 128 MiB conservative expanded payload estimate. These are logical allocation safeguards, not process-RSS or driver-memory guarantees. Native paths are immutable per process; authored registry packages remain hash-bound.

The five-second lookahead, 100 ms urgency grouping and geometric visibility ranges are development tuning. They do not change physics, collision, saved transforms or authored models.
