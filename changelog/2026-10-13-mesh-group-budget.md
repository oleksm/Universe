# Optional mesh-group reservations and upload staging

Added atomic whole-group GPU-buffer reservations for untextured meshes. CPU handles, frame staging, renderer cache entries and submitted retirement callbacks share the reservation. A cache receipt appears after both buffers are created; staging alone draws nothing. The existing default mesh path is unchanged. Cached mesh-buffer bytes are exposed separately from the reservation upper bound.

Canonical terrain can share its immutable surface with visual workers. This does not change terrain queries, physics, registry values or defaults. Planet contact-group publication and GPU validation remain separate pending work.
