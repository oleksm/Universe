//! An owned sandbox using exactly the world's normal ship step. This client
//! schedules commands and observes results; it implements no physical laws.
use std::sync::Arc;
use universe_engine::glam::{DQuat, DVec3};
use universe_sim::clocks::{Runtime, Subscriptions};
use universe_sim::world::registry::ClockKey;
use universe_sim::physics::{Atmosphere, Collider, RailBody};
use universe_sim::world::{
    World,
    ship::ClassSpec,
    units::{EARTH_RADIUS, G},
};
use universe_sim::{
    Body, BodyKind, Galaxy, GalaxyStar, Ship, ShipCommands, ShipEvent, ShipState, StarClass,
    StarSystem,
};

pub(super) struct LocalFlight {
    pub world: World,
    pub ship: Ship,
    system: usize,
    clock: Runtime,
    /// The tangent-frame origin used only by the camera and instruments.
    pub origin: DVec3,
    pub commands: ShipCommands,
    pub events: Vec<ShipEvent>,
}

impl LocalFlight {
    pub fn new(
        spec: Arc<ClassSpec>,
        devices: Option<universe_sim::world::ship_devices::Fit>,
        gravity: f64,
        air: usize,
        manual: bool,
    ) -> Result<Self, String> {
        if !gravity.is_finite() || gravity < 0.0 {
            return Err("sandbox gravity must be finite and nonnegative".into());
        }
        let system = sandbox(gravity, air);
        let galaxy = Galaxy {
            seed: 0,
            stars: vec![GalaxyStar {
                position: DVec3::ZERO,
                class: StarClass::G,
                seed: 0,
            }],
        };
        let world = World::isolated(galaxy, system)?;
        let origin = DVec3::Y * EARTH_RADIUS;
        let mut ship = Ship::with_spec(spec, origin, DVec3::ZERO, DQuat::IDENTITY);
        if let Some(fit) = devices {
            ship.configure_devices(fit, 1.0, universe_sim::world::heat::AMBIENT)?;
        }
        let position = origin + DVec3::Y * ship.rest_height();
        ship.position = position;
        ship.state = ShipState::Landed {
            body: 1,
            local_position: position,
            local_orientation: DQuat::IDENTITY,
        };
        ship.powered = false;
        ship.manual = manual;
        let commands = ship.holding();
        Ok(Self {
            world,
            ship,
            system: 0,
            // (The realtime clock alone: this world has no economy, market or rails.)
            clock: Runtime::new(
                &Subscriptions::of(&[ClockKey::Realtime]),
                1.0,
                universe_sim::universe::TICK_BUDGET as u32,
            )?,
            origin,
            commands,
            events: Vec::new(),
        })
    }

    pub fn advance(&mut self, elapsed: f64) -> Result<(), String> {
        let Self {
            world,
            ship,
            system,
            clock,
            commands,
            events,
            ..
        } = self;
        if !elapsed.is_finite() || elapsed < 0.0 {
            return Err("invalid elapsed time".into());
        }
        clock.advance(elapsed, &mut |_, _, dt| {
                // Stay destroyed until the user resets: normal world's respawn
                // would construct a stock ship, which is not this design.
                if matches!(ship.state, ShipState::Destroyed { .. }) {
                    return;
                }
                world.step_ship(ship, system, commands, dt, 1.0, events);
                // Edge commands persist until a tick consumes them (even at high FPS).
                commands.power = None;
                commands.manual = None;
                commands.throttle = ship.throttle;
            });
        Ok(())
    }

    pub fn density(&self) -> f64 {
        let sys = self.world.system(0);
        let mut positions = Vec::new();
        sys.positions(self.world.time, &mut positions);
        universe_sim::physics::air_at(&sys.bodies, self.ship.position, &positions, self.world.time)
            .map_or(0.0, |(density, _)| density)
    }

    /// Instruments measure against the same spherical body used by Dogma,
    /// while the ground grid remains a local tangent-frame drawing.
    pub fn altitude(&self, local_point: DVec3) -> f64 {
        let p = self.ship.position + self.ship.orientation * local_point;
        let sys = self.world.system(0);
        let ground = &sys.bodies[1];
        p.length() - ground.surface_radius_at(DVec3::ZERO, p, self.world.time)
    }

