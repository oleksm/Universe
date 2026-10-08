//! Named start-up scenarios for quickly checking features visually:
//! `UNIVERSE_SCENARIO=galaxy UNIVERSE_SCREENSHOT=out.png cargo run`

use universe_engine::glam::DVec3;
use universe_sim::ship::SHIP_RADIUS;
use universe_sim::{BodyKind, Controls, Event, GateFrame, NavTarget, PadFrame, Phase, ShipCommands, ShipEvent, ShipState, StationFrame};

use crate::observer::Focus;
use crate::{App, Mode};

pub fn apply(app: &mut App, name: &str) {
    // (Scenarios start in flight behind the home station, as a new pilot
    // once did, not parked on its deck.)
    if matches!(app.engine.universe().vessels[universe_sim::PLAYER].ship.state, ShipState::Landed { .. }) && app.engine.universe().world.time < 1.0 {
        app.engine.universe().start_in_flight();
    }
    // (UNIVERSE_HOURS: the scenario that many hours on, for the sun somewhere else.)
    if let Some(h) = std::env::var("UNIVERSE_HOURS").ok().and_then(|h| h.parse::<f64>().ok()) {
        app.engine.universe().world.time += h * 3600.0;
    }
    let home = app.engine.universe().world.home_system;
    let sys = app.engine.universe().system(home);
    let t = app.engine.universe().world.time;
    let mut positions = Vec::new();
    sys.positions(t, &mut positions);
    let outer = sys.bodies.iter().filter_map(|b| b.rail.orbit.as_ref().filter(|_| b.rail.parent == Some(0) && b.kind.is_planet())).map(|o| o.apoapsis()).fold(0.0, f64::max);
    let station = sys.station().unwrap_or(0);
    // (UNIVERSE_BODY: another body, by its key.)
    let planet = std::env::var("UNIVERSE_BODY").ok().and_then(|k| sys.bodies.iter().position(|b| b.key == k)).unwrap_or(sys.bodies[station].rail.parent.unwrap_or(0));
    let c = Ctx { home, sys, t, positions, outer, station, planet };

    if name.starts_with("look_") {
        look(app, name, &c);
    } else if let Some(s) = SCENARIOS.iter().find(|s| s.names.contains(&name)) {
        (s.run)(app, name, &c);
    } else {
        log::warn!("unknown scenario {name:?}; these are known (and look_<hull>):");
        for s in SCENARIOS {
            log::warn!("  {}: {}", s.names.join(" "), s.about);
        }
    }
    app.messages.clear();
    let u = app.engine.universe();
    log::info!("scenario {name}: pending events {:?}, clearance {:?}", u.events, u.avionics().clearance);
    if std::env::var_os("UNIVERSE_ATC_JOURNAL").is_some() {
        for c in u.services.atc.journal.iter().filter(|c| c.ship == universe_sim::PLAYER) {
            log::info!("atc: {c:?}");
        }
    }

    // Optional camera override: UNIVERSE_CAM=cockpit | map (observer watching the ship from afar).
    match std::env::var("UNIVERSE_CAM").as_deref() {
        Ok("cockpit") => app.chase_cam = false,
        Ok("map") => {
            app.mode = Mode::Observer;
            app.observer.focus = Focus::Ship;
            app.observer.distance = 7_000.0;
            app.observer.pitch = 0.25;
        }
        _ => {}
    }
}

/// `UNIVERSE_SOUND_TEST=1`: play every sound in sequence, for checking levels.
/// Returns false once the sequence is over.
pub fn sound_test(ctx: &universe_engine::Context, t: f64, last_t: f64) -> bool {
    let Some(a) = ctx.audio() else { return false };
    let at = |x: f64| last_t < x && t >= x;
    let events = [
        (0.5, Event::Ship(ShipEvent::TookOff)),
        (1.0, Event::Ship(ShipEvent::Landed { body: String::new(), station: false })),
        (1.8, Event::Ship(ShipEvent::HyperdriveEngaged)),
        (2.8, Event::Ship(ShipEvent::HyperdriveDisengaged)),
        (3.8, Event::Ship(ShipEvent::Crashed { body: String::new() })),
    ];
    for (time, event) in &events {
        if at(*time) {
            crate::sound::event(ctx, None, event);
        }
    }
    if at(0.2) {
        crate::sound::click(ctx, 1000.0);
    }
    // Engine rumble at full throttle 6-8 s, hyperdrive drone 8.5-10.5 s.
    a.set_engine(if (6.0..8.0).contains(&t) { 1.0 } else { 0.0 });
    a.set_drone(if (8.5..10.5).contains(&t) { 0.7 } else { 0.0 }, 150.0);
    t < 11.0
}

/// The pilot just inside the ship's hatch, facing out (down its ramp) and
/// turned `yaw` from there.
fn at_hatch(app: &mut App, yaw: f64) {
    let u = app.engine.universe();
    let sys = u.ship_system();
    let mut positions = Vec::new();
    sys.positions(u.world.time, &mut positions);
    if let Some(feet) = universe_sim::world::crew::inside_hatch(&u.vessels[universe_sim::PLAYER].ship, universe_sim::world::crew::ramp_angle(&sys, &u.vessels[universe_sim::PLAYER].ship)) {
        let ship = u.vessels[universe_sim::PLAYER].ship.clone();
        let (top, foot) = universe_sim::world::crew::stair(&ship);
        let out = foot - top;
        u.crew.stand(&sys, &ship, u.world.time, &positions, feet, f64::atan2(-out.x, -out.z) + yaw);
    }
}

/// Our hull the MC-07, as the game flies it by default.
fn mc07(app: &mut App) {
    if let Ok(h) = std::fs::read("assets/models/mc07.glb").map_err(|e| e.to_string()).and_then(|b| universe_sim::world::import::commission(&b, "assets/models/mc07.glb")) {
        let u = app.engine.universe();
        u.vessels[universe_sim::PLAYER].ship.class = h;
        u.vessels[universe_sim::PLAYER].ship.refresh();
        app.ship = app.engine.universe().vessels[universe_sim::PLAYER].ship.clone();
    }
}

/// A deck laid out for a look: a floor across the hold, a wall across it with a
/// door in the middle, a curved wall (on our hull, on the hold's floor).
fn demo_plan(app: &App) -> universe_sim::world::deckplan::DeckPlan {
    use universe_engine::glam::DVec2;
    use universe_sim::world::deckplan::{Deck, DeckPlan, Door, Wall};
    let floor = app.ship.spec().shape().walk.as_ref().map_or(-7.7, |m| m.lo.y + 5.1);
    let mut deck = Deck::at(floor);
    deck.planes.push(vec![DVec2::new(-14.0, -12.0), DVec2::new(14.0, -12.0), DVec2::new(14.0, 8.0), DVec2::new(-14.0, 8.0)]);
    deck.walls.push(Wall { points: vec![DVec2::new(-14.0, -3.0), DVec2::new(14.0, -3.0)], bulges: vec![0.0], doors: vec![Door { at: 14.0, width: 0.9, height: 2.1 }], rail: false });
    deck.walls.push(Wall { points: vec![DVec2::new(-6.0, -3.0), DVec2::new(-6.0, 6.0), DVec2::new(6.0, 6.0)], bulges: vec![0.0, 2.5], doors: vec![], rail: false });
    // A stair and a ladder up to a deck above it.
    deck.stairs.push(universe_sim::world::deckplan::Stair { from: DVec2::new(9.0, 6.0), to: DVec2::new(9.0, 0.0), width: 1.0 });
    deck.ladders.push(universe_sim::world::deckplan::Ladder { at: DVec2::new(-10.0, 3.0) });
    let mut upper = Deck::at(floor + 3.8);
    upper.planes.push(vec![DVec2::new(-14.0, -12.0), DVec2::new(14.0, -12.0), DVec2::new(14.0, 8.0), DVec2::new(-14.0, 8.0)]);
    DeckPlan { hull: app.ship.spec().key.clone(), decks: vec![deck, upper] }
}

/// Landed at Port Trethi, its warehouse holding a few tonnes of what the
/// registry's stock is (none is seeded yet).
fn trethi_stocked(app: &mut App) {
    let u = app.engine.universe();
    let home = u.vessels[universe_sim::PLAYER].system;
    let sys = u.ship_system();
    let Some(p) = sys.spaceports.iter().position(|s| s.name == "Port Trethi") else { return };
    let f = universe_sim::world::Facility::Spaceport(p);
    u.vessels[universe_sim::PLAYER].ship = u.world.ship_on(home, f, 0);
    for (key, t) in [("stock.al6061-pl-5", 40.0), ("stock.al6061-ingot", 120.0), ("good.bauxite", 900.0), ("stock.deuterium-liq", 30.0), ("good.bread", 12.0)] {
        if let Some(i) = universe_sim::world::goods::item(key) {
            u.services.markets.economy.put(home, f, i, t * 1000.0);
        }
    }
}


/// A scenario: the names it answers to, what it shows, and how it's set up (the names it was
/// asked by in `name`: one function serves its variants).
pub struct Scenario {
    pub names: &'static [&'static str],
    pub about: &'static str,
    pub run: fn(&mut App, &str, &Ctx),
}

/// What every scenario starts from: the home system, its bodies at the time, the outermost
/// planet's reach, the home station and the planet it circles (or `UNIVERSE_BODY`).
pub struct Ctx {
    pub home: usize,
    pub sys: std::sync::Arc<universe_sim::world::StarSystem>,
    pub t: f64,
    pub positions: Vec<DVec3>,
    pub outer: f64,
    pub station: usize,
    pub planet: usize,
}

/// The observer on `body` of system `home`, `distance` off, pitched `pitch`.
fn observe(app: &mut App, home: usize, body: usize, distance: f64, pitch: f64) {
    app.mode = Mode::Observer;
    app.observer.focus = Focus::Body { system: home, body };
    app.observer.distance = distance;
    app.observer.pitch = pitch;
}

