//! The player's cockpit (re-architecture R7): the player's pilot and ship
//! computers, a client like any NPC pilot. It reads a `CockpitView` (the
//! pilots' view of the world, with the crafts' transponders) and flies the
//! ship only by posting (`pilots::Posting`, due `COMMAND_DELAY` ticks after
//! the view it read): the human's stick and requests, and the programs
//! (autopilots, follow, fire control laying the gun).
//!
//! What it works out for the HUD (radar contacts, fire control, the flight
//! plan, the collision warning, approach guidance, follow status) is its
//! own: software modules are the client's.

use std::collections::HashMap;
use std::sync::Arc;

use glam::DVec3;
use universe_avionics::avionics::Approach;
use universe_avionics::collision::Prediction;
use universe_avionics::follow::{self, Anchor, Manoeuvre};
use universe_avionics::route::Stop;
use universe_avionics::fire_control::lead;
use universe_avionics::{Avionics, Bus, Clearance, Event, NavTarget, Plan, Solution, Track};
use universe_world::radar::RADAR_RANGE;
use universe_world::rules::Rules;
use universe_world::weapons::{GUN_MUZZLE, SLUG_LIFETIME};
use universe_world::{Controls, ShipCommands, StarSystem};

use crate::combat::{craft_id, PLAYER};
use crate::contacts::{Contact, LOCK_BEAM};
use crate::follow::FollowKind;
use crate::pilots::{self, Pilot, PilotView, PoolLink, Posting};
pub use crate::contract::{CockpitView, Transponder};

/// The player's pilot and ship computers.
pub struct Cockpit {
    pilot: Pilot,
    /// The human's stick (held until changed).
    stick: Controls,
    view: Option<Arc<CockpitView>>,
    /// Postings not yet handed over.
    outbox: Vec<Posting>,
    rules: HashMap<usize, Arc<Rules>>,
    /// The flight plan is worked out only for a display that shows it.
    pub plan_wanted: bool,
    // The ship computers' latest.
    pub contacts: Vec<Contact>,
    pub fire: Option<(Track, Option<Solution>)>,
    pub plan: Option<Arc<Plan>>,
    pub plan_serial: u64,
    pub plan_cost: f32,
    plan_age: f64,
    plan_for: Option<Clearance>,
    pub collision: Option<Prediction>,
    pub collision_at: f64,
    pub collision_cost: f32,
    collision_age: f64,
    /// Prediction (see `predict`): its own copy of the world's rules (the
    /// shared kernel, from the same seed), the turns it posted by due tick,
    /// and what its orders on their way will do to the ship: the difference
    /// they make to where it is and how it's turned, at their due tick.
    world: Option<universe_world::World>,
    turns: Vec<(u64, Option<Controls>)>,
    /// The turn held by the ship as of the view (the last posted that's due).
    held: Option<Controls>,
    pub prediction: Option<(DVec3, glam::DQuat)>,
}

impl Default for Cockpit {
    fn default() -> Self {
        Cockpit::new(Avionics::default())
    }
}

/// The plan's rebuild interval for a plan that cost `cost` seconds (about 5% of the time).
pub fn plan_every(cost: f32) -> f32 {
    (cost * 20.0).clamp(0.1, 1.0)
}

impl Cockpit {
    pub fn new(avionics: Avionics) -> Self {
        Cockpit {
            pilot: Pilot::new(avionics),
            stick: Controls::default(),
            view: None,
            outbox: Vec::new(),
            rules: HashMap::new(),
            plan_wanted: false,
            contacts: Vec::new(),
            fire: None,
            plan: None,
            plan_serial: 0,
            plan_cost: 0.0,
            plan_age: f64::INFINITY,
            plan_for: None,
            collision: None,
            collision_at: 0.0,
            collision_cost: 0.0,
            collision_age: f64::INFINITY,
            world: None,
            turns: Vec::new(),
            held: None,
            prediction: None,
        }
    }

    pub fn avionics(&self) -> &Avionics {
        &self.pilot.avionics
    }

