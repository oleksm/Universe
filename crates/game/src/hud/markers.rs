//! What's marked on the scene: the target, missiles, contacts, the gunsight, the impact point, the pilot's
//! overlay, the scanner, the sun's glare, turrets.

use super::*;

/// Always point the pilot at the nav target (or the station): a diamond on
/// screen, or an arrow at the edge.
pub(super) fn target_marker(frame: &mut Frame, app: &App) {
    let Some((name, target)) = &app.nav_marker else { return };
    if matches!(app.ship.state, ShipState::Landed { .. }) {
        return;
    }
    let c = if app.approach.is_some() { HUD } else { Color::hex(0x60c0ff) };
    bracket(frame, app, name, *target, c);
}

/// Missiles after us, in view: a red diamond on each, with its range (seen
/// far off, where the missile itself is a speck).
pub(super) fn missile_markers(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let back = app.now() - app.v.time;
    let ours: Vec<DVec3> = app.v.missiles.iter().filter(|m| m.4 && m.0 == app.view.origin).map(|m| m.1 + m.2 * back).collect();
    let nearest = ours.iter().copied().min_by(|a, b| a.distance(app.view.ship_pos).total_cmp(&b.distance(app.view.ship_pos)));
    for &at in &ours {
        let Some(s) = frame.project(at).filter(|s| s.x > 0.0 && s.y > 0.0 && s.x < size.x && s.y < size.y) else { continue };
        let d = [Vec2::new(0.0, -6.0), Vec2::new(6.0, 0.0), Vec2::new(0.0, 6.0), Vec2::new(-6.0, 0.0)];
        for i in 0..4 {
            frame.hud_line(s + d[i], s + d[(i + 1) % 4], RED);
        }
        // (The range on the nearest only: a salvo comes in close together.)
        if Some(at) == nearest {
            let range = fmt::distance(at.distance(app.view.ship_pos));
            frame.text(s + Vec2::new(-text_size(&range).x / 2.0, 8.0), &range, RED.scale(0.8));
        }
    }
}

/// Radar contacts in view: a small square on each ship, with its range when
/// near; the locked one gets a bracket like the nav target's.
/// Most tags on the radar's contacts at once (the nearest have them).
pub(super) const TAGS: usize = 6;