/// Every scenario, in the order they're listed.
pub const SCENARIOS: &[Scenario] = &[
    Scenario { names: &["settlement"], about: "Our ship on a pad at a port with ground recorded (UNIVERSE_PORT, else Port Trethi), the camera over it (UNIVERSE_DIST m off, UNIVERSE_YAW, UNIVERSE_PITCH).", run: s_settlement },
    Scenario { names: &["system"], about: "The home system from above, out past its outermost planet.", run: s_system },
    Scenario { names: &["inner"], about: "The home system's inner planets from above.", run: s_inner },
    Scenario { names: &["planet"], about: "The home station's planet (or UNIVERSE_BODY) from UNIVERSE_DIST radii off (6), turned UNIVERSE_YAW.", run: s_planet },
    Scenario { names: &["giant"], about: "The home system's ringed planet, else its gas giant.", run: s_giant },
    Scenario { names: &["rings"], about: "Nearest ringed planet in the neighbourhood.", run: s_rings },
    Scenario { names: &["galaxy"], about: "The galaxy from far outside.", run: s_galaxy },
    Scenario { names: &["neighbours"], about: "The stars round the home system.", run: s_neighbours },
    Scenario { names: &["cockpit"], about: "Flying, from the cockpit.", run: s_cockpit },
    Scenario { names: &["watch", "padwatch"], about: "Docked (padwatch: on the spaceport's pad), watching our ship (TAB's other view); UNIVERSE_DIST, UNIVERSE_PITCH, UNIVERSE_YAW to frame it.", run: s_watch },
    Scenario { names: &["standards"], about: "Docked, the standards registry open on UNIVERSE_STANDARD (a key; the first otherwise).", run: s_standards },
    Scenario { names: &["sunlit"], about: "Our ship in open space, watched, turned so the sun shines on it from over the eye's shoulder (like a model lit from the front).", run: s_sunlit },
    Scenario { names: &["showcase"], about: "Looking away from the sun, a little to one side and down.", run: s_showcase },
    Scenario { names: &["hyper"], about: "Aim at a nearby star on the open-sky side of the planet, and jump.", run: s_hyper },
    Scenario { names: &["landed"], about: "Drop the ship just above the station's planet, matching its surface motion.", run: s_landed },
    Scenario { names: &["approach", "lost", "approachkeep"], about: "Cleared to dock, flying manually.", run: s_approach },
    Scenario { names: &["passengers"], about: "Docked at the home station, hungry, a thousand waiting to leave; a passenger cabin fitted; the passengers panel open.", run: s_passengers },
    Scenario { names: &["cleared"], about: "The spawn point, with docking clearance granted.", run: s_cleared },
    Scenario { names: &["offcourse"], about: "Cleared, but 6 km off to the side and drifting sideways at 40 m/s.", run: s_offcourse },
    Scenario { names: &["galaxymap", "galaxyzoom"], about: "The galaxy map, having been to the gate network's systems.", run: s_galaxymap },
    Scenario { names: &["help"], about: "Flying, with the help card open.", run: s_help },
    Scenario { names: &["zoning"], about: "The economy panel on Port Trethi's ground (UNIVERSE_PICK.", run: s_zoning },
    Scenario { names: &["economy", "economyheard", "economymodules"], about: "Heard: the world run a while first, reports put out and on their way.", run: s_economy },
    Scenario { names: &["navmap", "netmap", "navzoom"], about: "The navigation map (netmap: the gate network shown; navzoom: zoomed in).", run: s_navmap },
    Scenario { names: &["landing", "padview", "autoland", "touchdown"], about: "Target the spaceport on the station's planet and get landing clearance.", run: s_landing },
    Scenario { names: &["holding"], about: "The port's nine pads taken (long stops), and we're queued.", run: s_holding },
    Scenario { names: &["asteroid", "swarm", "navlock", "orbitrock", "orbitrock1k"], about: "By the first field's remnant (sun behind), or among its swarm, moving with it, facing it; it's the nav target.", run: s_asteroid },
    Scenario { names: &["worlds"], about: "The planet studio, gone to UNIVERSE_WORLD (a world id.", run: s_worlds },
    Scenario { names: &["rig"], about: "Hadley Orbital Works, a rig with no model.", run: s_rig },
    Scenario { names: &["beltpick", "beltrock", "beltmining"], about: "In the home system's main belt, as it is (see `belts`).", run: s_beltpick },
    Scenario { names: &["mining", "cargo", "prospect", "closing", "closing10", "closingwatch", "minemode", "pulse", "picklist"], about: "By a rubble fragment of a home field, drifting with its surface.", run: s_mining },
    Scenario { names: &["gate", "gateauto", "transit", "gatearrive", "gateorbit", "gateorbit60", "gateorbitwatch", "orbitpick", "gateorbitjets"], about: "A gate out of the home system.", run: s_gate },
    Scenario { names: &["sam", "samfar"], about: "Aggressed, 9 km out from a defended station or port, looking at its turrets (far.", run: s_sam },
    Scenario { names: &["thrusters"], about: "The thrusters panel, the main drive at full and a turn under way.", run: s_thrusters },
    Scenario { names: &["burn", "burnside"], about: "The main drive at full, seen from the chase view (\"burnside\".", run: s_burn },
    Scenario { names: &["gateflash"], about: "9 km off the home system's first gate, facing it.", run: s_gateflash },
    Scenario { names: &["deckshadowside", "deckshadowfront", "deckshadowface"], about: "The home station's deck from above, at a moment in its turn when the sun is round to the side of the structure (its shadow across part of the deck) or in front of it (the ships' shadows long across the deck).", run: s_deckshadowside },
    Scenario { names: &["sunedge0", "sunedge1", "sunedge2"], about: "On the deck, looking at the sun just over the structure's top edge (500 m off).", run: s_sunedge0 },
    Scenario { names: &["alarms"], about: "In flight by the home station, the hull critical and the tank nearly dry.", run: s_alarms },
    Scenario { names: &["showship"], about: "Our ship as the hull UNIVERSE_HULL names (default the hauler), in sunlight by the home station, seen from the side and above.", run: s_showship },
    Scenario { names: &["manual"], about: "In flight by the home station, the flight computer off, a nose thruster and the opposite tail one held (a yaw).", run: s_manual },
    Scenario { names: &["platform", "platformdeck"], about: "The home station from off its corner, a little above its deck (or, \"platformdeck\", from just above the deck), looking at it.", run: s_platform },
    Scenario { names: &["orbit"], about: "7 km off the home station, orbiting it at 5 km.", run: s_orbit },
    Scenario { names: &["orbitship", "orbitshipwatch", "orbitshiplively"], about: "3 km off a settler cruising past the home station (its route stopped, its engine on low), orbiting it at 1 km.", run: s_orbitship },
    Scenario { names: &["hangrepro", "hangrepro2"], about: "As reported: in Reat, 117 km over a starport and coming down, combat mode, a pirate near, a rock still locked from home.", run: s_hangrepro },
    Scenario { names: &["taxiwatch"], about: "Landed at a port; a settler on the next pad over, a long stay begun.", run: s_taxiwatch },
    Scenario { names: &["fleet"], about: "Each hull beside us in a row, 150 m apart, as we fly.", run: s_fleet },
    Scenario { names: &["sunclose"], about: "A tenth of an AU from the star, facing it.", run: s_sunclose },
    Scenario { names: &["sun"], about: "Facing the star from the home orbit.", run: s_sun },
    Scenario { names: &["noon", "dusk", "night"], about: "2 km over the home planet, level, looking along the ground.", run: s_noon },
    Scenario { names: &["lowflight"], about: "Skimming 6 km over the home planet, 800 km from the port, looking ahead.", run: s_lowflight },
    Scenario { names: &["moon"], about: "A moon of the home system with ground, from a little over two radii off.", run: s_moon },
    Scenario { names: &["routemap"], about: "A settler's ten-stop route set as ours, on the navigation map.", run: s_routemap },
    Scenario { names: &["route"], about: "A settler route, flown headless through its first stops, then shown mid-leg.", run: s_route },
    Scenario { names: &["traffic"], about: "Let the settlers get going, then watch the home station.", run: s_traffic },
    Scenario { names: &["crowd"], about: "A crowded place without the wait.", run: s_crowd },
    Scenario { names: &["collision"], about: "Collision warning on, 3 km from the station and closing at 80 m/s, a little off its center.", run: s_collision },
    Scenario { names: &["pirates"], about: "A pirate goes after a trader near us; run until it's destroyed (the kill feed shows it), watching from alongside.", run: s_pirates },
    Scenario { names: &["aboard"], about: "Out of the seat, at the back of the cabin looking forward up the corridor.", run: s_aboard },
    Scenario { names: &["boarded"], about: "Just in through the hatch, facing into the ship.", run: s_boarded },
    Scenario { names: &["vending", "vendingopen"], about: "Landed at the port, on foot by its vending machine, facing it (\"vendingopen\".", run: s_vending },
    Scenario { names: &["rawland"], about: "Set down on open ground (no port), step out, walk off a way and face the ship.", run: s_rawland },
    Scenario { names: &["sunlook"], about: "Landed (on a pad at Port Trethi, by day), standing in the hold, looking straight at the sun (through the hull).", run: s_sunlook },
    Scenario { names: &["studiowalk"], about: "The demo layout walked: up the ramp (UNIVERSE_SIDE m off the centre line) and on forward, through the wall's door or into it (where you end: logged).", run: s_studiowalk },
    Scenario { names: &["layoutlook"], about: "Six decks (5.6 m up from the keel, 2.9 m apart), each filled, then down the ramp, out and off to the side (UNIVERSE_SIDE m), looking back up at the ship.", run: s_layoutlook },
    Scenario { names: &["studiopreview"], about: "The studio's walk-through on the demo plan.", run: s_studiopreview },
    Scenario { names: &["interior"], about: "The shipyard's interior studio on our hull (UNIVERSE_TURN=yaw,pitch.", run: s_interior },
    Scenario { names: &["studio"], about: "The shipyard's layout studio on our hull, with a deck laid out for a look (UNIVERSE_EMPTY.", run: s_studio },
    Scenario { names: &["rampup"], about: "Out and down the ramp, then back up it into the ship (where you end up.", run: s_rampup },
    Scenario { names: &["outside"], about: "Land on the pad, step out, turn round to look at the ship.", run: s_outside },
    Scenario { names: &["radar", "contacts", "gunnery"], about: "Fly alongside a settler under way, 6 km behind and to the side of it, lock it on the radar and face it.", run: s_radar },
    Scenario { names: &["follow"], about: "Follow a settler that's on an approach (docking, landing or a gate run).", run: s_follow },
    Scenario { names: &["network"], about: "The gate network's systems round home, from far above.", run: s_network },
    Scenario { names: &["autodock", "docked"], about: "Docking computer flying from the spawn point, captured mid-final (or docked).", run: s_autodock },
    Scenario { names: &["trades"], about: "Traffic under way until traders trade here (the feed shows them).", run: s_trades },
    Scenario { names: &["market"], about: "Landed at Port Trethi, its warehouse stocked with a few things, the market open; bought ten tonnes of plate.", run: s_market },
    Scenario { names: &["enemy"], about: "On approach to the home station, an enemy of the home system.", run: s_enemy },
    Scenario { names: &["newsdesk", "newsticker"], about: "Docked at home; traffic runs 12 minutes, the outlets and we listening as it goes; then the news panel.", run: s_newsdesk },
    Scenario { names: &["marketnear", "marketfar"], about: "Docked at home, the world run a while (boards put out), looking at another market's prices as its board reached us.", run: s_marketnear },
];

fn look(app: &mut App, name: &str, c: &Ctx) {
    let home = c.home;
    let look = name;
    // One hull ("look_drover"…) in front of us, turned three-quarters on, sunlit.
    app.mode = Mode::Pilot;
    app.chase_cam = false;
    let key = format!("hull.{}", &look[5..]);
    let u = app.engine.universe();
    let Some(hull) = universe_sim::world::content::content().handle::<universe_sim::world::ship::ClassSpec>(&key) else { return };
    let (at, v, o) = (u.vessels[universe_sim::PLAYER].ship.position, u.vessels[universe_sim::PLAYER].ship.velocity, u.vessels[universe_sim::PLAYER].ship.orientation);
    let size = universe_sim::world::content::content().get(hull).shape().mesh.bound();
    let c = &mut u.vessels[universe_sim::craft_id(0)];
    c.ship.class = hull;
    c.ship.state = ShipState::Flying;
    // (Low and to the left: clear of the home station's crowd ahead.)
    c.ship.position = at + o * DVec3::new(-0.9, -0.55, -3.6) * size;
    c.ship.velocity = v;
    // Nose to the left and a little toward us, its top tilted our way.
    c.ship.orientation = universe_sim::ship::facing(o * DVec3::new(-1.0, 0.25, 0.2).normalize(), o * DVec3::new(0.0, 0.45, 1.0).normalize());
    c.system = home;
    u.pilots()[0].avionics.route.active = false;
}

fn s_settlement(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    // Our ship on a pad at a port with ground recorded (UNIVERSE_PORT, else Port
    // Trethi), the camera over it (UNIVERSE_DIST m off, UNIVERSE_YAW, UNIVERSE_PITCH).
    let name = std::env::var("UNIVERSE_PORT").unwrap_or_else(|_| "Port Trethi".into());
    let Some(port) = sys.spaceports.iter().position(|p| p.name.eq_ignore_ascii_case(&name)) else { return };
    // (Land done first as in `zoning`: UNIVERSE_LAND, UNIVERSE_AFTER.)
    if std::env::var_os("UNIVERSE_LAND").is_some() {
        apply(app, "zoning");
        app.economy_panel = None;
    }
    let u = app.engine.universe();
    // (Our own hull, standing on its own feet.)
    let class = u.vessels[universe_sim::PLAYER].ship.class;
    // (A pad no ship stands on.)
    let body = sys.spaceports[port].body;
    let r = sys.bodies[body].rail.radius;
    let taken = |k: usize| {
        let d = universe_sim::world::spaceport::pad_direction(sys, port, k);
        u.vessels.crafts().iter().any(|c| matches!(c.ship.state, ShipState::Landed { body: b, local_position, .. } if b == body && local_position.normalize().angle_between(d) * r < 60.0))
    };
    let pad = (0..universe_sim::world::spaceport::PADS).find(|&k| !taken(k)).unwrap_or(0);
    let mut ship = u.world.ship_on(home, universe_sim::world::Facility::Spaceport(port), pad);
    ship.class = class;
    ship.refresh();
    u.world.resettle(home, &mut ship);
    u.vessels[universe_sim::PLAYER].ship = ship;
    app.mode = Mode::Observer;
    app.observer.focus = Focus::Ship;
    let env = |k: &str, d: f64| crate::devenv::num(k).unwrap_or(d);
    app.observer.distance = env("UNIVERSE_DIST", 2500.0);
    app.observer.yaw = env("UNIVERSE_YAW", 0.6);
    app.observer.pitch = env("UNIVERSE_PITCH", 0.45);
    app.observer.studio = std::env::var_os("UNIVERSE_STUDIO").is_some();
}

fn s_system(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let outer = c.outer;
    observe(app, home, 0, outer * 2.2, 0.6);
}

fn s_inner(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let outer = c.outer;
    observe(app, home, 0, outer * 0.25, 0.45);
}

fn s_planet(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    let planet = c.planet;
    observe(app, home, planet, sys.bodies[planet].rail.radius * std::env::var("UNIVERSE_DIST").ok().and_then(|d| d.parse().ok()).unwrap_or(6.0), 0.3);
    if let Some(y) = std::env::var("UNIVERSE_YAW").ok().and_then(|d| d.parse().ok()) {
        app.observer.yaw = y;
    }
}

fn s_giant(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    let giant = sys.bodies.iter().position(|b| b.rings.is_some()).or_else(|| sys.bodies.iter().position(|b| b.kind == BodyKind::GasGiant)).unwrap_or(0);
    observe(app, home, giant, sys.bodies[giant].rail.radius * 7.0, 0.35);
}

fn s_rings(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    // Nearest ringed planet in the neighbourhood.
    for n in std::iter::once(home).chain(app.engine.universe().world.galaxy.nearest(home, 60)) {
        let s = app.engine.universe().system(n);
        if let Some(i) = s.bodies.iter().position(|b| b.rings.is_some()) {
            app.mode = Mode::Observer;
            app.observer.focus = Focus::Body { system: n, body: i };
            app.observer.distance = s.bodies[i].rail.radius * 6.0;
            app.observer.pitch = 0.3;
            break;
        }
    }
}

fn s_galaxy(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    observe(app, home, 0, 2.5e20, 1.0);
}

fn s_neighbours(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    observe(app, home, 0, 6.0e17, 0.5);
}

fn s_cockpit(app: &mut App, _name: &str, _c: &Ctx) {
    app.mode = Mode::Pilot;
    app.chase_cam = false;
}

fn s_watch(app: &mut App, name: &str, _c: &Ctx) {
    // Docked (padwatch: on the spaceport's pad), watching our ship (TAB's
    // other view); UNIVERSE_DIST, UNIVERSE_PITCH, UNIVERSE_YAW to frame it.
    apply(app, if name == "padwatch" { "touchdown" } else { "docked" });
    app.mode = Mode::Observer;
    app.observer.focus = crate::observer::Focus::Ship;
    let env = crate::devenv::num;
    app.observer.distance = env("UNIVERSE_DIST").unwrap_or(160.0);
    if let Some(p) = env("UNIVERSE_PITCH") {
        app.observer.pitch = p;
    }
    if let Some(y) = env("UNIVERSE_YAW") {
        app.observer.yaw = y;
    }
}

fn s_standards(app: &mut App, _name: &str, _c: &Ctx) {
    // Docked, the standards registry open on UNIVERSE_STANDARD (a key; the first otherwise).
    apply(app, "docked");
    let mut view = crate::standards::StandardsView::new();
    view.focus(&std::env::var("UNIVERSE_STANDARD").unwrap_or_else(|_| "SFO/3.1/001".into()));
    app.standards = Some(view);
}