    pub fn avionics_mut(&mut self) -> &mut Avionics {
        &mut self.pilot.avionics
    }

    /// What happened to the ship (its sensors and devices report it).
    pub fn feed(&mut self, events: Vec<universe_world::ShipEvent>) {
        self.pilot.feed.extend(events);
    }

    /// The human's stick, held until changed.
    pub fn stick(&mut self, c: Controls) {
        self.stick = c;
    }

    /// Postings to hand to the world.
    pub fn take_postings(&mut self) -> Vec<Posting> {
        std::mem::take(&mut self.outbox)
    }

    /// The latest view, without thinking on it (to act on at once).
    pub fn look(&mut self, view: Arc<CockpitView>) {
        self.view = Some(view);
    }

    /// A new view of the world: the pilot thinks (its posting goes in the
    /// outbox), and the ship computers have their say.
    pub fn view(&mut self, view: Arc<CockpitView>) {
        let dt = view.world.dt;
        self.view = Some(view.clone());
        if let Some(p) = pilots::think(&mut self.pilot, PLAYER, &view.world, Some(self.stick), &Default::default(), &Default::default()) {
            if let Some(turn) = p.turn {
                self.turns.push((p.due(), turn));
            }
            self.outbox.push(p);
        }
        self.prediction = universe_prof::time("cockpit/prediction", || self.predict());
        // Radar, and the lock (lost if it's gone).
        self.contacts = contacts(&view);
        if let Some(id) = self.pilot.avionics.contact
            && !self.contacts.iter().any(|c| c.blip.id == id)
        {
            self.pilot.avionics.contact = None;
            self.run(|_, _, events| events.push(Event::ContactLost));
        }
        self.fire = self.fire_control();
        // The flight plan flies the autopilot ahead through the physics (1-10
        // ms): rebuilt ten times a second, less often when it's dearer, or at
        // once when the clearance or phase changes.
        self.plan_age += dt;
        let key = self.pilot.avionics.clearance;
        let changed = key.map(|c| (c.target, c.autopilot, c.phase)) != self.plan_for.map(|c| (c.target, c.autopilot, c.phase));
        let flying = self.ship().is_flying();
        if !self.plan_wanted {
            self.plan = None;
        } else if changed || self.plan_age >= plan_every(self.plan_cost) as f64 || !flying {
            let start = std::time::Instant::now();
            let plan = universe_prof::time("cockpit/flight plan", || self.compute_plan());
            self.plan_cost = start.elapsed().as_secs_f32();
            self.plan = plan.map(Arc::new);
            self.plan_serial += 1;
            self.plan_age = 0.0;
            self.plan_for = key;
        }
        // The collision warning, five times a second.
        self.collision_age += dt;
        if !self.pilot.avionics.collision_warning {
            self.collision = None;
        } else if self.collision_age >= 0.2 {
            self.collision_age = 0.0;
            let start = std::time::Instant::now();
            self.collision = universe_prof::time("cockpit/collision warning", || self.compute_collision());
            self.collision_cost = start.elapsed().as_secs_f32();
            self.collision_at = view.world.time;
        }
    }

