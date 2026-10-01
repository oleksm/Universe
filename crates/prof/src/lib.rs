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
            add(self.name, start.elapsed().as_secs_f64());
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_add_up_per_frame() {
        enable(true);
        for _ in 0..3 {
            for _ in 0..4 {
                let _s = scope("test/work");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            frame_end();
        }
        let r = report();
        let w = r.iter().find(|s| s.name == "test/work").expect("recorded");
        assert!((w.calls - 4.0).abs() < 1e-9);
        assert!(w.mean_ms >= 4.0 && w.max_ms >= w.mean_ms, "{w:?}");
    }
}