fn s_sunlit(app: &mut App, _name: &str, c: &Ctx) {
    let positions = &c.positions;
    // Our ship in open space, watched, turned so the sun shines on it from over the
    // eye's shoulder (like a model lit from the front): for comparing looks.
    // UNIVERSE_DIST, UNIVERSE_PITCH, UNIVERSE_YAW frame it.
    app.mode = Mode::Observer;
    app.observer.focus = crate::observer::Focus::Ship;
    let env = crate::devenv::num;
    app.observer.distance = env("UNIVERSE_DIST").unwrap_or(110.0);
    app.observer.pitch = env("UNIVERSE_PITCH").unwrap_or(0.25);
    app.observer.yaw = env("UNIVERSE_YAW").unwrap_or(2.3);
    let (p, y) = (app.observer.pitch, app.observer.yaw);
    let eye = DVec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos());
    let side = eye.cross(DVec3::Y).normalize_or(DVec3::X);
    let local_sun = (eye + DVec3::Y * 0.6 + side * 0.4).normalize();
    let u = app.engine.universe();
    let sun = (positions[0] - u.vessels[universe_sim::PLAYER].ship.position).normalize();
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(local_sun, sun);
}

fn s_showcase(app: &mut App, _name: &str, c: &Ctx) {
    let positions = &c.positions;
    // Looking away from the sun, a little to one side and down: the sun
    // over the eye's shoulder (for `UNIVERSE_MODEL`).
    app.mode = Mode::Pilot;
    app.chase_cam = false;
    let u = app.engine.universe();
    let away = (u.vessels[universe_sim::PLAYER].ship.position - positions[0]).normalize();
    let side = away.any_orthonormal_vector();
    let look = (away + side * 0.6 - away.cross(side) * 0.35).normalize();
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(look, side);
}

fn s_hyper(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let positions = &c.positions;
    let planet = c.planet;
    // Aim at a nearby star on the open-sky side of the planet, and jump.
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    let up = (u.vessels[universe_sim::PLAYER].ship.position - positions[planet]).normalize();
    let dir_to = |n: usize| (u.world.galaxy.offset(home, n) - u.vessels[universe_sim::PLAYER].ship.position).normalize();
    let candidates = u.world.galaxy.nearest(home, 12);
    let target = candidates.iter().copied().find(|&n| dir_to(n).dot(up) > 0.3).unwrap_or(candidates[0]);
    let dir = dir_to(target);
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, dir);
    u.toggle_hyperdrive();
    u.command(&ShipCommands { throttle: 1.0, ..u.vessels[universe_sim::PLAYER].ship.holding() });
    for _ in 0..4000 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        if !u.vessels[universe_sim::PLAYER].ship.hyperdrive {
            break;
        }
    }
}

fn s_landed(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let planet = c.planet;
    // Drop the ship just above the station's planet, matching its surface motion.
    app.mode = Mode::Pilot;
    let b = &sys.bodies[planet];
    let normal = (app.engine.universe().vessels[universe_sim::PLAYER].ship.position - positions[planet]).normalize();
    let offset = normal * (b.rail.radius + SHIP_RADIUS + 3.0);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = positions[planet] + offset;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(offset);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, normal.any_orthonormal_vector());
    for _ in 0..120 {
        app.engine.universe().step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
}

fn s_approach(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let station = c.station;
    // Cleared to dock, flying manually. "approach": 2.5 km above the deck, a little off, facing in.
    // "lost": facing away from the station, to show the off-screen marker.
    app.mode = Mode::Pilot;
    let f = StationFrame::new(sys, station, t, positions);
    let side = f.rotation * DVec3::X;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = f.pad(universe_sim::world::spaceport::CENTER_PAD) + f.up() * 2500.0 + side * 180.0;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = f.velocity - f.up() * 30.0;
    let facing = if name == "lost" { f.up() } else { -f.up() };
    let roll = universe_engine::glam::DQuat::from_axis_angle(facing, 0.35);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = roll * universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, facing);
    if name != "lost" {
        app.engine.universe().request_clearance();
    }
    // Cleared, and keeping station on the station meanwhile.
    if name == "approachkeep" {
        let u = app.engine.universe();
        u.set_nav_target(Some(universe_sim::NavTarget::Station(station)));
        u.follow(universe_sim::FollowKind::KeepAt, None);
        for _ in 0..60 * 3 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
    }
}

fn s_passengers(app: &mut App, _name: &str, _c: &Ctx) {
    // Docked at the home station, hungry, a thousand waiting to leave;
    // a passenger cabin fitted; the passengers panel open.
    apply(app, "docked");
    let u = app.engine.universe();
    let station = u.ship_system().station().unwrap();
    let home = u.vessels[universe_sim::PLAYER].system;
    if let Some(p) = u.services.markets.economy.place_mut(home, universe_sim::world::Facility::Station(station)) {
        p.fed = 0.6;
        p.waiting = 1.0;
    }
    let _ = u.refit("cargo", universe_sim::world::content::content().handle("equipment.cargo.cabin.s3"));
    app.engine.refresh();
    app.v = app.engine.view();
    app.passengers = Some(0);
}

fn s_cleared(app: &mut App, _name: &str, _c: &Ctx) {
    // The spawn point, with docking clearance granted.
    app.mode = Mode::Pilot;
    app.engine.universe().request_clearance();
}

fn s_offcourse(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let station = c.station;
    // Cleared, but 6 km off to the side and drifting sideways at 40 m/s.
    app.mode = Mode::Pilot;
    let f = StationFrame::new(sys, station, t, positions);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = f.center + f.up() * 2000.0 + f.rotation * DVec3::X * 6000.0;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = f.velocity + f.rotation * DVec3::Z * 40.0;
    let look = (f.center - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, look);
    app.engine.universe().request_clearance();
}

fn s_galaxymap(app: &mut App, name: &str, _c: &Ctx) {
    // The galaxy map, having been to the gate network's systems.
    app.mode = Mode::Pilot;
    let links = app.engine.universe().world.gate_links.clone();
    for (a, b) in links {
        app.explored.insert(a);
        app.explored.insert(b);
    }
    let mut map = crate::galaxymap::GalaxyMap::open(app, universe_engine::glam::Vec2::new(960.0, 540.0));
    // (UNIVERSE_ZOOM: closer by that much, or farther under 1.)
    if let Some(k) = std::env::var("UNIVERSE_ZOOM").ok().and_then(|z| z.parse().ok()) {
        map.zoom(k);
    } else if name == "galaxyzoom" {
        map.zoom(4.0);
    }
    app.galaxy_map = Some(map);
}

fn s_help(app: &mut App, _name: &str, _c: &Ctx) {
    app.mode = Mode::Pilot;
    app.show_help = true;
}

fn s_zoning(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    // The economy panel on Port Trethi's ground (UNIVERSE_PICK: f<k> a facility, p<n> a parcel).
    app.mode = Mode::Pilot;
    let mut panel: crate::economy::EconomyPanel = Default::default();
    let trethi = app.v.economy.iter().position(|p| matches!(p.facility, universe_sim::world::Facility::Spaceport(i) if sys.spaceports[i].name == "Port Trethi"));
    // (UNIVERSE_LAND: what's done first, as the player: "buy4", "claim:w,s,e,n", "build4:Trethi Power Station", comma... separated by ';'.)
    if let (Some(port), Ok(ops)) = (sys.spaceports.iter().position(|p| p.name == "Port Trethi"), std::env::var("UNIVERSE_LAND")) {
        let u = app.engine.universe();
        for op in ops.split(';') {
            let r = if let Some(n) = op.strip_prefix("buy") {
                u.buy_parcel(home, port, n.parse().unwrap_or(0))
            } else if let Some(c) = op.strip_prefix("claim:") {
                let v: Vec<f64> = c.split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 4 { u.claim_land(home, port, vec![(v[0], v[1]), (v[2], v[1]), (v[2], v[3]), (v[0], v[3])]) } else { Err("claim:w,s,e,n".into()) }
            } else if let Some((n, what)) = op.strip_prefix("build").and_then(|r| r.split_once(':')) {
                u.build_facility(home, port, n.parse().unwrap_or(0), what.to_string())
            } else {
                Err(format!("unknown '{op}'"))
            };
            log::info!("land {op}: {r:?}");
        }
        if let Ok(s) = std::env::var("UNIVERSE_AFTER") {
            let until = app.engine.universe().world.time + s.parse::<f64>().unwrap_or(0.0);
            while app.engine.universe().world.time < until {
                app.engine.universe().step_world(1.0, 1.0, &Controls::default());
            }
        }
        app.engine.refresh();
        app.v = app.engine.view();
    }
    if let Some(k) = trethi {
        panel.selected = k;
        panel.zoning = crate::zoning::Zoning::open(app, &app.v.economy[k]);
        if let (Some(z), Ok(pick)) = (panel.zoning.as_mut(), std::env::var("UNIVERSE_PICK")) {
            z.pick(&pick);
        }
    }
    app.economy_panel = Some(panel);
}

fn s_economy(app: &mut App, name: &str, _c: &Ctx) {
    // (Heard: the world run a while first, reports put out and on their way.)
    app.mode = Mode::Pilot;
    if name == "economyheard" {
        apply(app, "docked");
        while app.engine.universe().world.time < 400.0 {
            app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
        }
        app.engine.refresh();
        app.v = app.engine.view();
    }
    // From the registry's day 0, the economy run (alone: no ships flown) for
    // UNIVERSE_DAYS days (22: an MC-07 off Trethi Yard's dock), its companies seeing
    // to their works each step.
    let days: f64 = std::env::var("UNIVERSE_DAYS").ok().and_then(|d| d.parse().ok()).unwrap_or(22.0);
    let u = app.engine.universe();
    let step = universe_sim::services::economy::step();
    let end = u.world.time + days * 86_400.0;
    let mut t = u.services.markets.economy.stepped_to;
    while t + step <= end {
        t += step;
        u.services.markets.step(t, &mut u.services.land, &mut u.services.ledger, u.tick);
        universe_sim::company::run(u);
    }
    app.engine.refresh();
    app.v = app.engine.view();
    let at = app.v.economy.iter().position(|p| p.name == std::env::var("UNIVERSE_PLACE").as_deref().unwrap_or("Port Trethi")).unwrap_or(0);
    let site = app.v.economy.get(at).map(|p| p.site);
    let mut panel: crate::economy::EconomyPanel = Default::default();
    panel.selected = at;
    if name == "economymodules" {
        // (Its modules open, the yard's welding bay set to the MC-07's nose cap.)
        let u = app.engine.universe();
        let yard = u.services.markets.economy.works.iter().position(|w| w.name == "Trethi Yard");
        let cap = universe_sim::world::goods::item("part.mc07-01");
        if let (Some(k), Some(cap)) = (yard, cap) {
            let bay = u.services.markets.economy.works[k].setups.iter().position(|s| s.module.identity.key == "module.welding-bay").unwrap_or(0);
            // (As its owner sets it.)
            let owner = u.services.markets.economy.owner(&u.services.land, k);
            if let (Some(r), Some(owner)) = (universe_sim::world::recipes::of("module.welding-bay").iter().position(|r| r.makes == cap), owner) {
                let _ = u.services.markets.economy.set_up(&u.services.land, k, bay, Some(r), owner);
            }
            let before = u.services.markets.economy.works.iter().enumerate().filter(|(j, w)| *j < k && Some(w.site) == site).map(|(_, w)| w.setups.len()).sum::<usize>();
            panel.module = Some(before + bay);
        }
        app.engine.refresh();
        app.v = app.engine.view();
    }
    app.economy_panel = Some(panel);
}

fn s_navmap(app: &mut App, name: &str, _c: &Ctx) {
    app.mode = Mode::Pilot;
    let mut map = crate::navmap::NavMap::open(app);
    map.network = name == "netmap";
    if name == "navzoom" {
        map.set_view(5.0, universe_engine::glam::Vec2::new(150.0, -500.0));
    }
    app.nav_map = Some(map);
}

fn s_landing(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let planet = c.planet;
    // Target the spaceport on the station's planet and get landing clearance.
    app.mode = Mode::Pilot;
    let port = sys.spaceports.iter().position(|p| p.body == planet).expect("spaceport on the station's planet");
    let pad = PadFrame::new(sys, port, t, positions);
    if name == "padview" {
        // 12 km above and 6 km beside the pad, drifting, belly down.
        app.engine.universe().vessels[universe_sim::PLAYER].ship.position = pad.pad + pad.up * 12_000.0 + pad.up.any_orthonormal_vector() * 6000.0;
        app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = pad.frame_velocity(app.engine.universe().vessels[universe_sim::PLAYER].ship.position) - pad.up * 40.0;
        let fwd = pad.up.any_orthonormal_vector().cross(pad.up);
        app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::upright(pad.up, fwd);
    }
    app.engine.universe().set_nav_target(Some(NavTarget::Spaceport(port)));
    app.engine.universe().request_clearance();
    if name == "autoland" || name == "touchdown" {
        app.engine.universe().toggle_autopilot();
        for _ in 0..60 * 60 * 30 {
            app.engine.universe().step_world(1.0 / 60.0, 20.0, &Controls::default());
            let low = matches!(app.engine.universe().approach(), Some(universe_sim::Approach::Land { ref status, .. })
                if status.phase == Phase::Descent && status.altitude < std::env::var("UNIVERSE_ALT").ok().and_then(|a| a.parse().ok()).unwrap_or(1500.0));
            if (name == "autoland" && low) || !app.engine.universe().vessels[universe_sim::PLAYER].ship.is_flying() {
                break;
            }
        }
    }
}

fn s_holding(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let planet = c.planet;
    // The port's nine pads taken (long stops), and we're queued: the
    // holding circle 12 km over the port, two minutes in (traffic control
    // knows the pads are taken: it hands them out, not the ships on them).
    app.mode = Mode::Pilot;
    let port = sys.spaceports.iter().position(|p| p.body == planet).expect("spaceport on the station's planet");
    let u = app.engine.universe();
    for k in 0..universe_sim::world::spaceport::PADS {
        u.vessels[universe_sim::craft_id(k)].ship = u.world.ship_on(home, universe_sim::world::Facility::Spaceport(port), k);
        u.vessels[universe_sim::craft_id(k)].system = home;
        let now = u.world.time;
        u.services.atc.request_pad(home, universe_sim::world::Facility::Spaceport(port), universe_sim::craft_id(k), now);
    }
    {
        let mut pilots = u.pilots();
        for p in pilots.iter_mut().take(universe_sim::world::spaceport::PADS) {
            p.avionics.route.stops = vec![universe_sim::Stop { system: home, target: NavTarget::Spaceport(port) }];
            p.avionics.route.next = 0;
            p.avionics.route.active = true;
            p.avionics.route.dwell_until = Some(1.0e12);
            p.avionics.pirate = false;
        }
    }
    let pad = PadFrame::new(sys, port, t, positions);
    u.vessels[universe_sim::PLAYER].ship.position = pad.pad + pad.up * 20_000.0;
    u.vessels[universe_sim::PLAYER].ship.velocity = pad.frame_velocity(u.vessels[universe_sim::PLAYER].ship.position);
    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    u.request_clearance();
    u.toggle_autopilot();
    for _ in 0..60 * 120 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
}

