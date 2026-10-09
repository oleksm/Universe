//! Replay a recorded session (a tool, not a test):
//! `cargo run -p universe-sim --release --example replay -- session.json`
//! (record one with `UNIVERSE_RECORD=session.json cargo run --release`).
//! The world is run again from its seed and the log alone; its state hash
//! is printed, to compare.

use universe_sim::operator::WorldSave;
use universe_sim::Universe;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: replay <session.json>");
        std::process::exit(2);
    };
    let save: WorldSave = serde_json::from_str(&std::fs::read_to_string(&path).expect("read the session")).expect("a recorded session");
    let start = std::time::Instant::now();
    let u = Universe::replay(&save.log);
    let postings: usize = save.log.ticks.iter().map(|t| t.due.len()).sum();
    println!(
        "{} ticks ({:.1} game s), {} postings, {} settlers: replayed in {:.2?}; state hash {:x}",
        save.log.ticks.len(),
        u.world.time,
        postings,
        u.vessels.crafts().len(),
        start.elapsed(),
        u.state_hash()
    );
}
