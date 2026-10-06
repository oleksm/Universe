//! What each pass costs the GPU (ms), from timestamps the GPU writes at each pass's start and end
//! (where it can: `wgpu::Features::TIMESTAMP_QUERY`), read back a frame or two later and smoothed:
//! the time the GPU itself spends, which the frame's wait for the screen only shows summed.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The passes timed, in the order drawn (a shadow pass a cascade: `SHADOW_PASSES`).
pub const PASSES: [&str; 5] = ["shadows", "scene", "front", "hud", "upscale"];
const SHADOW_PASSES: u32 = 4;
/// Stamps a frame: a start and an end a pass (each shadow cascade's own).
const STAMPS: u32 = 2 * (SHADOW_PASSES + PASSES.len() as u32 - 1);
const SLOTS: usize = 3;

/// The latest smoothed times (ms), by `PASSES`; empty where the GPU can't time.
static TIMES: Mutex<Vec<f32>> = Mutex::new(Vec::new());

/// Each pass's GPU time (ms), smoothed over frames: (name, ms). Empty where not timed.
pub fn times() -> Vec<(&'static str, f32)> {
    let t = TIMES.lock().unwrap_or_else(|e| e.into_inner());
    PASSES.iter().copied().zip(t.iter().copied()).collect()
}

pub(crate) struct GpuTime {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    /// Read-back buffers, each with whether it's mapped and ready, and whether it's in use.
    staging: Vec<(wgpu::Buffer, Arc<AtomicBool>, bool)>,
    next: usize,
    copied: Option<usize>,
    /// Nanoseconds a tick.
    period: f32,
}

impl GpuTime {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("pass times"), ty: wgpu::QueryType::Timestamp, count: STAMPS });
        let bytes = STAMPS as u64 * 8;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor { label: Some("pass times"), size: bytes, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let staging = (0..SLOTS).map(|_| (device.create_buffer(&wgpu::BufferDescriptor { label: Some("pass times readback"), size: bytes, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), Arc::new(AtomicBool::new(false)), false)).collect();
        Some(GpuTime { set, resolve, staging, next: 0, copied: None, period: queue.get_timestamp_period() })
    }

    /// The stamps for shadow cascade `k`'s pass.
    pub fn shadow(&self, k: u32) -> wgpu::RenderPassTimestampWrites<'_> {
        self.writes(2 * k)
    }

    /// The stamps for `PASSES[pass]` (past the shadows).
    pub fn pass(&self, pass: usize) -> wgpu::RenderPassTimestampWrites<'_> {
        self.writes(2 * (SHADOW_PASSES + pass as u32 - 1))
    }

    fn writes(&self, at: u32) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites { query_set: &self.set, beginning_of_pass_write_index: Some(at), end_of_pass_write_index: Some(at + 1) }
    }

    /// This frame's stamps resolved and copied out (if a read-back is free).
    pub fn copy_out(&mut self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.set, 0..STAMPS, &self.resolve, 0);
        let slot = self.next;
        if self.staging[slot].2 {
            return;
        }
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.staging[slot].0, 0, STAMPS as u64 * 8);
        self.copied = Some(slot);
        self.next = (slot + 1) % SLOTS;
    }

    /// Once submitted: the copy mapped, to be read when it's done.
    pub fn submitted(&mut self) {
        let Some(slot) = self.copied.take() else { return };
        let (buffer, ready, busy) = &mut self.staging[slot];
        *busy = true;
        let ready = ready.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| ready.store(r.is_ok(), Ordering::Release));
    }

    /// Read-backs done since: into `times`, smoothed.
    pub fn collect(&mut self) {
        for (buffer, ready, busy) in &mut self.staging {
            if !(*busy && ready.load(Ordering::Acquire)) {
                continue;
            }
            if let Ok(data) = buffer.slice(..).get_mapped_range() {
                let t: &[u64] = bytemuck::cast_slice(&data);
                let ms = |a: usize, b: usize| (t[b].saturating_sub(t[a]) as f64 * self.period as f64 / 1e6) as f32;
                let s = SHADOW_PASSES as usize;
                let mut now = vec![(0..s).map(|k| ms(2 * k, 2 * k + 1)).sum::<f32>()];
                now.extend((1..PASSES.len()).map(|p| ms(2 * (s + p - 1), 2 * (s + p - 1) + 1)));
                let mut times = TIMES.lock().unwrap_or_else(|e| e.into_inner());
                if times.len() != now.len() {
                    *times = now;
                } else {
                    for (old, new) in times.iter_mut().zip(now) {
                        *old += (new - *old) * 0.1;
                    }
                }
            }
            buffer.unmap();
            ready.store(false, Ordering::Release);
            *busy = false;
        }
    }
}