fn s_asteroid(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    // By the first field's remnant (sun behind), or among its swarm,
    // moving with it, facing it; it's the nav target.
    app.mode = Mode::Pilot;
    let f = &sys.fields[0];
    let rock = f.body;
    let (at, v) = (positions[rock], sys.velocity(rock, t));
    let sun = -at.normalize();
    let side = sun.cross(DVec3::Y).normalize();
    let off = if name == "swarm" { (sun * 0.6 + side).normalize() * (sys.bodies[rock].rail.radius + 15_000.0) } else { (sun + side * 0.3).normalize() * (sys.bodies[rock].rail.radius + 3000.0) };
    let u = app.engine.universe();
    u.vessels[universe_sim::PLAYER].ship.position = at + off;
    u.vessels[universe_sim::PLAYER].ship.velocity = v;
    u.vessels[universe_sim::PLAYER].ship.angular_velocity = DVec3::ZERO;
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(-off.normalize(), DVec3::Y);
    u.set_nav_target(Some(NavTarget::Asteroid(rock)));
    if name == "navlock" {
        app.picker.hold_for_show();
    }
    if name.starts_with("orbitrock") {
        // Orbiting the remnant, a minute on (1k: as close as it allows, from 1 km).
        let u = app.engine.universe();
        u.follow(universe_sim::FollowKind::Orbit, (name == "orbitrock1k").then_some(1_000.0));
        for _ in 0..60 * 60 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
    }
}

fn s_worlds(app: &mut App, _name: &str, _c: &Ctx) {
    // The planet studio, gone to UNIVERSE_WORLD (a world id: TRD1 Harvest by default), in
    // UNIVERSE_LOOK (colour, geology, energy).
    let mut studio = crate::planet_studio::PlanetStudio::open();
    let want = std::env::var("UNIVERSE_WORLD").unwrap_or_else(|_| "TRD1".into());
    studio.selected = studio.list.iter().position(|r| r.world_id == want).unwrap_or(0);
    app.world_look = match std::env::var("UNIVERSE_LOOK").as_deref() {
        Ok("geology") => crate::planet_studio::Look::Geology,
        Ok("energy") => crate::planet_studio::Look::Energy,
        _ => crate::planet_studio::Look::Colour,
    };
    let r = studio.list.get(studio.selected).cloned();
    app.planet_studio = Some(studio);
    if let Some(r) = r {
        let _ = crate::planet_studio::go_to(app, &r);
    }
    // (UNIVERSE_HISTORY_FRAME: its history's frame n, counted from 0, in place of today.)
    if let Some(n) = crate::devenv::num("UNIVERSE_HISTORY_FRAME").map(|n| n as usize)
        && let Some((key, Some(history))) = app.planet_studio.as_ref().and_then(|s| s.shown.clone())
        && n < history.frames.len()
    {
        app.world_frame = Some(crate::planet_studio::WorldFrame { key, history, frame: n });
    }
    app.observer.yaw = 2.2;
}

fn s_rig(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    // Hadley Orbital Works, a rig with no model: its box, from a little way off.
    if let Some(rig) = sys.bodies.iter().position(|b| b.kind == BodyKind::Rig) {
        observe(app, home, rig, 1400.0, 0.35);
    }
}

fn s_beltpick(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let positions = &c.positions;
    // In the home system's main belt, as it is (see `belts`): "beltpick" surveyed with the
    // list held up; "beltrock" 300 m off the nearest rock the survey found, locked;
    // "beltmining" anchored to it and digging for half a minute.
    app.mode = Mode::Pilot;
    let t = app.engine.universe().world.time;
    let main = sys.belts.iter().find(|b| b.kind == universe_sim::world::belts::BeltKind::Main).expect("a main belt");
    let from = DVec3::new((main.inner + main.outer) / 2.0, 0.0, 0.0);
    let star = positions[0];
    let found = universe_sim::world::belts::survey(sys, from, t, universe_sim::world::belts::Survey::radar());
    let f = found.iter().find(|f| f.diameter > 30.0).expect("a belt rock in sight").clone();
    let bodies = sys.field_bodies(f.field);
    let mut pos = Vec::new();
    universe_sim::world::physics::positions(&bodies[..], t, &mut pos);
    let i = f.body;
    let b = &bodies[i];
    let sun = (star - pos[i]).normalize();
    let up = (sun + sun.any_orthonormal_vector() * 0.8).normalize();
    let gap = if name == "beltmining" { 15.0 } else { 300.0 };
    let near = pos[i] + up * (b.surface_radius(b.rotation(t).inverse() * up) + SHIP_RADIUS + gap);
    let u = app.engine.universe();
    if name == "beltpick" {
        u.vessels[universe_sim::PLAYER].ship.position = star + from;
        u.vessels[universe_sim::PLAYER].ship.velocity = universe_sim::world::physics::velocity(&bodies[..], 0, t) + DVec3::new(0.0, 0.0, -(universe_sim::units::G * sys.bodies[0].mass / from.length()).sqrt());
    } else {
        u.vessels[universe_sim::PLAYER].ship.position = near;
        u.vessels[universe_sim::PLAYER].ship.velocity = universe_sim::world::physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(near - pos[i]);
        u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(up.any_orthonormal_vector(), -up);
    }
    u.vessels[universe_sim::PLAYER].ship.angular_velocity = DVec3::ZERO;
    app.mining.on = true;
    crate::mining::prospect_for_show(app, 5.0);
    if name == "beltpick" {
        app.picker.hold_for_show();
    } else {
        app.engine.universe().cockpit().lock_rock(Some((f.field, i)));
    }
    if name == "beltmining" {
        let u = app.engine.universe();
        u.command(&ShipCommands { anchor: Some(true), ..u.vessels[universe_sim::PLAYER].ship.holding() });
        for _ in 0..10 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
        u.command(&ShipCommands { excavate: Some(true), ..u.vessels[universe_sim::PLAYER].ship.holding() });
        for _ in 0..60 * 30 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
    }
}

