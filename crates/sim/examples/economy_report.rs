use universe_sim::world::traffic::facilities;
use universe_sim::Universe;

// The economy as a trader and a miner find it: the home system and one
// gate out, after the economy alone (no ships trading) has run a while —
// the best runs for a full 20 t hold, what a career of them earns from the
// starting credits, what mined ore fetches. Fast (no ships flown).
//
//     cargo run --release -p universe-sim --example economy_report

fn main() {
  for days in [1.0, 3.0, 10.0, 30.0] {
    eprintln!("=== after {days} days (the economy alone, no traders) ===");
    let mut u = Universe::new(42);
    let t = u.world.time + days * 86_400.0;
    u.markets.economy.step_to(t);
    u.world.time = t;
    let home = u.ship_system;
    let goods = u.world.goods.clone();
    let mut systems = vec![home];
    systems.extend(u.gate_links_of(home).into_iter().map(|(s, _)| s));
    let hold = 20_000.0; // a Drover's hold (kg)
    let now = u.world.time;
    // (system, place, item, buy per unit (if sold), sell per unit, level)
    let mut q = Vec::new();
    for &s in &systems {
        let sys = u.system(s);
        for f in facilities(&sys) {
            for x in u.markets.quotes(s, &sys, f, now) {
                q.push((s, f, x.offer.item, x.buy, x.sell, x.level));
            }
        }
    }
    eprintln!("{} quotes in {} systems", q.len(), systems.len());
    // Raw spreads, no limits: per good, the cheapest buy and the dearest sale.
    let mut spreads = Vec::new();
    for (i, g) in goods.iter().enumerate() {
        let buys: Vec<_> = q.iter().filter(|x| x.2 == i && x.3.is_some()).collect();
        let sells: Vec<_> = q.iter().filter(|x| x.2 == i).collect();
        let Some(cheap) = buys.iter().min_by(|a, b| a.3.unwrap().total_cmp(&b.3.unwrap())) else { continue };
        let Some(dear) = sells.iter().max_by(|a, b| a.4.total_cmp(&b.4)) else { continue };
        spreads.push(((dear.4 - cheap.3.unwrap()) / g.mass * 1000.0, g.name.clone(), cheap.3.unwrap(), cheap.5, dear.4, dear.5, buys.len(), sells.len(), (cheap.0, cheap.1) == (dear.0, dear.1)));
    }
    spreads.sort_by(|a, b| b.0.total_cmp(&a.0));
    eprintln!("raw spreads (CR per tonne): best 8");
    for s in spreads.iter().take(8) {
        eprintln!("  {:>8.0}/t {:<28} buy {:>7.1} (stock {:>6.0}) sell {:>7.1} (demand {:>6.0})  {} sellers {} buyers same-place {}", s.0, s.1, s.2, s.3, s.4, s.5, s.6, s.7, s.8);
    }
    let mut routes = Vec::new();
    for a in &q {
        let Some(buy) = a.3 else { continue };
        for b in &q {
            if b.2 != a.2 || (b.0, b.1) == (a.0, a.1) {
                continue;
            }
            let g = &goods[a.2];
            let units = (hold / g.mass).floor().min(a.5).min(b.5).max(0.0);
            let profit = (b.4 - buy) * units;
            if profit > 0.0 {
                routes.push((profit, a.0 == b.0, g.name.clone(), g.mass, buy, b.4, units));
            }
        }
    }
    routes.sort_by(|x, y| y.0.total_cmp(&x.0));
    eprintln!("best runs, a full 20 t hold:");
    for r in routes.iter().take(12) {
        eprintln!("  {:>9.0} CR  {}  {:<28} {:>6.0} kg/unit  buy {:>7.1} sell {:>7.1}  x{:.0}", r.0, if r.1 { "in-system" } else { "1 gate  " }, r.2, r.3, r.4, r.5, r.6);
    }
    // A career: from the starting credits, the best in-system run your
    // credits allow each time (12 min a run), spreads as they are.
    let mut credits = 1000.0f64;
    let mut hours = 0.0;
    let mut marks = [(10_000.0, None), (50_000.0, None), (120_000.0, None), (430_000.0, None)];
    for _ in 0..2000 {
        let mut best = 0.0f64;
        for a in q.iter().filter(|a| a.0 == home) {
            let Some(buy) = a.3 else { continue };
            for b in q.iter().filter(|b| b.0 == home && b.2 == a.2 && (b.0, b.1) != (a.0, a.1)) {
                let g = &goods[a.2];
                let units = (hold / g.mass).floor().min(a.5).min(b.5).min((credits / buy).floor()).max(0.0);
                best = best.max((b.4 - buy) * units);
            }
        }
        if best <= 0.0 {
            break;
        }
        credits += best;
        hours += 0.2;
        for m in marks.iter_mut() {
            if m.1.is_none() && credits >= m.0 {
                m.1 = Some(hours);
            }
        }
    }
    eprintln!("trading career (in-system, 12 min a run): {:?}", marks.iter().map(|(c, h)| format!("{:.0}k CR after {}", c / 1000.0, h.map_or("never".into(), |h| format!("{h:.1} h")))).collect::<Vec<_>>());
    let in_sys: Vec<_> = routes.iter().filter(|r| r.1).collect();
    eprintln!("in-system routes: {}, best {:.0} CR, median of top 20 {:.0}", in_sys.len(), in_sys.first().map_or(0.0, |r| r.0), in_sys.get(10).map_or(0.0, |r| r.0));
    // Ore: what a tonne fetches at home.
    for (i, g) in goods.iter().enumerate().filter(|(_, g)| g.key.starts_with("ore.")) {
        let best = q.iter().filter(|x| x.2 == i && x.0 == home).map(|x| x.4).fold(0.0, f64::max);
        eprintln!("ore {:<24} sells {:>6.0} CR/t: mining 36 t/h = {:>6.0} CR/h", g.name, best / g.mass * 1000.0, best / g.mass * 1000.0 * 36.0);
    }
  }
}
