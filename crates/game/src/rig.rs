//! The mining rig, as it's seen (any ship's, ours or a miner's): anchored
//! with the excavator on, two laser emitters slide up out of the spine and
//! out to the sides, the spine's cargo hatch opens, the beams cut into the
//! rock below, and what they break loose streams back into the hatch, as
//! thick as the dig rate. Off, it all stows again. Spine to the rock (see
//! `follow`, `Manoeuvre::Surface`).

use std::collections::HashMap;

use universe_engine::glam::{DQuat, DVec3};
use universe_engine::{Color, Frame};
use universe_sim::world::{mining, Ship, StarSystem};
use universe_sim::ShipState;

use crate::scene::color;
use crate::App;

/// Seconds to deploy (or stow) the gear.
const DEPLOY_SECS: f32 = 2.0;
/// The emitters: stowed, raised out of the spine, out to the side (ship frame, m).
const STOWED: DVec3 = DVec3::new(4.0, 2.0, -9.0);
const RAISE: f64 = 7.0;
const SPREAD: f64 = 12.0;
/// The hatch on the spine: half its width, its fore and aft edges, its height (m).
const HATCH_HALF: f64 = 3.0;
const HATCH_FORE: f64 = 2.0;
const HATCH_AFT: f64 = 9.0;
const HATCH_Y: f64 = 4.6;

/// How far each ship's gear is out (0 stowed .. 1 deployed), by who: 0 us, craft i: i + 1.
#[derive(Default)]
pub struct Rigs {
    out: HashMap<usize, f32>,
}

fn digging(ship: &Ship) -> bool {
    matches!(ship.state, ShipState::Anchored { .. }) && ship.excavator
}

impl Rigs {
    /// The gear moves out while digging, in when not.
    pub fn update(&mut self, app_ships: impl Iterator<Item = (usize, bool)>, dt: f32) {
        for (who, dig) in app_ships {
            let k = self.out.entry(who).or_insert(0.0);
            let step = dt / DEPLOY_SECS;
            *k = (*k + if dig { step } else { -step }).clamp(0.0, 1.0);
        }
        self.out.retain(|_, k| *k > 0.0);
    }

    pub fn out(&self, who: usize) -> f32 {
        self.out.get(&who).copied().unwrap_or(0.0)
    }
}

impl App {
    pub fn update_rigs(&mut self, dt: f32) {
        let ships: Vec<(usize, bool)> = std::iter::once((0, digging(&self.v.ship))).chain(self.v.crafts.iter().enumerate().map(|(i, c)| (i + 1, c.system == self.view.origin && digging(&c.ship)))).collect();
        self.rigs.update(ships.into_iter(), dt);
    }
}

/// Every rig in view.
pub fn draw(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let t = app.now();
    let mut draw_one = |who: usize, ship: &Ship, pos: DVec3, turned: DQuat| {
        let k = app.rigs.out(who);
        if k <= 0.0 || frame.projected_radius(pos, 25.0) < 2.0 {
            return;
        }
        rig(frame, sys, ship, pos, turned, k as f64, t);
    };
    if app.view.origin == app.v.ship_system {
        draw_one(0, &app.ship, app.view.ship_pos, app.place(crate::Who::Me).1);
    }
    for (i, c) in app.v.crafts.iter().enumerate() {
        if c.system == app.view.origin && app.rigs.out(i + 1) > 0.0 {
            let (pos, turned) = app.place(crate::Who::Craft(i));
            draw_one(i + 1, &c.ship, pos, turned);
        }
    }
}

/// Where each emitter is (ship frame), `k` of the way out: up out of the
/// spine first, then out to the side.
fn emitter(side: f64, k: f64) -> DVec3 {
    let raise = (k * 2.0).min(1.0);
    let spread = (k * 2.0 - 1.0).clamp(0.0, 1.0);
    DVec3::new(side * (STOWED.x + SPREAD * spread), STOWED.y + RAISE * raise, STOWED.z)
}

