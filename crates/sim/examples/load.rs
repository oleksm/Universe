//! The world tick under load, profiled (a tool, not a test):
//! `cargo run -p universe-sim --release --example load -- 10000 [apart] [stress]`
//! Settlers as the game spawns them (their stops staggered over ten
//! minutes), or with `stress` all leaving at once; 120 ticks; where the time went.

use std::time::Instant;

use universe_sim::{Controls, Universe};

fn main() {
    let mut args = std::env::args().skip(1);
    let n: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(10_000);
    let rest: Vec<String> = args.collect();
    let apart = rest.iter().any(|a| a == "apart");
    let stress = rest.iter().any(|a| a == "stress");
    let mut u = Universe::new(1984);
    u.spawn_settlers(n, 3);
    if stress {
        let now = u.world.time;
        for p in u.pilots().iter_mut() {
            p.avionics.route.dwell_until = Some(now);
        }
    }
    if apart {
        u.run_pilots_apart(std::thread::available_parallelism().map_or(2, |n| n.get() / 2));
    }
    for _ in 0..30 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    universe_prof::enable(true);
    let start = Instant::now();
    for _ in 0..120 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        universe_prof::frame_end();
    }
    let per = start.elapsed() / 120;
    println!("{n} settlers{}, pilots {}: {per:?} per tick; late {} dropped {}", if stress { " (all leaving)" } else { "" }, if apart { "apart" } else { "in lockstep" }, u.pool.late, u.pool.dropped);
    println!("{}", universe_prof::report_text());
}
