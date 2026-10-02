//! The contract between the world (core and services) and its clients
//! (pilots: NPC and the player's): what clients read (the pilot view), what
//! they post, what they declare of themselves, what the world tells them,
//! and the timing. The world depends on this and never on a client's code;
//! clients likewise (see docs/rearchitecture.md §0).

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use glam::DVec3;
use universe_avionics::route::Stop;
use universe_avionics::{Clearance, Event, NavTarget};
use universe_services::market::Quote;
use universe_services::Board;
use universe_world::charts::Charts;
use universe_world::{Controls, Facility, Ship, ShipCommands, ShipEvent, StarSystem};

use crate::traffic::Snap;
use crate::vessel::Request;

/// Ticks from the snapshot a pilot read to its commands taking effect (k).
pub const COMMAND_DELAY: u64 = 2;
/// A posting this many ticks past due is dropped: too stale to act on.
pub const LATE_HORIZON: u64 = 30;
/// After this long with no posting from its pilot, a ship's engines are cut
/// and its weapons made safe (s).
pub const DEAD_MAN: f64 = 30.0;

/// What a pilot shows of itself: its transponder and flight plan, and what
/// its operator and the services know of it (published with each posting;
/// the world reads this, never the pilot itself).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Status {
    pub nav_target: Option<NavTarget>,
    pub clearance: Option<Clearance>,
    pub corridor_denied: bool,
    pub route_active: bool,
    pub route_next: usize,
    pub route_len: usize,
    /// The stop it's bound for.
    pub next_stop: Option<Stop>,
    /// Parked at a stop until its time is up.
    pub dwelling: bool,
    pub departing: bool,
}


/// A message for a pilot.
#[derive(Clone, Debug)]
pub enum Msg {
    /// What happened to its ship (its sensors and the devices report it).
    Feed(Vec<ShipEvent>),
    /// The market service's answer to its request for quotes.
    Market(Box<MarketAnswer>),
}


/// A defence turret, as the charts have it: where it is, how it moves, how
/// far it reaches, and what it guards.
#[derive(Clone, Copy, Debug)]
pub struct Gun {
    /// Its id (see `turrets::turret_id`), and where its gun points now (if
    /// the world says).
    pub id: usize,
    pub aim: Option<DVec3>,
    pub at: DVec3,
    pub velocity: DVec3,
    pub reach: f64,
    pub guards: universe_world::Facility,
}
pub type Guns = Arc<Vec<Gun>>;

/// What pilots read: the world as it stood at the end of a tick.
pub struct PilotView {
    pub tick: u64,
    pub time: f64,
    /// Game seconds a tick spans.
    pub dt: f64,
    pub charts: Arc<Charts>,
    /// The ships of the pilots awake this tick (and ours), with the system
    /// each is in, by combat id (ours 0, craft i: i + 1). The rest are in
    /// `snaps`: what anyone sees of anyone.
    pub ships: HashMap<usize, (usize, Ship), universe_physics::pairs::CellHash>,
    /// Every ship as others see it (by combat id), and who's aggressed and flying.
    pub(crate) snaps: Arc<Vec<Snap>>,
    pub aggressors: Vec<(usize, DVec3)>,
    pub board: Board,
    /// Bodies' positions, and the defence turrets' (where and how they move),
    /// per system with crafts in it.
    pub rails: HashMap<usize, Arc<Vec<DVec3>>>,
    pub turrets: HashMap<usize, Guns>,
}

/// What a pilot posts after thinking.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Posting {
    /// Whose: the ship's combat id (the player's 0, craft i: i + 1).
    pub id: usize,
    /// The tick of the snapshot it read (due `COMMAND_DELAY` after), and its world time.
    pub thought: u64,
    pub seen: f64,
    /// For its devices, in order, and its turn at the due tick (None: it
    /// says nothing of the turn; Some(None): none commanded).
    pub devices: Vec<ShipCommands>,
    pub turn: Option<Option<Controls>>,
    /// To traffic control.
    pub(crate) requests: Vec<Request>,
    /// What its programs report (route stops, traffic): for the services.
    pub events: Vec<Event>,
    pub status: Status,
    /// A turret gunner's orders for its gun (its id is the turret's).
    pub gun: Option<universe_protocol::TurretCommand>,
    /// Going to sleep: it needn't be in a view before this tick (unless a
    /// message wakes it). None: it says nothing of it.
    pub sleep_until: Option<u64>,
}

impl Posting {
    pub fn due(&self) -> u64 {
        self.thought + COMMAND_DELAY
    }
}