pub(super) fn contact_marker(frame: &mut Frame, app: &App) {
    let size = frame.size();
    // Each contact a small mark; the nearest few a tag beside it (its name
    // close in, its range) where it stands clear of the others' — a crowd
    // shows its nearest, not a heap of text.
    let mut near: Vec<_> = app.contacts.iter().filter(|c| app.v.avionics.contact != Some(c.blip.id)).collect();
    near.sort_by(|a, b| a.blip.distance.total_cmp(&b.blip.distance));
    let mut placed: Vec<(Vec2, Vec2)> = Vec::new();
    // (The nav target's bracket and label keep their place: see `bracket`.)
    if let Some((name, target)) = &app.nav_marker
        && let Some(p) = frame.project(*target)
    {
        let label = format!("{name} {}", fmt::distance(target.distance(app.view.ship_pos)));
        let w = text_size(&label).x;
        placed.push((p - Vec2::splat(12.0), Vec2::splat(24.0)));
        placed.push((p + Vec2::new(-w / 2.0 - 2.0, 12.0), Vec2::new(w + 4.0, GLYPH + 4.0)));
    }
    let mut tags = 0;
    for contact in near {
        let c = if contact.aggressed { RED } else { crate::scene::TRAFFIC };
        let at = app.place(crate::Who::of(contact.blip.id).unwrap_or(crate::Who::Me)).0;
        let Some(p) = frame.project(at).filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y) else { continue };
        // (Fainter the farther.)
        let k = (1.0f32 - (contact.blip.distance / 60_000.0) as f32).clamp(0.45, 1.0);
        frame.hud_box(p - Vec2::splat(3.0), Vec2::splat(6.0), c.scale(0.8 * k));
        if tags >= TAGS || contact.blip.distance > 50_000.0 {
            continue;
        }
        let range = fmt::distance(contact.blip.distance);
        let text = if contact.blip.distance < 5_000.0 { format!("{}  {range}", contact.name.to_uppercase()) } else { range };
        let pos = p + Vec2::new(6.0, -4.0);
        let extent = text_size(&text) + Vec2::new(6.0, 5.0);
        let overlaps = |&(q, e): &(Vec2, Vec2)| pos.x < q.x + e.x && q.x < pos.x + extent.x && pos.y < q.y + e.y && q.y < pos.y + extent.y;
        if placed.iter().any(overlaps) {
            continue;
        }
        placed.push((pos, extent));
        tags += 1;
        frame.text(pos, &text, c.scale(0.75 * k));
    }
    if let Some(locked) = app.contacts.iter().find(|c| Some(c.blip.id) == app.v.avionics.contact) {
        let c = if locked.aggressed { RED } else { crate::scene::TRAFFIC };
        let at = app.place(crate::Who::of(locked.blip.id).unwrap_or(crate::Who::Me)).0;
        bracket(frame, app, &locked.name, at, c);
        // Which way it's moving across our view: an arrow off its bracket.
        let v = locked.blip.velocity - app.ship.velocity;
        if v.length() > 0.5
            && let (Some(p), Some(q)) = (frame.project(at), frame.project(at + v * 2.0))
            && let Some(dir) = (q - p).try_normalize()
        {
            let (a, b) = (p + dir * 14.0, p + dir * 30.0);
            frame.hud_line(a, b, c);
            frame.hud_line(b, b - dir * 5.0 + dir.perp() * 3.0, c);
            frame.hud_line(b, b - dir * 5.0 - dir.perp() * 3.0, c);
        }
        // The lead (combat mode): fly it into the gimbal ring; fire control
        // lays the gun on it, and the circle doubles up when the gun is on.
        if app.ship.armed
            && let Some((_, Some(sol))) = &app.fire
            && let Some(p) = frame.project(app.view.ship_pos + sol.offset)
        {
            let ship = &app.ship;
            let c = if within_gimbal(ship, sol.aim) { AMBER } else { AMBER.scale(0.55) };
            frame.hud_ellipse(p, Vec2::splat(5.0), 12, c);
            if gun_on(ship, sol.aim) {
                frame.hud_ellipse(p, Vec2::splat(8.0), 16, c);
                frame.hud_ellipse(p, Vec2::splat(3.0), 8, c);
            }
            frame.hud_line(p - Vec2::new(2.0, 0.0), p + Vec2::new(2.0, 0.0), c);
            frame.hud_line(p - Vec2::new(0.0, 2.0), p + Vec2::new(0.0, 2.0), c);
            if let Some(q) = frame.project(app.place(crate::Who::of(locked.blip.id).unwrap_or(crate::Who::Me)).0) {
                let d = q - p;
                if d.length() > 12.0 {
                    frame.hud_line(p + d.normalize() * 6.0, q - d.normalize() * 10.0, AMBER.scale(0.4));
                }
            }
        }
    }
}

