# Stable registry and packing integration

Integrate the certified fso delivery `94cb9b0c` and packing candidate `0df4427c`
into local main. This brings in manufactured packaging and production-chain
records, package return/waste policy, current plant motion/mount definitions and
PlanetSim records, alongside existing runtime reserve/return, tare conservation,
package provenance saves and empty shell visuals.

The sole merge conflict was the generated registry page; it was regenerated from
the merged records. No registry values or engineering acceptance states were
invented during integration. Existing review flags and report gaps remain.

Audited 24 outstanding branch refs. Older branches whose patches already landed
were left alone. Stopped LOD experiments, historical YAML variants, staged visual
axes and the newly started spaceship-building workflow stay isolated. Owner
settings and untracked work were preserved. Exact heads and dispositions are in
`docs/diagnostics/stable-integration-20261020.json`.

Validation: registry generation passes. Twenty-eight distinct targeted regression
cases pass (30 executions): service/package conservation, save/load, real-model
empty visuals, plant layout/viewer, the celestial guard and 14 registry Python
checks. Rust tests use merged local standards without an external registry root.
Heavy checks ran one at a time with memory/time caps. No new GUI capture, release
installation or remote push is claimed.
