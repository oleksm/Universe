//! The ship's flight computer while the ship moves: the dock/land/gate
//! autopilot every substep, and the hyperdrive's navigation every frame
//! (where a jump to the nav target ends, steering around bodies on the way,
//! when to drop out and which frame to drop out in). It reads the ship and
//! the star system, keeps its own state in the avionics, and flies the ship
//! only through `ShipCommands` (see `universe_world::FlightComputer`).

use glam::{DQuat, DVec3};
use universe_physics::integrate::FINE_STEP;
use universe_physics::segment_distance;
use universe_world::{BodyKind, Destination, FlightComputer, GateFrame, HyperdriveCommand, Ship, ShipCommands, StarSystem, StationFrame};

use crate::avionics::{Avionics, Clearance, NavTarget};
use crate::docking;
use crate::gate;
use crate::landing::{self, PadFrame};

/// Hyperdrive drops out this close to a targeted station (m).
pub(crate) const HYPER_ARRIVE_STATION: f64 = 20_000.0;
/// Hyperdrive drops out this close to a targeted spaceport (m).
pub(crate) const HYPER_ARRIVE_PORT: f64 = 120_000.0;
/// The hyperdrive autopilot aims this far above a targeted pad (m).
const HYPER_PORT_ALTITUDE: f64 = 100_000.0;
/// Dropping out of hyperdrive within this distance of the nav target leaves
/// the ship moving with it (m).
const HYPER_NEAR_TARGET: f64 = 1.0e6;

/// Where a hyperdrive jump to the nav target ends.
pub(crate) struct HyperAim {
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

/// What a hyperdrive arrival at `target` is announced as.
fn arrival_name(sys: &StarSystem, target: NavTarget) -> String {
    match target {
        NavTarget::Station(b) | NavTarget::Gate(b) => sys.bodies[b].name.clone(),
        NavTarget::Spaceport(p) => sys.spaceports[p].name.clone(),
    }
}

/// Hyperdrive destination for the nav target, if it's in this system (ship at `ship_pos`, bodies at `positions` at `t`).
pub(crate) fn hyper_aim(sys: &StarSystem, target: NavTarget, t: f64, positions: &[DVec3], ship_pos: DVec3) -> Option<HyperAim> {
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
pub(crate) fn exit_velocity(aim: Option<&HyperAim>, ship_pos: DVec3) -> Option<DVec3> {
    aim.filter(|a| a.target.distance(ship_pos) < HYPER_NEAR_TARGET).map(|a| a.velocity)
}

/// The flight computer of one ship, for one step.
pub(crate) struct Computer<'a> {
    avionics: &'a mut Avionics,
    /// Where the hyperdrive autopilot is steering (for debugging).
    debug_way: &'a mut Option<DVec3>,
    /// The hyperdrive dropped out at the nav target (its name).
    pub arrived: Option<String>,
}

impl<'a> Computer<'a> {
    pub fn new(avionics: &'a mut Avionics, debug_way: &'a mut Option<DVec3>) -> Self {
        Self { avionics, debug_way, arrived: None }
    }
}

impl FlightComputer for Computer<'_> {
    /// The autopilot reacts every 0.05 s at most, so control is precise.
    fn interval(&self) -> f64 {
        if self.avionics.autopilot_engaged() { FINE_STEP } else { f64::INFINITY }
    }

    /// The dock/land/gate autopilot, if it's flying.
    fn substep(&mut self, sys: &StarSystem, ship: &Ship, t: f64, _h: f64, positions: &[DVec3]) -> Option<ShipCommands> {
        let c = self.avionics.clearance.filter(|c| c.autopilot)?;
        let cmd = match c.target {
            NavTarget::Station(station) => {
                let frame = StationFrame::new(sys, station, t, positions);
                docking::autopilot(&frame, ship, c.phase)
            }
            NavTarget::Spaceport(port) => {
                let pad = PadFrame::new(sys, port, t, positions);
                landing::autopilot(&pad, ship, sys.gravity(ship.position, positions), c.phase)
            }
            NavTarget::Gate(g) => {
                let frame = GateFrame::new(sys, g, t, positions);
                gate::autopilot(&frame, ship, c.phase)
            }
        };
        if cmd.phase != c.phase {
            self.avionics.clearance = Some(Clearance { phase: cmd.phase, ..c });
        }
        Some(cmd.commands())
    }

    /// Navigation in hyperdrive: drop out on arriving at the nav target,
    /// otherwise command its frame, destination and exit velocity, and under
    /// the hyperdrive autopilot the heading (around anything in the way).
    fn hyperdrive(&mut self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3]) -> ShipCommands {
        let p = ship.position;
        let target = self.avionics.nav_target;
        let aim = target.and_then(|target| hyper_aim(sys, target, t, positions, p));
        let mut commands = ship.holding();

        // Arrived at the nav target: drop out moving with it. Asking for
        // clearance is left to the pilot.
        if let (Some(a), Some(target)) = (&aim, target)
            && p.distance(a.target) < a.arrive
        {
            self.arrived = Some(arrival_name(sys, target));
            commands.hyperdrive = Some(HyperdriveCommand { engage: false, exit_velocity: Some(a.velocity), ..Default::default() });
            return commands;
        }

        let mut orders = HyperdriveCommand {
            engage: true,
            heading: None,
            // (The hyperdrive autopilot steers around obstacles itself.)
            steering: self.avionics.hyper_autopilot,
            frame_velocity: aim.as_ref().map(|a| a.frame_velocity),
            exit_velocity: exit_velocity(aim.as_ref(), p),
            destination: aim.as_ref().map(|a| Destination { point: a.target, body: a.body }),
        };
        // Direction of travel: where the nose points, unless the pilot handed
        // the jump to the hyperdrive autopilot.
        if let (Some(a), true) = (&aim, self.avionics.hyper_autopilot) {
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
            *self.debug_way = Some(way);
            let dir = (way - p).normalize();
            let target = DQuat::from_rotation_arc(ship.forward(), dir) * ship.orientation;
            commands.turn = Some(docking::attitude(ship, target, DVec3::ZERO));
            orders.heading = Some(dir);
        }
        commands.hyperdrive = Some(orders);
        commands
    }
}