/// Where the nose and the gun point, in any view: a small cross on the nose
/// (in the chase view; the cockpit has its crosshair); in combat mode the
/// gimbal's reach as a ring around it, and the gun's pipper. Plus sparks
/// where hits land, and HIT on the locked target when ours do.
pub(super) fn gunsight(frame: &mut Frame, app: &App) {
    let ship = &app.ship;
    let from = app.view.ship_pos;
    let far = 1.0e5;
    let col = if ship.weapons_hot() { RED } else if ship.armed { AMBER } else { HUD };
    if let Some(nose) = frame.project(from + ship.forward() * far) {
        if app.chase_cam && crosshair_wanted(app) {
            let k = if ship.armed { 1.0 } else { 0.6 };
            for d in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                frame.hud_line(nose + d * 3.0, nose + d * 7.0, col.scale(k));
            }
        }
        if ship.armed {
            // The gimbal's cone, projected: its radius on screen.
            let side = ship.orientation * DVec3::X;
            let edge = from + (ship.forward() * GIMBAL_LIMIT.cos() + side * GIMBAL_LIMIT.sin()) * far;
            if let Some(e) = frame.project(edge) {
                let r = e.distance(nose);
                frame.hud_ellipse(nose, Vec2::splat(r), 32, col.scale(0.6));
            }
            if let Some(g) = frame.project(from + ship.gun_forward() * far) {
                frame.hud_ellipse(g, Vec2::splat(3.0), 8, col);
                frame.hud_rect(g - Vec2::splat(0.5), Vec2::ONE, col);
            }
        }
    }
    // The lock beam: a ring of `LOCK_BEAM` around the nose, in combat mode
    // and for a moment after T.
    if (ship.armed || app.beam_shown > 0.0)
        && let Some(nose) = frame.project(from + ship.forward() * far)
    {
        let side = ship.orientation * DVec3::Y;
        let beam = universe_sim::LOCK_BEAM;
        if let Some(e) = frame.project(from + (ship.forward() * beam.cos() + side * beam.sin()) * far) {
            let k = if ship.armed { 0.7 } else { (app.beam_shown / 1.5).min(1.0) };
            let c = crate::scene::TRAFFIC.scale(k);
            let r = e.distance(nose);
            frame.hud_ellipse(nose, Vec2::splat(r), 48, c);
            for d in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                frame.hud_line(nose + d * (r - 4.0), nose + d * (r + 4.0), c);
            }
        }
    }
    // Sparks where hits landed (in view), ours brighter.
    for s in &app.sparks {
        if s.system != app.view.origin {
            continue;
        }
        let Some(p) = frame.project(s.point) else { continue };
        let k = (1.0 - s.age / SPARK_TIME).max(0.0);
        let c = if s.laser { Color::hex(0xff8060) } else { Color::hex(0xffe080) }.scale(k * if s.ours { 1.0 } else { 0.6 });
        let r = 3.0 + 6.0 * (s.age / SPARK_TIME);
        for i in 0..6 {
            let a = i as f32 * std::f32::consts::TAU / 6.0 + s.age * 3.0;
            let d = Vec2::new(a.cos(), a.sin());
            frame.hud_line(p + d * r * 0.4, p + d * r, c);
        }
    }
    // HIT on the locked target when one of ours lands.
    if let Some(locked) = app.contacts.iter().find(|c| Some(c.blip.id) == app.v.avionics.contact)
        && app.sparks.iter().any(|s| s.ours && s.target == locked.blip.id && s.age < 0.6)
        && let Some(p) = frame.project(app.place(crate::Who::of(locked.blip.id).unwrap_or(crate::Who::Me)).0)
    {
        frame.text(p + Vec2::new(12.0, -18.0), "HIT", RED);
    }
}

/// The nose crosshair is for fighting and for approaches (putting the nose
/// on the plan's cue); in plain travel it stays out of the way.
pub(super) fn crosshair_wanted(app: &App) -> bool {
    app.ship.armed || app.approach.is_some()
}

/// The impact warning is navigation's: it shows in Nav mode, where its
/// switch is (it keeps watching in the others, quietly).
pub fn impact_shown(app: &App) -> bool {
    active_mode(app) == ShipMode::Nav
}

/// The collision warning's impact, labelled on screen (or an arrow to it
/// from the edge).
pub(super) fn impact_label(frame: &mut Frame, app: &App) {
    let Some(p) = app.collision.as_ref().filter(|_| impact_shown(app)) else { return };
    let Some(c) = &p.collision else { return };
    let at = app.view.positions[p.reference] + c.offset;
    let left = (c.time - (app.v.time - app.collision_at)).max(0.0) / app.warp().max(1.0);
    let label = format!("IMPACT {}", fmt::countdown(left));
    bracket(frame, app, &label, at, RED);
}

/// How long a hit's spark shows (s).
pub const SPARK_TIME: f32 = 0.5;