    /// What our orders on their way will do: the ship stepped through the
    /// ticks until they're all due by the shared kernel, with them and
    /// without; the difference in where it is and how it's turned. (Our
    /// ship is drawn from the world's view plus this: input shows at once.)
    fn predict(&mut self) -> Option<(DVec3, glam::DQuat)> {
        let view = self.view.clone()?;
        let w = &view.world;
        // (The ship holds its turn until told otherwise.)
        if let Some(&(_, t)) = self.turns.iter().rfind(|(due, _)| *due <= w.tick) {
            self.held = t;
        }
        self.turns.retain(|(due, _)| *due > w.tick);
        let (system, ref ship) = w.ships[&PLAYER];
        if !ship.is_flying() || (self.turns.is_empty() && self.pilot.pending.is_empty()) {
            return None;
        }
        let world = self.world.take().unwrap_or_else(|| universe_world::World::new(w.charts.seed));
        let last = w.tick + pilots::COMMAND_DELAY;
        let run = |world: &universe_world::World, with: bool| {
            let (mut ship, mut system, mut clock) = (ship.clone(), system, w.time);
            let mut events = Vec::new();
            let mut turn = self.held;
            for tick in w.tick + 1..=last {
                if with {
                    for (_, c) in self.pilot.pending.iter().filter(|(due, _)| *due == tick) {
                        world.command_at(&mut ship, system, c, clock, &mut events);
                    }
                    if let Some(&(_, t)) = self.turns.iter().rfind(|(due, _)| *due == tick) {
                        turn = t;
                    }
                }
                let commands = ShipCommands { turn, ..ship.holding() };
                world.step_ship_at(&mut clock, &mut ship, &mut system, &commands, w.dt, 1.0, &mut events);
            }
            (ship, system)
        };
        let (with, s1) = run(&world, true);
        let (without, s2) = run(&world, false);
        self.world = Some(world);
        (s1 == s2 && with.is_flying() && without.is_flying()).then(|| (with.position - without.position, with.orientation * without.orientation.inverse()))
    }

    fn world(&self) -> &PilotView {
        &self.view.as_ref().expect("a view").world
    }

    /// Our ship as the latest view has it.
    pub fn ship(&self) -> &universe_world::Ship {
        &self.world().ships[&PLAYER].1
    }

    fn system(&self) -> (usize, Arc<StarSystem>, Arc<Vec<DVec3>>) {
        let w = self.world();
        let s = w.ships[&PLAYER].0;
        let sys = w.charts.system(s);
        let rails = w.rails.get(&s).cloned().unwrap_or_else(|| {
            let mut p = Vec::new();
            sys.positions(w.time, &mut p);
            Arc::new(p)
        });
        (s, sys, rails)
    }

    /// Run `f` on the pilot now, against the latest view: what it posts goes
    /// in the outbox.
    pub fn run<R>(&mut self, f: impl FnOnce(&mut Avionics, &mut PoolLink, &mut Vec<Event>) -> R) -> R {
        let view = self.view.clone().expect("a view");
        let (r, posting) = pilots::run(&mut self.pilot, PLAYER, &view.world, f);
        self.outbox.push(posting);
        r
    }

    // The human's requests.

    pub fn command(&mut self, c: &ShipCommands) {
        self.run(|a, link, events| a.command(link, c, events));
    }

    /// The throttle changed by `delta`, or set (from the settings as they'll be).
    pub fn throttle(&mut self, delta: f64, set: Option<f64>) {
        self.run(|a, link, events| {
            let now = link.ship().throttle;
            let throttle = set.unwrap_or(now + delta).clamp(0.0, 1.0);
            if throttle != now {
                let c = ShipCommands { throttle, ..link.ship().holding() };
                a.command(link, &c, events);
            }
        });
    }

    pub fn thrusters(&mut self, rcs: DVec3) {
        self.run(|a, link, events| {
            if rcs != link.ship().rcs {
                let c = ShipCommands { rcs, ..link.ship().holding() };
                a.command(link, &c, events);
            }
        });
    }

    pub fn toggle_hyperdrive(&mut self) {
        self.run(|a, link, events| a.toggle_hyperdrive(link, events));
    }

    pub fn set_nav_target(&mut self, target: Option<NavTarget>) {
        self.run(|a, link, events| a.set_nav_target(link, target, events));
    }

    pub fn request_clearance(&mut self) -> bool {
        self.run(|a, link, events| a.request_clearance(link, events))
    }

    pub fn cancel_clearance(&mut self) {
        self.run(|a, link, events| a.cancel_clearance(link, events));
    }

    pub fn toggle_autopilot(&mut self) {
        self.run(|a, link, events| a.toggle_autopilot(link, events));
    }

    pub fn toggle_route(&mut self) {
        self.run(|a, link, events| a.toggle_route(link, events));
    }

