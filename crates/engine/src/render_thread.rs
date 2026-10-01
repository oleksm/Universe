//! The render thread: it owns the GPU (device, surface, renderer), takes
//! finished frames — the draw lists the game built — and uploads, submits
//! and presents them, so the main thread (window, input, the game's update
//! and drawing) goes on to the next frame meanwhile.
//!
//! A finished frame goes in a one-frame slot. While one is still there, the
//! main thread waits for the render thread to take it, so it builds frames
//! at the pace they're shown, but only for so long (`PATIENCE`): while the
//! window is hidden or unfocused the compositor may stop handing out
//! surfaces for as long as it likes, and the main thread must keep answering
//! it meanwhile, or it's judged hung. Then the newer frame replaces the old.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use glam::UVec2;

use crate::frame::Frame;
use crate::gpu::Gpu;
use crate::renderer::Renderer;

/// What's waiting for the render thread: the newest frame (and a screenshot
/// to save from it), a resize, and whether to stop once it's done.
#[derive(Default)]
struct Slot {
    frame: Option<(Box<Frame>, Option<PathBuf>)>,
    resize: Option<(u32, u32)>,
    stop: bool,
}

/// What the render thread tells the main thread: the framebuffer sizes
/// (they change on a resize), and its last frame's timings (ms).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RenderState {
    pub low_res: UVec2,
    pub hud_size: UVec2,
    pub render_ms: f32,
    pub wait_ms: f32,
}

/// The longest the main thread waits for the render thread to take a frame.
const PATIENCE: std::time::Duration = std::time::Duration::from_millis(50);

pub(crate) struct RenderThread {
    slot: Arc<(Mutex<Slot>, Condvar)>,
    state: Arc<Mutex<RenderState>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl RenderThread {
    pub fn start(mut gpu: Gpu, mut renderer: Renderer) -> Self {
        let state = Arc::new(Mutex::new(RenderState { low_res: renderer.low_res(), hud_size: renderer.hud_size(), ..Default::default() }));
        let slot: Arc<(Mutex<Slot>, Condvar)> = Default::default();
        let (shared, waiting) = (state.clone(), slot.clone());
        let thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || loop {
                let (resize, frame) = {
                    let (m, ready) = &*waiting;
                    let mut s = lock(m);
                    while s.frame.is_none() && s.resize.is_none() && !s.stop {
                        s = ready.wait(s).unwrap_or_else(|e| e.into_inner());
                    }
                    if s.frame.is_none() && s.resize.is_none() {
                        return;
                    }
                    let taken = (s.resize.take(), s.frame.take());
                    // (The main thread may be waiting for the slot.)
                    ready.notify_all();
                    taken
                };
                if let Some((w, h)) = resize {
                    gpu.resize(w, h);
                    renderer.resize(&gpu);
                    let mut s = lock(&shared);
                    (s.low_res, s.hud_size) = (renderer.low_res(), renderer.hud_size());
                }
                if let Some((frame, capture)) = frame {
                    let start = std::time::Instant::now();
                    {
                        let _p = universe_prof::scope("render");
                        renderer.render(&mut gpu, &frame, capture.as_deref());
                    }
                    universe_prof::add("render/wait for the surface (vsync)", renderer.wait.as_secs_f64());
                    let mut s = lock(&shared);
                    s.wait_ms = renderer.wait.as_secs_f32() * 1000.0;
                    s.render_ms = start.elapsed().as_secs_f32() * 1000.0 - s.wait_ms;
                }
            })
            .expect("render thread");
        RenderThread { slot, state, thread: Some(thread) }
    }

    /// Hand over a finished frame, replacing one not yet taken (its
    /// screenshot, if it had one, goes with the new frame). Never waits.
    pub fn frame(&self, frame: Frame, capture: Option<PathBuf>) {
        let (m, ready) = &*self.slot;
        let mut s = lock(m);
        // Paced by the render thread: wait for the last frame to be taken,
        // within patience (a frame to be saved is never replaced, though).
        let start = std::time::Instant::now();
        while let Some((_, c)) = &s.frame {
            let saving = c.is_some() && capture.is_some();
            let left = PATIENCE.saturating_sub(start.elapsed());
            if left.is_zero() && !saving {
                break;
            }
            let wait = if saving { std::time::Duration::from_millis(5) } else { left };
            s = ready.wait_timeout(s, wait).unwrap_or_else(|e| e.into_inner()).0;
        }
        let capture = capture.or_else(|| s.frame.take().and_then(|(_, c)| c));
        s.frame = Some((Box::new(frame), capture));
        ready.notify_one();
    }

    pub fn resize(&self, width: u32, height: u32) {
        let (m, ready) = &*self.slot;
        lock(m).resize = Some((width, height));
        ready.notify_one();
    }

    pub fn state(&self) -> RenderState {
        *lock(&self.state)
    }
}

impl Drop for RenderThread {
    /// Finish what's waiting (a screenshot, say), then stop.
    fn drop(&mut self) {
        let (m, ready) = &*self.slot;
        lock(m).stop = true;
        ready.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
