use std::sync::Arc;
use universe_world::{
    ship::{ShipState, Thruster, ThrusterRole},
    ship_devices::{Battery, Fan, Fit, LandingLeg},
};

use glam::{DQuat, DVec3};
use universe_world::{
    galaxy::{Galaxy, GalaxyStar, StarClass},
    ship::{starter, Ship},
    system::StarSystem,
    World,
};

fn explicit() -> (Galaxy, StarSystem) {
    let galaxy = Galaxy {
        seed: 0,
        stars: vec![GalaxyStar {
            position: DVec3::ZERO,
            class: StarClass::G,
            seed: 0,
        }],
    };
    let system = StarSystem {
        index: 0,
        name: "Local vacuum".into(),
        class: StarClass::G,
        luminosity: 0.0,
        bodies: vec![universe_world::system::Body {
            key: "local.root".into(),
            name: "Inert root".into(),
            kind: universe_world::system::BodyKind::Star,
            mass: 0.0,
            color: [0.0; 3],
            rings: None,
            link: None,
            terrain: None,
            rock: None,
            rail: universe_physics::RailBody {
                parent: None,
                orbit: None,
                mu: 0.0,
                attracts: false,
                radius: 0.0,
                tilt: DQuat::IDENTITY,
                day: 1.0,
                collider: universe_physics::Collider::None,
                atmosphere: None,
                pulled_by: Vec::new(),
            },
        }],
        spaceports: Vec::new(),
        fields: Vec::new(),
        small: 0..0,
        belts: Vec::new(),
        belt_seed: 0,
        patches: Default::default(),
    };
    (galaxy, system)
}

