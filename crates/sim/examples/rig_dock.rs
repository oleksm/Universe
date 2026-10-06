//! Come alongside Hadley Orbital Works by hand: set down on its top, see what its owner trades.
use glam::DVec3;
use universe_sim::{Controls, Universe};

fn main() {
    let mut u = Universe::new(1984);
    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    let sys = u.ship_system();
    let rig = sys.bodies.iter().position(|b| b.kind == universe_sim::world::BodyKind::Rig).expect("a rig");
    let t = u.world.time;
    let mut pos = Vec::new();
    sys.positions(t, &mut pos);
    let at = pos[rig];
    let rot = sys.bodies[rig].rotation(t);
    let top = universe_sim::world::rigs::half(&sys.bodies[rig]).unwrap().y;
    let up = rot * DVec3::Y;
    u.start_in_flight();
    u.ship.state = universe_sim::world::ShipState::Flying;
    u.ship.position = at + up * (top + u.ship.rest_height() + 4.0);
    u.ship.velocity = sys.velocity(rig, t) - up * 1.0;
    u.ship.orientation = universe_sim::ship::facing(rot * DVec3::Z, up);
    for _ in 0..600 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    println!("state {:?}", u.ship.state);
    let f = u.docked_market();
    println!("docked at {:?}", f.map(|f| f.name(&u.ship_system())));
    if let Some(f) = f {
        for q in u.market_quotes(f).iter().take(12) {
            println!("  {:<28} level {:>8.1} t  buy {:?}  sell {:.0}", u.world.goods[q.offer.item].name, q.level, q.buy.map(|b| b.round()), q.sell);
        }
    }
}
