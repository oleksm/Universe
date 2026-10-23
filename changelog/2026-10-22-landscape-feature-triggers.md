# Define the missing landscape triggers

The66 catalogue-only feature kinds with no simulator trigger now have explicit proposed predicates, required evidence and resolution/unknown handling. Feature-local `trigger_params` reuse the existing SI parameter type, so consumers can read detector settings by registry kind without changing existing physical process parameters or maintaining code hand lists.

The rules cover biotic cover/edges, marine morphology and realms, rivers/lakes/ice bodies, tectonic realms, peaks/passes, oasis, lava dome and geothermal field. They retain `built: false` and draft/review status. Annual-climate suitability is distinguished from confirmed biome identity; missing groundwater/seasonality/flotation or unresolved fine terrain remains unknown. Thresholds are open and unused until the actual consuming detectors read/calibrate them. Source definitions and chosen screening defaults are separated in docs/planet-feature-trigger-rules-2026-10-22.md.

The generic record page keeps tiny nonzero SI values in scientific notation, so rainfall and plate-speed thresholds no longer render as zero at six decimal places.