// Synthetic physical limits exercise the shared mechanism; not product tuning.
fn craft() -> Ship {
    let mut spec = starter().clone();
    spec.power_output = 0.;
    spec.power_draw = 0.;
    spec.dry_mass = 1000.;
    spec.dry_com = DVec3::ZERO;
    spec.dry_inertia = glam::DMat3::IDENTITY * 1000.;
    spec.thrusters = vec![Thruster {
        nozzle: "fan".into(),
        role: ThrusterRole::Lift,
        at: DVec3::ZERO,
        push: DVec3::Y,
        thrust: 20000.,
        exhaust: 0.,
        efficiency: 1.,
    }];
    let mut s = Ship::with_spec(
        Arc::new(spec),
        DVec3::Y * 1000.,
        DVec3::ZERO,
        DQuat::IDENTITY,
    );
    s.fuel = 0.;
    s.cargo = 0.;
    s.powered = true;
    s.manual = true;
    s.held = 1;
    s.configure_devices(
        Fit {
            fans: vec![Fan {
                thruster: 0,
                full_power: 100000.,
                min_density: 0.3,
                reference_density: 1.2,
            }],
            batteries: vec![Battery {
                capacity: 1000000.,
                rate: 100000.,
                efficiency: 0.8,
                heat_capacity: 1000.,
                max_temperature: 350.,
            }],
            legs: vec![LandingLeg {
                foot: DVec3::NEG_Y * 2.,
                holds: 30000.,
                stroke: 0.5,
                efficiency: 0.8,
                sink_rate: 2.,
                spring: None,
                damper: None,
            }],
            propellants: Vec::new(),
        },
        1.,
        300.,
    )
    .unwrap();
    s.devices.as_mut().unwrap().density = 1.2;
    s
}
fn main() {
    let mut s = craft();
    s.drive(None, 1., true);
    assert!((s.applied.0.y - 20000.).abs() < 1e-8);
    assert_eq!(s.fuel_flow(), 0.);
    s.burn(0.25);
    let pack = &s.devices.as_ref().unwrap().packs[0];
    assert!((pack.energy - 968750.).abs() < 1e-8);
    assert!((pack.temperature - 306.25).abs() < 1e-8);
    let mut assisted = craft();
    assisted.manual = false;
    assisted.held = 0;
    assisted.rcs = DVec3::Y;
    assisted.drive(None, 0.1, true);
    assert!(
        assisted.applied.0.y > 19000.,
        "assisted fan allocation retains lift with zero yaw authority"
    );
    let mut split = craft();
    split.drive(None, 1., true);
    for _ in 0..4 {
        split.burn(0.25);
    }
    let mut whole = craft();
    whole.drive(None, 1., true);
    whole.burn(1.);
    assert_eq!(
        split.devices.as_ref().unwrap().packs[0].energy,
        whole.devices.as_ref().unwrap().packs[0].energy
    );
    assert!((whole.devices.as_ref().unwrap().packs[0].temperature - 325.).abs() < 1e-8);
    for condition in 0..3 {
        let mut s = craft();
        let d = s.devices.as_mut().unwrap();
        match condition {
            0 => d.packs[0].energy = 0.,
            1 => d.packs[0].temperature = 350.,
            _ => d.density = 0.,
        };
        s.drive(None, 1., true);
        assert_eq!(s.applied.0, DVec3::ZERO);
        assert_eq!(s.jets[0], 0.);
    }
    let mut last = craft();
    last.devices.as_mut().unwrap().packs[0].energy = 100.;
    last.drive(None, 1., true);
    assert!(last.applied.0.y > 0. && last.applied.0.y < 200.);
    last.burn(1.);
    assert!(last.devices.as_ref().unwrap().packs[0].energy.abs() < 1e-10);
    last.drive(None, 1., true);
    assert_eq!(last.applied.0, DVec3::ZERO);
    let mut thermal = craft();
    thermal.devices.as_mut().unwrap().packs[0].temperature = 349.9;
    thermal.drive(None, 1., true);
    thermal.burn(1.);
    assert!((thermal.devices.as_ref().unwrap().packs[0].temperature - 350.).abs() < 1e-10);
    let fit = &s.devices.as_ref().unwrap().fit;
    assert!(fit.landing().unwrap().hardest(1000., 10.) > 0.);
    assert_eq!(fit.landing().unwrap().hardest(4000., 10.), 0.);
    assert_eq!(s.rest_height(), 2.);
    let mut invalid = fit.clone();
    invalid.fans.push(invalid.fans[0].clone());
    assert!(invalid.validate(1).is_err());
    let (_, mut sys) = explicit();
    sys.bodies[0].rail.mu = 10e6;
    let mut rules = universe_world::rules::Rules::default();
    rules.set_release(
        0,
        universe_world::rules::Release::LiftOff {
            speed: 1.,
            lift: 1.,
            up: Some(DVec3::Y),
            says: universe_world::events::ShipEvent::SystemsOff,
        },
    );
    for empty in [false, true] {
        let mut s = craft();
        if empty {
            s.devices.as_mut().unwrap().packs[0].energy = 0.;
        }
        s.state = ShipState::Landed {
            body: 0,
            local_position: DVec3::Y * 1000.,
            local_orientation: DQuat::IDENTITY,
        };
        s.drive(None, 0.1, true);
        let released = universe_world::rules::release(
            &rules,
            &sys,
            0,
            DVec3::Y * 1000.,
            &mut s,
            0.,
            &mut Vec::new(),
        );
        assert_eq!(released, !empty);
    }
    for empty in [false, true] {
        let (galaxy, mut sys) = explicit();
        let b = &mut sys.bodies[0];
        b.kind = universe_world::system::BodyKind::Rocky;
        b.rail.mu = 10e6;
        b.rail.radius = 1000.;
        b.rail.day = 1e12;
        b.rail.atmosphere = Some(universe_physics::Atmosphere {
            surface_density: 1.2,
            scale_height: 10000.,
            top: 100000.,
        });
        let mut world = World::isolated(galaxy, sys).unwrap();
        let mut s = craft();
        if empty {
            s.devices.as_mut().unwrap().packs[0].energy = 0.;
        }
        s.state = ShipState::Landed {
            body: 0,
            local_position: DVec3::Y * 1000.,
            local_orientation: DQuat::IDENTITY,
        };
        let commands = s.holding();
        world.step_ship(&mut s, &mut 0, &commands, 0.01, 1., &mut Vec::new());
        assert_eq!(
            s.is_flying(),
            !empty,
            "real landed World tick must obey pack limits"
        );
        assert!(s.devices.as_ref().unwrap().density > 1.19);
        if !empty {
            assert!(s.devices.as_ref().unwrap().packs[0].energy < 1e6);
            let before = s.velocity.y;
            world.step_ship(&mut s, &mut 0, &commands, 0.01, 1., &mut Vec::new());
            assert!(
                s.velocity.y > before,
                "fuel-free fan must accelerate through the shared physics Driver"
            );
        }
    }
    println!("PASS: manual fuel-free fan, partial-span debit, thermal/energy/air limits, final charge, landing limits and realized-lift release");
}
