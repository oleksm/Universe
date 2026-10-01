//! The hyperdrive autopilot, and the navigation any hyperdrive jump needs:
//! where a jump to the nav target ends, which frame to move and drop out in,
//! steering around bodies on the way, and when it has arrived. (The drive
//! itself is the world's device: `universe_world::hyperdrive`.)

use glam::{DQuat, DVec3};
use universe_physics::integrate::FINE_STEP;
use universe_physics::segment_distance;
use universe_world::{BodyKind, Destination, HyperdriveCommand, Ship, ShipCommands, StarSystem};

use crate::docking;
use crate::landing::PadFrame;
use crate::nav::NavTarget;

/// Hyperdrive drops out this close to a targeted station (m).
pub const HYPER_ARRIVE_STATION: f64 = 20_000.0;
/// Hyperdrive drops out this close to a targeted spaceport (m).
pub const HYPER_ARRIVE_PORT: f64 = 120_000.0;
/// The hyperdrive autopilot aims this far above a targeted pad (m).
const HYPER_PORT_ALTITUDE: f64 = 100_000.0;
/// Dropping out of hyperdrive within this distance of the nav target leaves
/// the ship moving with it (m).
const HYPER_NEAR_TARGET: f64 = 1.0e6;

/// Where a hyperdrive jump to the nav target ends.
pub struct HyperAim {
    /// The target itself (station center or pad).
    pub target: DVec3,
    /// Drop out within this distance of `target`.
    arrive: f64,
    /// Where the hyperdrive autopilot heads for.
    aim: DVec3,
    /// The body the target sits on or orbits, which may block the way.
    body: usize,
    /// Velocity to match on arrival.
    pub velocity: DVec3,
    /// The frame hyperdrive motion is relative to: the target itself, or for
    /// a spaceport its planet (the ground's rotation only matters close in).
    frame_velocity: DVec3,
}

/// Within this many radii of a body, and short of the target, the drive
/// moves in the body's frame (see `navigate`).
const LOCAL_FRAME_RADII: f64 = 10.0;

/// What a hyperdrive arrival at `target` is announced as.
fn arrival_name(sys: &StarSystem, target: NavTarget) -> String {
    match target {
        NavTarget::Station(b) | NavTarget::Gate(b) => sys.bodies[b].name.clone(),
        NavTarget::Spaceport(p) => sys.spaceports[p].name.clone(),
        NavTarget::Asteroid(_) => target.name(sys),
    }
}

/// Hyperdrive destination for the nav target, if it's in this system (ship at `ship_pos`, bodies at `positions` at `t`).
pub fn aim(sys: &StarSystem, target: NavTarget, t: f64, positions: &[DVec3], ship_pos: DVec3) -> Option<HyperAim> {
    match target {
        NavTarget::Station(b) => {
            let body = sys.bodies.get(b).filter(|body| body.kind == BodyKind::Station)?;
            let at = positions[b];
            Some(HyperAim {
                target: at,
                arrive: HYPER_ARRIVE_STATION,
                aim: at,
                body: body.rail.parent.unwrap_or(0),
                velocity: sys.velocity(b, t),
                frame_velocity: sys.velocity(b, t),
            })
        }
        NavTarget::Gate(b) => {
            let body = sys.bodies.get(b).filter(|body| body.kind == BodyKind::Gate)?;
            let at = positions[b];
            Some(HyperAim {
                target: at,
                arrive: HYPER_ARRIVE_STATION,
                aim: at,
                body: body.rail.parent.unwrap_or(0),
                velocity: sys.velocity(b, t),
                frame_velocity: sys.velocity(b, t),
            })
        }
        NavTarget::Asteroid(b) => {
            // Out of its swarm on our side, moving with it. (Headed for
            // that point, not the rock itself, which would be in the way.)
            let field = sys.fields.iter().find(|f| f.body == b)?;
            let at = positions[b];
            let arrive = field.extent + HYPER_ARRIVE_STATION;
            Some(HyperAim {
                target: at,
                arrive,
                aim: at + (ship_pos - at).normalize_or(DVec3::Y) * arrive * 0.5,
                body: sys.bodies[b].rail.parent.unwrap_or(0),
                velocity: sys.velocity(b, t),
                frame_velocity: sys.velocity(b, t),
            })
        }
        NavTarget::Spaceport(p) => {
            sys.spaceports.get(p)?;
            let pad = PadFrame::new(sys, p, t, positions);
            Some(HyperAim {
                target: pad.pad,
                arrive: HYPER_ARRIVE_PORT,
                aim: pad.pad + pad.up * HYPER_PORT_ALTITUDE,
                body: sys.spaceports[p].body,
                velocity: pad.frame_velocity(ship_pos),
                frame_velocity: pad.body_velocity,
            })
        }
    }
}

