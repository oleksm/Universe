//! The game's background work: opening a world's ground, reading a world's maps, laying the lines round the
//! eye, counting what's in view. One small named pool ("background N") beside the world's ("crowd") and the
//! pilots', so a long job neither waits for the simulation's threads nor holds them up; in place of a thread
//! of its own for every job.

use std::sync::OnceLock;

/// How many jobs run at once.
const THREADS: usize = 2;

fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| rayon::ThreadPoolBuilder::new().num_threads(THREADS).thread_name(|i| format!("background {i}")).build().expect("the background pool"))
}

/// Run `job` in the background (in turn, with the others).
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    pool().spawn(job);
}
