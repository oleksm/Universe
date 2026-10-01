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
use universe_world::radar::{self, RADAR_RANGE};
use universe_world::rules::Rules;
use universe_world::weapons::{GUN_MUZZLE, SLUG_LIFETIME};
use universe_world::{Controls, ShipCommands, StarSystem};

use crate::combat::{craft_id, PLAYER};
use crate::contacts::{Contact, LOCK_BEAM};
use crate::follow::FollowKind;
use crate::pilots::{self, Pilot, PilotView, PoolLink, Posting};

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
    /// By craft index.
    pub transponders: Vec<Transponder>,
}

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
        if let Some(p) = pilots::think(&mut self.pilot, PLAYER, &view.world, Some(self.stick)) {
            self.outbox.push(p);
        }
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

    fn world(&self) -> &PilotView {
        &self.view.as_ref().expect("a view").world
    }

    /// Our ship as the latest view has it.
    pub fn ship(&self) -> &universe_world::Ship {
        &self.world().ships[PLAYER].1
    }

    fn system(&self) -> (usize, Arc<StarSystem>, Arc<Vec<DVec3>>) {
        let w = self.world();
        let s = w.ships[PLAYER].0;
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
            (None, Some(t @ (NavTarget::Station(_) | NavTarget::Gate(_)))) => Anchor::Place(t),
            _ => {
                self.refuse("FOLLOW: LOCK A SHIP (T), OR A STATION OR GATE (M)");
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
            Anchor::Ship(id) => crate::follow::mark_in(&w.snaps, w.ships[PLAYER].0, self.ship().position, id).map(|m| m.0),
            Anchor::Place(t) => t.position(sys, w.time, rails),
        }
    }

    // What the computers show.

    /// What we're following: the manoeuvre, the anchor's name, how far it is now (m).
    pub fn following_status(&self) -> Option<(Manoeuvre, String, f64)> {
        let f = self.pilot.avionics.following?;
        self.view.as_ref()?;
        let (_, sys, rails) = self.system();
        let name = match f.anchor {
            Anchor::Ship(id) => self.view.as_ref()?.transponders.get(id.checked_sub(1)?).map(|t| t.name.to_uppercase())?,
            Anchor::Place(t) => t.name(&sys).to_uppercase(),
        };
        let at = self.anchor_position(f.anchor, &sys, &rails)?;
        Some((f.manoeuvre, name, at.distance(self.ship().position)))
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

/// What our radar sees, nearest first, with each one's transponder.
fn contacts(view: &CockpitView) -> Vec<Contact> {
    let w = &view.world;
    let (system, ref ship) = w.ships[PLAYER];
    let crafts = w.ships.iter().enumerate().skip(1).map(|(id, (s, sh))| (id - 1, *s, sh));
    radar::sweep(ship, system, crafts)
        .into_iter()
        .filter(|b| b.distance < RADAR_RANGE)
        .filter_map(|blip| {
            let t = view.transponders.get(blip.id)?;
            Some(Contact { blip, name: t.name.to_uppercase(), activity: t.activity, destination: t.destination.clone(), hull: t.hull, aggressed: t.aggressed })
        })
        .collect()
}