    pub fn collision_warning(&mut self, on: bool) {
        self.pilot.avionics.collision_warning = on;
        self.collision_age = f64::INFINITY;
    }

    pub fn route_push(&mut self, stop: Stop) {
        self.pilot.avionics.route.stops.push(stop);
    }

    pub fn route_pop(&mut self) {
        self.pilot.avionics.route.pop();
    }

    pub fn route_set(&mut self, stops: Vec<Stop>) {
        self.pilot.avionics.route.clear();
        self.pilot.avionics.route.stops = stops;
    }

    /// Lock the next contact out from the one locked (the nearest, if none);
    /// past the farthest, unlock. The new lock.
    pub fn lock_next_contact(&mut self) -> Option<Contact> {
        let view = self.view.clone().expect("a view");
        self.contacts = contacts(&view);
        let from = self.locked_contact().map(|c| (c.blip.distance, c.blip.id));
        let next = self.contacts.iter().find(|c| from.is_none_or(|(d, id)| (c.blip.distance, c.blip.id) > (d, id))).cloned();
        self.pilot.avionics.contact = next.as_ref().map(|c| c.blip.id);
        if next.is_some() {
            self.pilot.avionics.rock_lock = None;
        }
        next
    }

    /// Lock in the beam: of the contacts within `LOCK_BEAM` of the nose, the
    /// one nearest the crosshair, or, if that's already locked, the next one
    /// out from it. Nothing in the beam: the lock is released. (Its event goes
    /// to the HUD.)
    pub fn lock_in_beam(&mut self) -> Option<Contact> {
        let view = self.view.clone().expect("a view");
        self.contacts = contacts(&view);
        let ship = self.ship().clone();
        let nose = ship.forward();
        let off = |c: &Contact| (c.blip.position - ship.position).angle_between(nose);
        let mut beam: Vec<Contact> = self.contacts.iter().filter(|c| off(c) <= LOCK_BEAM).cloned().collect();
        beam.sort_by(|a, b| off(a).total_cmp(&off(b)).then(a.blip.id.cmp(&b.blip.id)));
        let had = self.pilot.avionics.contact;
        let next = match had.and_then(|id| beam.iter().position(|c| c.blip.id == id)) {
            Some(i) => beam.into_iter().cycle().nth(i + 1),
            None => beam.into_iter().next(),
        };
        let next = next.filter(|c| Some(c.blip.id) != had || had.is_none());
        self.pilot.avionics.contact = next.as_ref().map(|c| c.blip.id);
        let e = match &next {
            Some(c) => Event::Lock { name: Some(c.name.clone()) },
            None if had.is_some() => Event::Lock { name: None },
            None => Event::NothingInBeam,
        };
        self.run(|_, _, events| events.push(e));
        next
    }

    /// Lock on radar contact `id` (picked from the list), if it's on the radar.
    pub fn lock_contact(&mut self, id: usize) {
        let view = self.view.clone().expect("a view");
        self.contacts = contacts(&view);
        let Some(c) = self.contacts.iter().find(|c| c.blip.id == id).cloned() else { return };
        self.pilot.avionics.contact = Some(id);
        self.pilot.avionics.rock_lock = None;
        self.run(|_, _, events| events.push(Event::Lock { name: Some(c.name) }));
    }

    /// Lock on a rock (field, body among its bodies), or let go of the lock:
    /// one lock at a time, so a ship's goes.
    pub fn lock_rock(&mut self, rock: Option<(usize, usize)>) {
        let (_, sys, _) = self.system();
        let name = rock.and_then(|(f, b)| sys.fields.get(f).and_then(|_| sys.field_bodies(f).get(b).map(|b| b.name.clone())));
        let rock = rock.filter(|_| name.is_some());
        self.pilot.avionics.rock_lock = rock;
        if rock.is_some() {
            self.pilot.avionics.contact = None;
        }
        self.run(|_, _, events| events.push(Event::Lock { name }));
    }