/// A diamond bracket around `target` with its name and range, or an arrow
/// at the screen's edge pointing to it.
pub(super) fn bracket(frame: &mut Frame, app: &App, name: &str, target: DVec3, c: Color) {
    let range = fmt::distance(target.distance(app.view.ship_pos));
    let size = frame.size();

    if let Some(p) = frame.project(target).filter(|p| p.x > 8.0 && p.y > 8.0 && p.x < size.x - 8.0 && p.y < size.y - 8.0) {
        let k = 10.0;
        let pts = [p + Vec2::new(0.0, -k), p + Vec2::new(k, 0.0), p + Vec2::new(0.0, k), p + Vec2::new(-k, 0.0)];
        for j in 0..4 {
            frame.hud_line(pts[j], pts[(j + 1) % 4], c);
        }
        let label = format!("{name} {range}");
        frame.text(p + Vec2::new(-text_size(&label).x / 2.0, 14.0), &label, c);
        return;
    }
    // Off screen: direction in camera space, pinned to an inset ellipse.
    let cam = &frame.camera;
    let local = (cam.orientation.inverse() * cam.relative(target)).xy();
    let dir = Vec2::new(local.x, -local.y).try_normalize().unwrap_or(Vec2::NEG_Y);
    let center = size / 2.0;
    let edge = center + dir * (size / 2.0 - Vec2::new(40.0, 40.0));
    let side = dir.perp();
    let tip = edge + dir * 10.0;
    frame.hud_line(tip, edge - dir * 6.0 + side * 8.0, c);
    frame.hud_line(tip, edge - dir * 6.0 - side * 8.0, c);
    frame.hud_line(edge - dir * 6.0 + side * 8.0, edge - dir * 6.0 - side * 8.0, c);
    let label_pos = edge - dir * 28.0 - Vec2::new(text_size(&range).x / 2.0, 4.0);
    frame.text(label_pos, &range, c);
}

pub(super) fn marker(frame: &mut Frame, at: Vec2, c: Color, cross: bool) {
    frame.hud_ellipse(at, Vec2::splat(6.0), 12, c);
    if cross {
        frame.hud_line(at + Vec2::new(-8.0, -8.0), at + Vec2::new(8.0, 8.0), c);
        frame.hud_line(at + Vec2::new(-8.0, 8.0), at + Vec2::new(8.0, -8.0), c);
    } else {
        for d in [Vec2::X, Vec2::NEG_X, Vec2::NEG_Y] {
            frame.hud_line(at + d * 7.0, at + d * 12.0, c);
        }
    }
}

