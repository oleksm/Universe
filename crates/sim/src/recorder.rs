//! The flight recorder: the last `KEEP` seconds of every ship, sampled every
//! `EVERY` seconds of game time — where it was, how it moved, what its devices
//! and its avionics were doing. When a ship is wrecked (a crash, a collision,
//! weapons fire), the recorder files an `Incident` with its trace, and for a
//! collision or a kill the other ship's too, so the cause can be read off one
//! run instead of guessed at from many.

use std::collections::VecDeque;
use std::fmt;

use glam::DVec3;
use universe_avionics::nav::PadSlot;
use universe_avionics::{NavTarget, Phase};
use universe_world::{Ship, ShipState};

/// Seconds between samples, and seconds of samples kept.
pub const EVERY: f64 = 0.25;
pub const KEEP: f64 = 15.0;
/// Incidents kept.
const INCIDENTS: usize = 50;

/// One ship at one moment.
#[derive(Clone, Debug)]
pub struct Sample {
    pub time: f64,
    pub system: usize,
    pub state: &'static str,
    pub position: DVec3,
    pub velocity: DVec3,
    pub throttle: f64,
    pub rcs: DVec3,
    pub hyperdrive: bool,
    pub armed: bool,
    pub hull: f64,
    pub clearance: Option<(NavTarget, Phase, PadSlot)>,
    pub route_next: usize,
    pub departing: bool,
    pub corridor_denied: bool,
}

impl Sample {
    pub fn of(time: f64, system: usize, ship: &Ship, pilot: &crate::contract::Status) -> Self {
        let state = match ship.state {
            ShipState::Flying if ship.hyperdrive => "hyperdrive",
            ShipState::Flying => "flying",
            ShipState::Landed { .. } => "landed",
            ShipState::Destroyed { .. } => "wrecked",
            ShipState::Transit { .. } => "transit",
        };
        Sample {
            time,
            system,
            state,
            position: ship.position,
            velocity: ship.velocity,
            throttle: ship.throttle,
            rcs: ship.rcs,
            hyperdrive: ship.hyperdrive,
            armed: ship.armed,
            hull: ship.hull,
            clearance: pilot.clearance.map(|c| (c.target, c.phase, c.pad)),
            route_next: pilot.route_next,
            departing: pilot.departing,
            corridor_denied: pilot.corridor_denied,
        }
    }
}

/// A wreck and what led to it.
#[derive(Clone, Debug)]
pub struct Incident {
    pub time: f64,
    /// The ship (its id and name), and what did it: a body's name, "COLLISION", "GUNFIRE"…
    pub id: usize,
    pub ship: String,
    pub cause: String,
    pub trace: Vec<Sample>,
    /// The other ship in a collision or a kill: its id, name and trace.
    pub other: Option<(usize, String, Vec<Sample>)>,
}

impl fmt::Display for Incident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "INCIDENT at t={:.1}: {} wrecked by {}", self.time, self.ship, self.cause)?;
        if let Some((_, name, _)) = &self.other {
            writeln!(f, "  with {name}")?;
        }
        for s in &self.trace {
            let ago = self.time - s.time;
            write!(
                f,
                "  -{ago:5.2}s {:<10} v {:7.1} m/s thr {:.2} rcs [{:+.1} {:+.1} {:+.1}] hull {:.2}{}{} clr {:?} route {}{}{}",
                s.state,
                s.velocity.length(),
                s.throttle,
                s.rcs.x,
                s.rcs.y,
                s.rcs.z,
                s.hull,
                if s.armed { " ARMED" } else { "" },
                if s.hyperdrive { " HYPER" } else { "" },
                s.clearance,
                s.route_next,
                if s.departing { " departing" } else { "" },
                if s.corridor_denied { " waiting-corridor" } else { "" },
            )?;
            // Against the other ship, at the same moment (its nearest sample,
            // carried there: ships are sampled a slice at a time).
            if let Some((_, _, other)) = &self.other
                && let Some(o) = other.iter().filter(|o| o.system == s.system && (o.time - s.time).abs() <= EVERY).min_by(|a, b| (a.time - s.time).abs().total_cmp(&(b.time - s.time).abs()))
            {
                let sep = s.position - (o.position + o.velocity * (s.time - o.time));
                let closing = -(s.velocity - o.velocity).dot(sep.normalize_or_zero());
                write!(f, " | other {:<10} sep {:8.1} m closing {:+7.1} m/s clr {:?}", o.state, sep.length(), closing, o.clearance)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// The recorder: a ring of samples per ship (by id), and the incidents filed.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    tracks: Vec<VecDeque<Sample>>,
    pub incidents: Vec<Incident>,
}

impl Recorder {

    /// Take a sample of ship `id`.
    pub fn record(&mut self, id: usize, sample: Sample) {
        if self.tracks.len() <= id {
            self.tracks.resize_with(id + 1, VecDeque::new);
        }
        let track = &mut self.tracks[id];
        let now = sample.time;
        track.push_back(sample);
        while track.front().is_some_and(|s| now - s.time > KEEP) {
            track.pop_front();
        }
    }


    /// The trace kept for ship `id`.
    pub fn trace(&self, id: usize) -> Vec<Sample> {
        self.tracks.get(id).map(|t| t.iter().cloned().collect()).unwrap_or_default()
    }

    /// File an incident: ship `id` (`name`) wrecked at `now` by `cause`,
    /// perhaps involving ship `other`.
    pub fn file(&mut self, now: f64, id: usize, name: String, cause: String, other: Option<(usize, String)>) {
        let incident = Incident {
            time: now,
            id,
            ship: name,
            cause,
            trace: self.trace(id),
            other: other.map(|(o, n)| (o, n, self.trace(o))),
        };
        self.incidents.push(incident);
        let excess = self.incidents.len().saturating_sub(INCIDENTS);
        self.incidents.drain(..excess);
    }
}