    /// The locked contact, if it's still on the radar.
    pub fn locked_contact(&self) -> Option<&Contact> {
        let id = self.pilot.avionics.contact?;
        self.contacts.iter().find(|c| c.blip.id == id)
    }

    /// Keep at a range from, or orbit, the locked contact (else the nav
    /// target, a station or gate). Asked again for the same kind: the next
    /// range out. The first range is the preset nearest where we are.
    pub fn follow(&mut self, kind: FollowKind) {
        let a = &self.pilot.avionics;
        let anchor = match (a.contact, a.nav_target) {
            (Some(c), _) => Anchor::Ship(craft_id(c)),
            (None, _) if self.pilot.avionics.rock_lock.is_some() => {
                let (field, body) = self.pilot.avionics.rock_lock.unwrap_or_default();
                Anchor::Rock { field, body }
            }
            (None, Some(t @ (NavTarget::Station(_) | NavTarget::Gate(_) | NavTarget::Asteroid(_)))) => Anchor::Place(t),
            _ => {
                self.refuse("FOLLOW: LOCK A SHIP (T), OR A STATION, GATE OR ASTEROID (M)");
                return;
            }
        };
        let (_, sys, rails) = self.system();
        let min = follow::min_range(&sys, anchor);
        let range = match self.pilot.avionics.following {
            Some(f) if f.anchor == anchor && same_kind(f.manoeuvre, kind) => follow::next_range(f.manoeuvre.range(), min),
            _ => {
                let Some(at) = self.anchor_position(anchor, &sys, &rails) else {
                    self.refuse("FOLLOW: TARGET NOT IN SIGHT");
                    return;
                };
                follow::nearest_range(at.distance(self.ship().position), min)
            }
        };
        let manoeuvre = match kind {
            FollowKind::KeepAt => Manoeuvre::KeepAt(range),
            FollowKind::Orbit => Manoeuvre::Orbit(range),
        };
        self.run(|a, link, events| a.follow(link, anchor, manoeuvre, events));
    }

    /// Close on a rock (body `body` among field `field`'s bodies) and hold
    /// just off its surface, turning with it: where the anchor reaches.
    pub fn close_on(&mut self, field: usize, body: usize) {
        let anchor = Anchor::Rock { field, body };
        self.run(|a, link, events| a.follow(link, anchor, Manoeuvre::Surface(universe_world::mining::CLOSE_STANDOFF), events));
    }

    pub fn stop_following(&mut self) {
        self.run(|a, link, events| a.stop_following(link, events));
    }

    fn refuse(&mut self, reason: &str) {
        let reason = reason.to_string();
        self.run(|_, _, events| events.push(Event::Refused { reason }));
    }

    fn anchor_position(&self, anchor: Anchor, sys: &StarSystem, rails: &[DVec3]) -> Option<DVec3> {
        let w = self.world();
        match anchor {
            Anchor::Ship(id) => crate::follow::mark_in(&w.snaps, w.ships[&PLAYER].0, self.ship().position, id).map(|m| m.0),
            Anchor::Place(t) => t.position(sys, w.time, rails),
            Anchor::Rock { field, body } => Some(sys.field_body_state(field, body, w.time).0),
        }
    }

    // What the computers show.

    /// What we're following: the manoeuvre, the anchor's name, how far it is now (m).
    pub fn following_status(&self) -> Option<(Manoeuvre, String, f64)> {
        let f = self.pilot.avionics.following?;
        self.view.as_ref()?;
        let (_, sys, rails) = self.system();
        let name = match f.anchor {
            Anchor::Ship(id) => self.view.as_ref()?.transponders.get(&id.checked_sub(1)?).map(|t| t.name.to_uppercase())?,
            Anchor::Place(t) => t.name(&sys).to_uppercase(),
            Anchor::Rock { field, body } => sys.field_bodies(field)[body].name.to_uppercase(),
        };
        let at = self.anchor_position(f.anchor, &sys, &rails)?;
        Some((f.manoeuvre, name, at.distance(self.ship().position)))
    }

