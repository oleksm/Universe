use glam::{DQuat, DVec3};
use std::{fs, sync::Arc};
use universe_world::{Ship, ShipState};
fn main() {
    for model in [
        "assets/models/mc07.glb",
        "assets/models/miner.glb",
        "assets/models/test_hull.glb",
    ] {
        let bytes = fs::read(model).unwrap();
        match universe_world::import::hull_from_gltf(&bytes, model) {
            Err(e) => println!("{model}: {e}"),
            Ok(spec) => {
                let mut ship =
                    Ship::with_spec(Arc::new(spec), DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
                ship.state = ShipState::Landed {
                    body: 0,
                    local_position: DVec3::ZERO,
                    local_orientation: DQuat::IDENTITY,
                };
                let s = ship.spec();
                println!(
                    "{model}: name={} mass={} fuel={} main={} lift={} walk={:?} seat={:?} inside={:?} stair={:?} ramp={:?}",
                    s.name,
                    ship.mass(),
                    ship.fuel,
                    ship.main_accel(),
                    ship.lift_accel(),
                    s.shape().walk.as_ref().map(|m| (m.lo, m.hi)),
                    universe_world::crew::beside_seat(&ship),
                    universe_world::crew::inside_hatch(&ship, 0.),
                    universe_world::crew::stair(&ship),
                    s.shape().ramp.as_ref().map(|r| (r.hinge, r.axis, r.length))
                );
            }
        }
    }
}
