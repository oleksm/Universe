//! A tiny frame profiler. Any layer times a stretch of work with
//! `let _p = scope("sim/crafts");` (recorded when it's dropped); the frame
//! loop calls `frame_end()` once a frame, which folds the frame's totals into
//! per-scope statistics over the last `WINDOW` frames: the mean and worst
//! time per frame, and the calls per frame. Names are paths
//! ("draw/scene/bodies") by convention, so the report reads as a tree.
//!
//! Scopes from any thread are counted (they add their time, so work spread
//! over threads shows as its total CPU time, not wall time; each thread
//! keeps its own tally, gathered at `frame_end`). Off unless `enable(true)`:
//! then a scope costs a clock read and an uncontended lock.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Frames the statistics cover.
pub const WINDOW: usize = 120;

static ON: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Each thread's tally for the frame so far (its own lock, so threads don't
/// contend), registered once; `frame_end` gathers them.
type Tally = std::sync::Arc<Mutex<HashMap<&'static str, (f64, u32)>>>;
static THREADS: Mutex<Vec<Tally>> = Mutex::new(Vec::new());

thread_local! {
    static MINE: Tally = {
        let t: Tally = Default::default();
        THREADS.lock().unwrap_or_else(|e| e.into_inner()).push(t.clone());
        t
    };
}

#[derive(Default)]
struct State {
    /// This frame so far: time (s) and calls per scope.
    frame: HashMap<&'static str, (f64, u32)>,
    /// The last frames' totals per scope (ring of `WINDOW`), and where the ring is.
    history: HashMap<&'static str, Vec<(f64, u32)>>,
    frames: usize,
    /// The last frame's totals, as it ended (for a slow frame's breakdown).
    last: Vec<(&'static str, f64, u32)>,
}

/// Turn profiling on or off.
pub fn enable(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// A timed stretch of work: its time goes to `name` when it's dropped.
pub struct Scope {
    name: &'static str,
    start: Option<Instant>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let took = start.elapsed();
            add(self.name, took.as_secs_f64());
            if TRACING.load(Ordering::Relaxed) && took.as_micros() >= TRACE_FLOOR_US {
                trace_event(self.name, start, took);
            }
        }
    }
}

// ---------------------------------------------------------------- tracing
// While tracing, every scope over `TRACE_FLOOR_US` is kept as an event (its
// thread, start and length): a timeline of who did what when, in Chrome's
// trace format (open it in Perfetto or chrome://tracing).

/// Shorter scopes aren't kept (a thousand crafts' steps a tick would swamp it).
pub const TRACE_FLOOR_US: u128 = 20;

static TRACING: AtomicBool = AtomicBool::new(false);
static EPOCH: Mutex<Option<Instant>> = Mutex::new(None);
/// One thread's events: (name, start µs since the trace began, length µs).
type Events = std::sync::Arc<Mutex<(String, Vec<(&'static str, u64, u64)>)>>;
static TRACE_THREADS: Mutex<Vec<Events>> = Mutex::new(Vec::new());

thread_local! {
    static MY_EVENTS: Events = {
        let name = std::thread::current().name().map_or_else(|| format!("{:?}", std::thread::current().id()), str::to_string);
        let e: Events = std::sync::Arc::new(Mutex::new((name, Vec::new())));
        TRACE_THREADS.lock().unwrap_or_else(|e| e.into_inner()).push(e.clone());
        e
    };
}

fn trace_event(name: &'static str, start: Instant, took: std::time::Duration) {
    let Some(epoch) = *EPOCH.lock().unwrap_or_else(|e| e.into_inner()) else { return };
    let at = start.saturating_duration_since(epoch).as_micros() as u64;
    MY_EVENTS.with(|e| e.lock().unwrap_or_else(|e| e.into_inner()).1.push((name, at, took.as_micros() as u64)));
}

/// Start a trace (profiling on as well).
pub fn start_trace() {
    for t in TRACE_THREADS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        t.lock().unwrap_or_else(|e| e.into_inner()).1.clear();
    }
    *EPOCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    enable(true);
    TRACING.store(true, Ordering::Relaxed);
}

pub fn tracing() -> bool {
    TRACING.load(Ordering::Relaxed)
}

/// Stop the trace: its events, as a Chrome trace (JSON text).
pub fn stop_trace() -> String {
    TRACING.store(false, Ordering::Relaxed);
    let mut out = String::from("{\"traceEvents\":[\n");
    let mut first = true;
    for (tid, t) in TRACE_THREADS.lock().unwrap_or_else(|e| e.into_inner()).iter().enumerate() {
        let (name, events) = &mut *t.lock().unwrap_or_else(|e| e.into_inner());
        if events.is_empty() {
            continue;
        }
        let sep = |first: &mut bool| if std::mem::replace(first, false) { "" } else { ",\n" };
        out += &format!("{}{{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{tid},\"args\":{{\"name\":\"{}\"}}}}", sep(&mut first), name.replace('"', "'"));
        for (n, at, dur) in events.drain(..) {
            out += &format!("{}{{\"name\":\"{n}\",\"ph\":\"X\",\"pid\":1,\"tid\":{tid},\"ts\":{at},\"dur\":{dur}}}", sep(&mut first));
        }
    }
    out += "\n]}\n";
    out
}

/// Time from now until the returned guard is dropped, as `name`.
#[must_use]
pub fn scope(name: &'static str) -> Scope {
    Scope { name, start: enabled().then(Instant::now) }
}

/// Run `f`, timed as `name`.
pub fn time<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    let _s = scope(name);
    f()
}

