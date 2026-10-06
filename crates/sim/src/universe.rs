//! The universe as the game runs it: the world, the player's ship with its
//! avionics, and the settlers' crafts with theirs, advanced one tick at a
//! time in a fixed order — the player's ship first, then every craft, each
//! from the same moment (see `vessel` for what a ship's turn is).

use std::sync::Arc;

use glam::{DQuat, DVec3};
use universe_avionics::route::{self, Stop};
use universe_avionics::{Event, NavTarget};
use universe_world::{Controls, Facility, Person, Place, Ship, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, WalkCommands, World};

use crate::traffic::{CrashReport, Craft};

/// The longest tick (game seconds): pilots act once a tick.
pub const TICK: f64 = 1.0 / 60.0 + 1e-9;
/// The most ticks a step may take (beyond it, under heavy warp, ticks stretch).
pub const TICK_BUDGET: usize = 8;

/// Traffic control's look at who's where, every this many ticks.
const PRESENCE_EVERY: u64 = 6;

/// A ship on its final run this close to the station or gate lets the next one start (m).
const CORRIDOR_RELEASE: f64 = 1_500.0;
/// A ship due out of a gate's tube within this keeps its entrance closed (s):
/// about a final run's length (4 km at 100 m/s), so no one is on it as it comes out.
const ENTRY_LEAD: f64 = 60.0;

/// What a new pilot starts with (credits).
/// (For now, while ships are being built and tried: enough to buy any.
/// The economy's balance pass sets it for real.)
pub const STARTING_CREDITS: f64 = 1_000_000.0;

pub struct Universe {
    /// The galaxy, its gate network, the clock and the star systems.
    pub world: World,
    pub ship: Ship,
    /// Galaxy index of the system the ship is in; ship coordinates are relative to its star.
    pub ship_system: usize,
    /// The player's client (its pilot and ship computers), when it thinks in
    /// step with the world (tests; in the game, the client has it).
    pub player: Option<Box<dyn crate::contract::PlayerClient>>,
    /// The player's ship's commands on their way to its devices, what its
    /// pilot shows, and what happened to it since its pilot last looked.
    pub(crate) player_inbox: crate::vessel::Inbox,
    pub player_status: crate::contract::Status,
    pub player_feed: Vec<ShipEvent>,
    /// With the cockpit at the client: this tick's view for it, and what
    /// happened to the ship, to send it.
    pub cockpit_out: Option<(Arc<crate::contract::CockpitView>, Vec<ShipEvent>)>,
    /// The input log, if recording (see `audit`), what's come in since the
    /// last tick, and when replaying, the postings due this tick.
    pub input_log: Option<crate::audit::InputLog>,
    pub(crate) between: Vec<crate::audit::Input>,
    pub(crate) replaying: bool,
    pub(crate) replay_due: Vec<crate::contract::Posting>,
    /// What happened to the player's ship, for the pilot (the game takes them).
    pub events: Vec<Event>,
    /// Other ships (settlers), each flying its own route.
    pub crafts: Vec<Craft>,
    /// Recent craft crashes (most recent last, capped).
    pub crash_log: Vec<CrashReport>,
    /// The pilot: in the seat, or on foot.
    pub crew: Person,
    /// Hulls' insides as laid out (by hull key): built, to walk in.
    pub layouts: std::collections::HashMap<String, Arc<universe_world::deckplan::Walkable>>,
    /// What each hull's inside is made of: its decks as built, the interior studio's
    /// walls (walked in together, as `layouts`).
    inside: std::collections::HashMap<String, (universe_world::deckplan::Built, Vec<[DVec3; 3]>)>,
    /// The flight recorder: every ship's last seconds, and the wrecks filed (see `recorder`).
    pub recorder: crate::recorder::Recorder,
    /// What happened, kept: kills (with causes), trades, traffic totals.
    pub records: universe_services::Records,
    positions: Vec<DVec3>,
    /// Aggressed ships flying this frame: system and position (who's worth judging).
    pub(crate) aggressors: Vec<(usize, DVec3)>,
    /// Every ship as it was at the start of the frame (by combat id), and
    /// when that was: what each sees of the others, so a ship stepped earlier
    /// in the frame isn't seen where it will be at its end.
    pub(crate) snaps: Arc<Vec<crate::traffic::Snap>>,
    /// The tick the snapshot was last taken at (at a tick's end: it does for
    /// the next tick's start).
    pub(crate) snapped_at: u64,
    pub(crate) snap_time: f64,
    /// Ticks run so far.
    pub tick: u64,
    /// This tick's event log: (ship, event), in order — what services' causes
    /// point into (`Cause::Event { tick, index }`).
    pub log: Vec<(usize, ShipEvent)>,
    /// The law: who's fair game, since when, and why (see `universe_services::law`).
    pub law: universe_services::Law,
    /// Turret gunners' orders on the way to the guns (due tick, turret, orders).
    pub(crate) turret_orders: std::collections::VecDeque<(u64, usize, universe_protocol::TurretCommand)>,
    /// Traffic control (clearance, pads, corridors): a service.
    pub atc: universe_services::TrafficControl,
    /// The ledger (credits, and what's in each hold) and the market service.
    pub ledger: universe_services::Ledger,
    /// Who owns which ground at each settlement, and what stands on it.
    pub land: universe_services::land::LandOffice,
    pub markets: universe_services::Markets,
    /// The price boards markets have put out over the hypernet (see `commerce::Boards`).
    pub(crate) boards: crate::commerce::Boards,
    /// Each faction's view of every pilot (see `standing`).
    pub standings: crate::standing::Standings,
    /// Messages sent to services so far (each one's id, for causes).
    pub(crate) messages: u64,
    /// The world's NPC clients (see `contract::Pilots`), postings that came
    /// late (applied at once) and too late (dropped), and postings not yet
    /// due (each takes effect whole at its due tick, however early it came).
    pub npcs: Box<dyn crate::contract::Pilots>,
    pub late: u64,
    pub dropped: u64,
    pending: Vec<crate::contract::Posting>,
    /// The charts, shared with the pilots.
    pub(crate) charts: Option<Arc<universe_world::charts::Charts>>,
}