pub(super) fn pilot_overlay(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let c = (size / 2.0).floor();
    if !app.chase_cam && crosshair_wanted(app) {
        let ship = &app.ship;
        let col = if ship.weapons_hot() { RED } else if ship.armed { AMBER } else { HUD };
        for (a, b) in [(Vec2::new(-14.0, 0.0), Vec2::new(-5.0, 0.0)), (Vec2::new(5.0, 0.0), Vec2::new(14.0, 0.0))] {
            frame.hud_line(c + a, c + b, col);
            frame.hud_line(c + a.perp(), c + b.perp(), col);
        }
    }
    gunsight(frame, app);

    let cam = frame.camera.position;

    // Docking/landing: motion relative to the target, plus where our motion should point.
    let cue = match &app.approach {
        Some(Approach::Dock { status, .. }) => Some((status.relative_velocity, status.guidance)),
        Some(Approach::Land { status, .. }) => Some((status.relative_velocity, status.guidance)),
        Some(Approach::Transit { status, .. }) => Some((status.relative_velocity, status.guidance)),
        None => None,
    };
    if let Some((v, _)) = cue {
        if v.length() > 0.3 {
            if let Some(p) = frame.project(cam + v.normalize() * 1.0e3) {
                marker(frame, p, PREDICT, false);
            }
            if let Some(p) = frame.project(cam - v.normalize() * 1.0e3) {
                marker(frame, p, PREDICT.scale(0.7), true);
            }
        }
        // Nose cue: where the plan wants the nose now. Put the crosshair on it.
        if let Some(first) = app.plan.current.as_ref().and_then(|p| p.points.first()) {
            let path_len = app.plan.current.as_deref().map_or(0.0, crate::scene::path_length);
            let nose = first.aim * DVec3::NEG_Z;
            let c = crate::scene::action_color(first.action).scale(1.2);
            let size = frame.size();
            match frame.project(cam + nose * 1.0e3).filter(|p| p.x > 12.0 && p.y > 12.0 && p.x < size.x - 12.0 && p.y < size.y - 12.0) {
                Some(p) => {
                    frame.hud_ellipse(p, Vec2::splat(9.0), 16, c);
                    frame.hud_line(p - Vec2::new(13.0, 0.0), p + Vec2::new(13.0, 0.0), c);
                    frame.hud_line(p - Vec2::new(0.0, 13.0), p + Vec2::new(0.0, 13.0), c);
                    // How far there is still to fly along the planned path.
                    let label = format!("PATH {}", fmt::distance(path_len));
                    frame.text(p + Vec2::new(-text_size(&label).x / 2.0, -26.0), &label, c);
                }
                None => {
                    let local = (frame.camera.orientation.inverse() * nose.as_vec3()).xy();
                    let dir = Vec2::new(local.x, -local.y).try_normalize().unwrap_or(Vec2::NEG_Y);
                    let edge = size / 2.0 + dir * (size / 2.0 - Vec2::new(60.0, 60.0));
                    let side = dir.perp();
                    frame.hud_line(edge + dir * 12.0, edge + side * 9.0, c);
                    frame.hud_line(edge + dir * 12.0, edge - side * 9.0, c);
                    frame.text(edge - dir * 22.0 - Vec2::new(16.0, 4.0), "TURN", c);
                }
            }
        }
        return;
    }

    // Prograde / retrograde relative to the dominant body: the key to landing.
    let Some(r) = app.view.reference else { return };
    let rel_vel = app.ship.velocity - app.view.system.velocity(r, app.v.time);
    if rel_vel.length() > 0.5 && !app.ship.hyperdrive {
        let dir = rel_vel.normalize() * 1.0e3;
        if let Some(p) = frame.project(cam + dir) {
            marker(frame, p, AMBER, false);
        }
        if let Some(p) = frame.project(cam - dir) {
            marker(frame, p, AMBER.scale(0.7), true);
        }
    }

    // Bracket the reference body.
    if let Some(p) = frame.project(app.view.positions[r]) {
        let k = 10.0;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let corner = p + Vec2::new(sx * k, sy * k);
            frame.hud_line(corner, corner - Vec2::new(sx * 5.0, 0.0), DIM);
            frame.hud_line(corner, corner - Vec2::new(0.0, sy * 5.0), DIM);
        }
    }
}

/// The 3D scanner: an ellipse seen in perspective, with height sticks.
/// Range is logarithmic from 1 km (center) to 10^12 m (rim).
pub(super) fn scanner(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let scope = Scope::draw(frame, Vec2::new((size.x / 2.0).floor(), size.y - 58.0), 1.0e3, 9.0);
    let inv = app.ship.orientation.inverse();
    for (i, b) in app.view.system.bodies.iter().enumerate() {
        let rel: DVec3 = inv * (app.view.positions[i] - app.view.ship_pos);
        scope.body(frame, rel, color(b.color), if b.kind == BodyKind::Star { 3.2 } else { 2.0 });
    }
    // Other ships the radar sees, as small cyan blips (red: aggressed); the locked one boxed.
    let locked = app.v.avionics.contact;
    for contact in &app.contacts {
        let tc = if contact.aggressed { RED } else { crate::scene::TRAFFIC };
        let rel: DVec3 = inv * (app.place(crate::Who::of(contact.blip.id).unwrap_or(crate::Who::Me)).0 - app.view.ship_pos);
        scope.craft(frame, rel, tc, locked == Some(contact.blip.id));
    }
}

/// A 3D scanner's scope as drawn: where its middle is, its radii, and its range:
/// logarithmic from `near` (m, its middle) over `decades` tens to its rim. The
/// flight HUD's and the test drive's, one drawing.
pub struct Scope {
    center: Vec2,
    radii: Vec2,
    near: f64,
    decades: f64,
}