fn s_mining(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    // By a rubble fragment of a home field, drifting with its
    // surface: "prospect" 300 m off it; "mining" anchored 15 m off
    // and digging for half a minute.
    app.mode = Mode::Pilot;
    let t = app.engine.universe().world.time;
    let (bodies, i) = (0..sys.fields.len())
        .find_map(|f| {
            let bodies = sys.field_bodies(f);
            let i = (sys.bodies.len()..bodies.len()).find(|&i| bodies[i].rail.radius > 20.0 && bodies[i].rock.as_ref().is_some_and(|r| universe_sim::world::mining::Rig::common().dig_rate(r) >= 10.0))?;
            Some((bodies, i))
        })
        .expect("a rubble fragment");
    let mut pos = Vec::new();
    universe_sim::world::physics::positions(&bodies[..], t, &mut pos);
    let b = &bodies[i];
    let sun = -pos[i].normalize();
    let up = (sun + sun.any_orthonormal_vector() * 0.8).normalize();
    let gap = match name {
        "mining" | "cargo" => 15.0,
        "closing" | "closing10" | "closingwatch" => 800.0,
        _ => 300.0,
    };
    let at = pos[i] + up * (b.surface_radius(b.rotation(t).inverse() * up) + universe_sim::world::ship::SHIP_RADIUS + gap);
    let u = app.engine.universe();
    u.vessels[universe_sim::PLAYER].ship.position = at;
    u.vessels[universe_sim::PLAYER].ship.velocity = universe_sim::world::physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(at - pos[i]);
    u.vessels[universe_sim::PLAYER].ship.angular_velocity = DVec3::ZERO;
    // Spine to the rock, nose along its surface (as the approach leaves it).
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(up.any_orthonormal_vector(), -up);
    if name == "cargo" {
        app.show_cargo = true;
    }
    if matches!(name, "minemode" | "pulse" | "picklist") {
        // Mining mode, a little farther off: prospected (or mid-pulse), a rock locked, or T held.
        u.vessels[universe_sim::PLAYER].ship.position = at + up * 2000.0;
        app.mining.on = true;
        let age = if name == "pulse" { -0.45 } else { 5.0 };
        crate::mining::prospect_for_show(app, age);
        if name == "picklist" {
            app.picker.hold_for_show();
        } else if let Some(f) = (0..sys.fields.len()).find(|&f| sys.field_rocks(f).any(|j| j == i) && sys.field_bodies(f).len() == bodies.len()) {
            app.engine.universe().cockpit().lock_rock(Some((f, i)));
        }
    }
    let u = app.engine.universe();
    if matches!(name, "closing" | "closing10" | "closingwatch") {
        // Closing on it, ninety (or ten) seconds on.
        let f = (0..sys.fields.len()).find(|&f| sys.field_bodies(f).len() == bodies.len() && sys.field_rocks(f).any(|j| j == i)).unwrap();
        u.close_on(f, i);
        for _ in 0..60 * if name == "closing" { 90 } else { 10 } {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
        if name == "closingwatch" {
            app.mode = Mode::Observer;
            app.observer.focus = Focus::Ship;
            app.observer.distance = 400.0;
            app.observer.pitch = 0.4;
        }
    }
    if matches!(name, "mining" | "cargo") {
        u.command(&ShipCommands { anchor: Some(true), ..u.vessels[universe_sim::PLAYER].ship.holding() });
        for _ in 0..10 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
        u.command(&ShipCommands { excavate: Some(true), ..u.vessels[universe_sim::PLAYER].ship.holding() });
        for _ in 0..60 * if name == "cargo" { 105 } else { 30 } {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
    }
}

fn s_gate(app: &mut App, name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    // A gate out of the home system: cleared for transit, 8 km out, off to one side.
    app.mode = Mode::Pilot;
    let (dest, _) = app.engine.universe().gate_links_of(home)[0].clone();
    let g = sys.gate_to(dest).unwrap();
    let f = GateFrame::new(sys, g, t, positions);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = f.center + f.axis() * 8000.0 + (f.rotation * DVec3::X) * 2500.0;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = f.velocity;
    let look = (f.center - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(look, f.rotation * DVec3::Z);
    app.engine.universe().set_nav_target(Some(NavTarget::Gate(g)));
    if name.starts_with("gateorbit") || name == "orbitpick" || name == "gateorbitjets" {
        // Arrived at the gate, and orbiting it (no clearance).
        let u = app.engine.universe();
        u.follow(universe_sim::FollowKind::Orbit, None);
        for _ in 0..60 * if name == "gateorbit" { 15 } else { 60 } {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
        if name == "orbitpick" {
            app.orbit_pick.hold_for_show();
        }
        if name == "gateorbitwatch" || name == "gateorbitjets" {
            app.mode = Mode::Observer;
            app.observer.focus = Focus::Ship;
            app.observer.distance = if name == "gateorbitjets" { 150.0 } else { 30_000.0 };
            app.observer.pitch = if name == "gateorbitjets" { -0.35 } else { 1.4 };
        }
        return;
    }
    app.engine.universe().request_clearance();
    if name != "gate" {
        app.engine.universe().toggle_autopilot();
        for _ in 0..60 * 60 * 10 {
            app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
            let running = matches!(app.engine.universe().approach(), Some(universe_sim::Approach::Transit { ref status, .. })
                if status.phase == Phase::Final && status.distance < 1500.0);
            let stop = match name {
                "gateauto" => running,
                // (So far into the tube: UNIVERSE_SINCE s, 20 by default.)
                "transit" => matches!(app.engine.universe().vessels[universe_sim::PLAYER].ship.state, ShipState::Transit { remaining, duration, .. } if duration - remaining > std::env::var("UNIVERSE_SINCE").ok().and_then(|v| v.parse().ok()).unwrap_or(20.0)),
                _ => app.engine.universe().vessels[universe_sim::PLAYER].system != home && app.engine.universe().vessels[universe_sim::PLAYER].ship.is_flying(),
            };
            if stop {
                break;
            }
        }
    }
}

fn s_sam(app: &mut App, name: &str, c: &Ctx) {
    let home = c.home;
    let positions = &c.positions;
    // Aggressed, 9 km out from a defended station or port, looking at
    // its turrets (far: 40 km out, the missiles on their way up).
    app.mode = Mode::Pilot;
    if let Some((t, at, v)) = app.engine.universe().world.turret_motions(home).into_iter().find(|(t, _, _)| !matches!(t.facility, universe_sim::world::Facility::Gate(_))) {
        let out = (at - positions[t.body]).normalize();
        app.engine.universe().vessels[universe_sim::PLAYER].ship.position = at + out * if name == "samfar" { 40_000.0 } else { 9_000.0 };
        app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = v;
        app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(-out, out.any_orthonormal_vector());
        let u = app.engine.universe();
        let now = u.world.time;
        u.services.law.declare(universe_sim::PLAYER, now + 600.0, now, universe_sim::protocol::Cause::Rules);
        if name == "samfar" {
            for _ in 0..60 * 26 {
                u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            }
        }
    }
}

fn s_thrusters(app: &mut App, _name: &str, _c: &Ctx) {
    // The thrusters panel, the main drive at full and a turn under way.
    app.mode = Mode::Pilot;
    app.show_thrusters = true;
    let u = app.engine.universe();
    u.command(&ShipCommands { throttle: 1.0, rcs: DVec3::new(0.3, 0.0, 0.0), ..u.vessels[universe_sim::PLAYER].ship.holding() });
}

fn s_burn(app: &mut App, name: &str, _c: &Ctx) {
    // The main drive at full, seen from the chase view ("burnside":
    // a Drover beside us at full, seen side on).
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    u.command(&ShipCommands { throttle: 1.0, ..u.vessels[universe_sim::PLAYER].ship.holding() });
    if name == "burnside" {
        let (at, v, o) = (u.vessels[universe_sim::PLAYER].ship.position, u.vessels[universe_sim::PLAYER].ship.velocity, u.vessels[universe_sim::PLAYER].ship.orientation);
        let c = &mut u.vessels[universe_sim::craft_id(0)];
        c.ship.class = universe_sim::world::ship::starting_hull();
        c.ship.state = ShipState::Flying;
        c.ship.position = at + o * DVec3::new(-70.0, -10.0, -120.0);
        c.ship.velocity = v;
        c.ship.orientation = o * universe_engine::glam::DQuat::from_rotation_y(-1.3);
        c.ship.throttle = 1.0;
        u.pilots()[0].avionics.route.active = false;
    }
}

fn s_gateflash(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    // 9 km off the home system's first gate, facing it: one ship just
    // gone through it (0.4 s ago), another about to come out (in 0.5 s).
    app.mode = Mode::Pilot;
    let g = sys.bodies.iter().position(|b| b.kind == BodyKind::Gate).expect("a gate");
    let dest = sys.bodies[g].link.expect("linked");
    let f = GateFrame::new(sys, g, t, positions);
    let u = app.engine.universe();
    u.vessels[universe_sim::PLAYER].ship.state = ShipState::Flying;
    u.vessels[universe_sim::PLAYER].ship.position = f.center + f.axis() * 9_000.0 + f.rotation * DVec3::X * 2_500.0;
    u.vessels[universe_sim::PLAYER].ship.velocity = f.velocity;
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(f.center - u.vessels[universe_sim::PLAYER].ship.position, f.rotation * DVec3::Z);
    // (UNIVERSE_SIDE: side on instead, 25 km off its axis, looking at the way out of it.)
    if std::env::var("UNIVERSE_SIDE").is_ok() {
        let look = f.center + f.axis() * 7_000.0;
        u.vessels[universe_sim::PLAYER].ship.position = look + f.rotation * DVec3::X * 25_000.0;
        u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(look - u.vessels[universe_sim::PLAYER].ship.position, f.rotation * DVec3::Z);
    }
    const TRANSIT_TIME: f64 = 10.0;
    // (How long since the first went in: UNIVERSE_SINCE s, 0.4 by default.)
    let since = std::env::var("UNIVERSE_SINCE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.4);
    for (k, (from, to, remaining, at)) in [(home, dest, TRANSIT_TIME - since, DVec3::new(400.0, 0.0, 250.0)), (dest, home, 0.5, DVec3::new(-500.0, 0.0, -300.0))].into_iter().enumerate() {
        let c = &mut u.vessels[universe_sim::craft_id(k)];
        c.system = from;
        c.ship.state = ShipState::Transit { to, from, remaining, duration: TRANSIT_TIME, local_velocity: DVec3::Y * 80.0, local_offset: at, local_orientation: universe_engine::glam::DQuat::IDENTITY };
        u.pilots()[k].avionics.route.active = false;
    }
}

fn s_deckshadowside(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    // The home station's deck from above, at a moment in its turn
    // when the sun is round to the side of the structure (its shadow
    // across part of the deck) or in front of it (the ships' shadows
    // long across the deck).
    let station = sys.station().expect("home station");
    let b = &sys.bodies[station];
    let star = sys.bodies.iter().position(|b| b.kind == universe_sim::BodyKind::Star).expect("a star");
    let want = if name == "deckshadowside" { 1.2f64 } else { 0.4 };
    // ("deckshadowface": low over the deck, looking at the structure's sunlit face.)
    let (eye, look) = if name == "deckshadowface" { (DVec3::new(120.0, -50.0, 380.0), DVec3::new(-60.0, -100.0, -250.0)) } else { (DVec3::new(-500.0, 420.0, 700.0), DVec3::new(0.0, 0.0, 100.0)) };
    let t0 = app.engine.universe().world.time;
    let mut pos = Vec::new();
    let (mut best, mut at_t) = (f64::INFINITY, t0);
    for k in 0..720 {
        let t = t0 + b.rail.day * k as f64 / 720.0;
        sys.positions(t, &mut pos);
        let sun = b.rotation(t).inverse() * (pos[star] - pos[station]).normalize();
        let miss = (sun.x.atan2(sun.z) - want).abs();
        if miss < best {
            (best, at_t) = (miss, t);
        }
    }
    app.engine.universe().world.time = at_t;
    sys.positions(at_t, &mut pos);
    log::info!("scenario {name}: sun in the station's frame {:.2}", b.rotation(at_t).inverse() * (pos[star] - pos[station]).normalize());
    let f = StationFrame::new(sys, station, at_t, &pos);
    let at = f.world(eye);
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    u.vessels[universe_sim::PLAYER].ship.state = ShipState::Flying;
    u.vessels[universe_sim::PLAYER].ship.position = at;
    u.vessels[universe_sim::PLAYER].ship.velocity = f.velocity_at(at);
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(f.world(look) - at, f.up());
    app.chase_cam = false;
}

fn s_sunedge0(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let positions = &c.positions;
    // On the deck, looking at the sun just over the structure's top
    // edge (500 m off): the line to it 4 m under the edge (hidden),
    // on it (half the disc), 4 m over (clear).
    app.mode = Mode::Pilot;
    let station = sys.station().expect("home station");
    let star = sys.bodies.iter().position(|b| b.kind == universe_sim::BodyKind::Star).expect("a star");
    let t = app.engine.universe().world.time;
    let f = StationFrame::new(sys, station, t, positions);
    let sun = f.rotation.inverse() * (positions[star] - f.center).normalize();
    let off = match name { "sunedge0" => -4.0, "sunedge1" => 0.0, _ => 4.0 };
    let edge = DVec3::new(0.0, 150.0 + off, universe_sim::world::station::DECK_FROM);
    let at = f.world(edge - sun * 500.0);
    let u = app.engine.universe();
    u.vessels[universe_sim::PLAYER].ship.state = ShipState::Flying;
    u.vessels[universe_sim::PLAYER].ship.position = at;
    u.vessels[universe_sim::PLAYER].ship.velocity = f.velocity_at(at);
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(positions[star] - at, f.up());
    app.chase_cam = false;
}

fn s_alarms(app: &mut App, _name: &str, _c: &Ctx) {
    // In flight by the home station, the hull critical and the tank nearly dry.
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    u.start_in_flight();
    u.vessels[universe_sim::PLAYER].ship.hull = 0.2;
    u.vessels[universe_sim::PLAYER].ship.fuel = 1_000.0;
}

fn s_showship(app: &mut App, _name: &str, _c: &Ctx) {
    // Our ship as the hull UNIVERSE_HULL names (default the hauler),
    // in sunlight by the home station, seen from the side and above.
    let key = std::env::var("UNIVERSE_HULL").unwrap_or_else(|_| "hull.hauler".into());
    let u = app.engine.universe();
    u.start_in_flight();
    if let Some(h) = universe_sim::world::content::content().handle(&key) {
        u.vessels[universe_sim::PLAYER].ship.class = h;
        u.vessels[universe_sim::PLAYER].ship.fit = None;
        u.vessels[universe_sim::PLAYER].ship.refresh();
    }
    app.mode = Mode::Observer;
    app.observer.focus = Focus::Ship;
    app.observer.distance = std::env::var("UNIVERSE_DIST").ok().and_then(|d| d.parse().ok()).unwrap_or(160.0);
    app.observer.pitch = std::env::var("UNIVERSE_PITCH").ok().and_then(|d| d.parse().ok()).unwrap_or(0.35);
    app.observer.yaw = std::env::var("UNIVERSE_YAW").ok().and_then(|d| d.parse().ok()).unwrap_or(2.3);
    // UNIVERSE_BURN: the mains held (to see the drive lit).
    if std::env::var("UNIVERSE_BURN").is_ok() {
        let u = app.engine.universe();
        u.vessels[universe_sim::PLAYER].ship.manual = true;
        u.vessels[universe_sim::PLAYER].ship.held = u.vessels[universe_sim::PLAYER].ship.spec().thrusters.iter().enumerate().filter(|(_, t)| t.role == universe_sim::world::ship::ThrusterRole::Main).map(|(k, _)| 1u64 << k).sum();
        for _ in 0..10 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
    }
}

fn s_manual(app: &mut App, _name: &str, _c: &Ctx) {
    // In flight by the home station, the flight computer off, a nose
    // thruster and the opposite tail one held (a yaw).
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    u.start_in_flight();
    let s = u.vessels[universe_sim::PLAYER].ship.spec();
    let bit = |name: &str| s.thrusters.iter().position(|t| t.nozzle.ends_with(name)).map_or(0, |k| 1u64 << k);
    u.vessels[universe_sim::PLAYER].ship.manual = true;
    u.vessels[universe_sim::PLAYER].ship.held = bit("nose_left_side") | bit("tail_right_side");
    // UNIVERSE_BURN: the mains held instead (to see the drive lit from behind).
    if std::env::var("UNIVERSE_BURN").is_ok() {
        u.vessels[universe_sim::PLAYER].ship.held = s.thrusters.iter().enumerate().filter(|(_, t)| t.role == universe_sim::world::ship::ThrusterRole::Main).map(|(k, _)| 1u64 << k).sum();
    }
    for _ in 0..20 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    app.chase_cam = true;
}

fn s_platform(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let positions = &c.positions;
    // The home station from off its corner, a little above its deck
    // (or, "platformdeck", from just above the deck), looking at it.
    app.mode = Mode::Pilot;
    let station = sys.station().expect("home station");
    let f = StationFrame::new(sys, station, app.engine.universe().world.time, positions);
    let at = if name == "platform" { f.world(DVec3::new(900.0, 450.0, 1100.0)) } else { f.world(DVec3::new(250.0, -40.0, 520.0)) };
    let u = app.engine.universe();
    u.vessels[universe_sim::PLAYER].ship.state = ShipState::Flying;
    u.vessels[universe_sim::PLAYER].ship.position = at;
    u.vessels[universe_sim::PLAYER].ship.velocity = f.velocity_at(at);
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(f.center - at, f.up());
    app.chase_cam = false;
}

fn s_orbit(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let positions = &c.positions;
    // 7 km off the home station, orbiting it at 5 km.
    app.mode = Mode::Pilot;
    let station = sys.station().expect("home station");
    let f = StationFrame::new(sys, station, app.engine.universe().world.time, positions);
    let side = f.up().any_orthonormal_vector();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = f.center + side * 7_000.0;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = f.velocity;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(-side, f.up());
    app.engine.universe().set_nav_target(Some(NavTarget::Station(station)));
    app.engine.universe().follow(universe_sim::FollowKind::Orbit, None);
}

fn s_orbitship(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let positions = &c.positions;
    // 3 km off a settler cruising past the home station (its route
    // stopped, its engine on low), orbiting it at 1 km.
    app.mode = Mode::Pilot;
    let station = sys.station().expect("home station");
    let f = StationFrame::new(sys, station, app.engine.universe().world.time, positions);
    let side = f.up().any_orthonormal_vector();
    let u = app.engine.universe();
    let at = f.center + side * 40_000.0;
    u.vessels[universe_sim::PLAYER].ship.position = at;
    u.vessels[universe_sim::PLAYER].ship.velocity = f.velocity;
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(-side, f.up());
    let (mut c, home) = (u.vessels[universe_sim::craft_id(0)].ship.clone(), u.vessels[universe_sim::PLAYER].system);
    c.state = ShipState::Flying;
    c.position = at + side * 3_000.0;
    c.velocity = f.velocity + f.up() * 20.0;
    c.orientation = universe_sim::ship::facing(f.up(), side);
    // (Lively: burning hard, 3 m/s².)
    c.throttle = if name == "orbitshiplively" { 0.1 } else { 0.01 };
    u.vessels[universe_sim::craft_id(0)].ship = c;
    u.vessels[universe_sim::craft_id(0)].system = home;
    u.pilots()[0].avionics.route.active = false;
    for _ in 0..30 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    u.cockpit().lock_contact(universe_sim::craft_id(0));
    u.follow(universe_sim::FollowKind::Orbit, Some(1_000.0));
    for _ in 0..60 * 40 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    if name == "orbitshipwatch" {
        app.mode = Mode::Observer;
        app.observer.focus = Focus::Ship;
        app.observer.distance = 6_000.0;
        app.observer.pitch = 0.9;
    }
}

fn s_hangrepro(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    // As reported: in Reat, 117 km over a starport and coming down,
    // combat mode, a pirate near, a rock still locked from home.
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    let reat = (0..u.world.galaxy.stars.len()).find(|&i| u.world.system(i).name == "Reat").expect("Reat");
    let rsys = u.world.system(reat);
    let port = rsys.spaceports.iter().position(|_| true).expect("a port");
    u.avionics_mut().rock_lock = Some((0, sys.bodies.len() + 80));
    let mut s = u.world.ship_on(reat, universe_sim::world::Facility::Spaceport(port), 0);
    let rpos = u.world.rails_now(reat);
    let pb = rsys.spaceports[port].body;
    let up = (s.position - rpos[pb]).normalize();
    s.state = ShipState::Flying;
    s.position += up * 117_000.0;
    s.velocity = rsys.velocity(pb, u.world.time) - up * 30.0;
    u.vessels[universe_sim::PLAYER].ship = s;
    u.vessels[universe_sim::PLAYER].system = reat;
    // A pirate 3 km off.
    let mut p = u.vessels[universe_sim::PLAYER].ship.clone();
    p.position += up.any_orthonormal_vector() * 3_000.0;
    u.vessels[universe_sim::craft_id(0)].ship = p;
    u.vessels[universe_sim::craft_id(0)].system = reat;
    u.pilots()[0].avionics.pirate = true;
    u.command(&ShipCommands { arm: Some(true), ..u.vessels[universe_sim::PLAYER].ship.holding() });
    for _ in 0..300 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    if name == "hangrepro" {
        u.cockpit().lock_contact(universe_sim::craft_id(0));
    } else {
        u.avionics_mut().rock_lock = Some((0, rsys.bodies.len() + 80));
    }
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
}

fn s_taxiwatch(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let planet = c.planet;
    // Landed at a port; a settler on the next pad over, a long stay
    // begun: past its turnaround it taxis to the hangar. Seen from above.
    let u = app.engine.universe();
    let home = u.vessels[universe_sim::PLAYER].system;
    let port = sys.spaceports.iter().position(|p| p.body == planet).expect("spaceport on the station's planet");
    u.vessels[universe_sim::PLAYER].ship = u.world.ship_on(home, universe_sim::world::Facility::Spaceport(port), 5);
    // Facing the hangar.
    if let ShipState::Landed { body, local_position, .. } = u.vessels[universe_sim::PLAYER].ship.state {
        let here = local_position.normalize();
        let hangar = universe_sim::world::spaceport::hangar_direction(sys, port);
        u.vessels[universe_sim::PLAYER].ship.state = ShipState::Landed { body, local_position, local_orientation: universe_sim::ship::upright(here, hangar - here) };
    }
    u.vessels[universe_sim::craft_id(0)].ship = u.world.ship_on(home, universe_sim::world::Facility::Spaceport(port), 13);
    u.vessels[universe_sim::craft_id(0)].system = home;
    let station = sys.station().unwrap();
    {
        let mut pilots = u.pilots();
        let r = &mut pilots[0].avionics.route;
        r.stops = vec![universe_sim::avionics::route::Stop { system: home, target: NavTarget::Spaceport(port) }, universe_sim::avionics::route::Stop { system: home, target: NavTarget::Station(station) }];
        r.next = 0;
        r.active = true;
        r.dwell_until = None;
        r.stay = Some(400.0);
    }
    for _ in 0..60 * 80 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    // Our ship 1.2 km over the port, nose down at the pads and the hangar.
    let t = u.world.time;
    let pos = u.world.rails_now(home);
    let pad = PadFrame::new(sys, port, t, &pos);
    u.vessels[universe_sim::PLAYER].ship.state = ShipState::Flying;
    u.vessels[universe_sim::PLAYER].ship.position = pad.pad + pad.up * 1_200.0;
    u.vessels[universe_sim::PLAYER].ship.velocity = pad.frame_velocity(u.vessels[universe_sim::PLAYER].ship.position);
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(-pad.up, pad.up.any_orthonormal_vector());
    app.mode = Mode::Pilot;
}

fn s_fleet(app: &mut App, _name: &str, _c: &Ctx) {
    // Each hull beside us in a row, 150 m apart, as we fly.
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    let (at, v, o) = (u.vessels[universe_sim::PLAYER].ship.position, u.vessels[universe_sim::PLAYER].ship.velocity, u.vessels[universe_sim::PLAYER].ship.orientation);
    let right = o * DVec3::X;
    let ahead = o * DVec3::NEG_Z;
    let home = u.vessels[universe_sim::PLAYER].system;
    for (k, key) in ["hull.sprint", "hull.interceptor", "hull.prospector", "hull.hauler"].iter().enumerate() {
        let c = &mut u.vessels[universe_sim::craft_id(k)];
        c.ship.class = universe_sim::world::content::content().handle(key).unwrap();
        c.ship.state = ShipState::Flying;
        c.ship.position = at + ahead * 220.0 + right * ((k as f64 - 1.5) * 150.0);
        c.ship.velocity = v;
        c.ship.orientation = o;
        c.system = home;
        u.pilots()[k].avionics.route.active = false;
    }
}

fn s_sunclose(app: &mut App, _name: &str, _c: &Ctx) {
    // A tenth of an AU from the star, facing it.
    app.mode = Mode::Pilot;
    let u = app.engine.universe();
    let out = u.vessels[universe_sim::PLAYER].ship.position.normalize();
    u.vessels[universe_sim::PLAYER].ship.position = out * 0.1 * universe_sim::units::AU;
    let look = -out;
    u.vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing((look + look.any_orthonormal_vector() * 0.15).normalize(), look.any_orthonormal_vector());
}

fn s_sun(app: &mut App, _name: &str, c: &Ctx) {
    let positions = &c.positions;
    // Facing the star from the home orbit.
    app.mode = Mode::Pilot;
    let look = (positions[0] - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing((look + look.any_orthonormal_vector() * 0.15).normalize(), look.any_orthonormal_vector());
}

fn s_noon(app: &mut App, name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let planet = c.planet;
    // 2 km over the home planet, level, looking along the ground: the
    // sun overhead, on the horizon, or below it.
    app.mode = Mode::Pilot;
    let b = &sys.bodies[planet];
    let sun = (positions[0] - positions[planet]).normalize();
    let side = sun.any_orthonormal_vector();
    let angle: f64 = match name {
        "noon" => 0.35,
        "dusk" => std::f64::consts::FRAC_PI_2 - 0.02,
        _ => 2.4,
    };
    // (UNIVERSE_SUN: the sun's angle from overhead instead; UNIVERSE_ALT: the height, m.)
    let env = crate::devenv::num;
    let angle = env("UNIVERSE_SUN").unwrap_or(angle);
    let up = (sun * angle.cos() + side * angle.sin()).normalize();
    let center = positions[planet];
    let ground = b.surface_radius_at(center, center + up, t);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = center + up * (ground + env("UNIVERSE_ALT").unwrap_or(2_000.0));
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(app.engine.universe().vessels[universe_sim::PLAYER].ship.position - center);
    // Look toward the sun's side along the horizon, a little down.
    let ahead = (sun - up * sun.dot(up)).normalize_or(side);
    let look = (ahead - up * std::env::var("UNIVERSE_DOWN").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.08)).normalize();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(look, up);
}

fn s_lowflight(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let planet = c.planet;
    // Skimming 6 km over the home planet, 800 km from the port, looking ahead.
    app.mode = Mode::Pilot;
    let b = &sys.bodies[planet];
    let rot = b.rotation(t);
    // Find mountains: the highest of a few hundred spots in a wide patch.
    let start = rot.inverse() * (app.engine.universe().vessels[universe_sim::PLAYER].ship.position - positions[planet]).normalize();
    let terrain = b.terrain.as_ref().unwrap();
    let mut rng = 1u64;
    let mut best = (f64::MIN, start);
    for _ in 0..600 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let jitter = DVec3::new(((rng >> 11) % 1000) as f64 - 500.0, ((rng >> 23) % 1000) as f64 - 500.0, ((rng >> 37) % 1000) as f64 - 500.0) / 1500.0;
        let d = (start + jitter).normalize();
        let h = terrain.surface(d);
        if h > best.0 {
            best = (h, d);
        }
    }
    // Stand off from the peak and look toward it. (UNIVERSE_LAT, UNIVERSE_LON: over that
    // place instead, as the registry gives it, looking north.)
    let at = std::env::var("UNIVERSE_LAT").ok().and_then(|a| a.parse().ok()).zip(std::env::var("UNIVERSE_LON").ok().and_then(|o| o.parse().ok()));
    let best = match at {
        Some((lat, lon)) => (0.0, universe_sim::world::worlds::direction(universe_sim::world::worlds::LonLat { lat, lon })),
        None => best,
    };
    let peak = best.1;
    let off = if at.is_some() { peak } else { (peak + peak.any_orthonormal_vector() * 0.02).normalize() };
    let up = rot * off;
    log::info!("lowflight: peak {:.0} m", best.0);
    // (Where the sun stands overhead now, in the registry's latitude and longitude.)
    let noon = universe_sim::world::worlds::lon_lat(rot.inverse() * (positions[0] - positions[planet]));
    log::info!("lowflight: the sun overhead at {:.1}, {:.1}", noon.lat, noon.lon);
    // (UNIVERSE_ALT: the height instead, m; UNIVERSE_SPEED: the ground speed, m/s.)
    let env = crate::devenv::num;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = positions[planet] + up * (b.surface_radius_at(positions[planet], positions[planet] + up, t) + env("UNIVERSE_ALT").unwrap_or(6000.0));
    let to_peak = rot * peak - up;
    let fwd = if at.is_some() {
        let north = rot * DVec3::Y;
        (north - up * north.dot(up)).normalize()
    } else {
        (to_peak - up * to_peak.dot(up)).normalize()
    };
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(app.engine.universe().vessels[universe_sim::PLAYER].ship.position - positions[planet]) + fwd * env("UNIVERSE_SPEED").unwrap_or(200.0);
    // (UNIVERSE_DOWN: how far down it looks, as a slope.)
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(fwd - up * env("UNIVERSE_DOWN").unwrap_or(0.15), up);
    app.chase_cam = std::env::var_os("UNIVERSE_CHASE").is_some();
}

fn s_moon(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let sys = &c.sys;
    let planet = c.planet;
    let moon = sys.bodies.iter().position(|b| b.kind == BodyKind::Moon && b.terrain.is_some()).unwrap_or(planet);
    observe(app, home, moon, sys.bodies[moon].rail.radius * 2.2, 0.4);
}

fn s_routemap(app: &mut App, _name: &str, _c: &Ctx) {
    app.mode = Mode::Pilot;
    let stops = app.engine.universe().settler_route(7, 10);
    app.engine.universe().cockpit().route_set(stops);
    app.nav_map = Some(crate::navmap::NavMap::open(app));
}

fn s_route(app: &mut App, _name: &str, _c: &Ctx) {
    // A settler route, flown headless through its first stops, then shown mid-leg.
    app.mode = Mode::Pilot;
    let stops = app.engine.universe().settler_route(7, 10);
    app.engine.universe().cockpit().route_set(stops);
    app.engine.universe().toggle_route();
    for _ in 0..60 * 60 * 60 {
        app.engine.universe().step_world(1.0 / 60.0, 20.0, &Controls::default());
        if app.engine.universe().avionics().route.next >= 3 && app.engine.universe().vessels[universe_sim::PLAYER].ship.hyperdrive {
            break;
        }
    }
}

fn s_traffic(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    let station = c.station;
    // Let the settlers get going, then watch the home station.
    for _ in 0..60 * 60 * 2 {
        app.engine.universe().step_world(1.0 / 60.0, 3.0, &Controls::default());
    }
    observe(app, home, station, 25_000.0, 0.35);
}

fn s_crowd(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let station = c.station;
    // A crowded place without the wait: 300 of the home system's
    // crafts flying their routes within 40 km of the station, and us
    // 8 km out looking at it (for profiling).
    app.mode = Mode::Pilot;
    let st = positions[station];
    let v = sys.velocity(station, t);
    let home = app.engine.universe().vessels[universe_sim::PLAYER].system;
    let mut rng = 7u64;
    let mut next = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    };
    let u = app.engine.universe();
    let picked: Vec<usize> = (0..u.vessels.crafts().len()).filter(|&i| u.vessels[universe_sim::craft_id(i)].system == home).take(300).collect();
    for &i in &picked {
        let c = &mut u.vessels[universe_sim::craft_id(i)];
        let off = DVec3::new(next(), next(), next()).normalize_or(DVec3::X) * (5_000.0 + 35_000.0 * next().abs());
        c.ship.state = ShipState::Flying;
        c.ship.hyperdrive = false;
        c.ship.position = st + off;
        c.ship.velocity = v + DVec3::new(next(), next(), next()) * 30.0;
    }
    let mut pilots = u.pilots();
    for &i in &picked {
        let r = &mut pilots[i].avionics.route;
        r.active = !r.stops.is_empty();
        r.dwell_until = None;
        r.departing = false;
    }
    drop(pilots);
    let side = (app.engine.universe().vessels[universe_sim::PLAYER].ship.position - st).normalize_or(DVec3::X);
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = st + side * 8_000.0;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = v;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(-side, side.any_orthonormal_vector());
}

fn s_collision(app: &mut App, _name: &str, c: &Ctx) {
    let sys = &c.sys;
    let t = c.t;
    let positions = &c.positions;
    let station = c.station;
    // Collision warning on, 3 km from the station and closing at 80 m/s, a little off its center.
    app.mode = Mode::Pilot;
    let st = positions[station];
    let toward = (st - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
    let side = toward.any_orthonormal_vector();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position = st - toward * 3_000.0 + side * 150.0;
    app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = sys.velocity(station, t) + toward * 80.0;
    let look = (st - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(look + side * 0.25, side);
    app.engine.universe().cockpit().collision_warning(true);
}

fn s_pirates(app: &mut App, _name: &str, _c: &Ctx) {
    // A pirate goes after a trader near us; run until it's destroyed
    // (the kill feed shows it), watching from alongside.
    app.mode = Mode::Pilot;
    // 30 km out from the station (out of its shelter), and we watch from there.
    app.engine.universe().vessels[universe_sim::PLAYER].ship.position += DVec3::new(30_000.0, 0.0, 0.0);
    let (sysi, pos, vel) = (app.engine.universe().vessels[universe_sim::PLAYER].system, app.engine.universe().vessels[universe_sim::PLAYER].ship.position, app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity);
    app.engine.universe().spawn_settlers(2, 7);
    let n = app.engine.universe().vessels.crafts().len();
    for (k, c) in app.engine.universe().vessels.crafts_mut()[n - 2..].iter_mut().enumerate() {
        c.system = sysi;
        c.ship.state = ShipState::Flying;
        c.ship.hyperdrive = false;
        c.ship.position = pos + DVec3::new(-2_000.0 + 5_000.0 * k as f64, 1_500.0, -3_000.0);
        c.ship.velocity = vel;
        let number = c.name.split(' ').next_back().unwrap_or("").to_string();
        c.name = format!("{} {number}", if k == 0 { "Pirate" } else { "Trader" }).into();
    }
    for (k, p) in app.engine.universe().pilots()[n - 2..].iter_mut().enumerate() {
        p.avionics.pirate = k == 0;
        p.trader = k == 1;
        p.avionics.route.active = false;
        p.avionics.route.dwell_until = None;
    }
    // Mid-fight: the pirate aggressed, the trader's hull going.
    for _ in 0..60 * 120 {
        app.engine.universe().step_world(1.0 / 60.0, 1.0, &Controls::default());
        if app.engine.universe().vessels[universe_sim::craft_id(n - 1)].ship.hull < 0.97 {
            break;
        }
    }
    let pirate = app.engine.universe().vessels[universe_sim::craft_id(n - 2)].ship.position;
    let look = (pirate - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
    app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(look, look.any_orthonormal_vector());
    log::info!("scenario pirates: kills {:?}", app.engine.universe().records.kills.last());
}

fn s_aboard(app: &mut App, _name: &str, _c: &Ctx) {
    // Out of the seat, at the back of the cabin looking forward up the corridor.
    app.mode = Mode::Pilot;
    app.engine.universe().crew.place = universe_sim::world::Place::Seat;
    app.engine.universe().walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 0.02);
}

fn s_boarded(app: &mut App, _name: &str, _c: &Ctx) {
    // Just in through the hatch, facing into the ship.
    apply(app, "touchdown");
    at_hatch(app, std::f64::consts::PI);
}

fn s_vending(app: &mut App, name: &str, _c: &Ctx) {
    // Landed at the port, on foot by its vending machine, facing it
    // ("vendingopen": its panel open).
    apply(app, "touchdown");
    let u = app.engine.universe();
    let sys = u.ship_system();
    let ShipState::Landed { body, local_position, .. } = u.vessels[universe_sim::PLAYER].ship.state else { return };
    let port = (0..sys.spaceports.len()).find(|&p| sys.spaceports[p].body == body && sys.on_pad(p, body, local_position.normalize())).unwrap_or(0);
    let d = universe_sim::world::spaceport::vending_direction(&sys, port);
    let (north, _) = universe_sim::world::spaceport::tangent(d);
    let b = &sys.bodies[body];
    let at = d * b.rail.radius + north * 1.8;
    let dir = at.normalize();
    u.crew.place = universe_sim::world::Place::Outside { body, position: dir * b.surface_radius(dir), velocity: DVec3::ZERO, yaw: std::f64::consts::PI, pitch: -0.1 };
    app.chase_cam = false;
    if name == "vendingopen" {
        app.vending = Some(0);
    }
}

fn s_rawland(app: &mut App, _name: &str, _c: &Ctx) {
    // Set down on open ground (no port), step out, walk off a way and face the ship.
    apply(app, "landed");
    at_hatch(app, 0.0);
    // (A hatch to use, or a ramp to walk down.)
    if !universe_sim::world::crew::walks_out(&app.engine.universe().vessels[universe_sim::PLAYER].ship) {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 0.02);
    }
    for _ in 0..250 {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
    }
    app.engine.universe().walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI, pitch: 0.1, ..Default::default() }, 0.02);
    let u = app.engine.universe();
    log::info!("scenario rawland: ship {:?}, crew {:?}", u.vessels[universe_sim::PLAYER].ship.state, u.crew.place);
}

fn s_sunlook(app: &mut App, _name: &str, _c: &Ctx) {
    // Landed (on a pad at Port Trethi, by day), standing in the hold, looking
    // straight at the sun (through the hull).
    apply(app, "settlement");
    app.mode = Mode::Pilot;
    app.chase_cam = false;
    at_hatch(app, std::f64::consts::PI);
    // (UNIVERSE_OUT: down the ramp and out from under the ship first, under the open sky.)
    if std::env::var_os("UNIVERSE_OUT").is_some() {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI, ..Default::default() }, 0.02);
        for _ in 0..1200 {
            app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, run: true, ..Default::default() }, 0.02);
        }
    }
    let u = app.engine.universe();
    let sys = u.ship_system();
    let mut positions = Vec::new();
    sys.positions(u.world.time, &mut positions);
    let star = sys.bodies.iter().position(|b| b.kind == universe_sim::world::system::BodyKind::Star).unwrap_or(0);
    if let universe_sim::world::Place::Outside { body, position, yaw, pitch, .. } = &mut u.crew.place {
        let inv = sys.bodies[*body].rotation(u.world.time).inverse();
        let eye = *position + position.normalize() * universe_sim::world::crew::EYE;
        let d = (inv * (positions[star] - positions[*body]) - eye).normalize();
        let up = position.normalize();
        let (north, east) = universe_sim::world::spaceport::tangent(up);
        *yaw = f64::atan2(-d.dot(east), d.dot(north));
        *pitch = d.dot(up).clamp(-1.0, 1.0).asin().clamp(-1.4, 1.4);
    }
}

