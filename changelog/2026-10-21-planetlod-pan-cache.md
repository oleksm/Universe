# Keep pyramid terrain warm during flight

The diagnostic pan added a longitude offset every frame, so its route changed with FPS and
crossed over 100 degrees at 120 fps. It left the supplied continent; empty selections corresponded
to views with no pyramid ground. Pan now oscillates locally ±0.25 degrees about its starting
longitude using elapsed seconds, and logs include the camera coordinates.

The cache also expired every unused tile after 120 updates, even below its 256 MiB target.
Remove that expiry: keep reusable geometry until memory pressure needs its space, evict by least
recent use, and retain the coarse root cover. Returning views can reuse their cached GPU tiles.

Regression tests cover frame-independent pan bounds, long absences without eviction, least-recent
use under pressure, root retention, and 451 actual-package camera samples across 45 seconds.
Native route validation and coverage analysis: docs/diagnostics/planetlod-pan-cache-20261021.json.
