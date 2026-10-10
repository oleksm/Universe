//! Halden Camp: its port, its land and works, a few days of its economy.
fn main() {
    let mut u = universe_sim::Universe::new(1984);
    let sys = u.ship_system();
    let port = sys.spaceports.iter().position(|p| p.name == "Halden Camp");
    println!("port: {:?} on {:?}", port, port.map(|p| &sys.bodies[sys.spaceports[p].body].name));
    let e = &u.services.markets.economy;
    let place = e.places.iter().find(|p| p.name == "Halden Camp");
    println!("place: {}", place.is_some());
    for w in e.works.iter().filter(|w| place.is_some_and(|p| p.site == w.site)) {
        println!("  {} (deposit {:?}): {:?}", w.name, w.deposit.as_ref().map(|(id, kg)| format!("{id} {:.4} Mt", kg / 1e9)), w.setups.iter().map(|s| (s.module.identity.key.clone(), s.recipe().map(|r| u.world.goods[r.makes].name.clone()))).collect::<Vec<_>>());
    }
    let start = u.world.time;
    while u.world.time < start + 3.0 * 86_400.0 {
        u.step_world(60.0, 1.0, &Default::default());
    }
    let e = &u.services.markets.economy;
    if let Some(p) = e.places.iter().find(|p| p.name == "Halden Camp") {
        for w in e.works.iter().filter(|w| w.site == p.site) {
            let held: Vec<String> = w.pool.stock.iter().map(|(i, kg)| format!("{} {:.0} t", u.world.goods[*i].name, kg / 1000.0)).collect();
            println!("  {} ran {:?}; deposit {:?}; holds {:?}", w.name, w.last.as_ref().map(|r| (r.rate, r.held_by.clone())), w.deposit.as_ref().map(|(id, kg)| format!("{id}: {:.4} Mt left", kg / 1e9)), held);
        }
    }
}