    /// What the target marker points at: the nav target (its name and where
    /// it is), else the nearest station.
    pub fn nav_marker(&self) -> Option<(String, DVec3)> {
        self.view.as_ref()?;
        let (_, sys, rails) = self.system();
        let t = self.world().time;
        if let Some(target) = self.pilot.avionics.nav_target {
            let pos = target.position(&sys, t, &rails)?;
            let name = match target {
                NavTarget::Station(_) | NavTarget::Gate(_) | NavTarget::Asteroid(_) => target.name(&sys),
                NavTarget::Spaceport(p) => sys.spaceports[p].name.clone(),
            };
            return Some((name.to_uppercase(), pos));
        }
        Some((String::new(), rails[sys.station()?]))
    }

    /// Guidance numbers for the HUD, if cleared to dock or land.
    pub fn approach(&self) -> Option<Approach> {
        self.view.as_ref()?;
        let (_, sys, rails) = self.system();
        self.pilot.avionics.approach(&sys, self.ship(), self.world().time, &rails)
    }

    /// The flight plan, worked out now.
    pub fn plan_now(&mut self) -> Option<Plan> {
        self.compute_plan()
    }

    /// The radar, read now.
    pub fn scan(&mut self) -> Vec<Contact> {
        let view = self.view.clone().expect("a view");
        self.contacts = contacts(&view);
        self.contacts.clone()
    }

    /// Fire control, now, on `contacts`.
    pub fn fire_control_on(&mut self, contacts: &[Contact]) -> Option<(Track, Option<Solution>)> {
        self.contacts = contacts.to_vec();
        self.fire_control()
    }

    /// The collision warning, now (if it's switched on).
    pub fn collision_now(&mut self, contacts: &[Contact]) -> Option<Prediction> {
        if !self.pilot.avionics.collision_warning {
            return None;
        }
        self.contacts = contacts.to_vec();
        self.compute_collision()
    }

    fn compute_plan(&mut self) -> Option<Plan> {
        let (s, sys, _) = self.system();
        let charts = self.world().charts.clone();
        let rules = self.rules.entry(s).or_insert_with(|| Arc::new(universe_world::structures::rules(&charts.galaxy, &sys))).clone();
        self.pilot.avionics.plan(&sys, &rules, self.ship(), self.world().time)
    }

    fn compute_collision(&self) -> Option<Prediction> {
        use universe_avionics::collision::{predict, Traffic, RANGE};
        let ship = self.ship();
        if !ship.is_flying() || ship.hyperdrive {
            return None;
        }
        let traffic: Vec<Traffic> = self.contacts.iter().filter(|c| c.blip.distance < RANGE).map(|c| Traffic { name: c.name.clone(), position: c.blip.position, velocity: c.blip.velocity }).collect();
        let (_, sys, rails) = self.system();
        Some(predict(&sys, ship, self.world().time, &rails, &traffic))
    }

    /// Fire control on the locked contact: keep the track going, and once
    /// it's settled, the gun's lead; in combat mode, lay the gun on it.
    fn fire_control(&mut self) -> Option<(Track, Option<Solution>)> {
        let ship = self.ship().clone();
        let Some(c) = self.locked_contact().cloned() else {
            self.pilot.avionics.track = None;
            if ship.gun_target.is_some() {
                self.command(&ShipCommands { gun_target: Some(None), ..ship.holding() });
            }
            return None;
        };
        let now = self.world().time;
        Track::update(&mut self.pilot.avionics.track, c.blip.id, c.blip.position, c.blip.velocity, now);
        let track = self.pilot.avionics.track?;
        // Gravity pulls the round as it does the target: lead on the rest of its acceleration.
        let (_, sys, rails) = self.system();
        let accel = track.acceleration - sys.gravity(c.blip.position, &rails);
        let solution = track.ready().then(|| lead(ship.position, ship.velocity, c.blip.position, c.blip.velocity, accel, GUN_MUZZLE, SLUG_LIFETIME)).flatten();
        let lay = solution.filter(|_| ship.armed).map(|s| s.aim);
        if lay != ship.gun_target {
            self.command(&ShipCommands { gun_target: Some(lay), ..ship.holding() });
        }
        Some((track, solution))
    }
}

