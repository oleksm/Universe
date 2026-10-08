//! The engine's developer settings from the environment (`UNIVERSE_*`), read once at start, not
//! every frame (each read takes the process's environment lock).

/// The settings, as they were at start.
pub struct DevFlags {
    /// `UNIVERSE_HITCH_MS`: a frame slower than this is a hitch (s).
    pub hitch: Option<f32>,
    /// `UNIVERSE_SCREENSHOT_AT`, `UNIVERSE_SCREENSHOT_FRAMES`: a screenshot run's first frame, and
    /// how many after it.
    pub screenshot_at: u64,
    pub screenshot_frames: u64,
    /// `UNIVERSE_MAX_FPS`: the most frames a second, over the config's.
    pub max_fps: Option<f32>,
    /// `UNIVERSE_SHADOW_DEBUG`: what's in shadow tinted red.
    pub shadow_debug: bool,
}

/// The settings (read the first time asked).
pub fn get() -> &'static DevFlags {
    static FLAGS: std::sync::OnceLock<DevFlags> = std::sync::OnceLock::new();
    FLAGS.get_or_init(|| {
        let num = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
        DevFlags {
            hitch: num("UNIVERSE_HITCH_MS").map(|ms| ms as f32 / 1000.0),
            screenshot_at: num("UNIVERSE_SCREENSHOT_AT").map_or(120, |v| v as u64),
            screenshot_frames: num("UNIVERSE_SCREENSHOT_FRAMES").map_or(0, |v| v as u64),
            max_fps: num("UNIVERSE_MAX_FPS").map(|v| v as f32),
            shadow_debug: std::env::var_os("UNIVERSE_SHADOW_DEBUG").is_some(),
        }
    })
}
