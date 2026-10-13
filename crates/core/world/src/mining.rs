//! Working an asteroid: hitting one, anchoring to one, digging it.
//!
//! An asteroid a kilometre across pulls with a ten-thousandth of a g; you
//! can't land on it, you hold on. The anchor is a harpoon on a tether: it
//! reaches its rig's `anchor_reach` from the hull, and it holds only if the ship is
//! drifting with the rock's surface there (slower than its `anchor_speed`
//! against it: a spinning rock's surface moves). Held, the ship rides the
//! rock's orbit and spin, its devices idle, until the anchor lets go and
//! leaves it drifting with that surface.
//!
//! Touching a rock is a collision like any other: a bounce, and the energy
//! lost in it goes into the hull (the rock, a billion times heavier, doesn't
//! notice).
//!
//! The excavator digs while anchored, at the rate its power buys against the
//! energy it takes to break a kilogram of that rock loose — scooping a rubble
//! pile's gravel is cheap, cutting a stone harder, cutting metal hardest —
//! and no faster than it can carry spoil (its `Rig`'s `excavator_power` and
//! `throughput`, `specific_energy`). What it digs is the rock's
//! ore (`ore`), into the hopper, and a tonne at a time into the hold. A rock
//! holds what its mass holds: the world remembers what's been dug out of
//! each (`World::dug`), and a rock worked out stays worked out.

use glam::DVec3;
use universe_physics::{Contact, RigidBody, Weld};

use crate::belt::{Rock, Structure};
use crate::damage;
use crate::events::ShipEvent;
use crate::goods::{Ore, TONNE};
use crate::ship::{Ship, ShipState, SHIP_RADIUS};
use crate::system::{Body, StarSystem};

/// A mining rig, as its product's record has it: the excavator's power (W)
/// and the most spoil it can carry off (kg/s); how far its anchor reaches
/// from the hull (m), and how slowly the ship must drift against the
/// surface for it to hold (m/s).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct Rig {
    pub excavator_power: f64,
    pub throughput: f64,
    pub anchor_reach: f64,
    pub anchor_speed: f64,
}

impl Rig {
    /// The rig fitted to a ship of `spec`, if one is.
    pub fn of(spec: &crate::ship::ClassSpec) -> Option<Rig> {
        let c = crate::content::content();
        spec.fit.iter().find_map(|(_, m)| match c.get(*m).does {
            crate::modules::Does::MiningRig(r) => Some(r),
            _ => None,
        })
    }

    /// A rig as the shipyards sell them (the registry's first): for weighing
    /// rocks up where no ship's own is at hand.
    pub fn common() -> Rig {
        crate::content::content()
            .modules
            .iter()
            .find_map(|(_, m)| match m.does {
                crate::modules::Does::MiningRig(r) => Some(r),
                _ => None,
            })
            .expect("a mining rig among the products")
    }

    /// How fast it digs `rock` (kg/s).
    pub fn dig_rate(&self, rock: &Rock) -> f64 {
        (self.excavator_power / specific_energy(rock)).min(self.throughput)
    }
}
/// A rubble pile, gravel held by its own weak gravity, scoops up at this
/// (J/kg), whatever it's made of.
const RUBBLE_ENERGY: f64 = 2_000.0;

/// Energy to break a kilogram of `rock` loose (J/kg): gravel scoops up; a
/// solid piece cuts as its class's record says (ice easily, stone harder,
/// nickel-iron hardest).
pub fn specific_energy(rock: &Rock) -> f64 {
    match rock.structure {
        Structure::Rubble => RUBBLE_ENERGY,
        Structure::Monolith => rock.class.record().mining.cut_energy.unwrap_or_else(|| panic!("{:?} has no cut energy", rock.class)),
    }
}

/// What digging `rock` yields: its class's rock (`yields`), or its rich rock
/// (`rich_yields`) where it holds more platinum-group metals than
/// `rich_above`; as the game's ore that good is.
pub fn ore(rock: &Rock) -> Ore {
    let m = &rock.class.record().mining;
    let rich = m.rich_above.is_some_and(|share| rock.composition.pgm_ppm * 1e-6 >= share);
    let good = if rich { m.rich_yields.as_ref() } else { m.yields.as_ref() }.unwrap_or_else(|| panic!("{:?} yields nothing", rock.class));
    Ore::from_key(good).unwrap_or_else(|| panic!("{good} is no ore the game's excavators dig"))
}

/// Closing on a rock to anchor, a ship holds this far off its surface (m).
pub const CLOSE_STANDOFF: f64 = 12.0;
/// Share of the closing speed kept in a bounce off a rock.
pub const RESTITUTION: f64 = 0.3;
/// Slower than this, touching a rock just eases the ship off it (m/s).
const IMPACT: f64 = 0.3;

/// The ship touched asteroid `rock` (contact `c`): the energy lost goes into
/// the hull. False if that wrecked it.
pub fn strike(ship: &mut Ship, rock: &Body, c: &Contact, events: &mut Vec<ShipEvent>) -> bool {
    let closing = -c.relative_velocity.dot(c.normal);
    if closing < IMPACT {
        return true;
    }
    let joules = 0.5 * ship.mass() * closing * closing * (1.0 - RESTITUTION * RESTITUTION);
    let damage = joules / ship.spec().hull_strength;
    ship.hull = (ship.hull - damage).max(0.0);
    events.push(ShipEvent::StruckRock { body: rock.name.clone(), speed: closing, damage });
    if ship.hull <= 0.0 {
        damage::destroy(ship, &rock.name, events);
        return false;
    }
    true
}

