# A frame cap, and the GPU's budgets

- 120 frames a second at most by default (was 240): smooth, and on the user's 240 Hz screen the
  GPU rests between frames instead of running flat out. `UNIVERSE_MAX_FPS` to change it.
- `UNIVERSE_GPU_BUDGET="scene=10,..."`: a screenshot run whose pass goes over its budget (ms)
  says so and exits 3. `UNIVERSE_SHOT_SIZE` (e.g. 3840x2160): a screenshot run's size (1920×1080
  by default).
- `tools/render/gpu_budget.sh`: four fixed views over Heath at 4K, each against the budgets (the
  scene 10 ms). Not in the test suite (seconds a view, the worlds store, a GPU). First run, at the
  default render scale: from 77 km 6.7 ms, 30 km 8.7, both within; over the sea from 14 km 16.7
  and over the range from 3 km 11.9, over. Without the clouds those two are 3.4 and 2.7: low
  down, a pixel is finer than the cloud cache's inner level and the clouds fall back to their
  noise. The lab's to tune.
