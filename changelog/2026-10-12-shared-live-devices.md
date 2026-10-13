Shared live Ship support for compiled electric fans, individual battery discharge/thermal limits and landing-leg capacity. Realized forces use existing control and physics. Landed manual firing now allocates thrust and releases only with sufficient realized lift. Device data is supplied by the compiler from registry records; no product numbers or registry records changed. Battery recharge/cooling and per-leg spring dynamics remain outside this adapter.

Avoid repeated balanced-authority solves for unchanged effective devices, and
skip those solves when no translation is requested. Exact physical inputs key a
single per-device-state cache; no resource, density or control approximation is
introduced. Added profiling of cold solves and exact invalidation checks.
