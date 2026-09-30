//! Spaceports (pads on the surface of planets and moons), and the landing
//! gear that sets a ship down on the ground.

use glam::DVec3;
use universe_physics::{Contact, Feature};

use crate::damage;
use crate::events::ShipEvent;
use crate::ship::{upright, Ship, ShipState, SHIP_RADIUS};
use crate::system::StarSystem;

/// Touching down within this distance of the pad center counts as landing at the port (m).
pub const PAD_RADIUS: f64 = 250.0;
/// Touching a surface slower than this lands instead of crashing (m/s).
pub const LAND_SPEED: f64 = 30.0;

/// Landing pads per spaceport, in a square grid (3 × 3), this far apart (m).
pub const PADS: usize = 9;
pub const PAD_SPACING: f64 = 150.0;
/// The pad in the middle of the grid.
pub const CENTER_PAD: usize = 4;
/// A ship on the ground within this of a pad's center is on that pad (m).
pub const PAD_SIZE: f64 = 45.0;

/// The local north and east on a body at `dir` (unit, body frame): north
/// toward the body's +Y pole.
pub fn tangent(dir: DVec3) -> (DVec3, DVec3) {
    let north = (DVec3::Y - dir * dir.y).try_normalize().unwrap_or_else(|| (DVec3::Z - dir * dir.z).normalize());
    (north, north.cross(dir))
}

/// Direction (body frame, unit) of pad `pad` (0..`PADS`, row by row from
/// the north-west) of spaceport `port`.
pub fn pad_direction(sys: &StarSystem, port: usize, pad: usize) -> DVec3 {
    let sp = &sys.spaceports[port];
    let r = sys.bodies[sp.body].rail.radius;
    let (north, east) = tangent(sp.direction);
    let (row, col) = ((pad / 3) as f64 - 1.0, (pad % 3) as f64 - 1.0);
    (sp.direction * r + north * (-row * PAD_SPACING) + east * (col * PAD_SPACING)).normalize()
}

/// The pad of spaceport `port` at a point on its body (body-frame direction), if any.
pub fn pad_at(sys: &StarSystem, port: usize, local_dir: DVec3) -> Option<usize> {
    let r = sys.bodies[sys.spaceports[port].body].rail.radius;
    (0..PADS).find(|&k| pad_direction(sys, port, k).angle_between(local_dir) * r < PAD_SIZE)
}

/// Where spaceport `port`'s pad center is at `t` (`positions` at `t`).
pub fn pad_position(sys: &StarSystem, port: usize, t: f64, positions: &[DVec3]) -> DVec3 {
    let sp = &sys.spaceports[port];
    let b = &sys.bodies[sp.body];
    let up = b.rotation(t) * sp.direction;
    positions[sp.body] + up * b.rail.radius
}

/// The landing gear, on surface contact at `t`: slow enough on solid ground
/// lands (belly down, engines off); an ocean, too fast, or a body that can't
/// be landed on destroys the ship.
pub fn touch_down(sys: &StarSystem, ship: &mut Ship, c: &Contact, t: f64, events: &mut Vec<ShipEvent>) {
    let b = &sys.bodies[c.body];
    if c.feature == (Feature::Surface { liquid: true }) {
        let name = format!("{} ocean", b.name);
        damage::destroy(ship, &name, events);
        return;
    }
    if b.kind.landable() && c.relative_velocity.length() < LAND_SPEED {
        let rot = b.rotation(t);
        let local = c.local;
        let orientation = upright(c.normal, ship.forward());
        ship.orientation = orientation;
        ship.throttle = 0.0;
        ship.angular_velocity = DVec3::ZERO;
        ship.state = ShipState::Landed {
            body: c.body,
            local_position: local * (b.surface_radius(local) + SHIP_RADIUS),
            local_orientation: rot.inverse() * orientation,
        };
        ship.rcs = DVec3::ZERO;
        match sys.port_at(c.body, local) {
            Some(p) => events.push(ShipEvent::LandedAtPort { port: sys.spaceports[p].name.clone() }),
            None => events.push(ShipEvent::Landed { body: b.name.clone(), station: false }),
        }
    } else {
        let name = b.name.clone();
        damage::destroy(ship, &name, events);
    }
}

/// The landing gear's lift-off, for a ship resting on the ground with local
/// vertical `normal`: a burn with the nose up, or the lift thrusters with the
/// belly down, lifts it clear. Returns whether it took off.
pub fn lift_off(ship: &mut Ship, normal: DVec3, events: &mut Vec<ShipEvent>) -> bool {
    let nose_up_burn = ship.throttle > 0.05 && ship.forward().dot(normal) > 0.2;
    let lift = ship.rcs.y > 0.1 && (ship.orientation * DVec3::Y).dot(normal) > 0.5;
    if !(nose_up_burn || lift) {
        return false;
    }
    ship.velocity += normal * 3.0;
    ship.position += normal * 2.0;
    ship.state = ShipState::Flying;
    events.push(ShipEvent::TookOff);
    true
}