/// Add `seconds` to `name` this frame (for time measured elsewhere).
pub fn add(name: &'static str, seconds: f64) {
    if !enabled() {
        return;
    }
    MINE.with(|t| {
        let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
        let e = t.entry(name).or_default();
        e.0 += seconds;
        e.1 += 1;
    });
}

/// The frame is over: fold its totals into the statistics.
pub fn frame_end() {
    if !enabled() {
        return;
    }
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(State::default);
    for t in THREADS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        for (name, (secs, calls)) in t.lock().unwrap_or_else(|e| e.into_inner()).drain() {
            let e = s.frame.entry(name).or_default();
            e.0 += secs;
            e.1 += calls;
        }
    }
    let slot = s.frames % WINDOW;
    let frame = std::mem::take(&mut s.frame);
    s.last = frame.iter().map(|(&n, &(t, c))| (n, t, c)).collect();
    s.last.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (name, h) in s.history.iter_mut() {
        h[slot] = frame.get(name).copied().unwrap_or_default();
    }
    for (name, v) in frame {
        s.history.entry(name).or_insert_with(|| {
            let mut h = vec![(0.0, 0); WINDOW];
            h[slot] = v;
            h
        });
    }
    s.frames += 1;
}

/// The last frame's scopes, the slowest first: (name, ms, calls).
pub fn last_frame() -> Vec<(&'static str, f64, u32)> {
    let g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    g.as_ref().map(|s| s.last.iter().map(|&(n, t, c)| (n, t * 1000.0, c)).collect()).unwrap_or_default()
}

/// One scope's statistics.
#[derive(Clone, Debug)]
pub struct Stat {
    pub name: &'static str,
    /// Mean and worst time per frame (ms), and mean calls per frame.
    pub mean_ms: f64,
    pub max_ms: f64,
    pub calls: f64,
}

/// Every scope's statistics, by name (so a path's children follow it).
pub fn report() -> Vec<Stat> {
    let g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = g.as_ref() else { return Vec::new() };
    let n = s.frames.clamp(1, WINDOW) as f64;
    let mut out: Vec<Stat> = s
        .history
        .iter()
        .map(|(&name, h)| Stat {
            name,
            mean_ms: h.iter().map(|x| x.0).sum::<f64>() / n * 1000.0,
            max_ms: h.iter().map(|x| x.0).fold(0.0, f64::max) * 1000.0,
            calls: h.iter().map(|x| x.1 as f64).sum::<f64>() / n,
        })
        .collect();
    // A path with no scope of its own gets a row summing its direct children.
    let mut parents: Vec<&'static str> = Vec::new();
    for st in &out {
        let mut name = st.name;
        while let Some(k) = name.rfind('/') {
            name = &name[..k];
            if !out.iter().any(|s| s.name == name) && !parents.contains(&name) {
                parents.push(name);
            }
        }
    }
    for p in parents {
        let kids: Vec<&Stat> = out.iter().filter(|s| s.name.strip_prefix(p).and_then(|r| r.strip_prefix('/')).is_some_and(|r| !r.contains('/'))).collect();
        let mean_ms = kids.iter().map(|s| s.mean_ms).sum();
        let max_ms = kids.iter().map(|s| s.max_ms).sum();
        out.push(Stat { name: p, mean_ms, max_ms, calls: 0.0 });
    }
    out.sort_by(|a, b| a.name.cmp(b.name));
    out
}

/// The report as text, one scope a line, indented by its depth.
pub fn report_text() -> String {
    let mut s = format!("{:<44} {:>8} {:>8} {:>9}\n", "SCOPE", "MEAN MS", "MAX MS", "CALLS/F");
    for st in report() {
        let depth = st.name.matches('/').count();
        let leaf = st.name.rsplit('/').next().unwrap_or(st.name);
        s += &format!("{:<44} {:>8.3} {:>8.3} {:>9.1}\n", format!("{}{leaf}", "  ".repeat(depth)), st.mean_ms, st.max_ms, st.calls);
    }
    s
}