/// How far the hull is from asteroid `i`'s surface (m), at `t`.
pub fn clearance(bodies: &[Body], i: usize, t: f64, positions: &[DVec3], p: DVec3) -> f64 {
    let b = &bodies[i];
    p.distance(positions[i]) - b.surface_radius_at(positions[i], p, t) - SHIP_RADIUS
}

/// Fire the anchor at the nearest asteroid among field `field`'s bodies, at `t`.
pub fn anchor(field: Option<(usize, std::sync::Arc<Vec<Body>>)>, ship: &mut Ship, t: f64, events: &mut Vec<ShipEvent>) {
    let fail = |events: &mut Vec<ShipEvent>, why: &str| events.push(ShipEvent::AnchorFailed { why: why.to_string() });
    if !matches!(ship.state, ShipState::Flying) || ship.hyperdrive {
        return fail(events, "NOT IN FREE FLIGHT");
    }
    let Some(rig) = Rig::of(ship.spec()) else { return fail(events, "NO MINING RIG FITTED") };
    let Some((f, bodies)) = field else { return fail(events, "NOTHING IN REACH") };
    let mut positions = Vec::with_capacity(bodies.len());
    universe_physics::positions(&bodies[..], t, &mut positions);
    let nearest = (0..bodies.len())
        .filter(|&i| bodies[i].kind.is_rock() && positions[i].distance(ship.position) < bodies[i].max_radius() + SHIP_RADIUS + rig.anchor_reach)
        .map(|i| (i, clearance(&bodies, i, t, &positions, ship.position)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((i, _)) = nearest.filter(|(_, gap)| *gap < rig.anchor_reach) else { return fail(events, "NOTHING IN REACH") };
    let b = &bodies[i];
    let surface = universe_physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(ship.position - positions[i]);
    let drift = (ship.velocity - surface).length();
    if drift > rig.anchor_speed {
        return fail(events, &format!("DRIFTING {drift:.1} M/S"));
    }
    let weld = Weld::capture(&bodies[..], i, t, &positions, &ship.rigid());
    ship.state = ShipState::Anchored { field: f, body: i, local_position: weld.local_position, local_orientation: weld.local_orientation };
    ship.throttle = 0.0;
    ship.rcs = DVec3::ZERO;
    events.push(ShipEvent::Anchored { body: b.name.clone() });
}

/// Let go: drifting with the surface where the ship was held. The
/// excavator stops; loose ore in the hopper drifts off.
pub fn release(ship: &mut Ship, events: &mut Vec<ShipEvent>) {
    if matches!(ship.state, ShipState::Anchored { .. }) {
        ship.state = ShipState::Flying;
        ship.excavator = false;
        ship.hopper = 0.0;
        events.push(ShipEvent::AnchorReleased);
    }
}

/// `dt` seconds of the excavator on an anchored ship, `dug` kilograms having
/// been dug out of its rock already (before this ship's hopper).
pub fn excavate(sys: &StarSystem, ship: &mut Ship, dug: f64, dt: f64, events: &mut Vec<ShipEvent>) {
    let ShipState::Anchored { field, body, .. } = ship.state else { return };
    if !ship.excavator {
        return;
    }
    let bodies = sys.field_bodies(field);
    let b = &bodies[body];
    let Some(rock) = b.rock.as_ref() else { return };
    let Some(rig) = Rig::of(ship.spec()) else { return };
    let mut stop = |ship: &mut Ship, why: &str| {
        ship.excavator = false;
        events.push(ShipEvent::ExcavatorStopped { why: why.to_string() });
    };
    // (As much as the hold's weight and its space allow, for this ore.)
    let density = ore(rock).bulk_density();
    let room = ship.takes(density);
    let left = b.mass - dug - ship.hopper;
    if room < 1.0 {
        return stop(ship, "HOLD FULL");
    }
    if left < 1.0 {
        return stop(ship, "ROCK WORKED OUT");
    }
    ship.hopper += (rig.dig_rate(rock) * dt).min(room).min(left);
    let item = ore(rock).item();
    while ship.hopper >= TONNE - 1e-9 {
        ship.hopper = (ship.hopper - TONNE).max(0.0);
        ship.cargo += TONNE;
        ship.cargo_volume += TONNE / 1000.0 / density;
        events.push(ShipEvent::Mined { field, rock: body, item });
    }
}

/// Held: carried with the rock, at `t`.
pub fn hold(sys: &StarSystem, ship: &mut Ship, t: f64) {
    let ShipState::Anchored { field, body, local_position, local_orientation } = ship.state else { return };
    let bodies = sys.field_bodies(field);
    // (The rock alone, not the whole swarm.)
    let center = universe_physics::position(&bodies[..], body, t);
    let rail = &bodies[body].rail;
    let rot = rail.rotation(t);
    let offset = rot * local_position;
    let mut rigid: RigidBody = ship.rigid();
    rigid.position = center + offset;
    rigid.velocity = universe_physics::velocity(&bodies[..], body, t) + rail.angular_velocity().cross(offset);
    rigid.orientation = rot * local_orientation;
    rigid.angular_velocity = DVec3::ZERO;
    ship.set_rigid(&rigid);
}