impl Scope {
    /// The scope drawn at `center`: a soft dark disc (no box), its range rings
    /// faint, the rim a little brighter with ticks round it, a sector ahead, our
    /// chevron.
    pub fn draw(frame: &mut Frame, center: Vec2, near: f64, decades: f64) -> Scope {
        let radii = Vec2::new(140.0, 42.0);
        let ring = Color::hex(0x7fb8cc);
        let disc = |frame: &mut Frame, r: Vec2, inner: Color, outer: Color| {
            let n = 128;
            let at = |k: u32| {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                center + Vec2::new(a.cos() * r.x, a.sin() * r.y)
            };
            for k in 0..n {
                let (p, q) = (at(k), at(k + 1));
                frame.hud_triangle_colored([center, p, q], [inner, outer, outer]);
            }
        };
        disc(frame, radii * 1.08, Color([0.015, 0.028, 0.04, 0.78]), Color([0.015, 0.028, 0.04, 0.55]));
        for (k, a) in [(1.0 / 3.0, 0.18), (2.0 / 3.0, 0.22)] {
            frame.hud_ellipse(center, radii * k, 128, Color([ring.0[0], ring.0[1], ring.0[2], a]));
        }
        frame.hud_ellipse(center, radii, 160, Color([ring.0[0], ring.0[1], ring.0[2], 0.55]));
        for k in 0..36 {
            let a = k as f32 / 36.0 * std::f32::consts::TAU;
            let dir = Vec2::new(a.cos(), a.sin());
            let len = if k % 9 == 0 { 0.08 } else { 0.035 };
            frame.hud_line_smooth(center + dir * radii, center + dir * radii * (1.0 - len), Color([ring.0[0], ring.0[1], ring.0[2], 0.5]));
        }
        // Ahead (up the scope): a faint sector.
        for s in [-1.0f32, 1.0] {
            let a = -std::f32::consts::FRAC_PI_2 + s * 0.45;
            frame.hud_line_smooth(center, center + Vec2::new(a.cos() * radii.x, a.sin() * radii.y), Color([ring.0[0], ring.0[1], ring.0[2], 0.16]));
        }
        frame.hud_line_smooth(center - Vec2::new(radii.x, 0.0), center + Vec2::new(radii.x, 0.0), Color([ring.0[0], ring.0[1], ring.0[2], 0.1]));
        let chevron = Color([0.9, 0.95, 1.0, 0.9]);
        frame.hud_line_smooth(center + Vec2::new(-3.0, 2.0), center + Vec2::new(0.0, -3.0), chevron);
        frame.hud_line_smooth(center + Vec2::new(3.0, 2.0), center + Vec2::new(0.0, -3.0), chevron);
        Scope { center, radii, near, decades }
    }

    /// Where `rel` (the ship's frame: +x right, +y up, ahead -z) lies on the scope:
    /// its foot on the plane and the top of its stalk, and whether it's above.
    fn place(&self, rel: DVec3) -> Option<(Vec2, Vec2, bool)> {
        let d = rel.length();
        if d < 1.0 {
            return None;
        }
        let r = ((d / self.near).max(1.0).log10() / self.decades).min(1.0) as f32;
        let (x, y, z) = ((rel.x / d) as f32, (rel.y / d) as f32, (rel.z / d) as f32);
        let base = self.center + Vec2::new(x * self.radii.x * r, z * self.radii.y * r);
        Some((base, base - Vec2::new(0.0, y * 36.0 * r), y > 0.0))
    }

    /// A body (or a place) on the scope: its stalk (above the plane a firm
    /// stalk; below, fainter) and a glowing dot `dot` across.
    pub fn body(&self, frame: &mut Frame, rel: DVec3, c: Color, dot: f32) {
        let Some((base, top, above)) = self.place(rel) else { return };
        frame.hud_line(base, top, Color([c.0[0], c.0[1], c.0[2], if above { 0.6 } else { 0.3 }]));
        frame.hud_glow(top, dot * 2.2, 10, Color([c.0[0], c.0[1], c.0[2], 0.45]), Color([c.0[0], c.0[1], c.0[2], 0.0]));
        frame.hud_glow(top, dot, 10, c, Color([c.0[0], c.0[1], c.0[2], 0.6]));
    }

