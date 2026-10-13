//! What each pass costs the GPU (ms), from timestamps the GPU writes at each pass's start and end
//! (where it can: `wgpu::Features::TIMESTAMP_QUERY`), read back a frame or two later and smoothed:
//! the time the GPU itself spends, which the frame's wait for the screen only shows summed.

use std::sync::Mutex;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Pass categories; all views and shadow cascades are summed by category.
pub const PASSES: [&str; 5] = ["shadows", "scene", "front", "hud", "upscale"];
// Each physical pass gets unique queries, across all views and UI sections.
// A bounded pool avoids unbounded profiling allocations. Overflow drops the
// entire sample rather than publishing misleading partial frame totals.
const STAMPS: u32 = 4096;
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
    staging: Vec<(wgpu::Buffer, Arc<AtomicBool>, bool, Vec<usize>)>,
    passes: RefCell<Vec<usize>>,
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
        let staging = (0..SLOTS).map(|_| (device.create_buffer(&wgpu::BufferDescriptor { label: Some("pass times readback"), size: bytes, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), Arc::new(AtomicBool::new(false)), false, Vec::new())).collect();
        Some(GpuTime { set, resolve, staging, passes: RefCell::new(Vec::new()), next: 0, copied: None, period: queue.get_timestamp_period() })
    }

    pub fn begin_frame(&mut self) {
        self.passes.get_mut().clear();
    }

    pub fn shadow(&self, _cascade: u32) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.pass(0)
    }

    /// Allocate once per physical pass; never reuse a query in this frame.
    pub fn pass(&self, pass: usize) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let mut passes = self.passes.borrow_mut();
        let at = passes.len() as u32 * 2;
        passes.push(pass);
        (at + 1 < STAMPS).then_some(wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(at),
            end_of_pass_write_index: Some(at + 1),
        })
    }

    /// This frame's stamps resolved and copied out (if a read-back is free).
    pub fn copy_out(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let passes = self.passes.get_mut();
        if passes.is_empty() || passes.len() > STAMPS as usize / 2 { return; }
        let count = passes.len() as u32 * 2;
        encoder.resolve_query_set(&self.set, 0..count, &self.resolve, 0);
        let slot = self.next;
        if self.staging[slot].2 {
            return;
        }
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.staging[slot].0, 0, count as u64 * 8);
        self.staging[slot].3.clone_from(passes);
        self.copied = Some(slot);
        self.next = (slot + 1) % SLOTS;
    }

    /// Once submitted: the copy mapped, to be read when it's done.
    pub fn submitted(&mut self) {
        let Some(slot) = self.copied.take() else { return };
        let (buffer, ready, busy, _) = &mut self.staging[slot];
        *busy = true;
        let ready = ready.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| ready.store(r.is_ok(), Ordering::Release));
    }

    /// Read-backs done since: into `times`, smoothed.
    pub fn collect(&mut self) {
        for (buffer, ready, busy, passes) in &mut self.staging {
            if !(*busy && ready.load(Ordering::Acquire)) {
                continue;
            }
            if let Ok(data) = buffer.slice(..).get_mapped_range() {
                let t: &[u64] = bytemuck::cast_slice(&data);
                let ms = |a: usize, b: usize| (t[b].saturating_sub(t[a]) as f64 * self.period as f64 / 1e6) as f32;
                let mut now = vec![0.; PASSES.len()];
                for (k, &pass) in passes.iter().enumerate() {
                    now[pass] += ms(2 * k, 2 * k + 1);
                }
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