fn s_studiowalk(app: &mut App, _name: &str, _c: &Ctx) {
    // The demo layout walked: up the ramp (UNIVERSE_SIDE m off the centre line)
    // and on forward, through the wall's door or into it (where you end: logged).
    apply(app, "touchdown");
    mc07(app);
    let plan = demo_plan(app);
    app.engine.universe().set_layout(&plan);
    app.deckplans.retain(|p| p.hull != plan.hull);
    app.deckplans.push(plan);
    at_hatch(app, 0.0);
    let side: f64 = std::env::var("UNIVERSE_SIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    // Down the ramp and out, then back up it, forward (and off to the side first).
    for _ in 0..300 {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
    }
    app.engine.universe().walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI, ..Default::default() }, 0.02);
    for _ in 0..(side.abs() / 1.6 / 0.02) as usize {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { right: side.signum(), ..Default::default() }, 0.02);
    }
    for _ in 0..900 {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
    }
    let u = app.engine.universe();
    let sys = u.ship_system();
    if let universe_sim::world::Place::Outside { body, position, .. } = u.crew.place {
        let mut positions = Vec::new();
        sys.positions(u.world.time, &mut positions);
        let inv = sys.bodies[body].rotation(u.world.time).inverse();
        let local = (inv * u.vessels[universe_sim::PLAYER].ship.orientation).inverse() * (position - inv * (u.vessels[universe_sim::PLAYER].ship.position - positions[body]));
        log::info!("scenario studiowalk: feet in the ship's frame {local:.2?}");
    }
}