/// The velocity to drop out of hyperdrive with: the nav target's, if it's
/// close; otherwise None (the drive leaves the ship with the dominant body).
pub fn exit_velocity(aim: Option<&HyperAim>, ship_pos: DVec3) -> Option<DVec3> {
    aim.filter(|a| a.target.distance(ship_pos) < HYPER_NEAR_TARGET).map(|a| a.velocity)
}

/// Navigation in hyperdrive, once a frame: drop out on arriving at the nav
/// `target` (returning its name), otherwise command its frame, destination
/// and exit velocity, and under the hyperdrive `autopilot` the heading
/// (around anything in the way; the point steered for goes to `debug_way`).
pub fn navigate(
    sys: &StarSystem,
    ship: &Ship,
    t: f64,
    positions: &[DVec3],
    target: Option<NavTarget>,
    autopilot: bool,
    debug_way: &mut Option<DVec3>,
) -> (ShipCommands, Option<String>) {
    let p = ship.position;
    let aim = target.and_then(|target| self::aim(sys, target, t, positions, p));
    let mut commands = ship.holding();

    // Arrived at the nav target: drop out moving with it. Asking for
    // clearance is left to the pilot.
    if let (Some(a), Some(target)) = (&aim, target)
        && p.distance(a.target) < a.arrive
    {
        commands.hyperdrive = Some(HyperdriveCommand { engage: false, exit_velocity: Some(a.velocity), ..Default::default() });
        return (commands, Some(arrival_name(sys, target)));
    }

    // Close to a body (just left, or passing) and not yet to the target, the
    // drive keeps the body's frame: in the target's, the body would sweep
    // into the ship at the difference of their speeds (a gate's orbital
    // speed round its own planet is enough).
    let local = sys.dominant(p, positions);
    let near_other = aim.as_ref().is_some_and(|a| a.target.distance(p) > HYPER_NEAR_TARGET && p.distance(positions[local]) < LOCAL_FRAME_RADII * sys.bodies[local].rail.radius);
    let mut orders = HyperdriveCommand {
        engage: true,
        start: false,
        heading: None,
        // (The hyperdrive autopilot steers around obstacles itself.)
        steering: autopilot,
        frame_velocity: aim.as_ref().filter(|_| !near_other).map(|a| a.frame_velocity),
        exit_velocity: exit_velocity(aim.as_ref(), p),
        destination: aim.as_ref().map(|a| Destination { point: a.target, body: a.body }),
    };
    // Direction of travel: where the nose points, unless the pilot handed
    // the jump to the hyperdrive autopilot.
    if let (Some(a), true) = (&aim, autopilot) {
        // Swing around whatever is in the way: the first body met along
        // the path, then re-check the detour leg itself (up to three
        // times). (For the target's own planet, "blocked" means dipping
        // below half the aim altitude, so the aim point just above a
        // pad never counts.)
        let first_blocker = |to: DVec3| -> Option<usize> {
            let seg = to - p;
            let len2 = seg.length_squared().max(1e-9);
            let mut best: Option<(f64, usize)> = None;
            for (i, b) in sys.bodies.iter().enumerate() {
                if b.kind.artificial() {
                    continue;
                }
                let c = positions[i];
                let margin = if i == a.body { 0.5 * HYPER_PORT_ALTITUDE } else { 0.1 * b.rail.radius };
                if segment_distance(p, to, c) < b.rail.radius + margin {
                    let along = ((c - p).dot(seg) / len2).clamp(0.0, 1.0);
                    if best.is_none_or(|(t, _)| along < t) {
                        best = Some((along, i));
                    }
                }
            }
            best.map(|(_, i)| i)
        };
        // Going around: fly along the curve (tangent to the body) and
        // steer toward a safe radius (at least 1.5 radii), rather than
        // at a point on the far side: a straight line to such a point
        // always starts off heading inward, and chasing it spirals in.
        let way_dir = match first_blocker(a.aim) {
            None => (a.aim - p).normalize(),
            Some(i) => {
                let (c, r) = (positions[i], sys.bodies[i].rail.radius);
                let from = (p - c).normalize();
                let to = (a.aim - c).normalize();
                let axis = from.cross(to).try_normalize().unwrap_or_else(|| from.any_orthonormal_vector());
                let tangent = axis.cross(from);
                let hold = (p - c).length().max(1.5 * r);
                let climb = ((hold - (p - c).length()) / (0.2 * hold)).clamp(-1.0, 1.0);
                (tangent + from * climb).normalize()
            }
        };
        let way = p + way_dir * 1.0e6;
        *debug_way = Some(way);
        let dir = (way - p).normalize();
        let target = DQuat::from_rotation_arc(ship.forward(), dir) * ship.orientation;
        // (Turned over a frame, well within the controller's reach.)
        commands.turn = Some(docking::attitude(ship, target, DVec3::ZERO, FINE_STEP));
        orders.heading = Some(dir);
        // Full speed: the drive's speed follows the room ahead, and the
        // destination counts in it, so it slows for the arrival itself.
        commands.throttle = 1.0;
    }
    commands.hyperdrive = Some(orders);
    (commands, None)
}
