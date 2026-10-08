# Compiled pipelines kept between runs (branch ground-pass)

- `engine::pipecache`: wgpu's pipeline cache where the GPU and driver have one, opened with the
  device from the user's cache folder (`XDG_CACHE_HOME`, `~/.cache` or `LOCALAPPDATA`;
  `universe-engine/pipelines-<vendor>-<device>-<driver>.bin`), given to all eleven pipelines the
  engine makes and to a plugged-in ground's (`GroundSetup::cache`), written back after the first
  frame. wgpu checks the data belongs to this adapter and driver and starts empty when it doesn't.
- Measured on the planet bench with the driver's own shader cache off (as on a machine whose driver
  has none, or after it's cleared): 3.4 s to the first frame without it, 0.46 s with it. With
  NVIDIA's cache on, both are warm (~0.45 s).
