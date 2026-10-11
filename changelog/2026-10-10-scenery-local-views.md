# Local scenery view checkpoint

Added a bounded canonical target/channel occlusion audit and six matched Vulkan
captures for the proposed local high/low views. Both targets pass sampled
visibility; high-to-low includes source and native channels, reverse only native.
Absolute camera position and sun are fixed, host radius is6281370m. The neutral
terrain remains visually gentle. This is read-only development evidence; no
terrain or runtime appearance tuning. See docs/pgs1-local-view-review.md.
Example built and ran on both surfaces; all six captures inspected. Combined
workspace validation follows in the owner-requested main checkpoint.
