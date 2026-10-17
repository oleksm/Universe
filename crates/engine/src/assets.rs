//! Engine-owned asynchronous preparation, demand scheduling and CPU residency.
//! Keys identify immutable content plus preparation options, not instances.
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Config {
    pub workers: usize,
    pub max_entries: usize,
    /// Resident data plus estimated in-flight peak allocations.
    pub budget_bytes: usize,
    pub idle_for: Duration,
    pub retry_after: Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            workers: 2,
            max_entries: 512,
            budget_bytes: 512 * 1024 * 1024,
            idle_for: Duration::from_secs(30),
            retry_after: Duration::from_secs(5),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Demand {
    /// Lower is more important: 0 required, 1 visible, 2 anticipated, 3 speculative.
    pub class: u8,
    pub time_to_need: Duration,
    /// Estimated peak preparation memory, including the eventual resident value.
    pub estimated_bytes: usize,
    pub estimated_cost: Duration,
}
impl Default for Demand {
    fn default() -> Self {
        Self {
            class: 1,
            time_to_need: Duration::ZERO,
            estimated_bytes: 16 * 1024 * 1024,
            estimated_cost: Duration::from_millis(100),
        }
    }
}
pub struct Loaded<T> {
    pub value: T,
    pub bytes: usize,
}
#[derive(Clone, Default, Debug, serde::Serialize)]
pub struct Snapshot {
    pub requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub coalesced: u64,
    pub loads: u64,
    pub unloads: u64,
    pub reloads: u64,
    pub failures: u64,
    pub cancelled: u64,
    pub rejected: u64,
    pub retries: u64,
    pub resident_bytes: usize,
    pub reserved_bytes: usize,
    pub peak_bytes: usize,
    pub queued: usize,
    pub loading: usize,
    pub queue_seconds: f64,
    pub prepare_seconds: f64,
    pub entries: Vec<EntrySnapshot>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Event {
    pub seconds: f64,
    pub key: String,
    pub kind: &'static str,
    pub bytes: usize,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct EntrySnapshot {
    pub key: String,
    pub state: &'static str,
    pub bytes: usize,
    pub class: u8,
    pub idle_seconds: f64,
    pub time_to_need_seconds: f64,
    pub estimated_cost_seconds: f64,
    pub leases: usize,
    pub error: Option<String>,
}
type Job<T> = Box<dyn FnOnce() -> Result<Loaded<T>, String> + Send>;
enum Status<T> {
    Queued(Option<Job<T>>),
    Loading,
    Ready(Arc<T>, usize),
    Failed(String),
}
struct Entry<T> {
    status: Status<T>,
    demand: Demand,
    deadline: Instant,
    priority_until: Instant,
    queued: Instant,
    used: Instant,
}
struct State<T> {
    entries: HashMap<String, Entry<T>>,
    stats: Snapshot,
    recent_unloads: std::collections::VecDeque<String>,
    stop: bool,
    costs: HashMap<String, Duration>,
    epoch: Instant,
    events: std::collections::VecDeque<Event>,
}
struct Shared<T> {
    state: Mutex<State<T>>,
    wake: Condvar,
    config: Config,
}
pub struct AssetCache<T> {
    shared: Arc<Shared<T>>,
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl<T: Send + Sync + 'static> AssetCache<T> {
    pub fn new(config: Config) -> Self {
        assert!(config.workers > 0 && config.max_entries > 0 && config.budget_bytes > 0);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: HashMap::new(),
                stats: Snapshot::default(),
                recent_unloads: Default::default(),
                stop: false,
                costs: HashMap::new(),
                epoch: Instant::now(),
                events: Default::default(),
            }),
            wake: Condvar::new(),
            config,
        });
        let workers = (0..shared.config.workers)
            .map(|i| {
                let s = shared.clone();
                std::thread::Builder::new()
                    .name(format!("asset {i}"))
                    .spawn(move || worker(s))
                    .expect("asset worker")
            })
            .collect();
        Self { shared, workers }
    }
    /// Never executes `load` on the caller. Repeated in-flight requests coalesce.
    /// Ready Arcs are leases: residency cannot be evicted while they are held.
    pub fn request(
        &self,
        key: String,
        mut demand: Demand,
        load: impl FnOnce() -> Result<Loaded<T>, String> + Send + 'static,
    ) -> Option<Arc<T>> {
        let now = Instant::now();
        let s = &self.shared;
        let mut st = s.state.lock().unwrap_or_else(|e| e.into_inner());
        st.stats.requests += 1;
        if let Some(cost) = st.costs.get(&key) {
            demand.estimated_cost = *cost;
        }

        let mut hit = None;
        let mut coalesced = false;
        let mut retry = false;
        if let Some(e) = st.entries.get_mut(&key) {
            let since = now.duration_since(e.used);
            match &e.status {
                Status::Ready(v, _) => hit = Some(v.clone()),
                Status::Queued(_) | Status::Loading => coalesced = true,
                Status::Failed(_) => retry = since >= s.config.retry_after,
            }
            // Failed retries are measured from failure, not every frame's poll.
            if !matches!(e.status, Status::Failed(_)) {
                e.used = now;
            }
            if now >= e.priority_until
                || demand.class < e.demand.class
                || (demand.class == e.demand.class && now + demand.time_to_need < e.deadline)
            {
                // Running reservations must never change underneath the worker.
                if now >= e.priority_until {
                    e.priority_until = now + Duration::from_millis(100);
                }
                e.demand.class = demand.class;
                e.deadline = now + demand.time_to_need;
            }
            if hit.is_some() {
                st.stats.hits += 1;
                return hit;
            }
            if coalesced {
                st.stats.coalesced += 1;
                s.wake.notify_one();
                return None;
            }
            if !retry {
                return None;
            }
        }
        if demand.estimated_bytes > s.config.budget_bytes {
            st.stats.rejected += 1;
            return None;
        }
        if retry {
            st.entries.remove(&key);
            st.stats.retries += 1;
        }
        evict(&mut st, &s.config, now, false);
        if st.entries.len() >= s.config.max_entries {
            evict(&mut st, &s.config, now, true);
        }
        if st.entries.len() >= s.config.max_entries {
            st.stats.rejected += 1;
            return None;
        }
        if st.recent_unloads.contains(&key) {
            st.stats.reloads += 1;
        }
        st.stats.misses += 1;
        event(&mut st, &key, "queued", demand.estimated_bytes);
        st.entries.insert(
            key,
            Entry {
                status: Status::Queued(Some(Box::new(load))),
                demand,
                deadline: now + demand.time_to_need,
                priority_until: now + Duration::from_millis(100),
                queued: now,
                used: now,
            },
        );
        s.wake.notify_one();
        None
    }
    /// Cancel unstarted speculation only; another consumer may still need shared work.
    pub fn cancel(&self, key: &str) -> bool {
        let mut st = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if st
            .entries
            .get(key)
            .is_some_and(|e| matches!(e.status, Status::Queued(_)))
        {
            st.entries.remove(key);
            st.stats.cancelled += 1;
            event(&mut st, key, "cancelled", 0);
            true
        } else {
            false
        }
    }
    /// Drop queued prefetch after a scene jump. Running jobs retain their
    /// reservations and immutable identity; no completion can replace another key.
    pub fn cancel_speculation(&self) {
        let mut st = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        let keys: Vec<_> = st
            .entries
            .iter()
            .filter(|(_, e)| e.demand.class >= 2 && matches!(e.status, Status::Queued(_)))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            st.entries.remove(&key);
            st.stats.cancelled += 1;
            event(&mut st, &key, "scene-cancelled", 0);
        }
    }
    pub fn maintain(&self) {
        let mut st = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        evict(&mut st, &self.shared.config, Instant::now(), false);
        self.shared.wake.notify_all();
    }
    pub fn pending_count(&self) -> usize {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .values()
            .filter(|e| matches!(e.status, Status::Queued(_) | Status::Loading))
            .count()
    }
    pub fn snapshot(&self) -> Snapshot {
        let st = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = st.stats.clone();
        let now = Instant::now();
        for (key, e) in &st.entries {
            let (state, bytes, leases, error) = match &e.status {
                Status::Queued(_) => {
                    out.queued += 1;
                    ("queued", 0, 0, None)
                }
                Status::Loading => {
                    out.loading += 1;
                    ("loading", 0, 0, None)
                }
                Status::Ready(v, n) => ("ready", *n, Arc::strong_count(v) - 1, None),
                Status::Failed(err) => ("failed", 0, 0, Some(err.clone())),
            };
            out.entries.push(EntrySnapshot {
                key: key.clone(),
                state,
                bytes,
                class: e.demand.class,
                idle_seconds: now.duration_since(e.used).as_secs_f64(),
                time_to_need_seconds: e.deadline.saturating_duration_since(now).as_secs_f64(),
                estimated_cost_seconds: e.demand.estimated_cost.as_secs_f64(),
                leases,
                error,
            });
        }
        out.events = st.events.iter().cloned().collect();
        out.entries.sort_by(|a, b| a.key.cmp(&b.key));
        out
    }
}
impl<T> Drop for AssetCache<T> {
    fn drop(&mut self) {
        {
            let mut st = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
            st.stop = true;
            self.shared.wake.notify_all();
        }
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}
fn event<T>(st: &mut State<T>, key: &str, kind: &'static str, bytes: usize) {
    st.events.push_back(Event {
        seconds: st.epoch.elapsed().as_secs_f64(),
        key: key.into(),
        kind,
        bytes,
    });
    while st.events.len() > 512 {
        st.events.pop_front();
    }
}
fn evict<T>(st: &mut State<T>, config: &Config, now: Instant, pressure: bool) {
    let mut candidates: Vec<_> = st
        .entries
        .iter()
        .filter_map(|(k, e)| {
            let eligible = match &e.status {
                Status::Ready(v, _) => Arc::strong_count(v) == 1,
                Status::Failed(_) => true,
                _ => false,
            };
            (eligible
                && (now.duration_since(e.used) >= config.idle_for
                    || (pressure && now.duration_since(e.used) >= Duration::from_secs(1))))
            .then_some((k.clone(), e.used))
        })
        .collect();
    candidates.sort_by_key(|(_, t)| *t);
    // Under pressure release one at a time, then re-evaluate admission.
    for (key, _) in candidates
        .into_iter()
        .take(if pressure { 1 } else { usize::MAX })
    {
        if let Some(e) = st.entries.remove(&key) {
            if let Status::Ready(_, n) = e.status {
                st.stats.resident_bytes -= n;
                st.stats.unloads += 1;
                event(
                    st,
                    &key,
                    if pressure {
                        "evicted-pressure"
                    } else {
                        "evicted-idle"
                    },
                    n,
                );
                st.recent_unloads.push_back(key);
                while st.recent_unloads.len() > config.max_entries {
                    st.recent_unloads.pop_front();
                }
            }
        }
    }
}
fn worker<T: Send + Sync + 'static>(shared: Arc<Shared<T>>) {
    loop {
        let (key, job, reservation) = {
            let mut st = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if st.stop {
                    return;
                }
                let now = Instant::now();
                evict(&mut st, &shared.config, now, false);
                let stale: Vec<_> = st
                    .entries
                    .iter()
                    .filter(|(_, e)| {
                        e.demand.class >= 2
                            && matches!(e.status, Status::Queued(_))
                            && now.duration_since(e.used) > Duration::from_secs(5)
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                for key in stale {
                    st.entries.remove(&key);
                    st.stats.cancelled += 1;
                    event(&mut st, &key, "expired-speculation", 0);
                }

                let mut next = st
                    .entries
                    .iter()
                    .filter(|(_, e)| matches!(e.status, Status::Queued(_)))
                    .min_by(|(_, a), (_, b)| {
                        // Age promotes speculative work after five seconds. Within a class,
                        // deadline slack starts expensive urgent work early; cost breaks ties.
                        let class = |e: &Entry<T>| {
                            if e.demand.class == 0 {
                                0
                            } else {
                                e.demand
                                    .class
                                    .saturating_sub(
                                        (now.duration_since(e.queued).as_secs() / 5).min(3) as u8,
                                    )
                                    .max(1)
                            }
                        };
                        let slack = |e: &Entry<T>| {
                            e.deadline.saturating_duration_since(now).as_secs_f64()
                                - e.demand.estimated_cost.as_secs_f64()
                        };
                        class(a)
                            .cmp(&class(b))
                            .then_with(|| {
                                (slack(a) / 0.1)
                                    .floor()
                                    .total_cmp(&(slack(b) / 0.1).floor())
                            })
                            .then(a.demand.estimated_cost.cmp(&b.demand.estimated_cost))
                            .then(a.queued.cmp(&b.queued))
                    })
                    .map(|(k, e)| (k.clone(), e.demand.estimated_bytes));
                let available = shared
                    .config
                    .budget_bytes
                    .saturating_sub(st.stats.resident_bytes)
                    .saturating_sub(st.stats.reserved_bytes);
                if next.as_ref().is_some_and(|(_, n)| *n > available) {
                    let before = st.entries.len();
                    evict(&mut st, &shared.config, now, true);
                    if st.entries.len() < before {
                        continue;
                    }
                    // Pinned/in-flight memory cannot be evicted. Let cheap work
                    // use the remaining reservation rather than idle both workers.
                    if let Some((key, e)) = st
                        .entries
                        .iter()
                        .filter(|(_, e)| {
                            matches!(e.status, Status::Queued(_))
                                && e.demand.estimated_bytes <= available
                        })
                        .min_by_key(|(_, e)| {
                            (
                                e.demand.class,
                                e.deadline,
                                e.demand.estimated_cost,
                                e.queued,
                            )
                        })
                    {
                        next = Some((key.clone(), e.demand.estimated_bytes));
                    }
                }
                if let Some((key, reservation)) = next {
                    if st
                        .stats
                        .resident_bytes
                        .saturating_add(st.stats.reserved_bytes)
                        .saturating_add(reservation)
                        <= shared.config.budget_bytes
                    {
                        let e = st.entries.get_mut(&key).unwrap();
                        let Status::Queued(job) = &mut e.status else {
                            unreachable!()
                        };
                        let job = job.take().unwrap();
                        let waited = now.duration_since(e.queued).as_secs_f64();
                        e.status = Status::Loading;
                        event(&mut st, &key, "started", reservation);
                        st.stats.queue_seconds += waited;
                        st.stats.reserved_bytes += reservation;
                        st.stats.peak_bytes = st
                            .stats
                            .peak_bytes
                            .max(st.stats.reserved_bytes + st.stats.resident_bytes);
                        break (key, job, reservation);
                    }
                    let before = st.entries.len();
                    evict(&mut st, &shared.config, now, true);
                    if st.entries.len() < before {
                        continue;
                    }
                }
                st = shared
                    .wake
                    .wait_timeout(st, Duration::from_millis(100))
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        };
        let started = Instant::now();
        let _scope = universe_prof::scope("assets/prepare");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
            .unwrap_or_else(|_| Err("asset preparation panicked".into()));
        let mut st = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        st.stats.reserved_bytes -= reservation;
        st.stats.prepare_seconds += started.elapsed().as_secs_f64();
        if st.costs.len() >= shared.config.max_entries && !st.costs.contains_key(&key) {
            if let Some(k) = st.costs.keys().next().cloned() {
                st.costs.remove(&k);
            }
        }
        st.costs.insert(key.clone(), started.elapsed());
        let status = match result {
            Ok(v) if v.bytes <= reservation => {
                st.stats.loads += 1;
                st.stats.resident_bytes += v.bytes;
                Status::Ready(Arc::new(v.value), v.bytes)
            }
            Ok(_) => {
                st.stats.failures += 1;
                Status::Failed("prepared asset exceeds memory reservation".into())
            }
            Err(e) => {
                st.stats.failures += 1;
                Status::Failed(e)
            }
        };
        let (kind, bytes) = match &status {
            Status::Ready(_, n) => ("loaded", *n),
            _ => ("failed", 0),
        };
        event(&mut st, &key, kind, bytes);
        if let Some(e) = st.entries.get_mut(&key) {
            e.status = status;
            e.used = Instant::now();
        }
        shared.wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    fn config() -> Config {
        Config {
            workers: 1,
            max_entries: 8,
            budget_bytes: 64,
            idle_for: Duration::from_secs(60),
            retry_after: Duration::from_secs(60),
        }
    }
    fn demand() -> Demand {
        Demand {
            estimated_bytes: 16,
            ..Default::default()
        }
    }
    fn ready(cache: &AssetCache<usize>, key: &str) -> Arc<usize> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(v) = cache.request(key.into(), demand(), || panic!("duplicate preparation"))
            {
                return v;
            }
            assert!(Instant::now() < deadline, "{:?}", cache.snapshot());
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn concurrent_demand_coalesces_and_ready_requests_hit() {
        let cache = AssetCache::new(config());
        let (tx, rx) = mpsc::channel();
        assert!(
            cache
                .request("a".into(), demand(), move || {
                    rx.recv().unwrap();
                    Ok(Loaded { value: 7, bytes: 8 })
                })
                .is_none()
        );
        for _ in 0..20 {
            assert!(
                cache
                    .request("a".into(), demand(), || panic!("duplicate"))
                    .is_none()
            );
        }
        tx.send(()).unwrap();
        let a = ready(&cache, "a");
        let b = ready(&cache, "a");
        assert!(Arc::ptr_eq(&a, &b));
        let s = cache.snapshot();
        assert_eq!(s.loads, 1);
        assert_eq!(s.misses, 1);
        assert!(s.coalesced >= 20);
        assert!(s.hits >= 2);
        assert_eq!(s.resident_bytes, 8);
    }
    #[test]
    fn priority_and_cancellation_control_queued_work() {
        let cache = AssetCache::new(config());
        let (release, gate) = mpsc::channel();
        let (started, start) = mpsc::channel();
        let (tx, rx) = mpsc::channel();
        cache.request("block".into(), demand(), move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
            Ok(Loaded { value: 0, bytes: 1 })
        });
        start.recv_timeout(Duration::from_secs(2)).unwrap();
        for (key, class) in [("later", 3), ("visible", 1), ("cancel", 0)] {
            let tx = tx.clone();
            cache.request(key.into(), Demand { class, ..demand() }, move || {
                tx.send(key).unwrap();
                Ok(Loaded { value: 1, bytes: 1 })
            });
        }
        assert!(cache.cancel("cancel"));
        release.send(()).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), "visible");
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), "later");
        assert_eq!(cache.snapshot().cancelled, 1);
    }
}