/// Where system `system`'s defence turrets are at `t`, and how they move.
pub(crate) fn turret_motions(charts: &Charts, system: usize, sys: &StarSystem, t: f64, positions: &[DVec3]) -> Vec<Gun> {
    universe_world::turrets::turrets(charts.seed, system, sys)
        .iter()
        .enumerate()
        .map(|(k, tu)| {
            let (at, velocity) = tu.motion(sys, t, positions);
            Gun { id: universe_world::turrets::turret_id(system, k), aim: None, at, velocity, reach: tu.range(), guards: tu.facility }
        })
        .collect()
}


/// The market service's answer to a trader's request for quotes: at its
/// market, at the others in the system, and its own account.
#[derive(Clone, Debug)]
pub struct MarketAnswer {
    pub system: usize,
    pub at: Facility,
    /// Every quote here, and here for each item held.
    pub here: Vec<Quote>,
    pub here_held: Vec<Option<Quote>>,
    /// The other markets, quoting `items` (held, then what's buyable here).
    pub items: Vec<usize>,
    pub there: Vec<(Facility, Vec<Option<Quote>>)>,
    pub credits: f64,
    pub hold: Vec<(usize, u32)>,
    /// What its cargo weighs (kg), and the most its hold takes (kg); the
    /// space left in it (m³).
    pub cargo: f64,
    pub capacity: f64,
    pub space: f64,
    /// Passage booked from here; the people waiting to leave each market in
    /// the system (thousands); its passengers aboard, where they're bound,
    /// and its free seats.
    pub bookings: Vec<crate::commerce::Booking>,
    pub waiting: Vec<(Facility, f64)>,
    pub passengers: u32,
    pub bound_for: Option<(usize, Facility)>,
    pub seats: u32,
}


/// A craft's transponder, as the player's radar reads it alongside the blip.
#[derive(Clone, Debug)]
pub struct Transponder {
    pub name: String,
    pub activity: &'static str,
    pub destination: Option<String>,
    pub hull: f64,
    pub aggressed: bool,
}

/// What the cockpit reads each tick.
pub struct CockpitView {
    pub world: Arc<PilotView>,
    /// Of the crafts our radar could see, by craft index.
    pub transponders: HashMap<usize, Transponder>,
}


/// A ship the operator puts in the world: its name, and where it starts.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Registration {
    pub name: String,
    pub at: Stop,
    pub pad: usize,
    /// The hull it's built as (content key; empty: the starting hull).
    pub hull: String,
    /// Modules fitted in place of its hull's stock ones: (slot, module key).
    #[serde(default)]
    pub fit: Vec<(String, String)>,
}

/// The world's NPC clients, as the world sees them: it hands them each
/// tick's view, collects what they post, and tells a pilot what's happened
/// to its ship or what a service answers. In-process now (a pool on its own
/// threads); across a network later.
pub trait Pilots: Send {
    /// A new view: in lockstep, their postings at once; apart, none (later, by `collect`).
    fn view(&mut self, view: Arc<PilotView>) -> Vec<Posting>;
    /// Postings come in since.
    fn collect(&mut self) -> Vec<Posting>;
    /// A message for craft `craft`'s pilot.
    fn tell(&mut self, craft: usize, msg: Msg);
    /// From now on, think apart from the world, on `threads` threads.
    fn run_apart(&mut self, _threads: usize) {}
    fn apart(&self) -> bool {
        false
    }
    /// Their own tally, for display: hunts begun, posses formed.
    fn tally(&self) -> (u64, u64) {
        (0, 0)
    }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// No NPC clients (a world on its own).
#[derive(Default)]
pub struct NoPilots;

impl Pilots for NoPilots {
    fn view(&mut self, _: Arc<PilotView>) -> Vec<Posting> {
        Vec::new()
    }
    fn collect(&mut self) -> Vec<Posting> {
        Vec::new()
    }
    fn tell(&mut self, _: usize, _: Msg) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The player's client, as the world sees it (when it's thinking in step
/// with the world: tests; in the game it's the client's own).
pub trait PlayerClient: Send {
    /// What happened to its ship.
    fn feed(&mut self, events: Vec<ShipEvent>);
    /// The human's stick (in step with the world, it's handed in).
    fn stick(&mut self, c: Controls);
    /// A new view: what it posts.
    fn view(&mut self, view: Arc<CockpitView>) -> Vec<Posting>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// (To hand it over to its own thread.)
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send>;
}
