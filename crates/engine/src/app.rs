use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glam::{UVec2, Vec2};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::audio::Audio;
use crate::camera::Camera;
use crate::frame::Frame;
use crate::gpu::Gpu;
use crate::input::Input;
use crate::render_thread::RenderThread;
use crate::renderer::Renderer;

pub struct Config {
    pub title: String,
    pub window_size: (u32, u32),
    /// Vertical resolution of the retro framebuffer; width follows the window aspect.
    pub low_res_height: u32,
    /// The HUD is drawn at this multiple of the scene resolution (smaller, crisper text).
    pub hud_scale: u32,
    pub vsync: bool,
    /// The most frames a second (0: as many as the display takes);
    /// `UNIVERSE_MAX_FPS` overrides it.
    pub max_fps: f32,
    /// Where slow frames are written down (see `HITCH`): the frame, its
    /// parts, and with the profiler on, every scope's time in it. None: the log only.
    pub hitch_log: Option<std::path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self { title: "Freefall".into(), window_size: (1440, 810), low_res_height: 540, hud_scale: 1, vsync: true, max_fps: 240.0, hitch_log: None }
    }
}

/// Where the last frames' time went (ms, smoothed) and what they drew.
#[derive(Clone, Debug, Default)]
pub struct Perf {
    /// Whole frame, start to start.
    pub frame_ms: f32,
    /// `Game::update` (the game's own tick, simulation included).
    pub update_ms: f32,
    /// `Game::draw`: building the draw lists.
    pub draw_ms: f32,
    /// Uploading and submitting to the GPU.
    pub render_ms: f32,
    /// Waiting for the next surface image (vsync): idle.
    pub wait_ms: f32,
    pub lines: u32,
    pub triangles: u32,
    pub points: u32,
    /// The last frames' times (ms, raw), oldest first (`HISTORY` of them).
    pub history: std::collections::VecDeque<f32>,
    /// Frames slower than `HITCH` since the start.
    pub hitches: u32,
    /// What the renderer holds (see `Resources`).
    pub resources: Resources,
}

/// What the renderer holds on the GPU, as of its last frame.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Resources {
    /// Meshes uploaded (kept while drawn lately), and the instances drawn last frame.
    pub meshes: usize,
    pub instances: usize,
    /// Textured (glTF) models uploaded.
    pub models: usize,
    /// Worlds' surface maps in GPU layers, of the layers there are.
    pub globe_layers: usize,
    pub globe_capacity: usize,
}

/// Frames kept in `Perf::history`.
pub const HISTORY: usize = 300;
/// The hitch log's size (bytes) before it starts over.
pub const HITCH_LOG_MAX: u64 = 1 << 20;
/// A frame this slow (s) is a hitch: written down (see `Config::hitch_log`).
pub const HITCH: f32 = 0.05;

impl Perf {
    fn smooth(old: f32, new: f32) -> f32 {
        if old == 0.0 { new } else { old + (new - old) * 0.05 }
    }
}

/// What the game sees each frame: input, timing, and a few window controls.
pub struct Context {
    pub input: Input,
    /// Slow frames written down (the log and `Config::hitch_log`): the game
    /// sets it while its debug view is on; off, they're only counted.
    pub watch_hitches: bool,
    /// Seconds since the last frame (clamped to avoid huge steps after stalls).
    pub dt: f32,
    /// Seconds since start.
    pub time: f64,
    pub fps: f32,
    /// Frame timing breakdown (see `Perf`).
    pub perf: Perf,
    /// Size of the retro framebuffer in pixels.
    pub low_res: UVec2,
    /// Size of the HUD's layout in pixels (what `Input::cursor` is in).
    pub hud_size: UVec2,
    window: Arc<Window>,
    audio: Option<Audio>,
    cursor_grabbed: bool,
    screenshot: Option<PathBuf>,
    exit: bool,
}

impl Context {
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    /// Sound output, if an audio device is available.
    pub fn audio(&self) -> Option<&Audio> {
        self.audio.as_ref()
    }

    pub fn cursor_grabbed(&self) -> bool {
        self.cursor_grabbed
    }

    /// Hide and lock the cursor for mouse-look, or release it.
    pub fn grab_cursor(&mut self, grab: bool) {
        if grab == self.cursor_grabbed {
            return;
        }
        let result = if grab {
            self.window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined))
        } else {
            self.window.set_cursor_grab(CursorGrabMode::None)
        };
        if let Err(e) = result {
            log::warn!("cursor grab failed: {e}");
        }
        self.window.set_cursor_visible(!grab);
        self.cursor_grabbed = grab;
    }

    /// Save the next rendered frame (at native low resolution) as a PNG.
    pub fn screenshot(&mut self, path: impl Into<PathBuf>) {
        self.screenshot = Some(path.into());
    }

    pub fn exit(&mut self) {
        self.exit = true;
    }
}

pub trait Game {
    /// Advance game state. Called once per frame before drawing.
    fn update(&mut self, ctx: &mut Context);
    /// The camera to render from this frame.
    fn camera(&self) -> Camera;
    /// Fill the frame's draw lists.
    fn draw(&self, frame: &mut Frame, ctx: &Context);
}

