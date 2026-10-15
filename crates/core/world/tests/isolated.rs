use std::sync::Arc;

use glam::{DQuat, DVec3};
use universe_world::{
    galaxy::{Galaxy, GalaxyStar, StarClass},
    ship::{starter, ClassSpec, Ship},
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
fn worlds_keep_explicit_systems_and_isolate_time_caches_and_events() {
    let mut a = local();
    let b = local();
    let a_system = a.system(0);
    let b_system = b.system(0);
    assert!(!Arc::ptr_eq(&a_system, &b_system));
    assert_eq!(a_system.bodies.len(), 1);
    assert!(a.gate_links.is_empty());
    assert!(a.goods.is_empty());
    assert!(a.neighbours(0).is_empty());
    let charts = a.charts();
    assert!(Arc::ptr_eq(&a_system, &charts.system(0)));
    assert!(!Arc::ptr_eq(&a.rails_now(0), &b.rails_now(0)));
    assert!(!Arc::ptr_eq(&a.rules_of(0), &b.rules_of(0)));
    a.dig(0, 1, 2);
    assert_eq!(b.dug(0, 1, 2), 0.0);
    a.freeze(&[0], 0.0, 0.25);
    a.thaw();
    assert!(Arc::ptr_eq(&a_system, &a.system(0)));

    let mut ship = Ship::with_spec(Arc::new(starter().clone()), DVec3::Y * 1000.0, DVec3::X, DQuat::IDENTITY);
    let mut events = Vec::new();
    let other_events: Vec<universe_world::events::ShipEvent> = Vec::new();
    let mut commands = ship.holding();
    commands.arm = Some(true);
    let result = a.step_ship(&mut ship, &mut 0, &commands, 0.25, 1.0, &mut events);
    assert_eq!(result.simulated, 0.25);
    assert_eq!(a.time, 0.25);
    assert_eq!(b.time, 0.0);
    assert!((ship.position.x - 0.25).abs() < 1e-12);
    assert!(!events.is_empty());
    assert!(other_events.is_empty());
    assert!(b.slugs.is_empty() && b.missiles.is_empty() && b.impacts.is_empty() && b.beams.is_empty());

    let weak = Arc::downgrade(&a_system);
    drop(a_system);
    drop(a);
    assert!(weak.upgrade().is_some(), "charts retain their explicit system");
    drop(charts);
    assert!(weak.upgrade().is_none(), "no global system cache retains a session");
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
fn owned_assembly_and_ship_clones_release_the_definition_and_shape() {
    let stock = starter();
    let shape = Arc::new(stock.shape().clone());
    let weak_shape = Arc::downgrade(&shape);
    let spec = ClassSpec::assemble_owned(stock.key.clone(), "Local".into(), shape, stock.frame.clone(), stock.fit.clone()).unwrap();
    assert_eq!(spec.dry_mass, stock.dry_mass);
    assert_eq!(spec.thrusters, stock.thrusters);
    let spec = Arc::new(spec);
    let weak_spec = Arc::downgrade(&spec);
    let ship = Ship::with_spec(spec, DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
    let mut clone = ship.clone();
    clone.refresh();
    assert!(std::ptr::eq(ship.spec(), clone.spec()));
    let rigid = ship.rigid();
    assert!(matches!(rigid.parts, std::borrow::Cow::Owned(_)));
    assert_eq!(&*rigid.parts, &ship.spec().shape().spheres);
    drop(ship);
    assert!(weak_spec.upgrade().is_some());
    drop(clone);
    assert!(weak_spec.upgrade().is_none());
    assert!(weak_shape.upgrade().is_none());
    // The physics snapshot owns its contact data, even after the definition dies.
    assert!(!rigid.parts.is_empty());
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

#[test]
fn owned_parts_preserve_the_builders_uniform_centroid_frame() {
    use universe_world::shape::Shape;
    let centre = DVec3::new(100.0, 20.0, 30.0);
    let cube = |at: DVec3| (0..8).map(|i| at + DVec3::new(
        if i & 1 == 0 { -3.0 } else { 3.0 },
        if i & 2 == 0 { -3.0 } else { 3.0 },
        if i & 4 == 0 { -3.0 } else { 3.0 },
    )).collect();
    let shape = Shape::from_parts("local.parts".into(), vec![cube(centre - DVec3::X * 10.0), cube(centre + DVec3::X * 10.0)],
        vec![("mount_test".into(), centre + DVec3::Y * 2.0, DVec3::NEG_Z)]).unwrap();
    assert!(shape.made_centre.distance(centre) < 1e-10);
    assert_eq!(shape.solid.centroid, DVec3::ZERO);
    assert!(shape.node("mount_test").unwrap().at.distance(DVec3::Y * 2.0) < 1e-10);
    assert_eq!(shape.parts.len(), 2);
    let origin = centre + DVec3::new(1.0, 2.0, 3.0);
    let shifted = Shape::from_parts_at("local.origin".into(), vec![cube(centre - DVec3::X * 10.0), cube(centre + DVec3::X * 10.0)],
        vec![("mount_test".into(), centre + DVec3::Y * 2.0, DVec3::NEG_Z)], origin).unwrap();
    let delta = centre - origin;
    assert_eq!(shifted.made_centre, origin);
    assert!(shifted.solid.centroid.distance(delta) < 1e-10);
    assert_eq!(shifted.solid.inertia, shape.solid.inertia);
    for (a, b) in shifted.mesh.points.iter().zip(&shape.mesh.points) {
        assert!(a.distance(*b + delta) < 1e-10);
    }
    assert!(shifted.node("mount_test").unwrap().at.distance(shape.node("mount_test").unwrap().at + delta) < 1e-10);
    for (a, b) in shifted.spheres.iter().zip(&shape.spheres) {
        assert!(a.at.distance(b.at + delta) < 1e-10);
        assert!((a.radius - b.radius).abs() < 1e-10);
    }
    for (a, b) in shifted.solids.iter().flatten().zip(shape.solids.iter().flatten()) {
        assert!(a.0.distance(b.0) < 1e-10);
        assert!((a.1 - (b.1 + b.0.dot(delta))).abs() < 1e-10);
    }
    for (a, b) in shifted.parts.iter().zip(&shape.parts) {
        assert!(a.centre.distance(b.centre + delta) < 1e-10);
        for (pa, pb) in a.probes.iter().zip(&b.probes) {
            assert!(pa.distance(*pb + delta) < 1e-10);
        }
    }
    assert!(Shape::from_parts("empty".into(), Vec::new(), Vec::new()).is_err());
    let weak = {
        let owned = Arc::new(shape);
        Arc::downgrade(&owned)
    };
    assert!(weak.upgrade().is_none());
}

#[test]
fn uncommissioned_build_keeps_missing_capabilities_and_power_shortfall_explicit() {
    use universe_world::modules::Does;
    use universe_world::ship::Slot;
    let stock = starter();
    let shape = Arc::new(stock.shape().clone());
    let mut frame = stock.frame.clone();
    frame.slots.clear();
    frame.nozzles.clear();
    let bare = ClassSpec::assemble_uncommissioned_owned("local.bare".into(), "Bare".into(), shape.clone(), frame.clone(), Vec::new()).unwrap();
    assert!(bare.fit.is_empty());
    assert_eq!(bare.dry_mass, frame.frame_mass);
    assert_eq!((bare.comm.capture, bare.comm.link, bare.comm.lag, bare.comm.capacity), (0.0, 0.0, 0.0, 0.0));
    assert_eq!((bare.turn_rate, bare.roll_rate), (0.0, 0.0));
    assert_eq!((bare.power_output, bare.power_draw), (0.0, 0.0));
    assert!(ClassSpec::assemble_owned("local.bare".into(), "Bare".into(), shape.clone(), frame.clone(), Vec::new()).is_err());
    let (handle, module) = universe_world::content::content().modules.iter()
        .find(|(_, m)| matches!(m.does, Does::Sensors { .. }) && m.power > 0.0).unwrap();
    frame.slots.push(Slot { name: "sensor".into(), kind: module.does.slot(), size: module.size, mount: None });
    let fit = vec![("sensor".into(), handle)];
    let spec = ClassSpec::assemble_uncommissioned_owned("local.sensor".into(), "Sensor".into(), shape, frame, fit.clone()).unwrap();
    assert_eq!(spec.fit, fit);
    assert_eq!(spec.power_output, 0.0);
    assert_eq!(spec.power_draw, module.power);
    assert_eq!(spec.dry_mass, stock.frame.frame_mass + module.mass);
    let ship = Ship::with_spec(Arc::new(spec), DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
    assert_eq!(ship.spare_power(), 0.0);
}
