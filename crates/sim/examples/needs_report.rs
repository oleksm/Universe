// Treistun's people living by their needs, the economy alone (no ships flown): each settlement's
// people, how fed, its needs met, every ten days for two months.
//
//     cargo run --release -p universe-sim --example needs_report
fn main() {
    let mut u = universe_sim::Universe::new(1984);
    let step = universe_sim::services::economy::STEP;
    let mut t = u.world.time;
    for day in 1..=60 {
        let end = t + 86_400.0;
        while t + step <= end {
            t += step;
            u.markets.step(t, &mut u.land, &mut u.ledger, u.tick);
            universe_sim::company::run(&mut u);
        }
        if day == 2 {
            works_at(&u, "Port Eikir");
        }
        if day % 10 == 0 || day == 1 || day == 3 {
            println!("--- day {day}");
            for p in &u.markets.economy.places {
                let short: Vec<String> = p.needs.iter().filter(|(_, (m, _))| *m < 0.999).map(|(k, (m, s))| format!("{} {:.0}% ({:.1} d short)", k.trim_start_matches("need."), m * 100.0, s / 86_400.0)).collect();
                println!("{:<16} {:>6.1}k people, fed {:>3.0}%, dying {:.2}k/day, waiting {:.1}k  {}", p.name, p.population, p.fed * 100.0, p.deaths, p.waiting, short.join(", "));
            }
        }
    }
}

#[allow(dead_code)]
fn works_at(u: &universe_sim::Universe, place: &str) {
    let e = &u.markets.economy;
    let p = e.places.iter().find(|p| p.name == place).unwrap();
    let g = &u.land.grounds[p.ground];
    for w in e.works.iter().filter(|w| w.ground == p.ground) {
        let run = &g.works[w.works].last;
        let set: Vec<String> = w.setups.iter().filter_map(|s| s.recipe().map(|r| u.world.goods[r.makes].name.clone())).take(6).collect();
        println!("  {:<24} {:?}  makes {:?}", w.name, run.as_ref().map(|r| (format!("{:.0}%", r.rate * 100.0), r.held_by.clone())), set);
    }
}
