use std::sync::Arc;

use glam::{DQuat, DVec3};
use universe_world::{
    galaxy::{Galaxy, GalaxyStar, StarClass},
    ship::{starter, Ship},
    system::StarSystem,
    World,
};

fn explicit() -> (Galaxy, StarSystem) {
    let galaxy = Galaxy { seed: 0, stars: vec![GalaxyStar { position: DVec3::ZERO, class: StarClass::G, seed: 0 }] };
    let system = StarSystem {
        index: 0, name: "Local vacuum".into(), class: StarClass::G, luminosity: 0.0,
        bodies: vec![universe_world::system::Body {
            key: "local.root".into(), name: "Inert root".into(), kind: universe_world::system::BodyKind::Star,
            mass: 0.0, color: [0.0; 3], rings: None, link: None, terrain: None, rock: None,
            rail: universe_physics::RailBody {
                parent: None, orbit: None, mu: 0.0, attracts: false, radius: 0.0,
                tilt: DQuat::IDENTITY, day: 1.0, collider: universe_physics::Collider::None,
                atmosphere: None, pulled_by: Vec::new(),
            },
        }], spaceports: Vec::new(), fields: Vec::new(), small: 0..0,
        belts: Vec::new(), belt_seed: 0, patches: Default::default(),
    };
    (galaxy, system)
}

fn local() -> World {
    let (galaxy, system) = explicit();
    World::isolated(galaxy, system).unwrap()
}

#[test]
fn isolated_rejects_inconsistent_indices_and_never_generates_unknown_systems() {
    let (mut galaxy, system) = explicit();
    galaxy.stars.clear();
    assert!(World::isolated(galaxy, system).is_err());
    let (galaxy, mut system) = explicit();
    system.index = 1;
    assert!(World::isolated(galaxy, system).is_err());
    let (galaxy, mut system) = explicit();
    system.bodies.clear();
    assert!(World::isolated(galaxy, system).is_err());
    let world = local();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.system(1))).is_err());
    let charts = world.charts();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| charts.system(1))).is_err());
    assert_eq!(world.system(0).name, "Local vacuum");
}

#[test]
fn repeated_edits_do_not_register_hulls_or_reuse_stale_authority() {
    let content = universe_world::content::content();
    let before = content.hulls.iter().count();
    let mut spec = starter().clone();
    let powered = spec.authority(spec.fuel_capacity, 0.0);
    assert!(powered.main > 0.0);
    // Same address, different solver inputs: an address-keyed cache fails here.
    spec.thrusters.iter_mut().for_each(|t| t.thrust = 0.0);
    assert_eq!(spec.authority(spec.fuel_capacity, 0.0).main, 0.0);
    for edit in 0..300 {
        let mut spec = starter().clone();
        spec.key = format!("local.edit-{edit}");
        let shape = Arc::new(spec.shape().clone());
        let weak_shape = Arc::downgrade(&shape);
        spec.shape_own = Some(shape);
        let spec = Arc::new(spec);
        let weak_spec = Arc::downgrade(&spec);
        let ship = Ship::with_spec(spec, DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        let _ = ship.authority();
        drop(ship);
        assert!(weak_spec.upgrade().is_none());
        assert!(weak_shape.upgrade().is_none());
    }
    assert_eq!(content.hulls.iter().count(), before);
}