    /// A craft on the scope: a small blip on its stalk, boxed if `locked`.
    pub fn craft(&self, frame: &mut Frame, rel: DVec3, tc: Color, locked: bool) {
        let Some((base, top, above)) = self.place(rel) else { return };
        frame.hud_line(base, top, Color([tc.0[0], tc.0[1], tc.0[2], if above { 0.55 } else { 0.25 }]));
        frame.hud_glow(top, 3.0, 8, Color([tc.0[0], tc.0[1], tc.0[2], 0.5]), Color([tc.0[0], tc.0[1], tc.0[2], 0.0]));
        frame.hud_glow(top, 1.3, 8, tc, tc);
        if locked {
            frame.hud_ellipse(top, Vec2::splat(5.0), 16, tc);
        }
    }
}

/// The sun's glare: a white-hot core, a halo, and rays in the star's colour,
/// on screen so it reads at any distance. It grows with how bright the sun is
/// here (see `Light`), and whatever stands in the way hides it: a body,
/// fading in as the sun clears its limb; anything drawn (walls round us,
/// a station, a ship), as much of the disc as it covers. Through a window, it shows.
pub(super) fn sun_glare(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let Some(star) = sys.bodies.iter().position(|b| b.kind == BodyKind::Star) else { return };
    let sun = app.view.positions[star];
    let cam = frame.camera.position;
    let to_sun = sun - cam;
    let dist = to_sun.length();
    let dir = to_sun / dist;
    // How much of the sun's disc is in sight: what planets and moons leave
    // of it (exactly), and what everything drawn leaves (the depth buffer
    // read over its disc on the GPU: a station's structure, a ship, the hull
    // round the eye, a building alike; a frame or two behind).
    let radius = sys.bodies[star].rail.radius;
    frame.sun_probe = Some((sun - dir * radius * 1.05, frame.projected_radius(sun, radius).max(2.0)));
    let visible = (frame.sun_visible(cam) as f32).min(universe_engine::sun_seen());
    if visible <= 0.0 {
        return;
    }
    let Some(p) = frame.project(sun) else { return };
    let size = frame.size();
    if p.x < -200.0 || p.y < -200.0 || p.x > size.x + 200.0 || p.y > size.y + 200.0 {
        return;
    }
    // The glare goes with the light itself: the square root of the
    // irradiance (1 at 1 AU from a sun-like star), unadapted — a hundred
    // times the light close in is ten times the glare.
    let irradiance = universe_engine::Light { position: sun, color: [1.0; 3], luminosity: sys.luminosity, reference: universe_sim::units::AU, radius: 0.0 }.irradiance_at(cam);
    let [r, g, b] = sys.class.color();
    let disc = frame.projected_radius(sun, sys.bodies[star].rail.radius).max(2.0);
    let k = (irradiance.sqrt() as f32).min(12.0);
    // Looking toward it, the view washes out: a veil over everything, the
    // nearer the star and the more squarely we face it.
    let facing = frame.camera.orientation.as_dquat() * DVec3::NEG_Z;
    let toward = facing.dot(dir).max(0.0).powi(6) as f32;
    let veil = (0.015 * k * toward).min(0.3);
    if veil > 0.005 {
        frame.hud_rect(Vec2::ZERO, size, Color([r.max(0.9), g.max(0.85), b.max(0.8), veil * visible]));
    }
    let k = k / 1.6;
    // The glare: light scattered in the eye, brightest at the sun and
    // falling away smoothly with angle (layered glows, each twice as wide
    // and half as strong: no edge), white near it, the star's colour out.
    let mix = |w: f32| [r + (1.0 - r) * w, g + (1.0 - g) * w, b + (1.0 - b) * w];
    let strength = (0.35 + 0.06 * k).min(0.8) * visible;
    let mut radius = disc * 1.6 + 5.0;
    for j in 0..5 {
        let [cr, cg, cb] = mix((1.0 - j as f32 / 3.0).max(0.0));
        let a = strength * 0.4f32.powi(j);
        frame.hud_glow(p, radius, 40, Color([cr, cg, cb, a]), Color([cr, cg, cb, 0.0]));
        radius *= 1.7;
    }
    // The core: in the scene, just this side of the star, so what stands in
    // front of it (a station's structure, a ship) hides it pixel by pixel;
    // white-hot (it blooms in the tone curve).
    let core = disc * 1.4 + 4.0 + 6.0 * k;
    let radius = sys.bodies[star].rail.radius;
    let centre = sun - dir * radius * 1.05;
    let world = core as f64 * radius / disc as f64 * (dist - radius * 1.05) / dist;
    let (u, v) = (dir.any_orthonormal_vector(), dir.cross(dir.any_orthonormal_vector()));
    let rim = |i: usize| {
        let a = i as f64 * std::f64::consts::TAU / 24.0;
        centre + (u * a.cos() + v * a.sin()) * world
    };
    let (white, edge) = (Color([6.0, 6.0, 5.8, 1.0]), Color([r, g, b, 0.0]));
    for i in 0..24 {
        frame.triangle3([centre, rim(i), rim(i + 1)], [white, edge, edge]);
    }
    // Streaks: many fine ones, uneven, a few longer; fading out.
    let long = disc * 2.0 + 60.0 + 200.0 * k;
    for i in 0..32 {
        let h = ((i as u32).wrapping_mul(2_654_435_761) >> 20) as f32 / 4096.0;
        let a = i as f32 * std::f32::consts::TAU / 32.0 + 0.11 + h * 0.08;
        let len = long * if i % 8 == 0 { 1.0 } else { 0.25 + 0.45 * h };
        let d = Vec2::new(a.cos(), a.sin());
        let w = mix(0.7);
        frame.hud_line2(p + d * disc, p + d * len, Color([w[0], w[1], w[2], (if i % 8 == 0 { 0.16 } else { 0.08 }) * visible]), Color([w[0], w[1], w[2], 0.0]));
    }
    // Lens ghosts: faint coloured discs on the line from the sun through
    // the middle of the view, stronger the nearer it is to the middle.
    let middle = size * 0.5;
    let off = middle - p;
    let near_middle = (1.0 - off.length() / size.length()).clamp(0.0, 1.0);
    for (f, rad, col) in [(0.35f32, 9.0f32, [1.0f32, 0.75, 0.35]), (0.7, 20.0, [0.35, 0.9, 0.6]), (1.15, 13.0, [0.35, 0.55, 1.0]), (1.6, 34.0, [0.7, 0.45, 1.0])] {
        let at = p + off * (1.0 + f);
        let a = 0.07 * near_middle * visible * (0.5 + 0.1 * k).min(1.5);
        frame.hud_glow(at, rad, 24, Color([col[0], col[1], col[2], a]), Color([col[0], col[1], col[2], a * 0.4]));
    }
}

