//! The economy alone, fast-forwarded (a tool, not a test): is the world's
//! coarse fabric balanced, and what does it take to carry it?
//!
//!   cargo run -p universe-sim --release --example economy [-- days]
//!
//! Prints what the settled places can make of each kind of goods against
//! what they use; then `days` with no ship calling (who goes short of what,
//! and when); then `days` with an ideal hauler moving what's short from
//! wherever has spare (does it sustain itself, and how many tonnes a day
//! must ships carry?).

use universe_services::economy::{Economy, PlaceKind, STEP};
use universe_sim::world::goods::Category;
use universe_sim::Universe;

const DAY: f64 = 86_400.0;

fn main() {
    let days: f64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(60.0);
    let u = Universe::new(1984);
    let start = u.markets.economy.clone();
    let kinds = |e: &Economy, k: PlaceKind| e.places.iter().filter(|p| p.kind == k).count();
    println!(
        "{} places: {} stations, {} farm worlds, {} mining worlds, {} outposts; {:.0}k people",
        start.places.len(),
        kinds(&start, PlaceKind::Station),
        kinds(&start, PlaceKind::Farm),
        kinds(&start, PlaceKind::Mine),
        kinds(&start, PlaceKind::Outpost),
        start.places.iter().map(|p| p.population).sum::<f64>()
    );
    println!("\n{:<12} {:>9} {:>9} {:>6}   (t/day at full work)", "KIND", "MAKES", "USES", "RATIO");
    for c in Category::all() {
        let make: f64 = start.places.iter().map(|p| p.makes(c)).sum();
        let need: f64 = start.places.iter().map(|p| p.needs(c)).sum();
        if make + need > 0.0 {
            println!("{:<12} {make:>9.1} {need:>9.1} {:>6.2}", c.name(), if need > 0.0 { make / need } else { f64::INFINITY });
        }
    }

    // No ship calling.
    let mut e = start.clone();
    println!("\nNO SHIPS, {days:.0} DAYS: when each kind first runs short somewhere, and how many places are short of it at the end");
    let mut first: Vec<Option<f64>> = vec![None; 20];
    let mut t = 0.0;
    while t < days * DAY {
        t += STEP;
        e.step_to(e.stepped_to + STEP);
        for c in Category::all() {
            if first[c as usize].is_none() && e.places.iter().any(|p| p.short[c as usize] > 1e-9) {
                first[c as usize] = Some(t / DAY);
            }
        }
    }
    for c in Category::all() {
        if let Some(d) = first[c as usize] {
            let n = e.places.iter().filter(|p| p.short[c as usize] > 1e-9).count();
            println!("  {:<12} from day {d:>5.1}   {n} places short", c.name());
        }
    }

    // An ideal hauler.
    let mut e = start.clone();
    let mut hauled = 0.0;
    let mut t = 0.0;
    while t < days * DAY {
        t += STEP;
        e.step_to(e.stepped_to + STEP);
        hauled += e.haul_ideally();
    }
    let totals = e.totals();
    let short: f64 = totals.iter().map(|l| l.3).sum();
    let made: f64 = totals.iter().map(|l| l.1).sum();
    println!("\nIDEAL HAULING, {days:.0} DAYS: {:.0} t/day carried ({:.0} holds of 20 t a day); made {made:.0} t/day; short {short:.1} t/day", hauled / days, hauled / days / 20.0);
    let mut lines: Vec<(Category, f64)> = Category::all().map(|c| (c, totals[c as usize].3)).filter(|l| l.1 > 1e-6).collect();
    lines.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (c, s) in lines {
        println!("  short {:<12} {s:.2} t/day", c.name());
    }
}