    pub fn climb(&self) -> f64 {
        self.ship
            .velocity
            .dot(self.ship.position.normalize_or_zero())
    }

    pub fn ground_speed(&self) -> f64 {
        let up = self.ship.position.normalize_or_zero();
        (self.ship.velocity - up * self.climb()).length()
    }

    pub fn pending(&self) -> f64 {
        self.clock.pending(ClockKey::Realtime)
    }
}

/// Explicit, unlit laboratory: an inert star marker and a nonrotating smooth
/// planet. Radius/air use shared definitions; gravity is the draft's setting.
/// Contacts, takeoff, drag and heat are all the existing World's rules.
fn sandbox(gravity: f64, air: usize) -> StarSystem {
    let body = |name: &str, kind, radius, mu, collider, atmosphere| Body {
        key: format!("studio.{}", name.to_lowercase()),
        name: name.into(),
        kind,
        mass: mu / G,
        color: [0.2, 0.4, 0.3],
        rings: None,
        link: None,
        terrain: None,
        rock: None,
        rail: RailBody {
            parent: None,
            orbit: None,
            mu,
            attracts: mu > 0.0,
            radius,
            tilt: DQuat::IDENTITY,
            day: f64::INFINITY,
            collider,
            atmosphere,
            pulled_by: Vec::new(),
        },
    };
    let atmosphere = (air != 3).then(|| {
        let mut a = Atmosphere::earthlike(gravity);
        a.surface_density *= [1.0, 0.5, 0.25, 0.0][air];
        a
    });
    StarSystem {
        index: 0,
        name: "Studio laboratory".into(),
        class: StarClass::G,
        luminosity: 0.0,
        bodies: vec![
            body("Origin", BodyKind::Star, 0.0, 0.0, Collider::None, None),
            body(
                "Ground",
                BodyKind::Rocky,
                EARTH_RADIUS,
                gravity * EARTH_RADIUS.powi(2),
                Collider::Surface,
                atmosphere,
            ),
        ],
        spaceports: Vec::new(),
        fields: Vec::new(),
        small: 0..0,
        belts: Vec::new(),
        belt_seed: 0,
        patches: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // This is deliberately a known stock rocket test fixture. Production never
    // substitutes this for a draft; its constructor requires the compiler's spec.
    fn fixture() -> Arc<ClassSpec> {
        Arc::new(universe_sim::ship::starter().clone())
    }

    #[test]
    fn scheduled_flight_matches_direct_world_step_and_keeps_other_world_untouched() {
        let spec = fixture();
        let mut local = LocalFlight::new(spec.clone(), None, 9.0, 3, false).unwrap();
        let mut direct = LocalFlight::new(spec, None, 9.0, 3, false).unwrap();
        for f in [&mut local, &mut direct] {
            f.ship.state = ShipState::Flying;
            f.ship.position += DVec3::Y * 100.0;
            f.ship.orientation =
                DQuat::from_euler(universe_engine::glam::EulerRot::XYZ, 0.4, 0.7, -0.6);
            f.commands = ShipCommands {
                power: Some(true),
                throttle: 0.5,
                turn: Some(universe_sim::Controls {
                    pitch: 0.3,
                    yaw: -0.4,
                    roll: 0.2,
                }),
                ..f.ship.holding()
            };
        }
        let untouched = LocalFlight::new(fixture(), None, 9.0, 3, false).unwrap();
        let initial = untouched.ship.position;
        let dt = universe_sim::clocks::tick();
        for _ in 0..10 {
            local.advance(dt).unwrap();
            direct.world.step_ship(
                &mut direct.ship,
                &mut direct.system,
                &direct.commands,
                dt,
                1.0,
                &mut direct.events,
            );
            direct.commands.power = None;
        }
        assert_eq!(local.ship.position, direct.ship.position);
        assert_eq!(local.ship.velocity, direct.ship.velocity);
        assert_eq!(local.ship.orientation, direct.ship.orientation);
        assert_eq!(local.ship.angular_velocity, direct.ship.angular_velocity);
        assert_eq!(local.ship.fuel, direct.ship.fuel);
        assert_eq!(local.ship.energy, direct.ship.energy);
        assert_eq!(local.ship.skin_temp, direct.ship.skin_temp);
        assert_eq!(local.ship.jets, direct.ship.jets);
        assert_eq!(local.events, direct.events);
        assert_eq!(untouched.world.time, 0.0);
        assert_eq!(untouched.ship.position, initial);
        assert!(untouched.events.is_empty());
    }
    /// Flying `secs` seconds in realtime ticks, with no commands.
    fn fly(f: &mut LocalFlight, secs: f64) {
        let dt = universe_sim::clocks::tick();
        for _ in 0..(secs / dt) as usize {
            f.advance(dt).unwrap();
        }
    }

    fn dropped(height: f64, tilt: f64) -> LocalFlight {
        let mut f = LocalFlight::new(fixture(), None, 9.81, 0, false).unwrap();
        f.ship.state = ShipState::Flying;
        f.ship.position += DVec3::Y * height;
        f.ship.orientation = DQuat::from_rotation_x(tilt);
        f
    }

    #[test]
    fn dropped_too_hard_its_legs_give_way_and_the_hull_takes_the_fall() {
        // (From 2 m: 6.3 m/s down, twice what its legs are made for; then the
        // rest of their 12 m onto its belly.)
        let mut f = dropped(2.0, 0.0);
        fly(&mut f, 10.0);
        assert!(f.events.iter().any(|e| matches!(e, ShipEvent::LegGaveWay { .. })));
        assert!(f.ship.hull < 1.0, "the hull struck the ground");
        // From 30 cm, set down tilted, the legs hold, the hull is untouched and
        // it comes to rest by itself, not stood up.
        let mut f = dropped(0.3, 0.15);
        fly(&mut f, 12.0);
        assert_eq!((f.ship.broken_legs, f.ship.hull), (0, 1.0));
        assert!(matches!(f.ship.state, ShipState::Landed { .. }) && f.ship.angular_velocity == DVec3::ZERO);
    }

    #[test]
    fn a_descent_like_the_autopilots_touches_down_and_comes_to_rest() {
        // Sinking at 1.5 m/s on the lift thrusters, engines off once down (as
        // `avionics::landing` does): landed, no leg lost, no hard landing.
        let mut f = dropped(20.0, 0.0);
        f.commands.power = Some(true);
        let dt = universe_sim::clocks::tick();
        for _ in 0..(40.0 / dt) as usize {
            if f.ship.is_flying() {
                let down = f.ship.ground.as_ref().is_some_and(|g| g.last.touching > 0);
                let up = f.ship.position.normalize();
                let accel = (up * -1.5 - f.ship.velocity) * 2.0 + up * 9.81;
                f.commands.rcs = if down { DVec3::ZERO } else { f.ship.thruster_command(f.ship.orientation.inverse() * accel) };
            }
            f.advance(dt).unwrap();
        }
        assert!(matches!(f.ship.state, ShipState::Landed { .. }), "landed, not {:?}", f.ship.state);
        assert_eq!(f.ship.broken_legs, 0);
        assert!(!f.events.iter().any(|e| matches!(e, ShipEvent::HardLanding { .. } | ShipEvent::Crashed { .. })), "{:?}", f.events);
        // At rest and powered up with nothing commanded, it stays put: it never
        // turns in place (the old weld spun a powered landed ship about its centre).
        let (p, q) = (f.ship.position, f.ship.orientation);
        f.commands.rcs = DVec3::ZERO;
        f.commands.power = Some(true);
        fly(&mut f, 3.0);
        assert!(f.ship.powered && matches!(f.ship.state, ShipState::Landed { .. }));
        assert!(f.ship.position.distance(p) < 1e-6 && f.ship.orientation.angle_between(q) < 1e-9, "never turned in place");
    }
}