fn same_kind(m: Manoeuvre, kind: FollowKind) -> bool {
    matches!((m, kind), (Manoeuvre::KeepAt(_), FollowKind::KeepAt) | (Manoeuvre::Orbit(_), FollowKind::Orbit))
}

/// What our radar sees, nearest first, with each one's transponder (from
/// the snapshot: what anyone sees of anyone).
fn contacts(view: &CockpitView) -> Vec<Contact> {
    let w = &view.world;
    let (system, ref ship) = w.ships[&PLAYER];
    let mut out: Vec<Contact> = w
        .snaps
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, s)| s.system == system && (s.flying || s.landed))
        .filter_map(|(id, s)| {
            let distance = s.position.distance(ship.position);
            if distance >= RADAR_RANGE {
                return None;
            }
            let t = view.transponders.get(&(id - 1))?;
            let blip = universe_world::radar::Blip { id: id - 1, position: s.position, velocity: s.velocity, distance };
            Some(Contact { blip, name: t.name.to_uppercase(), activity: t.activity, destination: t.destination.clone(), hull: t.hull, aggressed: t.aggressed })
        })
        .collect();
    out.sort_by(|a, b| a.blip.distance.total_cmp(&b.blip.distance).then(a.blip.id.cmp(&b.blip.id)));
    out
}

/// The player's cockpit, in step with the world (tests, and the engine
/// before the client takes it): the player's requests go through it.
impl crate::universe::Universe {
    /// The cockpit, here (it isn't when the client has it).
    pub fn cockpit(&mut self) -> &mut Cockpit {
        self.player.as_mut().and_then(|p| p.as_any_mut().downcast_mut::<Cockpit>()).expect("the cockpit is with the client")
    }

    /// The player's avionics (in the cockpit, here).
    pub fn avionics(&self) -> &Avionics {
        self.player.as_ref().and_then(|p| p.as_any().downcast_ref::<Cockpit>()).expect("the cockpit is with the client").avionics()
    }

    pub fn avionics_mut(&mut self) -> &mut Avionics {
        self.cockpit().avionics_mut()
    }

    /// The cockpit, looking at the world as it is now (to act at once).
    pub fn cockpit_now(&mut self) -> &mut Cockpit {
        let world = Arc::new(self.pilot_view(crate::universe::TICK));
        let view = Arc::new(self.cockpit_view(world));
        let c = self.cockpit();
        c.look(view);
        c
    }

    /// What the cockpit posted, in now (its devices' commands still at their due tick).
    pub(crate) fn flush_cockpit(&mut self) {
        if let Some(c) = self.player.as_mut().and_then(|p| p.as_any_mut().downcast_mut::<Cockpit>()) {
            let postings = c.take_postings();
            self.post_now(postings);
        }
    }

    /// The cockpit does `f` now, against the world as it is: what it posts goes in.
    pub(crate) fn in_cockpit<R>(&mut self, f: impl FnOnce(&mut Cockpit) -> R) -> R {
        let r = f(self.cockpit_now());
        self.flush_cockpit();
        r
    }

    /// Give the ship's devices new commands (through the cockpit, as the pilot would).
    pub fn command(&mut self, c: &ShipCommands) {
        self.in_cockpit(|k| k.command(c));
    }

    pub fn throttle(&mut self, delta: f64, set: Option<f64>) {
        self.in_cockpit(|k| k.throttle(delta, set));
    }

    pub fn thrusters(&mut self, rcs: DVec3) {
        self.in_cockpit(|k| k.thrusters(rcs));
    }

    pub fn toggle_hyperdrive(&mut self) {
        self.in_cockpit(|k| k.toggle_hyperdrive());
    }