/// One ship's rig, `k` deployed, at `t`.
fn rig(frame: &mut Frame, sys: &StarSystem, ship: &Ship, pos: DVec3, turned: DQuat, k: f64, t: f64) {
    let world = |p: DVec3| pos + turned * p;
    let gear = Color::hex(0xc0c8d0);
    // The emitters on their struts: small boxes.
    let mut tips = Vec::new();
    for side in [-1.0, 1.0] {
        let e = emitter(side, k);
        frame.line(world(DVec3::new(side * STOWED.x, HATCH_Y - 0.5, STOWED.z)), world(DVec3::new(e.x, e.y - 1.0, e.z)), gear.scale(0.7));
        let h = 0.9;
        let corners: Vec<DVec3> = (0..8).map(|i| e + DVec3::new(if i & 1 == 0 { -h } else { h }, if i & 2 == 0 { -h } else { h }, if i & 4 == 0 { -h * 1.6 } else { h * 1.6 })).collect();
        for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
            frame.line(world(corners[a]), world(corners[b]), gear);
        }
        tips.push(world(e + DVec3::Y * h));
    }
    // The hatch: its opening, and two doors swinging up toward the rock.
    let hatch = |x: f64, z: f64| world(DVec3::new(x, HATCH_Y, z));
    let opening = [hatch(-HATCH_HALF, HATCH_FORE), hatch(HATCH_HALF, HATCH_FORE), hatch(HATCH_HALF, HATCH_AFT), hatch(-HATCH_HALF, HATCH_AFT)];
    for i in 0..4 {
        frame.line(opening[i], opening[(i + 1) % 4], gear.scale(0.6));
    }
    let swing = (k * 2.0).min(1.0) * 1.9;
    for side in [-1.0, 1.0] {
        // Hinged along the opening's side edge; closed, the door covers its half.
        let hinge_x = side * HATCH_HALF;
        let edge = DVec3::new(hinge_x - side * HATCH_HALF * swing.cos(), HATCH_Y + HATCH_HALF * swing.sin(), 0.0);
        let door = [DVec3::new(hinge_x, HATCH_Y, HATCH_FORE), DVec3::new(edge.x, edge.y, HATCH_FORE), DVec3::new(edge.x, edge.y, HATCH_AFT), DVec3::new(hinge_x, HATCH_Y, HATCH_AFT)];
        for i in 0..4 {
            frame.line(world(door[i]), world(door[(i + 1) % 4]), gear);
        }
    }
    // Fully out and digging: the beams, and the extract flowing in.
    let ShipState::Anchored { field, body, .. } = ship.state else { return };
    if k < 1.0 || !ship.excavator {
        return;
    }
    let bodies = sys.field_bodies(field);
    let Some(b) = bodies.get(body) else { return };
    let Some(rock) = b.rock.as_ref() else { return };
    let (center, _) = sys.field_body_state(field, body, t);
    let ore = color(b.color);
    let beam = Color::hex(0xff5040);
    let flicker = 0.75 + 0.25 * ((t * 37.0).sin() * (t * 23.0).cos()) as f32;
    let into = world(DVec3::new(0.0, HATCH_Y, (HATCH_FORE + HATCH_AFT) / 2.0));
    let rate = mining::dig_rate(rock);
    let n = ((rate / mining::EXCAVATOR_THROUGHPUT) * 14.0).round().max(3.0) as usize;
    for (s, tip) in tips.iter().enumerate() {
        let dir = (*tip - center).normalize_or(DVec3::Y);
        let spot = center + dir * b.surface_radius_at(center, *tip, t);
        frame.line(*tip, spot, beam.scale(flicker));
        // The cut glows.
        let across = dir.any_orthonormal_vector();
        let other = dir.cross(across);
        for d in [across, other] {
            frame.line(spot - d * 1.2, spot + d * 1.2, Color::hex(0xffd080).scale(flicker));
        }
        // What it breaks loose, streaming up into the hatch: specks along an
        // arc from the cut, as many as the rate.
        let bulge = (*tip - spot) * 0.6;
        for i in 0..n {
            let u = ((t * 0.45 + i as f64 / n as f64 + s as f64 * 0.37) % 1.0).abs();
            let at = |u: f64| {
                let a = spot.lerp(spot + bulge, u);
                let b2 = (spot + bulge).lerp(into, u);
                a.lerp(b2, u)
            };
            let (p, q) = (at(u), at((u + 0.02).min(1.0)));
            frame.line(p, q, ore.scale(0.6 + 0.4 * (1.0 - u as f32)));
        }
    }
}
