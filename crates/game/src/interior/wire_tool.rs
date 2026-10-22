//! Player-authored routes. Registry ports are the only snap targets.
use super::*;
use serde_json::Value;
use universe_ship_design::{Wire, WireEnd, WirePin};
mod nets;

#[derive(Default)]
pub(super) struct Editor {
    start: Option<WireEnd>,
    points: Vec<[f64; 3]>,
    cable: usize,
}
impl Editor {
    pub fn active(&self) -> bool {
        self.start.is_some()
    }
    pub fn cancel(&mut self) {
        self.start = None;
        self.points.clear();
    }
}
#[derive(Clone)]
struct Port {
    end: WireEnd,
    at: Vec3,
    data: Value,
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn vector(v: &Value) -> Option<Vec3> {
    let a = Vec3::new(
        v[0].as_f64()? as f32,
        v[1].as_f64()? as f32,
        v[2].as_f64()? as f32,
    );
    a.is_finite().then_some(a)
}
// Read the same verified manifest centre as the renderer, without loading meshes
// or making endpoint positions depend on asynchronous model readiness.
fn centre(key: &str, record: &Value) -> Option<Vec3> {
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec3>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    *cache.entry(key.into()).or_insert_with(|| {
        use universe_sim::world::{assets, worlds::sha256};
        if let Some(v) = assets::visual(key) {
            let bytes = std::fs::read(assets::store()?.join(&v.path).join("manifest.json")).ok()?;
            if sha256(&bytes) != v.manifest_sha256 {
                return None;
            }
            let manifest: Value = serde_json::from_slice(&bytes).ok()?;
            Some((vector(&manifest["bounds_m"][0])? + vector(&manifest["bounds_m"][1])?) * 0.5)
        } else {
            Some(Vec3::Y * (record["physical"]["height"].as_f64()? as f32 * 0.5))
        }
    })
}
fn ports(i: &Interior) -> Vec<Port> {
    ports_for(&i.plan, &i.fit)
}
fn ports_for(plan: &Plan, fit: &[Fitted]) -> Vec<Port> {
    let reg = universe_sim::world::registry::registry();
    let mut out = Vec::new();
    for b in &plan.blocks {
        let Some(f) = fit.iter().find(|f| f.id == kind(&b.id)) else {
            continue;
        };
        let Some(e) = reg
            .equipment(&f.key)
            .and_then(|e| serde_json::to_value(e).ok())
        else {
            continue;
        };
        let Some(list) = e["ports"].as_array() else {
            continue;
        };
        let Some(c) = centre(&f.key, &e) else {
            continue;
        };
        let rotation = compile::orientation(b, f);
        for p in list
            .iter()
            .filter(|p| matches!(s(p, "medium"), "power" | "signal"))
        {
            let Some(at) = vector(&p["at"]) else { continue };
            if s(p, "connector").is_empty() {
                continue;
            }
            out.push(Port {
                end: WireEnd {
                    module: ship_code::named(plan, "module", b.uid),
                    port: s(p, "id").into(),
                    connector: s(p, "connector").into(),
                },
                at: b.at + rotation * (Vec3::new(at.x, at.z, -at.y) - c),
                data: p.clone(),
            });
        }
    }
    out
}
fn same(a: &WireEnd, b: &WireEnd) -> bool {
    a.module == b.module && a.port == b.port
}
fn occupied(i: &Interior, e: &WireEnd) -> bool {
    i.plan
        .wiring
        .as_ref()
        .is_some_and(|w| w.values().any(|w| same(e, &w.from) || same(e, &w.to)))
}
fn hovered<'a>(ports: &'a [Port], cam: &Camera, q: Vec2) -> Option<&'a Port> {
    ports
        .iter()
        .filter_map(|p| cam.project(p.at).map(|(xy, z)| (p, xy.distance(q), z)))
        .filter(|(_, d, _)| *d < 10.0)
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|x| x.0)
}
fn cables() -> Vec<Value> {
    universe_sim::world::registry::registry()
        .runs
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .filter(|r| {
            s(r, "kind") == "cable" && r["conductors"].as_array().is_some_and(|x| !x.is_empty())
        })
        .collect()
}
/// The cables a wire may be laid with here: of the registry's cables, those it
/// lays aboard a ship (a ship hold spacing: rule.run-support), and, once a port
/// is picked, those that can mate it.
fn available(i: &Interior) -> Vec<Value> {
    let mut list = cables();
    list.retain(|c| c["hold"]["ship"].as_f64().is_some());
    if let Some(a) = i
        .wire_editor
        .start
        .as_ref()
        .and_then(|end| ports(i).into_iter().find(|p| same(&p.end, end)))
    {
        let mut other = a.clone();
        other.end.module.push_str("_candidate");
        other.data["flow"] = serde_json::json!(if s(&a.data, "flow") == "out" {
            "in"
        } else {
            "out"
        });
        list.retain(|c| compatible(&a, &other, c).is_ok());
    }
    list.sort_by(|a, b| {
        a["per_m"]["mass"]
            .as_f64()
            .unwrap_or(0.)
            .total_cmp(&b["per_m"]["mass"].as_f64().unwrap_or(0.))
    });
    list
}
fn compatible(a: &Port, b: &Port, cable: &Value) -> Result<Vec<WirePin>, String> {
    if same(&a.end, &b.end) {
        return Err("Choose a different port".into());
    }
    if s(&a.data, "medium") != s(&b.data, "medium") || s(cable, "carries") != s(&a.data, "medium") {
        return Err("Cable and ports carry different services".into());
    }
    if s(&a.data, "channel").is_empty() || s(&a.data, "channel") != s(&b.data, "channel") {
        return Err("Ports use different channels".into());
    }
    if s(&a.data, "key") != s(&b.data, "key") {
        return Err("Port keys do not match".into());
    }
    let flow = (s(&a.data, "flow"), s(&b.data, "flow"));
    if flow == ("in", "in") || flow == ("out", "out") {
        return Err("Port directions cannot connect".into());
    }
    let reg = universe_sim::world::registry::registry();
    for p in [a, b] {
        let connector = reg
            .connector(&p.end.connector)
            .and_then(|r| serde_json::to_value(r).ok())
            .ok_or("Unknown connector")?;
        if s(&connector, "interface") != s(&p.data, "interface")
            || s(&connector, "provides") != "both"
        {
            return Err("Connector does not mate with this port".into());
        }
        if !matches!(s(&p.data, "half"), "male" | "female") {
            return Err("Port mating half missing".into());
        }
        if let Some(v) = p.data["voltage"].as_f64() {
            if cable["rating"]["voltage"].as_f64().unwrap_or(0.0) < v
                || connector["rating"]["voltage"].as_f64().unwrap_or(0.0) < v
            {
                return Err("Cable or connector voltage rating is too low".into());
            }
        }
        // (Each sized conductor within the contacts' size; a screen has no
        // cross-section in its record, so nothing to check.)
        for c in cable["conductors"]
            .as_array()
            .ok_or("Cable has no conductors")?
        {
            if c["size"]
                .as_f64()
                .is_some_and(|size| size > connector["size"].as_f64().unwrap_or(0.0) * 1.000001)
            {
                return Err("Cable conductor does not fit the connector".into());
            }
        }
    }
    let ap = a.data["pins"].as_array().ok_or("Source contacts missing")?;
    let bp = b.data["pins"]
        .as_array()
        .ok_or("Destination contacts missing")?;
    let mut result = Vec::new();
    for c in cable["conductors"]
        .as_array()
        .ok_or("Cable conductors missing")?
    {
        let role = s(c, "role");
        let matches = |p: &&Value| {
            s(p, "role") == role || (role == "line" && matches!(s(p, "role"), "plus" | "minus"))
        };
        let aa: Vec<_> = ap.iter().filter(matches).collect();
        let bb: Vec<_> = bp.iter().filter(matches).collect();
        if aa.len() != 1
            || bb.len() != 1
            || s(aa[0], "role") != s(bb[0], "role")
            || s(aa[0], "line") != s(bb[0], "line")
        {
            return Err("Cable contacts cannot be mapped unambiguously".into());
        }
        result.push(WirePin {
            conductor: s(c, "id").into(),
            from: s(aa[0], "contact").into(),
            to: s(bb[0], "contact").into(),
        });
    }
    for (pins, from) in [(ap, true), (bp, false)] {
        if pins.iter().any(|p| {
            s(p, "role") != "spare"
                && !result
                    .iter()
                    .any(|r| (if from { &r.from } else { &r.to }) == s(p, "contact"))
        }) {
            return Err("Cable lacks a required conductor".into());
        }
    }
    Ok(result)
}
pub(super) fn click(i: &mut Interior, cam: &Camera, plane: f32) {
    let all = ports(i);
    if let Some(p) = hovered(&all, cam, i.cursor) {
        if occupied(i, &p.end) {
            i.note("PORT ALREADY CONNECTED — DELETE ITS WIRE FIRST");
            return;
        }
        if let Some(start) = &i.wire_editor.start {
            let Some(a) = all.iter().find(|p| same(&p.end, start)) else {
                i.wire_editor.cancel();
                return;
            };
            let list = available(i);
            let Some(c) = list.get(i.wire_editor.cable % list.len().max(1)) else {
                i.note("NO CABLE RECORDS AVAILABLE");
                return;
            };
            match compatible(a, p, c) {
                Err(e) => i.note(&e),
                Ok(pins) => {
                    let w = Wire {
                        from: a.end.clone(),
                        to: p.end.clone(),
                        cable: s(&c["identity"], "key").into(),
                        waypoints_m: i.wire_editor.points.clone(),
                        pins,
                    };
                    let points = route(&w, &all).unwrap();
                    if points.windows(2).any(|p| p[0].distance(p[1]) <= 1e-6) {
                        i.note(
                            "ROUTE HAS A ZERO-LENGTH SEGMENT — BACKSPACE TO REMOVE ITS WAYPOINT",
                        );
                        return;
                    }
                    let sup = match support(&points, c) {
                        Err(e) => {
                            i.note(&e.to_uppercase());
                            return;
                        }
                        Ok(sup) => sup,
                    };
                    if let Some(k) = sup.tight.first() {
                        i.note(&format!(
                            "BEND AT WAYPOINT {k} TOO TIGHT FOR {:.3} M RADIUS — BACKSPACE TO MOVE IT",
                            sup.radius
                        ));
                        return;
                    }
                    let carried = match load(a, p, c, length(&points)) {
                        Err(e) => {
                            i.note(&e.to_uppercase());
                            return;
                        }
                        Ok(l) => l,
                    };
                    let map = i.plan.wiring.get_or_insert_with(Default::default);
                    let mut n = 1;
                    while map.contains_key(&format!("wire_{n}")) {
                        n += 1;
                    }
                    map.insert(format!("wire_{n}"), w);
                    i.wire_editor.cancel();
                    let carried = carried.map_or(String::new(), |l| {
                        format!(
                            " — {:.1} A OF {:.0} A IN VACUUM, DROP {:.1}%",
                            l.amps,
                            l.rating,
                            l.drop * 100.0
                        )
                    });
                    i.note(&format!(
                        "WIRE LAID — NEEDS {} X {} EVERY {:.1} M (NOT PLACED){carried}",
                        sup.stations.len(),
                        sup.clip.trim_start_matches("connector.").to_uppercase(),
                        sup.spacing
                    ));
                }
            }
        } else {
            i.wire_editor.start = Some(p.end.clone());
            i.wire_editor.cable = 0;
            i.wire_editor.points.clear();
        }
    } else if i.wire_editor.active() {
        if let Some(at) = on_plane(i, cam, plane, i.cursor, false) {
            let p = at.as_dvec3().to_array();
            if i.wire_editor.points.last() != Some(&p) {
                i.wire_editor.points.push(p);
            }
        }
    } else {
        i.note("CLICK A PORT MARKER TO START A WIRE");
    }
}
fn route(w: &Wire, ports: &[Port]) -> Option<Vec<Vec3>> {
    let a = ports.iter().find(|p| same(&p.end, &w.from))?;
    let b = ports.iter().find(|p| same(&p.end, &w.to))?;
    Some(
        std::iter::once(a.at)
            .chain(
                w.waypoints_m
                    .iter()
                    .map(|p| Vec3::from_array(p.map(|v| v as f32))),
            )
            .chain(std::iter::once(b.at))
            .collect(),
    )
}
/// How a route is held and bent, by its cable's record: the run clip and the
/// most spacing between clips aboard a ship (`hold`, rule.run-support), the
/// stations along the route that spacing calls for (one at each end, the rest
/// no farther apart than it: the fewest, none of them placed), the least
/// radius its corners turn at (`elbow.radius`) and the waypoints whose corners
/// have not the straight that radius takes.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Support {
    pub clip: String,
    pub spacing: f64,
    pub stations: Vec<Vec3>,
    pub radius: f64,
    pub tight: Vec<usize>,
}
fn support(path: &[Vec3], cable: &Value) -> Result<Support, String> {
    let h = &cable["hold"];
    let spacing = h["ship"]
        .as_f64()
        .filter(|s| *s > 0.0)
        .ok_or("Cable is not laid aboard a ship: no ship hold spacing")?;
    let clip = s(h, "clip");
    if clip.is_empty() {
        return Err("Cable names no run clip".into());
    }
    let size = universe_sim::world::registry::registry()
        .connector(clip)
        .map(|c| c.size)
        .ok_or("Run clip unknown")?;
    let outside = cable["outside"]
        .as_array()
        .map(|o| o.iter().filter_map(Value::as_f64).fold(0.0, f64::max))
        .unwrap_or(f64::INFINITY);
    if outside > size * 1.000001 {
        return Err("Cable does not fit its run clip".into());
    }
    let radius = cable["elbow"]["radius"]
        .as_f64()
        .filter(|r| *r > 0.0)
        .ok_or("Cable has no least bend radius")?;
    Ok(Support { clip: clip.into(), spacing, stations: stations(path, spacing), radius, tight: tight(path, radius) })
}
/// What a power wire must carry and what carries it. Its duty is the greater
/// of its two ports' currents: the load's at its rating and the feed's (a
/// board's way is its breaker, and a cable is protected by what feeds it). Its
/// cable's rating is the one in vacuum (`rating.current`: radiation alone
/// cools it; a bay is not yet said to hold air, and bundling is not counted),
/// each end connector's is its own, and out and back at that current it may
/// lose no more than `wiring::MAX_DROP` of its voltage (the studio's limit, an
/// electrical convention, not a registry rule). A signal wire states no
/// current: nothing to check.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Load {
    pub amps: f64,
    pub volts: f64,
    /// W the ports state, the greater.
    pub watts: f64,
    /// A the cable carries in vacuum, and in free air where its record says.
    pub rating: f64,
    pub air: Option<f64>,
    /// The share of its voltage lost out and back.
    pub drop: f64,
}
fn load(a: &Port, b: &Port, cable: &Value, length: f64) -> Result<Option<Load>, String> {
    let amps = [a, b].iter().filter_map(|p| p.data["current"].as_f64()).fold(0.0, f64::max);
    if s(&a.data, "medium") != "power" || amps <= 0.0 {
        return Ok(None);
    }
    let volts = a.data["voltage"]
        .as_f64()
        .or(b.data["voltage"].as_f64())
        .filter(|v| *v > 0.0)
        .ok_or("Power port states no voltage")?;
    let watts = [a, b].iter().filter_map(|p| p.data["power"].as_f64()).fold(0.0, f64::max);
    let rating = cable["rating"]["current"].as_f64().ok_or("Cable has no current rating")?;
    let air = cable["rating"]["current_air"].as_f64();
    if rating < amps {
        return Err(format!(
            "Ports need {amps:.1} A, cable carries {rating:.0} A in vacuum{} — choose a heavier cable (N)",
            air.map_or(String::new(), |a| format!(" ({a:.0} A in air)"))
        ));
    }
    let reg = universe_sim::world::registry::registry();
    for p in [a, b] {
        let r = reg
            .connector(&p.end.connector)
            .and_then(|c| c.rating.current)
            .ok_or("Connector has no current rating")?;
        if r < amps {
            return Err(format!(
                "Ports need {amps:.1} A, connector {} carries {r:.0} A",
                p.end.connector.trim_start_matches("connector.")
            ));
        }
    }
    // (A pole's cross-section: the conductor that takes plus, minus or line,
    // else the cable's size; its conductivity from its first material's record.)
    let area = cable["conductors"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| matches!(s(c, "role"), "plus" | "minus" | "line"))
        .and_then(|c| c["size"].as_f64())
        .or(cable["size"].as_f64())
        .filter(|a| *a > 0.0)
        .ok_or("Cable conductor has no cross-section")?;
    let sigma = cable["per_m"]["built_of"][0]["item"]
        .as_str()
        .and_then(|m| reg.material(m))
        .and_then(|m| m.electrical_magnetic.electrical_conductivity)
        .unwrap_or(1.0 / wiring::COPPER);
    let drop = 2.0 * length * amps / (sigma * area) / volts;
    if drop > wiring::MAX_DROP {
        return Err(format!(
            "Drops {:.1}% of {volts:.0} V over {length:.1} m, the studio allows {:.0}% — a shorter way or a heavier cable (N)",
            drop * 100.0,
            wiring::MAX_DROP * 100.0
        ));
    }
    Ok(Some(Load { amps, volts, watts, rating, air, drop }))
}
/// m along a route.
fn length(path: &[Vec3]) -> f64 {
    path.windows(2).map(|p| p[0].distance(p[1]) as f64).sum()
}
/// The point `d` m along a route.
fn along(path: &[Vec3], mut d: f32) -> Vec3 {
    for p in path.windows(2) {
        let l = p[0].distance(p[1]);
        if d <= l {
            return p[0].lerp(p[1], if l > 0.0 { d / l } else { 0.0 });
        }
        d -= l;
    }
    *path.last().unwrap_or(&Vec3::ZERO)
}
/// Where a route needs holding at `spacing` at most: both ends and the fewest
/// between them, evenly along it.
fn stations(path: &[Vec3], spacing: f64) -> Vec<Vec3> {
    let length: f32 = path.windows(2).map(|p| p[0].distance(p[1])).sum();
    let n = ((f64::from(length) / spacing).ceil() as usize).max(1);
    (0..=n).map(|k| along(path, length * k as f32 / n as f32)).collect()
}
/// The waypoints (1-based, as the route's vertices) whose corners have not the
/// straight their bend takes: a corner turning `theta` at radius `radius` uses
/// radius * tan(theta / 2) of each leg, and a leg must hold both its corners'.
fn tight(path: &[Vec3], radius: f64) -> Vec<usize> {
    let tangent: Vec<f32> = (0..path.len())
        .map(|k| {
            if k == 0 || k + 1 == path.len() {
                return 0.0;
            }
            let (a, b) = (path[k] - path[k - 1], path[k + 1] - path[k]);
            if a.length() <= 1e-6 || b.length() <= 1e-6 {
                return 0.0;
            }
            let theta = a.angle_between(b);
            if theta < 1e-3 {
                return 0.0;
            }
            if theta > std::f32::consts::PI - 1e-3 {
                return f32::INFINITY;
            }
            radius as f32 * (theta / 2.0).tan()
        })
        .collect();
    let mut out: Vec<usize> = Vec::new();
    for (i, p) in path.windows(2).enumerate() {
        if p[0].distance(p[1]) < tangent[i] + tangent[i + 1] {
            for k in [i, i + 1] {
                if tangent[k] > 0.0 && !out.contains(&k) {
                    out.push(k);
                }
            }
        }
    }
    out.sort_unstable();
    out
}
pub(super) fn remove(i: &mut Interior, cam: &Camera) {
    let all = ports(i);
    let hit = i.plan.wiring.as_ref().and_then(|map| {
        map.iter()
            .filter_map(|(key, w)| {
                let points = route(w, &all)?;
                let d = points
                    .windows(2)
                    .filter_map(|p| {
                        let a = cam.project(p[0])?.0;
                        let b = cam.project(p[1])?.0;
                        let t = ((i.cursor - a).dot(b - a) / (b - a).length_squared().max(0.001))
                            .clamp(0.0, 1.0);
                        Some(i.cursor.distance(a + (b - a) * t))
                    })
                    .min_by(f32::total_cmp)?;
                (d < 8.0).then_some((key.clone(), d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    });
    if let Some(key) = hit {
        i.plan.wiring.as_mut().unwrap().remove(&key);
        i.note("WIRE REMOVED");
    }
}
pub(super) fn next(i: &mut Interior) {
    i.wire_editor.cable += 1;
}
pub(super) fn back(i: &mut Interior) {
    i.wire_editor.points.pop();
}
pub(super) fn draw(frame: &mut Frame, i: &Interior, cam: &Camera, plane: f32) {
    let all = ports(i);
    let reg = universe_sim::world::registry::registry();
    if let Some(wires) = &i.plan.wiring {
        for w in wires.values() {
            if let Some(path) = route(w, &all) {
                for s in path.windows(2) {
                    draw_line(frame, cam, s[0], s[1], PICKED);
                }
                // (Where it must be clipped: wanted, none placed.)
                if let Some(sup) = reg
                    .runs
                    .iter()
                    .find(|c| c.identity.key == w.cable)
                    .and_then(|c| serde_json::to_value(c).ok())
                    .and_then(|c| support(&path, &c).ok())
                {
                    draw_stations(frame, cam, &sup.stations, CLASH);
                }
            }
        }
    }
    if i.tool != Tool::Wire {
        return;
    }
    let list = available(i);
    let chosen = list.get(i.wire_editor.cable % list.len().max(1));
    let start = i
        .wire_editor
        .start
        .as_ref()
        .and_then(|e| all.iter().find(|p| same(e, &p.end)));
    for p in &all {
        if let Some((q, _)) = cam.project(p.at) {
            let col = if occupied(i, &p.end) {
                LABEL.scale(0.4)
            } else if start.is_some_and(|a| chosen.is_none_or(|c| compatible(a, p, c).is_err())) {
                CLASH
            } else {
                PICKED
            };
            frame.hud_box(q - Vec2::splat(4.0), Vec2::splat(8.0), col);
        }
    }
    let hover = hovered(&all, cam, i.cursor);
    if let Some(p) = hover {
        frame.text_scaled(
            i.cursor + Vec2::new(12., 12.),
            &format!(
                "{} / {}  {}  {}",
                p.end.module,
                p.end.port,
                s(&p.data, "channel"),
                s(&p.data, "flow")
            ),
            PICKED,
            0.7,
        );
        frame.text_scaled(
            i.cursor + Vec2::new(12., 26.),
            &p.end.connector,
            LABEL,
            0.65,
        );
        if let Some(a) = start
            && let Some(c) = chosen
            && let Err(e) = compatible(a, p, c)
        {
            frame.text_scaled(i.cursor + Vec2::new(12., 40.), &e, CLASH, 0.65);
        }
    }
    if let Some(a) = i
        .wire_editor
        .start
        .as_ref()
        .and_then(|e| all.iter().find(|p| same(&p.end, e)))
    {
        let mut points = vec![a.at];
        points.extend(
            i.wire_editor
                .points
                .iter()
                .map(|p| Vec3::from_array(p.map(|v| v as f32))),
        );
        if let Some(at) = hover
            .map(|p| p.at)
            .or_else(|| on_plane(i, cam, plane, i.cursor, false))
        {
            points.push(at);
        }
        for s in points.windows(2) {
            draw_line(frame, cam, s[0], s[1], PICKED);
        }
        if let Some(sup) = chosen.and_then(|c| support(&points, c).ok()) {
            draw_stations(frame, cam, &sup.stations, LABEL);
            for &k in &sup.tight {
                if let Some((q, _)) = cam.project(points[k]) {
                    frame.hud_box(q - Vec2::splat(6.0), Vec2::splat(12.0), CLASH);
                }
            }
        }
    }
}
fn draw_stations(frame: &mut Frame, cam: &Camera, at: &[Vec3], col: Color) {
    for p in at {
        if let Some((q, _)) = cam.project(*p) {
            frame.hud_box(q - Vec2::splat(2.0), Vec2::splat(4.0), col);
        }
    }
}
fn cable_row(n: usize) -> Rect {
    (
        PANEL.0 + Vec2::new(8., 110. + n as f32 * 25.),
        Vec2::new(PANEL.1.x - 16., 24.),
    )
}
pub(super) fn panel_click(i: &mut Interior) {
    let list = available(i);
    let base = (i.wire_editor.cable % list.len().max(1)) / 6 * 6;
    if let Some(n) = (0..6).find(|&n| base + n < list.len() && inside(cable_row(n), i.cursor)) {
        i.wire_editor.cable = base + n;
    }
}
pub(super) fn panel(frame: &mut Frame, i: &Interior) {
    let list = available(i);
    let selected = i.wire_editor.cable % list.len().max(1);
    let base = selected / 6 * 6;
    for (n, c) in list.iter().skip(base).take(6).enumerate() {
        let (at, size) = cable_row(n);
        let col = if base + n == selected { PICKED } else { LABEL };
        frame.hud_box(at, size, col.scale(0.5));
        let key = s(&c["identity"], "key").trim_start_matches("run.");
        for (line, text) in crate::fmt::wrap(key, 40).into_iter().take(2).enumerate() {
            frame.text_scaled(at + Vec2::new(3., 3. + line as f32 * 10.), &text, col, 0.6);
        }
    }
    let label = if list.is_empty() {
        "NO COMPATIBLE CABLES".into()
    } else {
        format!("CABLE {}/{} — N: NEXT", selected + 1, list.len())
    };
    frame.text_scaled(PANEL.0 + Vec2::new(8., 96.), &label, LABEL, 0.65);
    if let Some(c) = list.get(selected) {
        let hold = match (c["hold"]["clip"].as_str(), c["hold"]["ship"].as_f64(), c["elbow"]["radius"].as_f64()) {
            (Some(clip), Some(every), Some(r)) => format!(
                "{} EVERY {every:.1} M · BEND R {r:.3} M",
                clip.trim_start_matches("connector.").to_uppercase()
            ),
            _ => "NOT LAID ABOARD A SHIP".into(),
        };
        frame.text_scaled(PANEL.0 + Vec2::new(8., 262.), &hold, LABEL, 0.6);
        let rating = match (c["rating"]["current"].as_f64(), c["rating"]["current_air"].as_f64(), c["rating"]["voltage"].as_f64()) {
            (Some(a), air, Some(v)) => format!(
                "CARRIES {a:.0} A IN VACUUM{} · {v:.0} V",
                air.map_or(String::new(), |x| format!(", {x:.0} A IN AIR"))
            ),
            _ => "NO CURRENT RATING".into(),
        };
        frame.text_scaled(PANEL.0 + Vec2::new(8., 276.), &rating, LABEL, 0.6);
    }
    for (n, text) in [
        "CLICK ROW TO SELECT CABLE",
        "BACKSPACE: LAST WAYPOINT",
        "ESC / RIGHT CLICK: CANCEL",
        "DEL OVER WIRE: REMOVE",
        "SAVE AND UNDO AS USUAL",
    ]
    .iter()
    .enumerate()
    {
        frame.text_scaled(
            PANEL.0 + Vec2::new(8., 290. + n as f32 * 13.),
            text,
            LABEL,
            0.65,
        );
    }
    if ports(i).is_empty() {
        frame.text_scaled(
            PANEL.0 + Vec2::new(8., 356.),
            "NO RESOLVED PORTS IN THIS DESIGN",
            CLASH,
            0.6,
        );
    }
}

fn draw_line(frame: &mut Frame, cam: &Camera, a: Vec3, b: Vec3, col: Color) {
    if let (Some((a, _)), Some((b, _))) = (cam.project(a), cam.project(b)) {
        frame.hud_line(a, b, col);
    }
}

/// Explicit wires replace the inferred harness, including its mass accounting.
/// Each is checked for its ends (ports, connectors, contacts) and its hold (the
/// clips its cable calls for, which nothing places yet; its bends), current
/// rating and drop. Static contact nets do not yet establish runtime power.
pub(super) fn harness(plan: &Plan, fit: &[Fitted]) -> wiring::Harness {
    let mut h = wiring::Harness::default();
    let all = ports_for(plan, fit);
    let reg = universe_sim::world::registry::registry();
    for (id, w) in plan.wiring.iter().flat_map(|m| m.iter()) {
        let Some(path) = route(w, &all) else {
            h.faults.push(format!("WIRE {id}: ENDPOINT UNRESOLVED"));
            continue;
        };
        let Some(c) = reg
            .runs
            .iter()
            .find(|c| c.identity.key == w.cable)
            .and_then(|c| serde_json::to_value(c).ok())
        else {
            h.faults.push(format!("WIRE {id}: CABLE UNKNOWN"));
            continue;
        };
        let a = all.iter().find(|p| same(&p.end, &w.from)).unwrap();
        let b = all.iter().find(|p| same(&p.end, &w.to)).unwrap();
        match compatible(a, b, &c) {
            Ok(pins)
                if pins == w.pins
                    && a.end.connector == w.from.connector
                    && b.end.connector == w.to.connector => {}
            Ok(_) => h.faults.push(format!(
                "WIRE {id}: CONTACT MAP OR CONNECTOR DOES NOT MATCH PORTS"
            )),
            Err(e) => h.faults.push(format!("WIRE {id}: {e}")),
        }
        // (Its hold: the clips it needs along its way, none of them placed yet, and
        // the corners too tight for its cable.)
        match support(&path, &c) {
            Err(e) => h.faults.push(format!("WIRE {id}: {e}")),
            Ok(sup) => {
                if !sup.tight.is_empty() {
                    h.faults.push(format!(
                        "WIRE {id}: BEND AT WAYPOINT {} TOO TIGHT FOR {:.3} M RADIUS",
                        sup.tight.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", "),
                        sup.radius
                    ));
                }
                h.faults.push(format!(
                    "WIRE {id}: NEEDS {} X {} EVERY {:.1} M, NOT PLACED",
                    sup.stations.len(),
                    sup.clip.trim_start_matches("connector."),
                    sup.spacing
                ));
            }
        }
        let length = length(&path);
        let (watts, drop) = match load(a, b, &c, length) {
            Err(e) => {
                h.faults.push(format!("WIRE {id}: {e}"));
                (0.0, 0.0)
            }
            Ok(None) => (0.0, 0.0),
            Ok(Some(l)) => (l.watts, l.drop),
        };
        let ends: f64 = [&w.from, &w.to]
            .iter()
            .filter_map(|e| reg.connector(&e.connector)?.physical.mass)
            .sum();
        let mass = c["per_m"]["mass"].as_f64().unwrap_or(0.) * length + ends;
        h.runs.push(wiring::WireRun {
            module: w.to.module.clone(),
            volts: a.data["voltage"].as_f64().unwrap_or(0.),
            watts,
            cable: Some(w.cable.clone()),
            pairs: 1,
            path,
            length,
            mass,
            drop,
        });
    }
    let mut records = std::collections::BTreeMap::new();
    for b in &plan.blocks {
        let name = ship_code::named(plan, "module", b.uid);
        match fit
            .iter()
            .find(|f| f.id == kind(&b.id))
            .and_then(|f| reg.equipment(&f.key))
            .and_then(|e| serde_json::to_value(e).ok())
        {
            Some(record) => {
                records.insert(name, record);
            }
            None => h.faults.push(format!("MODULE {name}: EQUIPMENT RECORD UNRESOLVED")),
        }
    }
    let empty = std::collections::BTreeMap::new();
    let topology = match nets::compile(&records, plan.wiring.as_ref().unwrap_or(&empty)) {
        Ok(topology) => {
            h.faults.extend(topology.pending());
            format!("{} CONTACT NETS", topology.nets.len())
        }
        Err(faults) => {
            h.faults.extend(faults);
            "CONTACT TOPOLOGY INVALID".into()
        }
    };
    h.faults.push(format!(
        "AUTHORED WIRING: {topology}; RATED IN VACUUM, UNBUNDLED; RUNTIME POWER NOT YET VALIDATED"
    ));
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    #[ignore = "requires installed distribution-board and navigation model manifests"]
    fn registry_board_to_navigation_route() {
        let reg = universe_sim::world::registry::registry();
        let mut plan = Plan::default();
        let mut fit = Vec::new();
        for (uid, key) in [
            (1, "equipment.power.distribution.28v-16way"),
            (2, "equipment.avionics.nav-basic.s1"),
        ] {
            let f = fitted(reg.equipment(key).unwrap(), key).unwrap();
            plan.blocks.push(Block {
                uid: Id(uid),
                id: key.into(),
                at: Vec3::X * uid as f32 * 2.,
                size: f.size,
                push: None,
                rotation: None,
            });
            fit.push(f);
        }
        let ports = ports_for(&plan, &fit);
        let a = ports
            .iter()
            .find(|p| p.end.port == "way-1")
            .expect("board port geometry");
        let b = ports
            .iter()
            .find(|p| p.end.port == "dc-in")
            .expect("navigation port geometry");
        let cable = cables()
            .into_iter()
            .find(|c| s(&c["identity"], "key") == "run.cable-dc-600v-3x1mm2")
            .unwrap();
        let pins = compatible(a, b, &cable).expect("real registry contacts mate");
        assert_eq!(pins.len(), 3);
        let wire = Wire {
            from: a.end.clone(),
            to: b.end.clone(),
            cable: "run.cable-dc-600v-3x1mm2".into(),
            waypoints_m: vec![[0., 2., 0.]],
            pins,
        };
        plan.wiring = Some(std::collections::BTreeMap::from([("wire_1".into(), wire)]));
        let h = harness(&plan, &fit);
        assert_eq!(h.runs.len(), 1);
        assert!(h.runs[0].mass > 0. && h.runs[0].length > 0.);
        // (Its unplaced clips; the way's 15 A breaker above the 1 mm2 cable's
        // 6 A in vacuum; its unresolved breaker group; and runtime power.)
        assert_eq!(h.faults.len(), 4, "{:?}", h.faults);
        assert!(h.faults[0].starts_with("WIRE wire_1: NEEDS ") && h.faults[0].contains("run-clip-d10 EVERY 0.6 M"), "{:?}", h.faults);
        assert!(h.faults[1].contains("need 15.0 A, cable carries 6 A in vacuum (11 A in air)"), "{:?}", h.faults);
        assert!(h.faults[2].starts_with("SWITCH ") && h.faults[3].contains("CONTACT NETS"), "{:?}", h.faults);
        assert_eq!((h.runs[0].watts, h.runs[0].drop), (0.0, 0.0), "a refused wire carries nothing on the sheet");
        // (The registry's 2.5 mm2 three-core, the one sized to the shell: 12 A in
        // vacuum, still under the way's breaker; its way is said as much.)
        let wiring = plan.wiring.as_mut().unwrap();
        wiring.get_mut("wire_1").unwrap().cable = "run.cable-dc-600v-3x2p5mm2".into();
        let h = harness(&plan, &fit);
        assert!(h.faults.iter().any(|f| f.contains("need 15.0 A, cable carries 12 A in vacuum (21 A in air)")), "{:?}", h.faults);
    }
    #[test]
    fn a_wire_is_held_at_its_cable_spacing_and_bent_no_tighter_than_it_turns() {
        // (A 10 mm hook-up wire's figures: clipped every 0.6 m, turning at 38 mm.)
        let cable = json!({"outside":[0.004,0.004],"elbow":{"radius":0.038},"hold":{"clip":"connector.run-clip-d10","ship":0.6,"plant":1.5}});
        let path = [Vec3::ZERO, Vec3::X * 2.0, Vec3::new(2.0, 1.0, 0.0)];
        let sup = support(&path, &cable).expect("laid aboard, fits its clip, has a radius");
        assert_eq!((sup.clip.as_str(), sup.spacing, sup.radius), ("connector.run-clip-d10", 0.6, 0.038));
        // (3 m at 0.6 m: five stretches, six stations, the ends among them.)
        assert_eq!(sup.stations.len(), 6);
        assert_eq!(sup.stations[0], Vec3::ZERO);
        assert_eq!(*sup.stations.last().unwrap(), Vec3::new(2.0, 1.0, 0.0));
        assert!((sup.stations[1] - Vec3::X * 0.6).length() < 1e-5 && (sup.stations[4] - Vec3::new(2.0, 0.4, 0.0)).length() < 1e-5);
        assert!(sup.tight.is_empty(), "a right angle at 38 mm wants 38 mm of each leg");
        // (A leg too short for its corners' tangents: 10 mm between two right angles.)
        let hairpin = [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 0.01, 0.0), Vec3::new(0.0, 0.01, 0.0)];
        assert_eq!(tight(&hairpin, 0.038), vec![1, 2]);
        // (A straight run has no corner to speak of; a reversal has no radius.)
        assert!(tight(&[Vec3::ZERO, Vec3::X, Vec3::X * 2.0], 0.038).is_empty());
        assert_eq!(tight(&[Vec3::ZERO, Vec3::X, Vec3::ZERO], 0.038), vec![1]);
        // (The registry's rules: no ship spacing is not laid aboard; a cable too
        // big for its clip is a registry fault; no radius, no bend check.)
        let mut plant = cable.clone();
        plant["hold"].as_object_mut().unwrap().remove("ship");
        assert!(support(&path, &plant).is_err());
        let mut fat = cable.clone();
        fat["outside"] = json!([0.02, 0.02]);
        assert!(support(&path, &fat).unwrap_err().contains("fit"));
        let mut stiff = cable.clone();
        stiff["elbow"] = json!({});
        assert!(support(&path, &stiff).unwrap_err().contains("radius"));
    }
    #[test]
    fn a_power_wire_is_rated_for_the_greater_of_its_ports_and_loses_little() {
        let make = |name: &str, flow: &str, amps: f64, watts: f64| Port {
            end: WireEnd {
                module: name.into(),
                port: "dc".into(),
                connector: "connector.circular-shell-17".into(),
            },
            at: Vec3::ZERO,
            data: json!({"medium":"power","channel":"dc-28v","flow":flow,"voltage":28,"current":amps,"power":watts}),
        };
        // (A board's 15 A way to a 3.57 A load: the way's breaker sets the duty.)
        let way = make("board", "out", 15.0, 420.0);
        let nav = make("nav", "in", 3.57, 100.0);
        let cable = json!({"carries":"power","size":2.5e-6,"rating":{"voltage":600,"current":15,"current_air":30},
            "conductors":[{"id":"plus","role":"plus","size":2.5e-6},{"id":"minus","role":"minus","size":2.5e-6},{"id":"earth","role":"earth","size":2.5e-6}],
            "per_m":{"mass":0.1,"built_of":[{"what":"Conductors","item":"material.copper","mass":0.07}]}});
        let l = load(&way, &nav, &cable, 4.0).unwrap().unwrap();
        assert_eq!((l.amps, l.volts, l.watts, l.rating, l.air), (15.0, 28.0, 420.0, 15.0, Some(30.0)));
        // (2 x 4 m of 2.5 mm2 copper at 15 A: 0.054 ohm, 0.81 V of 28.)
        assert!((l.drop - 2.0 * 4.0 * 15.0 / (59e6 * 2.5e-6) / 28.0).abs() < 1e-9 && l.drop < 0.03, "{l:?}");
        let mut thin = cable.clone();
        thin["rating"]["current"] = json!(12);
        assert!(load(&way, &nav, &thin, 4.0).unwrap_err().contains("12 A in vacuum (30 A in air)"));
        // (Long enough, the same wire drops too much: 25 m is 5.1 V of 28.)
        assert!(load(&way, &nav, &cable, 25.0).unwrap_err().starts_with("Drops 18"));
        // (A port above the connector's 15 A.)
        let big = make("heater", "in", 20.0, 560.0);
        let mut fat = cable.clone();
        fat["rating"]["current"] = json!(28);
        assert!(load(&way, &big, &fat, 1.0).unwrap_err().contains("connector circular-shell-17 carries 15 A"));
        // (Two batteries' ports both feeding and fed take the greater; a signal wire has no current.)
        let (p, q) = (make("battery", "both", 333.33, 0.0), make("board", "both", 400.0, 11200.0));
        assert!(load(&p, &q, &cable, 1.0).unwrap_err().starts_with("Ports need 400.0 A"));
        let mut can = make("nav", "both", 0.0, 0.0);
        can.data = json!({"medium":"signal","channel":"can-a","flow":"both"});
        assert_eq!(load(&can, &can, &cable, 1.0), Ok(None));
    }
    #[test]
    fn player_wires_require_matching_ports_and_keep_their_route() {
        let make = |name: &str, x: f32, flow: &str| Port {
            end: WireEnd {
                module: name.into(),
                port: "dc".into(),
                connector: "connector.circular-shell-17".into(),
            },
            at: Vec3::X * x,
            data: json!({"medium":"power","channel":"dc-28v","flow":flow,"interface":"interface.circular-shell-17","half":"male","voltage":28,
                "pins":[{"contact":"1","role":"plus"},{"contact":"2","role":"minus"},{"contact":"3","role":"earth"}]}),
        };
        let a = make("battery", 0., "out");
        let b = make("nav", 2., "in");
        let c = json!({"carries":"power","rating":{"voltage":600},"conductors":[{"id":"+","role":"plus","size":1e-6},{"id":"-","role":"minus","size":1e-6},{"id":"E","role":"earth","size":1e-6}]});
        let pins = compatible(&a, &b, &c)
            .expect("mating connector pair at each end, both equipment halves may be male");
        assert_eq!(
            pins.iter().map(|p| p.from.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
        let mut bad = b.clone();
        bad.data["channel"] = json!("dc-270v");
        assert!(compatible(&a, &bad, &c).is_err());
        bad = b.clone();
        bad.data["flow"] = json!("out");
        assert!(compatible(&a, &bad, &c).is_err());
        bad = b.clone();
        bad.data["pins"][0]["role"] = json!("minus");
        assert!(compatible(&a, &bad, &c).is_err());
        let mut big = c.clone();
        big["conductors"][0]["size"] = json!(1e-3);
        assert!(compatible(&a, &b, &big).is_err());
        let mut screened = c.clone();
        screened["conductors"][2] = json!({"id":"S","role":"earth"});
        assert!(compatible(&a, &b, &screened).is_ok(), "an unsized screen is not a conductor too big");
        let mut low = c.clone();
        low["rating"]["voltage"] = json!(12);
        assert!(compatible(&a, &b, &low).is_err());
        let wire = Wire {
            from: a.end.clone(),
            to: b.end.clone(),
            cable: "run.test".into(),
            waypoints_m: vec![[1., 1., 0.]],
            pins,
        };
        assert_eq!(
            route(&wire, &[a.clone(), b.clone()]).unwrap(),
            vec![Vec3::ZERO, Vec3::new(1., 1., 0.), Vec3::new(2., 0., 0.)]
        );
        let mut moved = b.clone();
        moved.at = Vec3::new(3., 0., 0.);
        assert_eq!(
            route(&wire, &[a.clone(), moved]).unwrap()[1],
            Vec3::new(1., 1., 0.),
            "only endpoints follow equipment"
        );
        assert!(
            route(&wire, &[a.clone()]).is_none(),
            "deleted equipment leaves an unresolved endpoint"
        );
        let mut i = Interior::default();
        i.plan.wiring = Some(std::collections::BTreeMap::from([("wire_1".into(), wire)]));
        assert!(occupied(&i, &a.end));
        assert!(occupied(&i, &b.end));
        i.wire_editor.start = Some(a.end);
        i.wire_editor.points.push([1., 1., 0.]);
        i.wire_editor.cancel();
        assert!(!i.wire_editor.active() && i.wire_editor.points.is_empty());
        assert_eq!(
            i.plan.wiring.as_ref().unwrap().len(),
            1,
            "cancel cannot erase a finished route"
        );
    }
}