struct Running {
    /// The GPU's own thread (see `render_thread`).
    render: RenderThread,
    ctx: Context,
    last_frame: Instant,
    fps_timer: f32,
    fps_frames: u32,
    frame_count: u64,
    /// The last frame's update, draw and hand-over (ms), for a hitch's report.
    last_parts: (f32, f32, f32),
    /// `UNIVERSE_SCREENSHOT=path`: capture a frame shortly after startup, then exit.
    auto_screenshot: Option<PathBuf>,
}

struct Runner<G> {
    config: Config,
    game: G,
    state: Option<Running>,
    start: Instant,
}

pub fn run<G: Game>(config: Config, game: G) {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner { config, game, state: None, start: Instant::now() };
    event_loop.run_app(&mut runner).expect("event loop error");
}

impl<G: Game> Runner<G> {
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(s) = &mut self.state else { return };
        let now = Instant::now();
        let raw_dt = (now - s.last_frame).as_secs_f32();
        s.last_frame = now;

        s.fps_timer += raw_dt;
        s.fps_frames += 1;
        if s.fps_timer >= 0.5 {
            s.ctx.fps = s.fps_frames as f32 / s.fps_timer;
            s.fps_timer = 0.0;
            s.fps_frames = 0;
        }

        s.ctx.dt = raw_dt.min(0.1);
        // The frame times; a slow one written down, with what the frame before it did.
        let p = &mut s.ctx.perf;
        p.history.push_back(raw_dt * 1000.0);
        while p.history.len() > HISTORY {
            p.history.pop_front();
        }
        // (UNIVERSE_HITCH_MS: a lower bar, to catch the small dips too.)
        let bar = std::env::var("UNIVERSE_HITCH_MS").ok().and_then(|v| v.parse::<f32>().ok()).map_or(HITCH, |ms| ms / 1000.0);
        if raw_dt > bar && s.frame_count > 60 {
            p.hitches += 1;
        }
        if raw_dt > bar && s.frame_count > 60 && s.ctx.watch_hitches {
            let (u, d, h) = s.last_parts;
            let rs = s.render.state();
            let mut text = format!(
                "hitch: frame {} took {:.1} ms (update {:.1}, draw {:.1}, hand-over {:.1}; render thread {:.1}, waiting for the screen {:.1})\n",
                s.frame_count, raw_dt * 1000.0, u, d, h, rs.render_ms, rs.wait_ms
            );
            for (name, ms, calls) in universe_prof::last_frame().iter().filter(|x| x.1 >= 0.5).take(30) {
                text += &format!("    {ms:>8.2} ms  {name}  ({calls}x)\n");
            }
            log::warn!("{}", text.trim_end());
            if let Some(path) = &self.config.hitch_log {
                use std::io::Write;
                // (Kept small: past `HITCH_LOG_MAX` it starts over, the last one kept as .old.)
                if std::fs::metadata(path).is_ok_and(|m| m.len() > HITCH_LOG_MAX) {
                    let _ = std::fs::rename(path, path.with_extension("log.old"));
                }
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{} {text}", chrono_stamp());
                }
            }
        }
        s.ctx.time = (now - self.start).as_secs_f64();
        let rs = s.render.state();
        s.ctx.low_res = rs.low_res;
        s.ctx.hud_size = rs.hud_size;

        s.frame_count += 1;
        // UNIVERSE_SCREENSHOT_FRAMES=n: also the n frames after it (name_1.png…),
        // to compare frame to frame.
        // (UNIVERSE_SCREENSHOT_AT=n: the first at frame n, not 120: a later moment without every
        // frame before it written down.)
        let extra: u64 = std::env::var("UNIVERSE_SCREENSHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let first: u64 = std::env::var("UNIVERSE_SCREENSHOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
        let shot = s.frame_count >= first && s.frame_count <= first + extra && s.auto_screenshot.is_some();
        let auto_capture = s.frame_count == first + extra && s.auto_screenshot.is_some();
        if shot {
            let k = s.frame_count - first;
            s.ctx.screenshot = s.auto_screenshot.clone().map(|p| {
                if k == 0 {
                    p
                } else {
                    p.with_file_name(format!("{}_{k}.png", p.file_stem().unwrap_or_default().to_string_lossy()))
                }
            });
        }

        let t0 = Instant::now();
        let update = universe_prof::scope("update");
        self.game.update(&mut s.ctx);
        drop(update);
        s.ctx.input.end_frame();
        if s.ctx.exit {
            event_loop.exit();
            return;
        }

        let t1 = Instant::now();
        let mut frame = Frame::new(self.game.camera(), rs.low_res.as_vec2(), rs.hud_size.as_vec2());
        {
            let _p = universe_prof::scope("draw");
            self.game.draw(&mut frame, &s.ctx);
        }
        let t2 = Instant::now();
        let capture = s.ctx.screenshot.take();
        let counts = frame.counts();
        // Off to the render thread (waiting only if the last is still queued).
        {
            let _p = universe_prof::scope("hand over to the render thread");
            s.render.frame(frame, capture);
        }
        universe_prof::frame_end();

        let ms = |d: std::time::Duration| d.as_secs_f32() * 1000.0;
        s.last_parts = (ms(t1 - t0), ms(t2 - t1), ms(Instant::now() - t2));
        let p = &mut s.ctx.perf;
        p.frame_ms = Perf::smooth(p.frame_ms, raw_dt * 1000.0);
        p.update_ms = Perf::smooth(p.update_ms, ms(t1 - t0));
        p.draw_ms = Perf::smooth(p.draw_ms, ms(t2 - t1));
        p.render_ms = Perf::smooth(p.render_ms, rs.render_ms);
        p.wait_ms = Perf::smooth(p.wait_ms, rs.wait_ms);
        p.resources = rs.resources;
        (p.lines, p.triangles, p.points) = counts;
        if auto_capture {
            let elapsed = (now - self.start).as_secs_f64();
            log::info!("{} frames in {elapsed:.2}s ({:.2} ms/frame)", s.frame_count, elapsed * 1000.0 / s.frame_count as f64);
            if universe_prof::enabled() {
                log::info!("profile (last {} frames):\n{}", universe_prof::WINDOW, universe_prof::report_text());
            }
            event_loop.exit();
        }
    }
}