fn s_layoutlook(app: &mut App, _name: &str, _c: &Ctx) {
    // Six decks (5.6 m up from the keel, 2.9 m apart), each filled, then down the
    // ramp, out and off to the side (UNIVERSE_SIDE m), looking back up at the ship.
    apply(app, "touchdown");
    mc07(app);
    let Some(mesh) = app.ship.spec().shape().walk.clone() else { return };
    let mut plan = demo_plan(app);
    plan.decks = (0..6).map(|k| {
        let mut d = universe_sim::world::deckplan::Deck::at(mesh.lo.y + 5.6 + 2.9 * k as f64);
        if std::env::var_os("UNIVERSE_EMPTY").is_none() {
            d.planes = universe_sim::world::deckplan::fill(&mesh, d.floor);
        }
        d
    }).collect();
    for d in &plan.decks {
        let sides = universe_sim::world::deckplan::deck_sides(&mesh, d.floor);
        let areas: Vec<String> = d.planes.iter().map(|p| format!("{:.0}", universe_sim::world::deckplan::floor_strips(p, &sides, &[]).iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum::<f64>())).collect();
        log::info!("scenario layoutlook: deck at {:.1} m up, floors m2 {}, corners {:?}", d.floor - mesh.lo.y, areas.join(" "), d.planes.iter().map(|p| p.len()).collect::<Vec<_>>());
        // (Any slab corner not under the hull and over it: a floor poking out through it.)
        let mut out = 0;
        let mut all = 0;
        for p in &d.planes {
            for (z0, z1, x0, x1) in universe_sim::world::deckplan::floor_strips(p, &sides, &[]) {
                for (x, z) in [(x0 + 0.05, z0 + 0.02), (x1 - 0.05, z0 + 0.02), (x0 + 0.05, z1 - 0.02), (x1 - 0.05, z1 - 0.02)] {
                    for y in [d.floor - universe_sim::world::deckplan::DECK + 0.02, d.floor - 0.02] {
                        all += 1;
                        if !universe_sim::world::deckplan::roofed(&mesh, universe_engine::glam::DVec2::new(x, z), y, 0.0, true) {
                            out += 1;
                        }
                    }
                }
            }
        }
        log::info!("scenario layoutlook:   slab corners out from under the hull: {out} of {all}");
    }
    app.engine.universe().set_layout(&plan);
    app.deckplans.retain(|p| p.hull != plan.hull);
    app.deckplans.push(plan);
    at_hatch(app, 0.0);
    for _ in 0..900 {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
    }
    let side: f64 = std::env::var("UNIVERSE_SIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    for _ in 0..(side.abs() / 1.6 / 0.02) as usize {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { right: side.signum(), ..Default::default() }, 0.02);
    }
    app.engine.universe().walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI - side.signum() * 0.9, pitch: 0.2, ..Default::default() }, 0.02);
}

fn s_studiopreview(app: &mut App, _name: &str, _c: &Ctx) {
    // The studio's walk-through on the demo plan: on foot in the hold, facing the wall and its door.
    apply(app, "studio");
    let floor = demo_plan(app).decks[0].floor;
    // (UNIVERSE_AT=x,z,yaw,pitch: stood there instead, looking that way.)
    let look: Vec<f64> = crate::devenv::list("UNIVERSE_AT");
    let (x, z, yaw, pitch) = match look[..] {
        [x, z, yaw, pitch] => (x, z, yaw, pitch),
        _ => (0.0, 4.0, 0.0, 0.05),
    };
    // (UNIVERSE_FLOOR=m: one deck that far up from the keel, filled, instead.)
    let mut plan = demo_plan(app);
    let mut floor = floor;
    if let Some(up) = std::env::var("UNIVERSE_FLOOR").ok().and_then(|v| v.parse::<f64>().ok())
        && let Some(mesh) = app.ship.spec().shape().walk.clone()
    {
        floor = mesh.lo.y + up;
        let mut d = universe_sim::world::deckplan::Deck::at(floor);
        d.planes = universe_sim::world::deckplan::fill(&mesh, floor);
        plan.decks = vec![d];
    }
    let at = (universe_engine::glam::DVec3::new(x, floor + 0.05, z), yaw);
    app.engine.universe().set_layout(&plan);
    app.engine.universe().preview(Some(at));
    app.preview = app.shipyard.take();
    app.mode = Mode::Pilot;
    app.chase_cam = false;
    app.engine.universe().walk(&universe_sim::world::WalkCommands { pitch, ..Default::default() }, 0.02);
}

fn s_interior(app: &mut App, _name: &str, _c: &Ctx) {
    // The shipyard's interior studio on our hull (UNIVERSE_TURN=yaw,pitch: looked
    // at from there, rad).
    apply(app, "docked");
    mc07(app);
    let turn: Vec<f32> = crate::devenv::list("UNIVERSE_TURN");
    let mut y = match turn[..] {
        [yaw, pitch] => crate::shipyard::Shipyard::interior_turned(yaw, pitch),
        _ => crate::shipyard::Shipyard::interior(app),
    };
    // (UNIVERSE_NEW=none|<hull key>: a new design; UNIVERSE_DIALOG=new|open: that
    // dialog open.)
    if let Ok(h) = std::env::var("UNIVERSE_NEW") {
        y.interior_mut().start_new((h != "none").then_some(h.as_str()));
    }
    if let Ok(id) = std::env::var("UNIVERSE_OPEN") {
        y.interior_mut().open_saved(&id);
    }
    // (UNIVERSE_STAND=x,y,z,yaw, or 1: the design on its test stand.)
    if crate::devenv::flag("UNIVERSE_STAND") {
        y.interior_mut().stand_test(<[f64; 4]>::try_from(crate::devenv::list::<f64>("UNIVERSE_STAND")).ok());
    }
    if let Ok(d) = std::env::var("UNIVERSE_DIALOG") {
        y.interior_mut().show_dialog(d == "open");
    }
    // (UNIVERSE_MODULES: the ship's fit laid out, the MODULES tool in hand.)
    if std::env::var_os("UNIVERSE_MODULES").is_some() {
        let spec = app.ship.spec();
        y.interior_mut().sample_modules(spec);
    }
    // (UNIVERSE_FRAME_TOOL: the FRAME tool in hand.)
    if crate::devenv::flag("UNIVERSE_FRAME_TOOL") {
        y.interior_mut().frame_tool();
    }
    // (UNIVERSE_DECKMODE: the FRAME tool in DECK mode.)
    if std::env::var_os("UNIVERSE_DECKMODE").is_some() {
        y.interior_mut().deck_tool();
    }
    // (UNIVERSE_DECK_STUDIO: the deck studio open instead.)
    if crate::devenv::flag("UNIVERSE_DECK_STUDIO") {
        y.open_decks();
    }
    // (UNIVERSE_HIDE=k,k,...: those layers hidden.)
    for k in crate::devenv::list::<usize>("UNIVERSE_HIDE") {
        y.interior_mut().hide(k);
    }
    // (UNIVERSE_WALKAT=x,y,z,yaw: a walk-through there, as WALK HERE.)
    let v = crate::devenv::list::<f64>("UNIVERSE_WALKAT");
    if v.len() == 4 {
        y.interior_mut().walk = Some((universe_engine::glam::DVec3::new(v[0], v[1], v[2]), v[3]));
    }
    // (UNIVERSE_PLAN: a sample access plan drawn.)
    if std::env::var_os("UNIVERSE_PLAN").is_some() {
        let spec = app.ship.spec();
        y.interior_mut().sample(&spec.key, spec.shape());
        // (And the deck studio's demo decks, to be seen here too.)
        let decks = demo_plan(app);
        app.deckplans.retain(|p| p.hull != decks.hull);
        app.deckplans.push(decks);
        // (UNIVERSE_WALK=k: walked in the middle of line k, as WALK HERE.)
        if let Some(k) = std::env::var("UNIVERSE_WALK").ok().and_then(|v| v.parse().ok()) {
            y.interior_mut().walk_line(k);
        }
    }
    app.shipyard = Some(y);
}