/// Defence turrets: a small marker on each in view within 60 km (SAM by it
/// when close), and, while we're aggressed, the reach of their guns as red rings.
pub(super) fn turret_markers(frame: &mut Frame, app: &App) {
    let me = app.view.ship_pos;
    let hunted = app.v.aggressed_until.is_some();
    let cam = frame.camera.position;
    let size = frame.size();
    for (turret, at) in &app.turrets {
        let d = at.distance(me);
        if d > 60_000.0 {
            continue;
        }
        let c = if hunted { RED } else { Color::hex(0xff9040) };
        if let Some(p) = frame.project(*at).filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y) {
            frame.hud_line(p + Vec2::new(-4.0, 3.0), p + Vec2::new(4.0, 3.0), c);
            frame.hud_line(p + Vec2::new(4.0, 3.0), p + Vec2::new(0.0, -4.0), c);
            frame.hud_line(p + Vec2::new(0.0, -4.0), p + Vec2::new(-4.0, 3.0), c);
            if d < 15_000.0 {
                frame.text(p + Vec2::new(6.0, -4.0), "SAM", c.scale(0.8));
            }
        }
        if hunted {
            // The reach, as a ring seen face on.
            let to = (*at - cam).normalize();
            let (u, v) = (to.any_orthonormal_vector(), to.cross(to.any_orthonormal_vector()));
            let pts: Vec<Option<Vec2>> = (0..=32)
                .map(|i| {
                    let a = i as f64 / 32.0 * std::f64::consts::TAU;
                    frame.project(*at + (u * a.cos() + v * a.sin()) * turret.range())
                })
                .collect();
            for w in pts.windows(2) {
                if let (Some(a), Some(b)) = (w[0], w[1]) {
                    frame.hud_line(a, b, RED.scale(0.5));
                }
            }
        }
    }
}