impl<G: Game> ApplicationHandler for Runner<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let (w, h) = self.config.window_size;
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&self.config.title)
                        .with_inner_size(LogicalSize::new(w, h)),
                )
                .expect("failed to create window"),
        );
        let gpu = Gpu::new(window.clone(), event_loop.owned_display_handle(), self.config.vsync);
        // Screenshot runs render 16:9 regardless of how the window manager sized us.
        let screenshot_run = std::env::var_os("UNIVERSE_SCREENSHOT").is_some();
        // UNIVERSE_PROFILE=1: profile from the start (F3 in the game toggles it too).
        if std::env::var_os("UNIVERSE_PROFILE").is_some() {
            universe_prof::enable(true);
        }
        let renderer = Renderer::new(&gpu, self.config.low_res_height, self.config.hud_scale, screenshot_run.then_some(16.0 / 9.0));
        let ctx = Context {
            input: Input::default(),
            watch_hitches: false,
            dt: 0.0,
            time: 0.0,
            fps: 0.0,
            perf: Perf::default(),
            low_res: renderer.low_res(),
            hud_size: UVec2::ONE,
            window,
            // Automated screenshot runs stay silent.
            audio: if std::env::var_os("UNIVERSE_SCREENSHOT").is_some() { None } else { Audio::new() },
            cursor_grabbed: false,
            screenshot: None,
            exit: false,
        };
        self.state = Some(Running {
            render: RenderThread::start(gpu, renderer),
            ctx,
            last_frame: Instant::now(),
            fps_timer: 0.0,
            fps_frames: 0,
            frame_count: 0,
            last_parts: (0.0, 0.0, 0.0),
            auto_screenshot: std::env::var_os("UNIVERSE_SCREENSHOT").map(PathBuf::from),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(s) = &mut self.state else { return };
        // Automated screenshot runs ignore the keyboard and mouse, so a stray
        // keypress into the popped-up window can't change what gets captured.
        let scripted = s.auto_screenshot.is_some();
        match event {
            WindowEvent::KeyboardInput { .. } | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } if scripted => {}
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => s.render.resize(size.width, size.height),
            WindowEvent::Focused(false) => {
                s.ctx.input.release_all();
                s.ctx.grab_cursor(false);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    s.ctx.input.key(code, event.state == ElementState::Pressed);
                }
                if event.state == ElementState::Pressed
                    && let Some(text) = &event.text
                {
                    s.ctx.input.typed.extend(text.chars().filter(|c| !c.is_control()));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                s.ctx.input.button(button, state == ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                // (In the HUD's pixels: the window stretched over its layout.)
                let (win, hud) = (s.ctx.window.inner_size(), s.render.state().hud_size);
                s.ctx.input.cursor = Vec2::new(position.x as f32 / win.width.max(1) as f32 * hud.x as f32, position.y as f32 / win.height.max(1) as f32 * hud.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                s.ctx.input.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (Some(s), DeviceEvent::MouseMotion { delta }) = (&mut self.state, event)
            && s.auto_screenshot.is_none()
        {
            s.ctx.input.mouse_delta += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(s) = &self.state else { return };
        // At most `max_fps`: early, wait (events still come in) till the next frame's due.
        let max_fps = std::env::var("UNIVERSE_MAX_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(self.config.max_fps);
        if max_fps > 0.0 {
            let due = s.last_frame + std::time::Duration::from_secs_f32(1.0 / max_fps);
            if Instant::now() < due {
                event_loop.set_control_flow(ControlFlow::WaitUntil(due));
                return;
            }
        }
        event_loop.set_control_flow(ControlFlow::Poll);
        s.ctx.window.request_redraw();
    }
}

/// Seconds since the Unix epoch, for the hitch log (no calendar: no dependency).
fn chrono_stamp() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    format!("[{t:.3}]")
}
