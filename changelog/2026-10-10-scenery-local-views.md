# Local scenery view checkpoint

Added a bounded canonical target/channel occlusion audit and six matched Vulkan
captures for the proposed local high/low views. Both targets pass sampled
visibility; high-to-low includes source and native channels, reverse only native.
Absolute camera position and sun are fixed, host radius is6281370m. The neutral
terrain remains visually gentle. This is read-only development evidence; no
terrain or runtime appearance tuning. See docs/pgs1-local-view-review.md.
Example built and ran on both surfaces; all six captures inspected. Combined
workspace validation follows in the owner-requested main checkpoint.

Final captures calibrate launch altitudes from actual frame logs; paired actual
heights now agree within1um. Archived six PNGs/logs under the engine checkpoint.
Independent saved-array attribution review reproduced all114 flank counts and
local-departure RMS/correlations on1480/1310 samples, including vector32 to
baseline-plus-vector2 before solver. No independent native extraction claimed.
