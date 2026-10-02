//! Manual thrusters: the flight computer off, each thruster fired by a key
//! of its own while it's held (the numpad, each key where its thruster
//! sits on the ship seen from above, nose up; W the main drive). Nothing
//! steadies the ship: it turns and moves by just what's fired. A small plan
//! of the ship at the left of the screen shows each thruster's key, lit
//! while it fires.

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{Color, Frame, Input, KeyCode};
use universe_sim::world::ship::{ClassSpec, ThrusterRole};

use crate::App;

/// The numpad's keys, where each sits (column, row: nose up), and their labels.
const PAD: [(KeyCode, f64, f64, &str); 16] = [
    (KeyCode::NumpadDivide, 1.0, 0.0, "/"),
    (KeyCode::NumpadMultiply, 2.0, 0.0, "*"),
    (KeyCode::NumpadSubtract, 3.0, 0.0, "-"),
    (KeyCode::Numpad7, 0.0, 1.0, "7"),
    (KeyCode::Numpad8, 1.0, 1.0, "8"),
    (KeyCode::Numpad9, 2.0, 1.0, "9"),
    (KeyCode::NumpadAdd, 3.0, 1.5, "+"),
    (KeyCode::Numpad4, 0.0, 2.0, "4"),
    (KeyCode::Numpad5, 1.0, 2.0, "5"),
    (KeyCode::Numpad6, 2.0, 2.0, "6"),
    (KeyCode::Numpad1, 0.0, 3.0, "1"),
    (KeyCode::Numpad2, 1.0, 3.0, "2"),
    (KeyCode::Numpad3, 2.0, 3.0, "3"),
    (KeyCode::NumpadEnter, 3.0, 3.5, "ENT"),
    (KeyCode::Numpad0, 0.5, 4.0, "0"),
    (KeyCode::NumpadDecimal, 2.0, 4.0, "."),
];

/// Each of a hull's thrusters' key (index into `PAD`): the mains none (W
/// fires them together); the rest, each to the free key nearest where it
/// sits on the ship seen from above (nose up), the nearest pairs first.
/// More thrusters than keys: the last unbound.
pub fn keys(spec: &ClassSpec) -> Vec<Option<usize>> {
    let ts = &spec.thrusters;
    let others: Vec<usize> = (0..ts.len()).filter(|&k| ts[k].role != ThrusterRole::Main).collect();
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for &k in &others {
        lo = lo.min(ts[k].at);
        hi = hi.max(ts[k].at);
    }
    let span = (hi - lo).max(DVec3::splat(1e-6));
    // Where it'd sit on the pad: across 0..3 (left to right), down 0..4 (nose to tail).
    let spot = |k: usize| ((ts[k].at.x - lo.x) / span.x * 3.0, (ts[k].at.z - lo.z) / span.z * 4.0);
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for &k in &others {
        let (c, r) = spot(k);
        for (p, &(_, pc, pr, _)) in PAD.iter().enumerate() {
            pairs.push(((c - pc).powi(2) + (r - pr).powi(2), k, p));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut out = vec![None; ts.len()];
    let mut taken = [false; 16];
    for (_, k, p) in pairs {
        if out[k].is_none() && !taken[p] {
            out[k] = Some(p);
            taken[p] = true;
        }
    }
    out
}

/// The thrusters held now (bit `k`: thruster `k`): each bound key down, W the mains.
pub fn held(input: &Input, spec: &ClassSpec) -> u64 {
    let mut bits = 0u64;
    for (k, key) in keys(spec).into_iter().enumerate().take(64) {
        let down = match key {
            Some(p) => input.down(PAD[p].0),
            None => spec.thrusters[k].role == ThrusterRole::Main && input.down(KeyCode::KeyW),
        };
        if down {
            bits |= 1 << k;
        }
    }
    bits
}

/// Each thruster's key as shown ("W" the mains; unbound, none).
pub fn labels(spec: &ClassSpec) -> Vec<String> {
    keys(spec).into_iter().enumerate().map(|(k, p)| match p {
        Some(p) => PAD[p].3.to_string(),
        None if spec.thrusters[k].role == ThrusterRole::Main => "W".into(),
        None => String::new(),
    }).collect()
}

/// The plan at the left of the screen, in manual: the ship from above, each
/// thruster with its key (lit while it fires).
pub fn draw(frame: &mut Frame, app: &App) {
    let ship = &app.v.ship;
    if !ship.manual || !app.v.crew.seated() || app.show_thrusters {
        return;
    }
    let s = ship.spec();
    let size = frame.size();
    let (w, h) = (230.0f32, 250.0f32);
    let at = Vec2::new(8.0, ((size.y - h) * 0.5).floor());
    frame.hud_rect(at, Vec2::new(w, h), Color([0.0, 0.02, 0.01, 0.75]));
    let labels = labels(s);
    let picture = crate::thrusterpanel::Picture { spec: s, jets: &ship.jets, com: ship.centre_of_mass(), mounts: false, picked: &[], labels: &labels };
    crate::thrusterpanel::view(frame, &picture, at, Vec2::new(w, h - 14.0), DVec3::X, DVec3::NEG_Z, "MANUAL THRUSTERS");
    frame.text(at + Vec2::new(6.0, h - 12.0), &format!("NUMPAD JETS  W MAINS  {} EXIT", crate::keys::key(crate::keys::Act::Manual)), Color::hex(0x208838));
}
