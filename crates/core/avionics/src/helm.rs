//! The helm: how a pilot's hands fly a ship, the same for every client (the
//! game's cockpit, the shipyard's test drive). Client-side pilot software,
//! like the autopilots: the core never holds a pilot's intentions. The client samples its keys
//! and mouse into a `HelmInput`; the helm applies the pilot's rules to it for
//! the ship as it is and says what to order (`Order`), how to turn and what
//! to tell the pilot. The client sends the orders its own way (the game
//! through its cockpit; a sandbox straight to its ship).
//!
//! The rules: flight systems power up or down only while set down; with no
//! flight computer fitted, manual thrusters are the only way to fly it, on the
//! ground too; with one, manual thrusters (the flight computer off, each
//! thruster fired by hand) only in flight, outside the hyperdrive, and going
//! manual lets the autopilots go;
//! an autopilot taken up, or the ship set down, puts the flight computer back
//! on; manual jets are ordered only when the held set changes; while an
//! autopilot flies, the hands are off the stick; otherwise the throttle moves
//! by the pilot's delta, the thrusters push as held and the stick turns.

use glam::DVec3;
use universe_protocol::{Controls, ShipCommands};
use universe_world::Ship;

/// What the pilot's hands are doing this frame (sampled by the client).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HelmInput {
    /// The stick: turn rates wanted, each -1..1.
    pub turn: Controls,
    /// The translation thrusters held (body frame, each -1..1; +Y lift).
    pub translation: DVec3,
    /// How far the throttle moves this frame.
    pub throttle_delta: f64,
    /// Manual thrusters held, a bit each (in the hull's thruster order).
    pub held: u64,
    /// The flight systems key, and the manual thrusters key, pressed this frame.
    pub systems: bool,
    pub manual: bool,
}

/// What the helm orders.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    /// Device settings to change (power, manual, jets; the rest held).
    Ship(Box<ShipCommands>),
    /// The throttle moved by this much (clamped to 0..1 where it's applied: `throttle_after`).
    Throttle(f64),
    /// The translation thrusters set so.
    Thrusters(DVec3),
    /// Every autopilot lets go (following, route, clearance's).
    ReleaseAutopilots,
}

/// What the helm says this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HelmOut {
    pub orders: Vec<Order>,
    /// The stick to fly (none while an autopilot or manual thrusters fly).
    pub turn: Controls,
    /// A line for the pilot.
    pub say: Option<&'static str>,
}

/// A pilot's helm: remembers the manual thrusters last ordered, and whether
/// the design it last flew can turn itself (by its design's address).
#[derive(Clone, Debug, Default)]
pub struct Helm {
    jets_held: u64,
    steers: Option<(usize, bool)>,
}

/// Has `ship` a flight computer (its turn rates come from one)?
pub fn has_computer(ship: &Ship) -> bool {
    ship.spec().turn_rate > 0.0
}

/// The throttle `delta` moves `ship`'s to.
pub fn throttle_after(ship: &Ship, delta: f64) -> f64 {
    (ship.throttle + delta).clamp(0.0, 1.0)
}

impl Helm {
    /// The pilot's `input` for `ship` as it is (`autopilot`: one is flying it).
    pub fn fly(&mut self, ship: &Ship, autopilot: bool, input: &HelmInput) -> HelmOut {
        let mut out = HelmOut::default();
        let landed = matches!(ship.state, universe_world::ShipState::Landed { .. });
        if landed && input.systems {
            out.orders.push(Order::Ship(Box::new(ShipCommands { power: Some(!ship.powered), ..ship.holding() })));
        }
        // No flight computer: manual thrusters are all it has, anywhere.
        if !has_computer(ship) {
            if input.manual {
                out.say = Some("NO FLIGHT COMPUTER - MANUAL THRUSTERS ONLY");
            }
            if !ship.manual {
                self.jets_held = 0;
                out.orders.push(Order::Ship(Box::new(ShipCommands { manual: Some(true), throttle: 0.0, rcs: DVec3::ZERO, ..ship.holding() })));
                return out;
            }
            if input.held != self.jets_held {
                self.jets_held = input.held;
                out.orders.push(Order::Ship(Box::new(ShipCommands { jets: Some(input.held), ..ship.holding() })));
            }
            return out;
        }
        let flying = ship.is_flying() && !ship.hyperdrive;
        if input.manual {
            if !ship.manual && !flying {
                out.say = Some("MANUAL THRUSTERS IN FLIGHT ONLY");
            } else {
                let on = !ship.manual;
                if on {
                    out.orders.push(Order::ReleaseAutopilots);
                }
                self.jets_held = 0;
                out.orders.push(Order::Ship(Box::new(ShipCommands { manual: Some(on), throttle: 0.0, rcs: DVec3::ZERO, ..ship.holding() })));
                out.say = Some(if on { "MANUAL THRUSTERS - FLIGHT COMPUTER OFF" } else { "FLIGHT COMPUTER ON" });
            }
        }
        if ship.manual {
            // An autopilot taken up, or set down: the flight computer's back on.
            if autopilot || !flying {
                out.orders.push(Order::Ship(Box::new(ShipCommands { manual: Some(false), ..ship.holding() })));
                return out;
            }
            if input.held != self.jets_held {
                self.jets_held = input.held;
                out.orders.push(Order::Ship(Box::new(ShipCommands { jets: Some(input.held), ..ship.holding() })));
            }
            return out;
        }
        if autopilot {
            return out;
        }
        if input.throttle_delta != 0.0 {
            out.orders.push(Order::Throttle(input.throttle_delta));
        }
        // Thrust asked for, from a design whose thrusters can't turn it: the
        // flight computer holds it all back, having nothing to keep it straight.
        let asked = ship.throttle > 0.0 || input.throttle_delta > 0.0 || input.translation != DVec3::ZERO;
        if asked && ship.powered && !ship.hyperdrive && ship.fuel > 0.0 && !self.steers(ship) && out.say.is_none() {
            out.say = Some("NO THRUST - THE FLIGHT COMPUTER CAN'T KEEP IT STRAIGHT (FIT THRUSTERS TO TURN IT)");
        }
        out.orders.push(Order::Thrusters(input.translation));
        out.turn = input.turn;
        out
    }

    /// Can `ship`'s thrusters turn it about every axis (its design's, worked out once)?
    fn steers(&mut self, ship: &Ship) -> bool {
        let spec = ship.spec();
        let id = spec as *const universe_world::ship::ClassSpec as usize;
        if let Some((at, steers)) = self.steers
            && at == id
        {
            return steers;
        }
        let thrusters = universe_world::trim::thrusters(spec, &ship.trim);
        let steers = universe_world::thrusters::turn_envelope(&thrusters, ship.centre_of_mass(), ship.mass(), ship.inertia()).min_element() > 0.0;
        self.steers = Some((id, steers));
        steers
    }
}
