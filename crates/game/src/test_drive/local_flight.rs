//! An owned sandbox using exactly the world's normal ship step. This client
//! schedules commands and observes results; it implements no physical laws.
use std::sync::Arc;
use universe_engine::glam::{DQuat, DVec3};
use universe_engine::scene::TickClock;
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
    clock: TickClock,
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
            clock: TickClock::new(universe_sim::clocks::tick(), 40).map_err(str::to_owned)?,
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
        clock
            .advance(elapsed, |_, dt| {
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
            })
            .map_err(str::to_owned)?;
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
        self.clock.pending()
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

    #[test]
    fn edges_survive_subtick_frames_and_reset_releases_owned_world_and_spec() {
        let spec = fixture();
        let weak = Arc::downgrade(&spec);
        let mut local = LocalFlight::new(spec, None, 9.0, 0, false).unwrap();
        let system = Arc::downgrade(&local.world.system(0));
        local.commands.power = Some(true);
        let dt = universe_sim::clocks::tick();
        local.advance(dt * 0.25).unwrap();
        assert_eq!(local.world.time, 0.0);
        assert_eq!(local.commands.power, Some(true));
        local.advance(dt).unwrap();
        assert!(local.ship.powered);
        assert_eq!(local.commands.power, None);
        local.advance(dt * 100.0).unwrap();
        assert!(
            local.pending() >= dt * 59.0,
            "catchup cap must retain elapsed time"
        );
        drop(local);
        assert!(weak.upgrade().is_none());
        assert!(system.upgrade().is_none());
        let reset = LocalFlight::new(fixture(), None, 9.0, 3, false).unwrap();
        assert_eq!(reset.world.time, 0.0);
        assert_eq!(reset.pending(), 0.0);
        assert!(reset.events.is_empty());
        assert!(!reset.ship.powered);
        assert_eq!(reset.density(), 0.0);
        let mut zero_g = LocalFlight::new(fixture(), None, 0.0, 3, true).unwrap();
        zero_g.ship.state = ShipState::Flying;
        zero_g.ship.position += DVec3::Y * 100.0;
        zero_g.advance(dt * 4.0).unwrap();
        assert!(zero_g.ship.position.is_finite() && zero_g.ship.velocity.is_finite());
        assert!(zero_g.ship.orientation.is_finite());
    }
    #[test]
    fn battery_fan_manual_takeoff_uses_shared_devices_and_cannot_lift_in_vacuum() {
        use universe_sim::world::{
            ship::{Thruster, ThrusterRole},
            ship_devices::{Battery, Fan, Fit},
        };
        // Explicit laboratory hardware: one centred fan, one full battery,
        // zero propellant and zero plant output. No fuel-gate workaround.
        let mut spec = (*fixture()).clone();
        spec.fuel_capacity = 0.0;
        spec.power_output = 0.0;
        spec.power_draw = 0.0;
        spec.thrusters = vec![Thruster {
            nozzle: "test_fan".into(),
            at: spec.dry_com,
            push: DVec3::Y,
            thrust: spec.dry_mass * 9.0 * 2.0,
            role: ThrusterRole::Lift,
            exhaust: 0.0,
            efficiency: 1.0,
        }];
        let spec = Arc::new(spec);
        let fit = Fit {
            fans: vec![Fan {
                thruster: 0,
                full_power: 1e6,
                min_density: 0.1,
                reference_density: universe_sim::physics::laws::EARTH_AIR_DENSITY,
            }],
            batteries: vec![Battery {
                capacity: 1e9,
                rate: 2e6,
                efficiency: 1.0,
                heat_capacity: 1e6,
                max_temperature: 500.0,
            }],
            legs: Vec::new(),
        };
        let mut air = LocalFlight::new(spec.clone(), Some(fit.clone()), 9.0, 0, true).unwrap();
        let mut vacuum = LocalFlight::new(spec, Some(fit), 9.0, 3, true).unwrap();
        for f in [&mut air, &mut vacuum] {
            f.commands.power = Some(true);
            f.commands.jets = Some(1);
            for _ in 0..100 {
                f.advance(universe_sim::clocks::tick()).unwrap();
            }
        }
        assert!(
            air.ship.is_flying(),
            "manual fan should take off through shared ground rules"
        );
        assert_eq!(air.ship.fuel, 0.0);
        assert!(air.ship.devices.as_ref().unwrap().packs[0].energy < 1e9);
        assert!(air.ship.position.is_finite() && air.ship.velocity.is_finite());
        assert!(matches!(vacuum.ship.state, ShipState::Landed { .. }));
        assert_eq!(vacuum.ship.devices.as_ref().unwrap().packs[0].energy, 1e9);
        assert!(vacuum.ship.applied.0.length() < 1e-10);
    }
}
