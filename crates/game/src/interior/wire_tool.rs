//! Player-authored routes. Registry ports are the only snap targets.
use super::*;
use serde_json::Value;
use universe_ship_design::{Wire, WireEnd, WirePin};

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
fn available(i: &Interior) -> Vec<Value> {
    let mut list = cables();
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
        for c in cable["conductors"]
            .as_array()
            .ok_or("Cable has no conductors")?
        {
            if c["size"].as_f64().unwrap_or(f64::INFINITY)
                > connector["size"].as_f64().unwrap_or(0.0) * 1.000001
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
                    let map = i.plan.wiring.get_or_insert_with(Default::default);
                    let mut n = 1;
                    while map.contains_key(&format!("wire_{n}")) {
                        n += 1;
                    }
                    map.insert(format!("wire_{n}"), w);
                    i.wire_editor.cancel();
                    i.note("WIRE LAID — ROUTE SUPPORT AND ELECTRICAL LOAD CHECKS STILL REQUIRED");
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
    if let Some(wires) = &i.plan.wiring {
        for w in wires.values() {
            if let Some(path) = route(w, &all) {
                for s in path.windows(2) {
                    draw_line(frame, cam, s[0], s[1], PICKED);
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
            PANEL.0 + Vec2::new(8., 270. + n as f32 * 14.),
            text,
            LABEL,
            0.65,
        );
    }
    if ports(i).is_empty() {
        frame.text_scaled(
            PANEL.0 + Vec2::new(8., 342.),
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
        let length: f64 = path.windows(2).map(|p| p[0].distance(p[1]) as f64).sum();
        let ends: f64 = [&w.from, &w.to]
            .iter()
            .filter_map(|e| reg.connector(&e.connector)?.physical.mass)
            .sum();
        let mass = c["per_m"]["mass"].as_f64().unwrap_or(0.) * length + ends;
        h.runs.push(wiring::WireRun {
            module: w.to.module.clone(),
            volts: a.data["voltage"].as_f64().unwrap_or(0.),
            watts: 0.,
            cable: Some(w.cable.clone()),
            pairs: 1,
            path,
            length,
            mass,
            drop: 0.,
        });
    }
    h.faults
        .push("AUTHORED WIRING: SUPPORTS AND ELECTRICAL LOADS NOT YET VALIDATED".into());
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