fn s_studio(app: &mut App, _name: &str, _c: &Ctx) {
    // The shipyard's layout studio on our hull, with a deck laid out for a look
    // (UNIVERSE_EMPTY: none; not saved).
    apply(app, "docked");
    mc07(app);
    let key = app.ship.spec().key.clone();
    app.deckplans.retain(|p| p.hull != key);
    let mut y = crate::shipyard::Shipyard::laying_out(app);
    // (UNIVERSE_PLAN: the 3D studio's sample access plan, to be seen here.)
    if std::env::var_os("UNIVERSE_PLAN").is_some() {
        let spec = app.ship.spec();
        y.interior_mut().sample(&spec.key, spec.shape());
    }
    // (UNIVERSE_TOOL: plane, wall, door, ladder or stair in hand.)
    y.studio_mut().tool = match std::env::var("UNIVERSE_TOOL").as_deref() {
        Ok("plane") => crate::studio::Tool::Plane,
        Ok("wall") => crate::studio::Tool::Wall,
        Ok("door") => crate::studio::Tool::Door,
        Ok("ladder") => crate::studio::Tool::Ladder,
        Ok("stair") => crate::studio::Tool::Stair,
        _ => crate::studio::Tool::Select,
    };
    // (UNIVERSE_FLIP=side,end: those views flipped.)
    let flip = std::env::var("UNIVERSE_FLIP").unwrap_or_default();
    y.studio_mut().side_flip = flip.contains("side");
    y.studio_mut().end_flip = flip.contains("end");
    // (UNIVERSE_CURSOR=x,y: the mouse there, in pixels.)
    if let Some((cx, cy)) = std::env::var("UNIVERSE_CURSOR").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?)))) {
        y.studio_mut().cursor = universe_engine::glam::Vec2::new(cx, cy);
    }
    if std::env::var_os("UNIVERSE_EMPTY").is_none() {
        let mut plan = demo_plan(app);
        // (UNIVERSE_CARVE=x,z: deck 1's floor carved round that point instead.)
        if let Some((x, z)) = std::env::var("UNIVERSE_CARVE").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?))))
            && let Some(mesh) = app.ship.spec().shape().walk.as_ref()
        {
            let deck = &mut plan.decks[0];
            let sides = universe_sim::world::deckplan::deck_sides(mesh, deck.floor);
            deck.planes = universe_sim::world::deckplan::carve(&sides, &deck.walls, universe_engine::glam::DVec2::new(x, z)).into_iter().collect();
            if let Some(poly) = deck.planes.first() {
                let (lo, hi) = poly.iter().fold((universe_engine::glam::DVec2::MAX, universe_engine::glam::DVec2::MIN), |m, p| (m.0.min(*p), m.1.max(*p)));
                let area: f64 = universe_sim::world::deckplan::floor_strips(poly, &sides, &[]).iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum();
                log::info!("scenario studio: carved a floor of {} points, x {:.1}..{:.1} z {:.1}..{:.1}, {area:.0} m2", poly.len(), lo.x, hi.x, lo.y, hi.y);
            }
        }
        // (UNIVERSE_DECKS=n: decks stacked up to n, each 2.9 m over the last.)
        let n = crate::devenv::num("UNIVERSE_DECKS").map_or(0, |v| v as usize);
        while plan.decks.len() < n {
            let last = plan.decks.last().expect("a deck");
            let deck = universe_sim::world::deckplan::Deck { floor: last.floor + 2.9, headroom: last.headroom, planes: Vec::new(), walls: Vec::new(), ladders: Vec::new(), stairs: Vec::new() };
            plan.decks.push(deck);
        }
        // (UNIVERSE_FLOOR=m: deck 1 that far up from the keel, alone.)
        if let Some(up) = std::env::var("UNIVERSE_FLOOR").ok().and_then(|v| v.parse::<f64>().ok())
            && let Some(mesh) = app.ship.spec().shape().walk.as_ref()
        {
            plan.decks = vec![universe_sim::world::deckplan::Deck::at(mesh.lo.y + up)];
        }
        // (UNIVERSE_FILL: deck 1 filled, as the floors' FILL button does.)
        if std::env::var_os("UNIVERSE_FILL").is_some()
            && let Some(mesh) = app.ship.spec().shape().walk.as_ref()
        {
            let deck = &mut plan.decks[0];
            let sides = universe_sim::world::deckplan::deck_sides(mesh, deck.floor);
            deck.planes = universe_sim::world::deckplan::fill(mesh, deck.floor);
            let areas: Vec<String> = deck.planes.iter().map(|p| format!("{:.0}", universe_sim::world::deckplan::floor_strips(p, &sides, &[]).iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum::<f64>())).collect();
            log::info!("scenario studio: filled {} floors, m2 {}", deck.planes.len(), areas.join(" "));
        }
        app.deckplans.push(plan);
    }
    app.shipyard = Some(y);
}

fn s_rampup(app: &mut App, _name: &str, _c: &Ctx) {
    // Out and down the ramp, then back up it into the ship (where you end up: logged).
    apply(app, "outside");
    app.engine.universe().walk(&universe_sim::world::WalkCommands { pitch: -0.15, ..Default::default() }, 0.02);
    for _ in 0..500 {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
    }
    let u = app.engine.universe();
    let sys = u.ship_system();
    if let universe_sim::world::Place::Outside { body, position, .. } = u.crew.place {
        let mut positions = Vec::new();
        sys.positions(u.world.time, &mut positions);
        let inv = sys.bodies[body].rotation(u.world.time).inverse();
        let local = (inv * u.vessels[universe_sim::PLAYER].ship.orientation).inverse() * (position - inv * (u.vessels[universe_sim::PLAYER].ship.position - positions[body]));
        log::info!("scenario rampup: feet in the ship's frame {local:.2?}");
    }
}

fn s_outside(app: &mut App, _name: &str, _c: &Ctx) {
    // Land on the pad, step out, turn round to look at the ship.
    apply(app, "touchdown");
    at_hatch(app, 0.0);
    // (A hatch to use, or a ramp to walk down.)
    if !universe_sim::world::crew::walks_out(&app.engine.universe().vessels[universe_sim::PLAYER].ship) {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 0.02);
    }
    // Walk away from the ship a while, then face it.
    for _ in 0..300 {
        app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
    }
    app.engine.universe().walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI, pitch: 0.15, ..Default::default() }, 0.02);
    log::info!("scenario outside: crew {:?}", app.engine.universe().crew.place);
}

fn s_radar(app: &mut App, name: &str, _c: &Ctx) {
    // Fly alongside a settler under way, 6 km behind and to the side
    // of it, lock it on the radar and face it.
    for _ in 0..60 * 60 * 2 {
        app.engine.universe().step_world(1.0 / 60.0, 3.0, &Controls::default());
    }
    let (ship_system, pos) = (app.engine.universe().vessels[universe_sim::PLAYER].system, app.engine.universe().vessels[universe_sim::PLAYER].ship.position);
    let near = app.engine.universe().vessels.crafts().iter().filter(|c| c.system == ship_system && c.ship.is_flying() && !c.ship.hyperdrive).min_by(|a, b| {
        a.ship.position.distance(pos).total_cmp(&b.ship.position.distance(pos))
    });
    if let Some(c) = near {
        let (p, v) = (c.ship.position, c.ship.velocity);
        let back = v.try_normalize().unwrap_or(DVec3::X);
        app.engine.universe().vessels[universe_sim::PLAYER].ship.position = p - back * 5_000.0 + back.any_orthonormal_vector() * 3_000.0;
        app.engine.universe().vessels[universe_sim::PLAYER].ship.velocity = v;
    }
    app.mode = Mode::Pilot;
    // "contacts": the same, but nothing locked (every ship marked).
    let lock = if name == "contacts" { app.engine.universe().contacts().into_iter().next() } else { app.engine.universe().lock_next_contact() };
    if let Some(c) = lock {
        let to = (c.blip.position - app.engine.universe().vessels[universe_sim::PLAYER].ship.position).normalize();
        app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(to, to.any_orthonormal_vector());
    }
    if name == "gunnery" {
        // Track it for two seconds, then fire the gun and laser at the
        // lead (the tracers are in flight for the screenshot).
        for _ in 0..120 {
            app.engine.universe().step_world(1.0 / 60.0, 1.0, &Controls::default());
            let contacts = app.engine.universe().contacts();
            app.fire = app.engine.universe().fire_control(&contacts);
        }
        // The nose 2° off the lead: the gimbal lays the gun on it.
        if let Some((_, Some(sol))) = app.fire {
            let off = universe_engine::glam::DQuat::from_rotation_z(2f64.to_radians()) * sol.aim;
            app.engine.universe().vessels[universe_sim::PLAYER].ship.orientation = universe_sim::ship::facing(off, sol.aim.any_orthonormal_vector());
        }
        let u = app.engine.universe();
        u.command(&ShipCommands { arm: Some(true), ..u.vessels[universe_sim::PLAYER].ship.holding() });
        app.engine.universe().vessels[universe_sim::PLAYER].ship.arming = 0.0;
        app.engine.universe().vessels[universe_sim::PLAYER].ship.triggers = universe_sim::world::Triggers { gun: true, laser: true };
    }
}

fn s_follow(app: &mut App, _name: &str, _c: &Ctx) {
    // Follow a settler that's on an approach (docking, landing or a gate run).
    app.mode = Mode::Observer;
    for _ in 0..60 * 60 * 5 {
        app.engine.universe().step_world(1.0 / 60.0, 3.0, &Controls::default());
        if let Some(i) = app.engine.universe().vessels.crafts().iter().position(|c| c.ship.is_flying() && !c.ship.hyperdrive && c.status.clearance.is_some_and(|x| x.autopilot)) {
            app.observer.focus = Focus::Craft(i);
            app.observer.distance = 300.0;
            app.observer.pitch = 0.3;
            break;
        }
    }
}

fn s_network(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    app.mode = Mode::Observer;
    app.observer.focus = Focus::Body { system: home, body: 0 };
    app.observer.distance = 4.0e17;
    app.observer.pitch = 0.9;
}

fn s_autodock(app: &mut App, name: &str, _c: &Ctx) {
    // Docking computer flying from the spawn point, captured mid-final (or docked).
    app.mode = Mode::Pilot;
    app.engine.universe().toggle_autopilot();
    for _ in 0..60 * 60 * 3 {
        app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
        let final_run = app.engine.universe().docking_status().is_some_and(|(_, s)| s.phase == universe_sim::Phase::Final && s.height < 2200.0);
        if (name == "autodock" && final_run) || matches!(app.engine.universe().vessels[universe_sim::PLAYER].ship.state, ShipState::Landed { .. }) {
            break;
        }
    }
}

fn s_trades(app: &mut App, _name: &str, c: &Ctx) {
    let home = c.home;
    // Traffic under way until traders trade here (the feed shows them).
    app.mode = Mode::Pilot;
    for _ in 0..60 * 60 * 60 {
        app.engine.universe().step_world(1.0 / 60.0, 5.0, &Controls::default());
        let u = app.engine.universe();
        let now = u.world.time;
        let n = u.records.trades.iter().rev().take_while(|r| now - r.time < 5.0).filter(|r| r.system == home).count();
        if n >= 2 {
            break;
        }
    }
    let u = app.engine.universe();
    log::info!("scenario trades: {} trades so far, last {:?}", u.records.stats.trades, u.records.trades.last().map(|r| (&r.trader, &r.item)));
}

fn s_market(app: &mut App, _name: &str, _c: &Ctx) {
    // Landed at Port Trethi, its warehouse stocked with a few things, the
    // market open; bought ten tonnes of plate.
    apply(app, "docked");
    trethi_stocked(app);
    let u = app.engine.universe();
    if let Some(f) = u.docked_market() {
        let quotes = u.market_quotes(f);
        let plate = universe_sim::world::goods::item("stock.al6061-pl-5");
        if let Some(q) = quotes.iter().find(|q| q.buy.is_some() && Some(q.offer.item) == plate).or(quotes.iter().find(|q| q.buy.is_some())) {
            let item = q.offer.item;
            let r = u.trade(f, item, 10);
            log::info!("scenario market: bought 10 of {}: {r:?}", u.world.goods[item].name);
        }
        app.engine.send(universe_sim::Command::WatchMarket(Some(f)));
    }
    app.engine.refresh();
    app.v = app.engine.view();
    app.market = Some(crate::market::MarketView::open(app));
}

fn s_enemy(app: &mut App, _name: &str, _c: &Ctx) {
    // On approach to the home station, an enemy of the home system.
    apply(app, "approach");
    let home = app.charts.home_system;
    app.engine.universe().services.standings.set(universe_sim::PLAYER, home, -100.0);
    app.engine.refresh();
    app.v = app.engine.view();
}

fn s_newsdesk(app: &mut App, name: &str, _c: &Ctx) {
    // Docked at home; traffic runs 12 minutes, the outlets and we
    // listening as it goes; then the news panel. (The ticker: just
    // past the first digests, the panel shut.)
    apply(app, "docked");
    let until = if name == "newsticker" { 612.0 } else { 720.0 };
    while app.engine.universe().world.time < until {
        for _ in 0..30 {
            app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
        }
        app.engine.refresh();
        app.v = app.engine.view();
        let (now, sys) = (app.v.time, app.v.ship_system);
        let room = app.newsroom.get_or_insert_with(|| universe_sim::newsroom::Newsroom::new(&app.charts, 0.0));
        room.update(&app.charts, now, &app.v.kills, &app.v.trade_log);
        let casts = room.broadcasts();
        let us = universe_sim::news::Listener { system: sys, at: app.v.ship.position, comm: app.v.ship.spec().comm, player: true, in_tube: false };
        app.news.update(&app.charts, now, &us, &universe_sim::news::Happenings { kills: &app.v.kills, trades: &app.v.trade_log, broadcasts: &casts, sightings: &[] });
    }
    log::info!("scenario newsdesk: {} digests", app.newsroom.as_ref().map_or(0, |r| r.digests.len()));
    app.news_panel = name == "newsdesk";
}

fn s_marketnear(app: &mut App, name: &str, _c: &Ctx) {
    // Docked at home, the world run a while (boards put out), looking at
    // another market's prices as its board reached us: a port close by, or far.
    apply(app, "docked");
    for _ in 0..2400 {
        app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
    }
    let u = app.engine.universe();
    let names = u.markets();
    trethi_stocked(app);
    let pick = if name == "marketnear" { "Port Eikir" } else { "Port Zaudalein" };
    let f = names.iter().find(|(_, n)| n.starts_with(pick)).or(names.last()).map(|m| m.0);
    app.engine.send(universe_sim::Command::WatchMarket(f));
    app.engine.refresh();
    app.v = app.engine.view();
    let mut m = crate::market::MarketView::open(app);
    m.shown = m.markets.iter().position(|(g, _)| Some(*g) == f).unwrap_or(0);
    m.refresh(app);
    app.market = Some(m);
}