    /// Lock (or clear) the navigation target.
    pub fn set_nav_target(&mut self, target: Option<NavTarget>) {
        self.in_cockpit(|k| k.set_nav_target(target));
    }

    /// Ask traffic control for permission to dock/land at the locked nav
    /// target (or the nearest station).
    pub fn request_clearance(&mut self) -> bool {
        self.in_cockpit(|k| k.request_clearance())
    }

    /// Give the clearance up (stopping its autopilot).
    pub fn cancel_clearance(&mut self) {
        self.in_cockpit(|k| k.cancel_clearance());
    }

    /// Engage or release the autopilot. In hyperdrive it steers to the nav
    /// target; otherwise it docks or lands (requesting clearance if needed).
    pub fn toggle_autopilot(&mut self) {
        self.in_cockpit(|k| k.toggle_autopilot());
    }

    /// Start or stop the route autopilot.
    pub fn toggle_route(&mut self) {
        self.in_cockpit(|k| k.toggle_route());
    }

    /// Guidance numbers for the HUD, if cleared to dock or land.
    pub fn approach(&mut self) -> Option<Approach> {
        self.cockpit_now().approach()
    }

    /// The flight plan to the cleared target: what to do from here, as the
    /// autopilot would do it.
    pub fn plan(&mut self) -> Option<Plan> {
        self.cockpit_now().plan_now()
    }

    /// Docking guidance only (convenience for tests and tools).
    pub fn docking_status(&mut self) -> Option<(usize, universe_avionics::DockingStatus)> {
        match self.approach()? {
            Approach::Dock { station, status } => Some((station, status)),
            Approach::Land { .. } | Approach::Transit { .. } => None,
        }
    }

    /// Keep at a range from, or orbit, the locked contact (else the nav
    /// target, a station or gate): the cockpit's follow program.
    pub fn follow(&mut self, kind: FollowKind) {
        self.in_cockpit(|k| k.follow(kind));
    }

    pub fn close_on(&mut self, field: usize, body: usize) {
        self.in_cockpit(|k| k.close_on(field, body));
    }

    /// What we're following, for the display: the manoeuvre, the anchor's
    /// name, and how far it is now (m).
    pub fn following_status(&mut self) -> Option<(Manoeuvre, String, f64)> {
        self.cockpit_now().following_status()
    }

    /// Stop following.
    pub fn stop_following(&mut self) {
        self.in_cockpit(|k| k.stop_following());
    }

    /// Ships the radar sees, nearest first (the cockpit's radar, now).
    pub fn contacts(&mut self) -> Vec<Contact> {
        self.cockpit_now().scan()
    }

    /// Lock the next contact out from the one locked (the nearest, if none);
    /// past the farthest, unlock. Returns the new lock.
    pub fn lock_next_contact(&mut self) -> Option<Contact> {
        self.in_cockpit(|k| k.lock_next_contact())
    }

    /// With the collision warning on, and flying in normal space: the path
    /// ahead and what it would hit.
    pub fn collision_warning(&mut self, contacts: &[Contact]) -> Option<universe_avionics::collision::Prediction> {
        self.cockpit_now().collision_now(contacts)
    }

    /// Lock in the beam (see `Cockpit::lock_in_beam`).
    pub fn lock_in_beam(&mut self) -> Option<Contact> {
        self.in_cockpit(|k| k.lock_in_beam())
    }

    /// The locked contact, if it's still on the radar.
    pub fn locked_contact_in<'a>(&self, contacts: &'a [Contact]) -> Option<&'a Contact> {
        let id = self.avionics().contact?;
        contacts.iter().find(|c| c.blip.id == id)
    }
}

impl crate::contract::PlayerClient for Cockpit {
    fn feed(&mut self, events: Vec<universe_world::ShipEvent>) {
        Cockpit::feed(self, events);
    }
    fn stick(&mut self, c: Controls) {
        Cockpit::stick(self, c);
    }
    fn view(&mut self, view: Arc<CockpitView>) -> Vec<Posting> {
        Cockpit::view(self, view);
        self.take_postings()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}
