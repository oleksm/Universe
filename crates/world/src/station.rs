//! Stations: a platform in orbit. A square deck of landing pads (a 4 × 4
//! grid, like a spaceport's) with the station's main structure standing
//! along one edge of it, its hangar inside. Ships land on the pads (gently:
//! no gravity here, only the station's slow turn), taxi into the hangar for
//! a long stay, and lift off the deck to leave.
//!
//! In the station's own frame: the deck faces +Y, the main structure stands
//! at its −Z edge (its door facing the pads), metres from the station's
//! centre. The station turns once an orbit about the deck's normal (the
//! orbit's), keeping the same side to its planet.

use glam::{DQuat, DVec3};
use universe_physics::{Blocks, Frame};

use crate::ship::SHIP_RADIUS;
use crate::spaceport::{GRID, PAD_SIZE, PAD_SPACING};
use crate::system::StarSystem;

/// The deck's half-width across (local X) (m).
pub const DECK_HALF: f64 = 300.0;
/// The deck's top, and the station's bottom, along local Y (m).
pub const DECK_TOP: f64 = -100.0;
const BOTTOM: f64 = -150.0;
/// The pad area along local Z (m): from the main structure's face out.
pub const DECK_FROM: f64 = -225.0;
pub const DECK_TO: f64 = 375.0;
/// The main structure: from the far edge to the deck, and how high (m).
pub const STRUCTURE_FROM: f64 = -375.0;
pub const STRUCTURE_TOP: f64 = 150.0;
/// About the station's size: its bounding radius, rounded (m).
pub const STATION_SIZE: f64 = 500.0;
/// Touching the deck slower than this lands (m/s); faster wrecks.
pub const DECK_SPEED: f64 = 10.0;
/// Hull contact slower than this bounces instead of destroying the ship (m/s).
pub const BUMP_SPEED: f64 = 15.0;

/// The station's real shape: the deck (a slab) and the main structure.
pub fn hull() -> Blocks {
    Blocks::new(
        vec![
            (DVec3::new(-DECK_HALF, BOTTOM, DECK_FROM), DVec3::new(DECK_HALF, DECK_TOP, DECK_TO)),
            (DVec3::new(-DECK_HALF, BOTTOM, STRUCTURE_FROM), DVec3::new(DECK_HALF, STRUCTURE_TOP, DECK_FROM)),
        ],
        vec![0],
    )
}

/// Where a ship rests on the deck above local point `at` (station frame).
pub fn rest(at: DVec3) -> DVec3 {
    DVec3::new(at.x, DECK_TOP + SHIP_RADIUS, at.z)
}

/// Where a ship rests on pad `pad` (0..`PADS`, row by row from the main
/// structure), in the station's frame.
pub fn pad_local(pad: usize) -> DVec3 {
    let half = (GRID as f64 - 1.0) / 2.0;
    let (row, col) = ((pad / GRID) as f64 - half, (pad % GRID) as f64 - half);
    rest(DVec3::new(col * PAD_SPACING, 0.0, (DECK_FROM + DECK_TO) / 2.0 + row * PAD_SPACING))
}

/// The pad a ship resting at `local` (station frame) is on, if any.
pub fn pad_at(local: DVec3) -> Option<usize> {
    let flat = |p: DVec3| DVec3::new(p.x, 0.0, p.z);
    (local.y < DECK_TOP + 3.0 * SHIP_RADIUS).then(|| (0..crate::spaceport::PADS).find(|&k| flat(pad_local(k)).distance(flat(local)) < PAD_SIZE)).flatten()
}

/// The hangar inside the main structure (where ships in it rest, out of
/// sight), and its door onto the deck.
pub fn hangar_local() -> DVec3 {
    rest(DVec3::new(0.0, 0.0, (STRUCTURE_FROM + DECK_FROM) / 2.0))
}

pub fn door_local() -> DVec3 {
    rest(DVec3::new(0.0, 0.0, DECK_FROM))
}

/// Upright on the deck, the nose toward the main structure.
pub fn parked() -> DQuat {
    // (The ship's own axes are the station's: nose −Z, top +Y.)
    DQuat::IDENTITY
}

/// A station's pose and motion at one instant, in the system frame.
#[derive(Clone, Copy, Debug)]
pub struct StationFrame {
    pub center: DVec3,
    pub velocity: DVec3,
    pub rotation: DQuat,
    pub angular_velocity: DVec3,
}

impl StationFrame {
    pub fn new(sys: &StarSystem, station: usize, t: f64, positions: &[DVec3]) -> Self {
        let f = Frame::of(&sys.bodies, station, t, positions);
        Self { center: f.center, velocity: f.velocity, rotation: f.rotation, angular_velocity: f.angular_velocity }
    }

    /// Up from the deck.
    pub fn up(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }

    /// A point of the station's frame, in the system's.
    pub fn world(&self, local: DVec3) -> DVec3 {
        self.center + self.rotation * local
    }

    /// Where a ship rests on pad `pad` (system frame).
    pub fn pad(&self, pad: usize) -> DVec3 {
        self.world(pad_local(pad))
    }

    /// Velocity of the station's material at world point `p` (includes its turn).
    pub fn velocity_at(&self, p: DVec3) -> DVec3 {
        self.velocity + self.angular_velocity.cross(p - self.center)
    }

    /// Upright on the deck, the nose toward the main structure: how a ship
    /// sets down on a pad.
    pub fn landing_orientation(&self) -> DQuat {
        self.rotation * parked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::ShipState;
    use crate::testkit::Probe;

    #[test]
    fn pads_lie_on_the_deck_clear_of_the_structure() {
        for k in 0..crate::spaceport::PADS {
            let p = pad_local(k);
            assert_eq!(pad_at(p), Some(k));
            assert!(p.x.abs() + PAD_SIZE <= DECK_HALF && p.z - PAD_SIZE >= DECK_FROM && p.z + PAD_SIZE <= DECK_TO, "pad {k} at {p}");
        }
        assert!(hull().contact(0, &Frame { center: DVec3::ZERO, velocity: DVec3::ZERO, rotation: DQuat::IDENTITY, angular_velocity: DVec3::ZERO }, pad_local(5), DVec3::ZERO, SHIP_RADIUS + 0.1).is_some());
    }

    #[test]
    fn lifting_off_a_pad_clears_the_station() {
        let mut p = Probe::new(42);
        let station = p.sys().station().unwrap();
        p.ship.state = ShipState::Landed { body: station, local_position: pad_local(5), local_orientation: parked() };
        p.ship.rcs = DVec3::Y;
        p.step(1.0 / 60.0, 1.0);
        assert!(p.ship.is_flying(), "{:?}", p.events);
        for _ in 0..600 {
            p.ship.rcs = DVec3::Y;
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(p.ship.is_flying(), "lifting off should not hit the station: {:?}", p.events);
    }
}
