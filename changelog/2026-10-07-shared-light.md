# One shadow lookup for scenes and models (audit phase 4, #10)

- The sun through the shadow map (`sunlit`, its ground and near cascades, the 3×3 compared
  samples, the texel) was written twice, line for line, in scene.wgsl and pbr.wgsl: it's
  `shaders/light.wgsl` now, concatenated into both modules (pbr.wgsl's built the same way as the
  scene's, no longer `include_wgsl!`). Frames of the docked deck and of Heath's range in evening
  sun look the same before and after (pixel counts differ as much between two runs of one build:
  the world moves).
