# What the GPU spends, and a render scale for 4K

- The GPU's own time a pass, stamped where it can (`TIMESTAMP_QUERY`; `engine::gputime`): the
  shadow cascades, the scene, the front layer, the HUD and the upscale, smoothed, on the F3 panel
  ("GPU 9.3: SHADOWS 0.0 SCENE 9.2 ...") and logged at a screenshot run's end.
- Measured over Heath from 77 km at 1080p (the lab's clouds as merged): the scene 7.8 ms, of
  which the clouds over the ground 7.5 (off: 0.2), their shadow on it 1.3, the air 0.2; shadows,
  HUD and upscale under 0.1 together. At 4K that is about four times: the user's GPU-bound frames.
- The scene is drawn at a render scale (`UNIVERSE_RENDER_SCALE`, 0.25–1; by default as much as
  keeps it to 1440 lines: 0.67 on a 4K screen) and upscaled; the HUD and the composite stay at the
  screen's full resolution. 1080p, the same view: 9.2 ms at full scale, 4.1 at 0.67.
- `planet` merged (the lab's clouds: translucent edges, relief shading, lighter cirrus, the air
  marched once a pixel; the low shell at the condensation level over the ground). The lab's ground
  detail generator came with it, held off (`detail::ACTIVE` false) until it's been looked at on
  screen after the overnight bake.