impl Universe {
    /// A world of its own, without clients (see `setup` for one with them).
    pub(crate) fn bare(seed: u64) -> Self {
        let world = World::new(seed);
        let goods = std::sync::Arc::new(world.goods.clone());
        let mut u = Self {
            world,
            ship: Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY),
            ship_system: 0,
            player: None,
            player_inbox: Default::default(),
            player_status: Default::default(),
            player_feed: Vec::new(),
            cockpit_out: None,
            input_log: None,
            between: Vec::new(),
            replaying: false,
            replay_due: Vec::new(),
            events: Vec::new(),
            crafts: Vec::new(),
            crash_log: Vec::new(),
            crew: Person::default(),
            layouts: Default::default(),
            inside: Default::default(),
            recorder: Default::default(),
            records: Default::default(),
            aggressors: Vec::new(),
            snaps: Default::default(),
            snapped_at: u64::MAX,
            snap_time: f64::NAN,
            tick: 0,
            log: Vec::new(),
            law: Default::default(),
            turret_orders: Default::default(),
            atc: Default::default(),
            ledger: Default::default(),
            land: Default::default(),
            markets: universe_services::Markets::new(goods),
            boards: Default::default(),
            standings: Default::default(),
            messages: 0,
            npcs: Box::new(crate::contract::NoPilots),
            late: 0,
            dropped: 0,
            pending: Vec::new(),
            charts: None,
            positions: Vec::new(),
        };
        // The settled systems' economy (the gate network's).
        let mut settled: Vec<usize> = u.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        settled.sort_unstable();
        settled.dedup();
        let systems: Vec<(usize, Arc<StarSystem>)> = settled.into_iter().map(|i| (i, u.world.system(i))).collect();
        // The land office, from the registry: each settlement recorded, at its system and port.
        let content = universe_world::content::content();
        u.land = universe_services::land::LandOffice::seed(systems.iter().flat_map(|(i, sys)| {
            sys.spaceports.iter().enumerate().filter_map(move |(p, sp)| content.settlement(&sys.name, &sys.bodies[sp.body].key, &sp.name).map(|s| (*i, p, s)))
        }));
        // The settlements' economy: their facilities and markets, on the land office's ground.
        u.markets.economy = universe_services::economy::Economy::new(&u.land, u.world.time);
        // The rigs (see `world::rigs`): each its own market, its owner's.
        for (i, sys) in &systems {
            for (b, body) in sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == universe_world::BodyKind::Rig) {
                u.markets.economy.add_rig(&mut u.land, *i, &sys.name, b, &body.key);
            }
        }
        u.start_docked();
        u.events.clear();
        u.player_feed.clear();
        // What a new pilot starts with, from the world's account.
        u.ledger.settle(universe_services::Party::Pilot(crate::combat::PLAYER), universe_services::Asset::Credits, STARTING_CREDITS, 0, universe_protocol::Cause::Rules);
        u
    }

    /// Systems linked to `i` by gates, with their names.
    pub fn gate_links_of(&self, i: usize) -> Vec<(usize, String)> {
        self.world.gate_links_of(i)
    }

    /// Get (generating and caching if needed) the star system at galaxy index `i`.
    pub fn system(&mut self, i: usize) -> Arc<StarSystem> {
        self.world.system(i)
    }

    pub fn ship_system(&mut self) -> Arc<StarSystem> {
        self.world.system(self.ship_system)
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.world.distance_ly(a, b)
    }

    /// Advance the player's ship by `real_dt * warp` seconds (less if warp is
    /// limited): the commands its pilot posted that are due reach its
    /// devices, and the world steps it. The world clock moves with it. (The
    /// pilot's stick, `controls`, goes to the cockpit if it's here.)
    pub(crate) fn step(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        if let Some(c) = &mut self.player {
            c.stick(*controls);
        }
        let mut happened = Vec::new();
        let t = self.world.time;
        let turn = self.player_inbox.deliver(&self.world, &mut self.ship, self.ship_system, t, self.tick, &mut happened);
        let commands = ShipCommands { turn, ..self.ship.holding() };
        let result = universe_prof::time("sim/player/world step", || self.world.step_ship(&mut self.ship, &mut self.ship_system, &commands, real_dt, warp, &mut happened));
        let fresh: Vec<Event> = happened.iter().cloned().map(Event::Ship).collect();
        self.player_events(happened);
        self.log_events(crate::combat::PLAYER, &fresh);
        self.traffic_events(crate::combat::PLAYER, &fresh);
        self.book_mined(crate::combat::PLAYER, &fresh);
        self.book_tolls(crate::combat::PLAYER, &fresh);
        for e in self.book_jolts(crate::combat::PLAYER, &fresh) {
            self.events.push(Event::Ship(e));
        }
        // Wrecked on something (collisions and weapons are filed by the combat phase).
        let crashed = fresh.iter().find_map(|e| match e {
            Event::Ship(ShipEvent::Crashed { body }) if !matches!(body.as_str(), "COLLISION" | "GUNFIRE" | "LASER FIRE" | "MISSILE") => Some(body.clone()),
            _ => None,
        });
        if let Some(body) = crashed {
            self.recorder.file(self.world.time, crate::combat::PLAYER, "YOU".into(), body, None);
        }
        result
    }

    /// What happened to the player's ship: for the HUD, and its pilot.
    pub(crate) fn player_events(&mut self, happened: Vec<ShipEvent>) {
        self.events.extend(happened.iter().cloned().map(Event::Ship));
        self.player_feed.extend(happened);
    }

    /// A message for craft `i`'s pilot (it wakes it).
    pub(crate) fn tell(&mut self, i: usize, msg: crate::contract::Msg) {
        if let Some(c) = self.crafts.get_mut(i) {
            c.asleep_until = 0;
        }
        self.npcs.tell(i, msg);
    }

    /// Postings to take effect at once (their devices' commands still at
    /// their due tick): noted in the input log.
    pub(crate) fn post_now(&mut self, postings: Vec<crate::contract::Posting>) {
        if postings.is_empty() {
            return;
        }
        let logged = postings.clone();
        self.note(|| crate::audit::Input::Post(logged));
        self.post(postings);
    }

    /// Postings from a pilot apart (the client's cockpit): each takes effect
    /// at its due tick.
    pub fn accept(&mut self, postings: Vec<crate::contract::Posting>) {
        self.pending.extend(postings);
    }

    /// A pilot's request (from its posting), to the service it's for.
    pub(crate) fn request(&mut self, id: usize, r: crate::vessel::Request) {
        use crate::vessel::Request;
        match r {
            Request::Pad { system, port, ship, now } => {
                self.atc.request_pad(system, port, ship, now);
            }
            Request::Corridor { system, body, ship, now } => {
                self.atc.request_corridor(system, body, ship, now);
            }
            market => self.market_request(id, market),
        }
    }

    /// The charts (shared with clients).
    pub(crate) fn charts(&mut self) -> Arc<universe_world::charts::Charts> {
        self.charts.get_or_insert_with(|| Arc::new(self.world.charts())).clone()
    }

    /// Ship `id` (the player's 0, craft i: i + 1): its id, system and ship.
    pub(crate) fn ship_by_id(&self, id: usize) -> Option<(usize, usize, &Ship)> {
        if id == crate::combat::PLAYER {
            Some((id, self.ship_system, &self.ship))
        } else {
            self.crafts.get(id - 1).map(|c| (id, c.system, &c.ship))
        }
    }

    /// Advance the whole world by `real_dt` real seconds at `warp`, in ticks
    /// of at most `TICK` game seconds (pilots act once a tick, so their
    /// control rate stays in game time whatever the warp), up to
    /// `TICK_BUDGET` ticks; past that, ticks stretch and the step says so.
    pub fn step_world(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let wanted = (real_dt * warp / TICK).ceil().max(1.0);
        let n = wanted.min(TICK_BUDGET as f64) as usize;
        let mut result = StepResult::default();
        for _ in 0..n {
            let r = self.tick(real_dt / n as f64, warp, controls);
            result.simulated += r.simulated;
            result.warp_limited |= r.warp_limited;
        }
        result.warp_limited |= wanted > TICK_BUDGET as f64;
        result
    }

    /// One tick: the player's ship, then every craft, all from the same
    /// moment; the clock moves once (as far as the player's ship went).
    pub(crate) fn tick(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        self.tick += 1;
        self.log.clear();
        self.atc.because(self.tick, universe_protocol::Cause::Rules);
        // What the pilots posted that's due (or late) now (replaying: as logged).
        let due = if self.replaying {
            std::mem::take(&mut self.replay_due)
        } else {
            let came = self.npcs.collect();
            self.pending.extend(came);
            let tick = self.tick;
            let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending).into_iter().partition(|p| p.due() <= tick);
            self.pending = later;
            due
        };
        if let Some(log) = &mut self.input_log {
            log.ticks.push(crate::audit::TickInputs { tick: self.tick, before: std::mem::take(&mut self.between), real_dt, warp, due: due.clone() });
        }
        universe_prof::time("sim/postings", || self.post(due));
        let t0 = self.world.time;
        if self.snapped_at.wrapping_add(1) != self.tick {
            universe_prof::time("sim/snapshot", || self.snapshot());
        }
        let result = universe_prof::time("sim/player", || self.step(real_dt, warp, controls));
        let t1 = self.world.time;
        {
            let _p = universe_prof::scope("sim/crafts");
            // (Each craft keeps its own clock from t0; the world's stays at t1.)
            self.fly_crafts(t0, real_dt, warp);
        }
        self.world.time = t1;
        universe_prof::time("sim/combat", || self.combat(t1 - t0));
        // Traffic control looks around ten times a second (pads freed when
        // their ships leave, corridors when they're through): plenty, at a
        // sixth of the cost.
        if self.tick.is_multiple_of(PRESENCE_EVERY) {
            universe_prof::time("sim/traffic presence", || self.traffic_presence());
        }
        universe_prof::time("sim/recorder", || self.record());
        let stepped = self.markets.economy.stepped_to;
        universe_prof::time("sim/economy", || self.markets.step(self.world.time, &mut self.land, &mut self.ledger, self.tick));
        self.land.levy(&mut self.ledger, self.world.time, self.tick);
        // (The companies see to their works once a step of the economy.)
        if self.markets.economy.stepped_to > stepped {
            universe_prof::time("sim/companies", || crate::company::run(self));
        }
        self.publish_boards();
        self.update_standings();
        // (The dead-man rule counts in seconds: a look once a second.)
        if self.tick.is_multiple_of(60) {
            universe_prof::time("sim/dead man", || self.dead_man());
        }
        // The pilots get the world as it now is (replaying, what they did is logged).
        if self.replaying {
            // (The snapshot is taken here, as it was, for the next tick's start.)
            self.snapshot();
            self.snapped_at = self.tick;
            return result;
        }
        let view = Arc::new(universe_prof::time("sim/pilot view", || self.pilot_view(t1 - t0)));
        let thought = universe_prof::time("sim/pilots", || self.npcs.view(view.clone()));
        self.pending.extend(thought);
        let view = Arc::new(self.cockpit_view(view));
        let feed = std::mem::take(&mut self.player_feed);
        match &mut self.player {
            Some(c) => {
                c.feed(feed);
                let postings = universe_prof::time("sim/cockpit", || c.view(view));
                self.pending.extend(postings);
            }
            None => self.cockpit_out = Some((view, feed)),
        }
        result
    }

    /// The flight recorder's samples: a slice of the ships each tick, so
    /// every ship is sampled every `recorder::EVERY` (not all at once).
    fn record(&mut self) {
        let now = self.world.time;
        let slices = ((crate::recorder::EVERY / TICK).round() as u64).max(1);
        let k = self.tick % slices;
        if k == 0 {
            self.recorder.record(crate::combat::PLAYER, crate::recorder::Sample::of(now, self.ship_system, &self.ship, &self.player_status));
        }
        for (i, c) in self.crafts.iter().enumerate().skip(((k + slices - 1) % slices) as usize).step_by(slices as usize) {
            self.recorder.record(crate::combat::craft_id(i), crate::recorder::Sample::of(now, c.system, &c.ship, &c.status));
        }
    }

    /// What traffic control needs to hear from a ship's events this tick:
    /// its holds end when its clearance does, when it docks or goes through a
    /// gate, and when it's wrecked, replaced or leaves the system.
    pub(crate) fn traffic_events(&mut self, id: usize, events: &[Event]) {
        let done = events.iter().any(|e| {
            matches!(
                e,
                Event::Traffic(universe_world::TrafficEvent::ClearanceCancelled)
                    | Event::Ship(
                        ShipEvent::Crashed { .. }
                            | ShipEvent::Respawned
                            | ShipEvent::GateEntered { .. }
                            | ShipEvent::EnteredSystem { .. }
                            | ShipEvent::Landed { station: true, .. }
                    )
            )
        });
        if done {
            // Because of what ended its business (as logged), or its pilot's word.
            let cause = self.logged(id, |e| matches!(e, ShipEvent::Crashed { .. } | ShipEvent::Respawned | ShipEvent::GateEntered { .. } | ShipEvent::EnteredSystem { .. } | ShipEvent::Landed { station: true, .. }));
            let cause = cause.unwrap_or_else(|| self.atc.request_from(id));
            self.atc.because(self.tick, cause);
            self.atc.release(id);
            self.atc.because(self.tick, universe_protocol::Cause::Rules);
        }
        // A new ship: a clean record, and an empty hold (what was in the old
        // one went with it), because of the respawn, as logged.
        if let Some(cause) = self.logged(id, |e| matches!(e, ShipEvent::Respawned)) {
            let brought_on = self.brought_on(id);
            self.law.forget(id);
            // Its debt with the system where it comes back paid by its loss:
            // no longer an enemy there (but no friend).
            if let Some(system) = self.ship_by_id(id).map(|s| s.1)
                && self.standings.of(id, system) <= crate::standing::HOSTILE
            {
                self.standings.set(id, system, crate::standing::HOSTILE + 1.0);
            }
            self.ledger.write_off(id, self.tick, cause);
            if id == crate::combat::PLAYER {
                self.insure(brought_on, cause);
            }
        }
    }

    /// Ship `id`'s events go into the tick's log.
    pub(crate) fn log_events(&mut self, id: usize, events: &[Event]) {
        for e in events {
            if let Event::Ship(e) = e {
                self.log.push((id, e.clone()));
            }
        }
    }

    /// The latest of ship `id`'s events in this tick's log matching `which`, as a cause.
    pub(crate) fn logged(&self, id: usize, which: impl Fn(&ShipEvent) -> bool) -> Option<universe_protocol::Cause> {
        let index = self.log.iter().rposition(|(s, e)| *s == id && which(e))?;
        Some(universe_protocol::Cause::Event { tick: self.tick, index: index as u32 })
    }

    /// Traffic control's view of where every ship is, this frame: who stands
    /// on a pad or is in the column over it (below 5 km, within 800 m), and
    /// who is done with the corridor it holds (on its final run within
    /// `CORRIDOR_RELEASE`, or not cleared for it and 3 km clear of it).
    fn traffic_presence(&mut self) {
        use rayon::prelude::*;
        use universe_avionics::nav::Phase;
        use universe_avionics::NavTarget;
        let now = self.world.time;
        // Each system's ports, worked out once: where each is now and how
        // its planet is turned.
        struct Port {
            port: universe_world::Facility,
            center: DVec3,
            unturn: DQuat,
            /// A world's radius and the port's direction on it; None: a station's deck.
            ground: Option<(f64, DVec3)>,
        }
        type Seen = (Arc<StarSystem>, Arc<Vec<DVec3>>, Vec<Port>);
        // Corridors held, by ship: (system, body).
        let mut held: std::collections::HashMap<usize, Vec<(usize, usize)>> = std::collections::HashMap::new();
        for (system, body, ship) in self.atc.corridors_held() {
            held.entry(ship).or_default().push((system, body));
        }
        // Who's where (on the ground, or flying in normal space).
        type Where = (usize, usize, DVec3, bool, Option<(NavTarget, Phase)>);
        let ships: Vec<Where> = std::iter::once((crate::combat::PLAYER, self.ship_system, &self.ship, self.player_status.clearance))
            .chain(self.crafts.iter().enumerate().map(|(i, c)| (crate::combat::craft_id(i), c.system, &c.ship, c.status.clearance)))
            // (In a hangar: on no pad, in no column.)
            .filter(|(_, _, s, _)| (matches!(s.state, ShipState::Landed { .. }) && s.hangar.is_none() && s.taxi.is_none()) || (s.is_flying() && !s.hyperdrive))
            .map(|(id, system, s, clearance)| (id, system, s.position, matches!(s.state, ShipState::Landed { .. }), clearance.map(|c| (c.target, c.phase))))
            .collect();
        let mut systems: std::collections::HashMap<usize, Seen> = std::collections::HashMap::new();
        for &(_, system, ..) in &ships {
            if let std::collections::hash_map::Entry::Vacant(e) = systems.entry(system) {
                let sys = self.world.system(system);
                let positions = self.world.rails_now(system);
                let grounds = sys.spaceports.iter().enumerate().map(|(i, sp)| Port {
                    port: universe_world::Facility::Spaceport(i),
                    center: positions[sp.body],
                    unturn: sys.bodies[sp.body].rotation(now).inverse(),
                    ground: Some((sys.bodies[sp.body].rail.radius, sp.direction)),
                });
                let decks = sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == universe_world::BodyKind::Station).map(|(i, b)| Port {
                    port: universe_world::Facility::Station(i),
                    center: positions[i],
                    unturn: b.rotation(now).inverse(),
                    ground: None,
                });
                let ports = grounds.chain(decks).collect();
                e.insert((sys, positions, ports));
            }
        }
        // Each ship's facts, side by side.
        let present: Vec<universe_services::Presence> = ships
            .par_iter()
            .filter_map(|&(id, system, pos, landed, clearance)| {
                let (sys, positions, ports) = &systems[&system];
                let mut p = universe_services::Presence { ship: id, system, landed, ..Default::default() };
                // Pads: on one, or in the column over it.
                for sp in ports {
                    let off = pos - sp.center;
                    let local = sp.unturn * off;
                    let near = match sp.ground {
                        Some((radius, direction)) => off.length() - radius <= 5_000.0 && local.normalize().angle_between(direction) * radius <= 800.0,
                        // Over the deck, up to a few km above it.
                        None => local.x.abs() < 400.0 && local.z > -300.0 && local.z < 450.0 && local.y > -150.0 && local.y < 3_000.0,
                    };
                    if !near {
                        continue;
                    }
                    // (In a pad's own column, not merely nearer it than the others: a ship
                    // coming in over the field, nearest one pad then the next, had "been and
                    // gone" from its own and freed it for the next in line while still on its
                    // way down to it.)
                    let flat = |v: DVec3, up: DVec3| v - up * v.dot(up);
                    let off_pad = |k| {
                        let pad = universe_world::port::pad(sys, sp.port, k, universe_world::ship::SHIP_RADIUS);
                        let up = universe_world::port::up(sp.port, pad);
                        flat(local - pad, up).length()
                    };
                    if let Some(k) = (0..universe_world::spaceport::PADS).find(|&k| off_pad(k) < universe_world::spaceport::PAD_SIZE) {
                        p.pad = Some((sp.port, k));
                    }
                }
                // Corridors it holds and is done with.
                for &(s, b) in held.get(&id).map_or(&[][..], |v| &v[..]) {
                    if s != system {
                        continue;
                    }
                    let d = positions[b].distance(pos);
                    let cleared_for = clearance.filter(|(t, _)| matches!(t, NavTarget::Station(x) | NavTarget::Gate(x) if *x == b));
                    let done = match cleared_for {
                        Some((_, Phase::Final)) => d < CORRIDOR_RELEASE,
                        Some(_) => false,
                        None => !landed && d > 3_000.0,
                    };
                    if done {
                        p.clear_of.push(b);
                    }
                }
                (p.pad.is_some() || !p.clear_of.is_empty()).then_some(p)
            })
            .collect();
        self.atc.presence(&present);
        // Gates with traffic coming out of their tubes: due out shortly, or out
        // and heading away down the run-in, not clear of it yet.
        let mut out = Vec::new();
        let mut frames: std::collections::HashMap<usize, Vec<(usize, universe_world::GateFrame)>> = std::collections::HashMap::new();
        let all = std::iter::once((self.ship_system, &self.ship)).chain(self.crafts.iter().map(|c| (c.system, &c.ship)));
        for (system, s) in all {
            match s.state {
                ShipState::Transit { to, from, remaining, .. } if remaining < ENTRY_LEAD => out.extend(self.world.system(to).gate_to(from).map(|g| (to, g))),
                _ if s.is_flying() && !s.hyperdrive => {
                    let gates = frames.entry(system).or_insert_with(|| {
                        let sys = self.world.system(system);
                        let positions = self.world.rails_now(system);
                        sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == universe_world::BodyKind::Gate).map(|(g, _)| (g, universe_world::GateFrame::new(&sys, g, now, &positions))).collect()
                    });
                    for (g, f) in gates.iter() {
                        if universe_avionics::gate::in_final_zone(f, s.position) && (s.velocity - f.velocity).dot(f.axis()) < -1.0 {
                            out.push((system, *g));
                        }
                    }
                }
                _ => {}
            }
        }
        self.atc.outbound(out);
    }

    // The pilot's requests, to the ship's avionics (or, for `command`,
    // straight to its devices), taking effect at once.

    /// The pilot on foot (or getting up, sitting down) for `real_dt` real
    /// seconds. A new ship puts the pilot back in its seat.
    pub fn walk(&mut self, c: &WalkCommands, real_dt: f64) {
        let logged = *c;
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Walk(logged, real_dt)));
        if self.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::Respawned))) || !self.ship.is_flying() && !matches!(self.ship.state, ShipState::Landed { .. } | ShipState::Anchored { .. }) {
            self.crew = Person::default();
        }
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        let mut events = Vec::new();
        let around = self.around_crew(&sys);
        let layout = self.layouts.get(&self.ship.spec().key).cloned();
        self.crew.step(&sys, &self.ship, self.world.time, &self.positions, &around, layout.as_deref(), c, real_dt, &mut events);
        self.events.extend(events.into_iter().map(Event::Crew));
    }

    /// A hull's inside as laid out: built (each deck trimmed to the hull as it
    /// is at that height) and kept, to walk in; nothing laid out, none.
    /// The studio's walk-through: the pilot on foot at `feet` (ship frame) facing
    /// `yaw` (aboard, or on the ground the ship rests on), or (None) back in the seat.
    pub fn preview(&mut self, at: Option<(DVec3, f64)>) {
        match at {
            Some((feet, yaw)) => {
                let sys = self.ship_system();
                sys.positions(self.world.time, &mut self.positions);
                let ship = self.ship.clone();
                self.crew.stand(&sys, &ship, self.world.time, &self.positions, feet, yaw);
            }
            None => self.crew.place = universe_world::Place::Seat,
        }
    }

    /// A hull's inside as walls (its frame's triangles): walked in and bumped into.
    /// (Kept beside its decks: both walked in together.)
    pub fn set_walls(&mut self, hull: &str, walls: &[[DVec3; 3]]) {
        self.inside.entry(hull.to_string()).or_default().1 = walls.to_vec();
        self.rebuild_inside(hull);
    }

    /// A hull's walked-in inside made again from its decks and its walls.
    fn rebuild_inside(&mut self, hull: &str) {
        let Some((built, walls)) = self.inside.get(hull) else { return };
        let mut tris = built.triangles();
        tris.extend_from_slice(walls);
        if tris.is_empty() {
            self.layouts.remove(hull);
        } else {
            self.layouts.insert(hull.to_string(), Arc::new(universe_world::deckplan::Walkable { mesh: universe_world::walk::WalkMesh::new(&tris), climbs: built.climbs.clone() }));
        }
    }

    pub fn set_layout(&mut self, plan: &universe_world::deckplan::DeckPlan) {
        let Some(h) = universe_world::content::content().handle::<universe_world::ship::ClassSpec>(&plan.hull) else { return };
        let Some(mesh) = universe_world::content::content().get(h).shape().walk.clone() else { return };
        let sides: Vec<_> = plan.decks.iter().map(|d| universe_world::deckplan::deck_sides(&mesh, d.floor)).collect();
        let built = universe_world::deckplan::build(plan, &sides);
        self.inside.entry(plan.hull.clone()).or_default().0 = built;
        self.rebuild_inside(&plan.hull);
    }

    /// What stands near the pilot on a body, to walk on and bump into (its
    /// frame): the buildings of its ports (as far as they're built), the ships
    /// landed close by.
    fn around_crew(&self, sys: &StarSystem) -> Vec<universe_world::walk::Collider<'static>> {
        use universe_world::walk::Collider;
        let mut out = Vec::new();
        let Place::Outside { body, position, .. } = self.crew.place else { return out };
        let b = &sys.bodies[body];
        let now = self.world.time;
        for (port, sp) in sys.spaceports.iter().enumerate().filter(|(_, sp)| sp.body == body) {
            let r = b.surface_radius(sp.direction);
            let origin = sp.direction * r;
            if origin.distance(position) > 5_000.0 {
                continue;
            }
            let Some(g) = self.land.ground(self.ship_system, port) else { continue };
            // (As the game draws them: x east, y up, z south from the port; the ground falling away with the curve.)
            let east = universe_world::spaceport::tangent(sp.direction).1;
            let rot = DQuat::from_mat3(&glam::DMat3::from_cols(east, sp.direction, east.cross(sp.direction)));
            for w in &g.works {
                for (k, bl) in w.blocks.iter().enumerate() {
                    let p = w.progress(k, now);
                    if p <= 0.0 {
                        continue;
                    }
                    let (he, hn) = bl.half_extent();
                    let (e, n) = bl.centre;
                    let floor = -(e * e + n * n) / (2.0 * r) - 1.0;
                    let top = floor + (bl.height * p).max(0.5) + 1.0;
                    out.push(Collider::Box { at: origin + rot * DVec3::new(e, (floor + top) / 2.0, -n), rot, half: DVec3::new(he, (top - floor) / 2.0, hn) });
                }
            }
        }
        let inv = b.rotation(now).inverse();
        let center = self.positions[body];
        for craft in self.crafts.iter().filter(|c| c.system == self.ship_system && matches!(c.ship.state, ShipState::Landed { body: cb, .. } if cb == body)) {
            let at = inv * (craft.ship.position - center);
            if at.distance(position) < 400.0 {
                let ramp = universe_world::crew::ramp_angle(sys, &craft.ship);
                universe_world::crew::ship_colliders(&craft.ship, at, inv * craft.ship.orientation, ramp, &mut out);
            }
        }
        out
    }

    /// Where the pilot's eyes are and which way they look (seated: `seat_eye`).
    pub fn pilot_eye(&mut self, seat_eye: DVec3) -> (DVec3, DQuat) {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        self.crew.eye(&sys, &self.ship, self.world.time, &self.positions, seat_eye)
    }

    /// What the pilot on foot could use now.
    pub fn pilot_reach(&mut self) -> Option<universe_world::crew::Reach> {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        self.crew.reach(&sys, &self.ship, self.world.time, &self.positions)
    }

    /// The markets of the system we're in (visible from anywhere in it), with names.
    pub fn markets(&mut self) -> Vec<(Facility, String)> {
        let sys = self.ship_system();
        universe_world::traffic::facilities(&sys).into_iter().map(|f| (f, f.name(&sys))).collect()
    }

    /// The market we're docked or landed at, if any.
    pub fn docked_market(&mut self) -> Option<Facility> {
        let sys = self.ship_system();
        universe_world::traffic::docked_at(&sys, &self.ship)
    }

    /// A market in our system: its quotes.
    pub fn market_quotes(&mut self, f: Facility) -> Vec<universe_services::market::Quote> {
        let (system, sys, now) = (self.ship_system, self.ship_system(), self.world.time);
        self.markets.quotes(system, &sys, f, now)
    }

    /// A market's quote for one item (listed, or of a kind it wants), if any.
    pub fn quote_for(&mut self, f: Facility, item: usize) -> Option<universe_services::market::Quote> {
        let (system, sys, now) = (self.ship_system, self.ship_system(), self.world.time);
        self.markets.quote_for(system, &sys, f, item, now)
    }

    /// Our credits, and what's in our hold, as the ledger has them.
    pub fn credits(&self) -> f64 {
        self.ledger.credits(universe_services::Party::Pilot(crate::combat::PLAYER))
    }

    pub fn hold(&self) -> Vec<(usize, u32)> {
        self.ledger.hold(crate::combat::PLAYER)
    }

    /// Buy (`units` > 0) or sell (< 0) `item` at the market `f` we're docked
    /// at: credits paid (negative: received), or why not.
    pub fn trade(&mut self, f: Facility, item: usize, units: i64) -> Result<f64, String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Trade { market: f, item, units }));
        let r = self.pilot_trade(crate::combat::PLAYER, f, item, units);
        if let Ok(amount) = r {
            let sys = self.ship_system();
            let record = universe_services::records::TradeRecord {
                time: self.world.time,
                system: self.ship_system,
                market: f.name(&sys),
                pilot: crate::combat::PLAYER,
                place: Some(f),
                trader: "YOU".into(),
                deal: if units > 0 { universe_services::records::Deal::Bought } else { universe_services::records::Deal::Sold },
                bought: units > 0,
                item: self.world.goods[item].name.to_uppercase(),
                units: units.unsigned_abs() as u32,
                amount: amount.abs(),
                cargo: self.ship.cargo,
                credits: self.credits(),
            };
            self.log_trade(record);
        }
        r
    }

    /// A new pilot's first ship: parked, powered down, on a pad of the home
    /// station's deck that traffic control gives it (held for it as any).
    fn start_docked(&mut self) {
        let home = self.world.home_system;
        let sys = self.world.system(home);
        let Some(station) = sys.station() else { return self.start_in_flight() };
        let port = universe_world::Facility::Station(station);
        let pad = match self.atc.request_pad(home, port, crate::combat::PLAYER, self.world.time) {
            universe_services::PadGrant::Pad(k) => k,
            universe_services::PadGrant::Queued(_) => universe_world::spaceport::CENTER_PAD,
        };
        self.ship = self.world.ship_on(home, port, pad);
        self.ship.fuel = self.ship.spec().fuel_capacity;
        self.ship_system = home;
    }

    /// The player in flight 4 km behind the home station, its ship as it
    /// is (no insurer, nothing said): where tests and dev scenarios start.
    pub fn start_in_flight(&mut self) {
        self.atc.release(crate::combat::PLAYER);
        let mut events = Vec::new();
        self.world.respawn(&mut self.ship, &mut self.ship_system, &mut events);
    }

    /// Give the ship up: a new one from the insurer (see `insure`).
    pub fn respawn(&mut self) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Respawn));
        let brought_on = self.brought_on(crate::combat::PLAYER);
        let mut events = Vec::new();
        // (A new ship somewhere else: whatever traffic control held for the old one goes.)
        self.atc.because(self.tick, universe_protocol::Cause::Rules);
        self.atc.release(crate::combat::PLAYER);
        self.world.respawn(&mut self.ship, &mut self.ship_system, &mut events);
        self.player_events(events);
        self.insure(brought_on, universe_protocol::Cause::Rules);
    }

    /// The offence ship `id` brought its loss on by, if any: fair game for
    /// firing on the innocent is piracy.
    pub(crate) fn brought_on(&self, id: usize) -> Option<universe_world::registry::Offence> {
        self.law.aggressed(id as universe_protocol::BodyId, self.world.time).then_some(universe_world::registry::Offence::Piracy)
    }

    /// The player's ship lost and replaced by its insurer, on its terms
    /// (`world::order::insurer`): the excess paid, the same hull and fit
    /// delivered parked where it says (a yard). A loss it refuses (brought on
    /// by an offence it names), or an excess the pilot can't pay: the basic
    /// ship instead, delivered the same. (NPCs' operator stands its own losses.)
    fn insure(&mut self, brought_on: Option<universe_world::registry::Offence>, cause: universe_protocol::Cause) {
        use universe_services::{Asset, Party};
        let me = Party::Pilot(crate::combat::PLAYER);
        let terms = universe_world::order::insurer();
        let refused = brought_on.filter(|o| terms.is_some_and(|(_, t)| t.refuses.contains(o)));
        let excess = terms.map_or(0.0, |(_, t)| t.excess) * Universe::ship_value(&self.ship);
        let paid = match terms {
            Some((org, _)) if refused.is_none() => {
                let insurer = self.land.enlist(&org.identity.key);
                self.ledger.transfer(me, insurer, Asset::Credits, excess, self.tick, cause).is_ok()
            }
            _ => false,
        };
        if !paid {
            self.ship.class = universe_world::ship::starting_hull();
            self.ship.refresh_stock();
        }
        let yard = terms.filter(|(_, t)| t.delivered_at == universe_world::registry::OrgInsuranceDeliveredAt::Yard).and_then(|_| self.yard());
        let at = match yard {
            Some((system, port)) => {
                self.deliver(system, port);
                let sys = self.world.system(system);
                sys.spaceports[port].name.clone()
            }
            None => String::new(),
        };
        self.events.push(Event::Insured { excess: paid.then_some(excess), refused: refused.map(universe_world::order::offence_name), at });
    }

    /// A yard (a works with a building dock), the home system's first: its system and port.
    fn yard(&self) -> Option<(usize, usize)> {
        let e = &self.markets.economy;
        let mut yards: Vec<(usize, usize)> = e
            .works
            .iter()
            .filter(|w| w.setups.iter().any(|s| s.module.identity.key == "module.building-dock"))
            .filter_map(|w| match w.site {
                universe_services::economy::Site::Ground(g) => self.land.grounds.get(g).map(|g| (g.system, g.port)),
                universe_services::economy::Site::Rig(..) => None,
            })
            .collect();
        yards.sort_by_key(|&(s, _)| s != self.world.home_system);
        yards.first().copied()
    }

    /// The player's ship, as it is, set down parked on a pad of port `port` in `system`.
    fn deliver(&mut self, system: usize, port: usize) {
        let at = universe_world::Facility::Spaceport(port);
        let pad = match self.atc.request_pad(system, at, crate::combat::PLAYER, self.world.time) {
            universe_services::PadGrant::Pad(k) => k,
            universe_services::PadGrant::Queued(_) => universe_world::spaceport::CENTER_PAD,
        };
        let old = self.ship.clone();
        self.ship = self.world.ship_on(system, at, pad);
        self.ship.class = old.class;
        self.ship.trim = old.trim.clone();
        if let Some(fit) = old.fit {
            let _ = self.ship.refit((*fit).clone());
        }
        self.ship.refresh_stock();
        self.ship.fuel = self.ship.spec().fuel_capacity;
        self.ship.energy = self.ship.spec().capacitor_capacity;
        self.ship_system = system;
    }

    // What the avionics show the pilot.

    /// Display name of a target in the ship's system.
    pub fn target_name(&mut self, target: NavTarget) -> String {
        target.name(&self.ship_system())
    }

    /// Current world position of a target, and whether it's still valid.
    pub fn target_position(&mut self, target: NavTarget) -> Option<DVec3> {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        target.position(&sys, self.world.time, &self.positions)
    }

    /// Name of a stop, with its system if it's elsewhere.
    pub fn stop_name(&mut self, stop: Stop) -> String {
        route::stop_name(&self.system(stop.system), stop)
    }

}

/// The universe can move to its own thread (the world engine runs apart from the client).
#[allow(dead_code)]
fn universe_is_send() {
    fn send<T: Send>() {}
    send::<Universe>();
}
