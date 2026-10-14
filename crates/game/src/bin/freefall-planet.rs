//! The Freefall planet viewer, alone: every released world, `[world id]` the one to open at.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    game::planet(&args);
}
