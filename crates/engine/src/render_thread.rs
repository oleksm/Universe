//! The render thread: it owns the GPU (device, surface, renderer), takes
//! finished frames — the draw lists the game built — and uploads, submits
//! and presents them, so the main thread (window, input, the game's update
//! and drawing) goes on to the next frame meanwhile. One frame can wait in
//! between; a second blocks the sender until the first is taken.

use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};

use glam::UVec2;

use crate::frame::Frame;
use crate::gpu::Gpu;
use crate::renderer::Renderer;

enum ToRender {
    /// A frame to show (and save as a screenshot, if a path is given).
    Frame(Box<Frame>, Option<PathBuf>),
    Resize(u32, u32),
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

pub(crate) struct RenderThread {
    tx: Option<SyncSender<ToRender>>,
    state: Arc<Mutex<RenderState>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RenderThread {
    pub fn start(mut gpu: Gpu, mut renderer: Renderer) -> Self {
        let state = Arc::new(Mutex::new(RenderState { low_res: renderer.low_res(), hud_size: renderer.hud_size(), ..Default::default() }));
        let (tx, rx) = sync_channel::<ToRender>(1);
        let shared = state.clone();
        let thread = std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                while let Ok(m) = rx.recv() {
                    match m {
                        ToRender::Frame(frame, capture) => {
                            let start = std::time::Instant::now();
                            {
                                let _p = universe_prof::scope("render");
                                renderer.render(&mut gpu, &frame, capture.as_deref());
                            }
                            universe_prof::add("render/wait for the surface (vsync)", renderer.wait.as_secs_f64());
                            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                            s.wait_ms = renderer.wait.as_secs_f32() * 1000.0;
                            s.render_ms = start.elapsed().as_secs_f32() * 1000.0 - s.wait_ms;
                        }
                        ToRender::Resize(w, h) => {
                            gpu.resize(w, h);
                            renderer.resize(&gpu);
                            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                            (s.low_res, s.hud_size) = (renderer.low_res(), renderer.hud_size());
                        }
                    }
                }
            })
            .expect("render thread");
        RenderThread { tx: Some(tx), state, thread: Some(thread) }
    }

    /// Hand over a finished frame (waits while one is still queued).
    pub fn frame(&self, frame: Frame, capture: Option<PathBuf>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(ToRender::Frame(Box::new(frame), capture));
        }
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(ToRender::Resize(width, height));
        }
    }

    pub fn state(&self) -> RenderState {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for RenderThread {
    /// Finish what's queued (a screenshot, say), then stop.
    fn drop(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
