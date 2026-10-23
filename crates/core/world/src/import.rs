//! Hulls from glTF: a ship modelled in a real tool (Blender), made a hull like
//! any other — its collision from its `COL_*` meshes (convex parts; none: the
//! whole model's convex hull), its nozzles, mounts, gear, docks and cockpit
//! from empties named as the shapes' nodes are (`nozzle_*`, `mount_*`,
//! `gear_*`, `dock_*`, `cockpit`; Blender's `.001` suffixes ignored), its slots
//! standard for its size class with as many hardpoints, racks and utility
//! slots as it has mounts for, its frame's mass from its size (as a design's).
//! Everything about how it flies then follows from the physics: its mass and
//! inertia from where things sit, its thrust from where its nozzles push.
//!
//! In Blender: the nose along +Y, up +Z (glTF, our frame: nose −Z, up +Y); an
//! empty's +Y arrow points where a nozzle's exhaust goes (a dock's way out, the
//! pilot's view). Optional scene properties: `freefall_name`, `freefall_class`.

use glam::{DMat4, DVec3};

use crate::content::content;
use crate::design::{standard_slots, stock_fit, FRAME_PER_AREA, PRICE_PER_KG, PRICE_PER_SLOT_SIZE, STRENGTH_PER_KG};
use crate::modules::Module;
use crate::ship::{ClassSpec, Hull};

/// What a glTF file holds for a hull.
struct Read {
    name: Option<String>,
    class: Option<u8>,
    /// The `COL_*` meshes' points (each a convex part), by name.
    collision: Vec<(String, Vec<DVec3>)>,
    /// Every other mesh's points (for the hull when there's no collision given).
    visual: Vec<DVec3>,
    /// Every other mesh's triangles: what's walked on and bumped into.
    tris: Vec<[DVec3; 3]>,
    /// A `*Ramp*` mesh's triangles (and what hangs from it): a part that swings down.
    ramp: Vec<[DVec3; 3]>,
    /// Authored CargoRamp mesh origin and local hinge axis, in model coordinates.
    ramp_frame: Option<(DVec3, DVec3)>,
    /// The named empties: (name, where, which way).
    nodes: Vec<(String, DVec3, DVec3)>,
    /// Each named mesh's box: (name, least corner, most corner).
    pieces: Vec<(String, DVec3, DVec3)>,
    /// Glass and screen meshes' pieces one by one, each's box.
    islands: Vec<(String, DVec3, DVec3)>,
}

/// A hull from a glTF file's bytes (`.glb`); `visual`: the file's path, for
/// drawing it. Its key is made from the path, so the same file is the same hull.
pub fn hull_from_gltf(bytes: &[u8], visual: &str) -> Result<ClassSpec, String> {
    let read = read(bytes)?;
    let stem = std::path::Path::new(visual).file_stem().and_then(|s| s.to_str()).unwrap_or("imported");
    let key = format!("hull.import.{}", stem.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "_"));
    // Collision: its convex parts (the one named body first, else the first); none given: all of it.
    let mut parts = read.collision;
    parts.sort_by_key(|(n, _)| (!n.to_lowercase().contains("body"), n.clone()));
    let (body, parts): (Vec<DVec3>, Vec<Vec<DVec3>>) = if parts.is_empty() {
        (read.visual, Vec::new())
    } else {
        let mut it = parts.into_iter().map(|p| p.1);
        (it.next().unwrap_or_default(), it.collect())
    };
    if body.len() < 4 {
        return Err("no geometry to make its hull from".into());
    }
    let record = crate::registry::registry().hulls.iter().find(|h| h.model.as_deref() == Some(visual));
    let count = |prefix: &str| read.nodes.iter().filter(|n| n.0.starts_with(prefix)).count() as u8;
    let inferred_slots = standard_slots(read.class.unwrap_or(2), count("mount_cargo").max(1), count("mount_hardpoint"), count("mount_utility"));
    let slots = match record.filter(|h| !h.slots.is_empty()) {
        // Keep registry names/sizes for represented kinds (including additional
        // air/water/cabin slots). Unimplemented kinds stay explicitly unfitted.
        Some(h) => h.slots.iter().filter_map(|s| {
            crate::modules::SlotKind::from_record(s.kind)
                .map(|kind| (s.name.clone(), kind, s.size as u8))
        }).collect(),
        None => inferred_slots,
    };
    // Its engines' nozzles as its record binds them (exactly: see `bind_engines`); every other nozzle driven by a
    // slot by its name.
    let nozzles: Vec<&str> = read.nodes.iter().filter(|n| n.0.starts_with("nozzle_")).map(|n| n.0.as_str()).collect();
    let engines = match record {
        Some(h) => bind_engines(&h.thrusters.iter().map(|t| (t.nozzle.clone(), t.slot.clone(), t.share)).collect::<Vec<_>>(), &slots, &nozzles)?,
        None => Vec::new(),
    };
    let mut thrusters: Vec<(String, String, f64)> = read
        .nodes
        .iter()
        .filter(|n| n.0.starts_with("nozzle_") && !engines.iter().any(|e| e.0 == n.0))
        .map(|n| {
            let slot = if n.0.starts_with("nozzle_main") { "drive" } else if n.0.starts_with("nozzle_lift") { "lift" } else { "thrusters" };
            (n.0.clone(), slot.to_string(), 1.0)
        })
        .collect();
    // (An engine's nozzle may be the main drive: whether one pushes along the nose is the ship's to check.)
    if !thrusters.iter().any(|t| t.1 == "drive") && engines.is_empty() {
        return Err("no main drive nozzle (an empty named nozzle_main_*, or an engine's nozzle its record binds)".into());
    }
    thrusters.extend(engines);
    let mut shape = crate::shape::ShapeDef::made(format!("shape.{key}"), body, parts, Vec::new(), read.nodes).build()?;
    // (Its surfaces in its frame, centred as the shape is.)
    let c = shape.made_centre;
    let tris: Vec<[DVec3; 3]> = read.tris.iter().map(|t| t.map(|p| p - c)).collect();
    shape.walk = Some(std::sync::Arc::new(crate::walk::WalkMesh::new(&tris)));
    shape.ramp = ramp(&shape, read.ramp_frame.map(|(p, a)| (p - c, a)), read.ramp.iter().map(|t| t.map(|p| p - c)).collect());
    shape.pieces = read.pieces.iter().map(|(n, lo, hi)| (n.clone(), *lo - c, *hi - c)).collect();
    shape.islands = read.islands.iter().map(|(n, lo, hi)| (n.clone(), *lo - c, *hi - c)).collect();
    let (lo, hi) = shape.mesh.extent();
    let size = hi - lo;
    // (A hull the registry describes by this model weighs what its parts do, at its price;
    // any other, by its size.)

    // (A registered hull is commissioned at its measured mass only: never the tonne stock building falls back to.)
    if let Some(h) = record {
        match crate::goods::hull_mass(crate::registry::registry(), &h.identity.key) {
            Some(crate::goods::HullMass::Partial { weighed, unweighed }) => {
                return Err(format!("{}: its mass is partial ({weighed:.0} kg weighed; no mass for {})", h.identity.key, unweighed.join(", ")));
            }
            Some(crate::goods::HullMass::None) => {
                return Err(format!("{}: no mass (neither physical.mass nor weighed parts)", h.identity.key));
            }
            _ => {}
        }
    }
    let recorded = record.and_then(|h| crate::goods::item(&h.identity.key)).map(|i| &content().stock[i]);
    let frame_mass = recorded.map_or(FRAME_PER_AREA * shape.solid.volume.powf(2.0 / 3.0), |h| h.mass);
    let price = recorded.map_or(PRICE_PER_KG * frame_mass + PRICE_PER_SLOT_SIZE * slots.iter().map(|s| f64::from(s.2)).sum::<f64>(), |h| h.price);
    // (Fitted as its record says, with its own hold, where the registry describes it: an
    // ore bay, not racks. Any other: the cheapest that fits each slot.)
    let fit = match record {
        Some(h) if !h.fit.is_empty() => h.fit.iter().filter(|f| slots.iter().any(|s| s.0 == f.slot)).filter(|f| {
            let available = content().handle::<Module>(&f.item).is_some();
            if !available { log::warn!("{} slot {}: {} is not in the playable module catalogue; fitting held", h.identity.key, f.slot, f.item); }
            available
        }).map(|f| (f.slot.clone(), f.item.clone())).collect(),
        _ => stock_fit(&slots)?,
    };
    let bay = record.filter(|h| !h.fit.iter().any(|f| f.slot == "cargo")).map_or((0.0, 0.0), |h| (h.capacity.hold.unwrap_or(0.0), h.capacity.hold_volume.unwrap_or(0.0)));
    // (Its flight figures its record's, where it has them; any other's, by its size.)
    let flight = record.map(|h| &h.flight);
    let radius = flight.and_then(|f| f.radius).unwrap_or(0.3 * size.max_element() * 0.5 + 6.0);
    let drag = flight.and_then(|f| f.drag_area).unwrap_or(size.x * size.y * 0.6);
    let strength = flight.and_then(|f| f.hull_strength).unwrap_or(STRENGTH_PER_KG * frame_mass);
    let name = read.name.unwrap_or_else(|| stem.to_uppercase());
    // (The mount each slot offers, where its record says.)
    let mounts = record.map_or_else(Vec::new, |h| h.slots.iter().filter_map(|s| s.mount.clone().map(|m| (s.name.clone(), m))).collect());
    let def = crate::ship::HullDef::made(key, name, shape.key.clone(), frame_mass, price, slots, fit, thrusters, radius, drag, strength).with_bay(bay.0, bay.1).with_mounts(mounts);
    let shape = std::sync::Arc::new(shape);
    let any = content().shapes.iter().next().map(|(h, _)| h).expect("the content has shapes");
    let module = |k: &str| content().handle::<Module>(k).map(|h| (h, content().get(h)));
    let mut spec = def.build(any, &shape, module)?;
    spec.shape_own = Some(shape);
    spec.visual = Some(visual.to_string());
    Ok(spec)
}

/// The nozzles a hull's record binds to its engine slots (`thrusters`: nozzle, slot, share), checked strictly: each
/// to an engine slot it has, a nozzle the model has, once, at a share over 0 and up to 1, an engine's shares
/// summing to 1; each engine slot driving at least one. Bindings to other slots are left to the
/// name rule (as the MC-07's are).
fn bind_engines(record: &[(String, String, f64)], slots: &[(String, crate::modules::SlotKind, u8)], nozzles: &[&str]) -> Result<Vec<(String, String, f64)>, String> {
    use crate::modules::SlotKind;
    let engine = |slot: &str| slots.iter().any(|s| s.0 == slot && s.1 == SlotKind::Engine);
    let mut bound: Vec<(String, String, f64)> = Vec::new();
    for (nozzle, slot, share) in record {
        if !engine(slot) {
            continue;
        }
        if !nozzles.contains(&nozzle.as_str()) {
            return Err(format!("thruster {nozzle} (slot '{slot}'): the model has no such nozzle"));
        }
        if !(share.is_finite() && *share > 0.0 && *share <= 1.0) {
            return Err(format!("thruster {nozzle} (slot '{slot}'): share {share} is not over 0 and up to 1"));
        }
        if bound.iter().any(|b| &b.0 == nozzle) {
            return Err(format!("thruster {nozzle}: bound twice"));
        }
        bound.push((nozzle.clone(), slot.clone(), *share));
    }
    for (name, ..) in slots.iter().filter(|s| s.1 == SlotKind::Engine) {
        let total: f64 = bound.iter().filter(|b| &b.1 == name).map(|b| b.2).sum();
        if total == 0.0 {
            return Err(format!("engine slot '{name}': its record binds it no nozzle"));
        }
        // (An engine's thrust is its nozzles' together: a four-nozzle 0.5 kN block is 0.5 kN, shared, never 4 × 0.5.)
        if (total - 1.0).abs() > 1e-9 {
            return Err(format!("engine slot '{name}': its nozzles' shares sum to {total}, not 1"));
        }
    }
    Ok(bound)
}

/// Prefer the authored CargoRamp hinge. Older unnamed ramps use the hatch
/// convention; a boarding marker is not necessarily the mesh hinge.
fn ramp(shape: &crate::shape::Shape, authored: Option<(DVec3, DVec3)>, tris: Vec<[DVec3; 3]>) -> Option<crate::shape::Ramp> {
    let hinge = authored.map(|f| f.0).or_else(|| shape.node("hatch").map(|n| n.at))?;
    if tris.is_empty() {
        return None;
    }
    let points: Vec<DVec3> = tris.iter().flatten().copied().collect();
    let middle = points.iter().sum::<DVec3>() / points.len() as f64;
    let out = authored.map_or_else(|| DVec3::new(middle.x - hinge.x, 0.0, middle.z - hinge.z), |f| f.1.cross(DVec3::Y)).try_normalize()?;
    let length = points.iter().map(|p| (*p - hinge).dot(out)).fold(0.0, f64::max);
    Some(crate::shape::Ramp { hinge, axis: authored.map_or_else(|| DVec3::Y.cross(out).normalize(), |f| f.1), length, walk: std::sync::Arc::new(crate::walk::WalkMesh::new(&tris)) })
}

/// Imported: a hull among the others for good (the same file, the same hull).
pub fn commission(bytes: &[u8], visual: &str) -> Result<Hull, String> {
    let spec = hull_from_gltf(bytes, visual)?;
    if let Some(h) = content().handle::<ClassSpec>(&spec.key) {
        return Ok(h);
    }
    content().hulls.add(spec)
}

fn read(bytes: &[u8]) -> Result<Read, String> {
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = g.blob.as_deref();
    let scene = g.default_scene().or_else(|| g.scenes().next()).ok_or("no scene")?;
    let extras: serde_json::Value = scene.extras().as_ref().and_then(|r| serde_json::from_str(r.get()).ok()).unwrap_or_default();
    let mut read = Read {
        name: extras.get("freefall_name").and_then(|v| v.as_str()).map(|s| s.to_uppercase()),
        class: extras.get("freefall_class").and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))).map(|c| c as u8),
        collision: Vec::new(),
        visual: Vec::new(),
        tris: Vec::new(),
        ramp: Vec::new(),
        ramp_frame: None,
        nodes: Vec::new(),
        pieces: Vec::new(),
        islands: Vec::new(),
    };
    for node in scene.nodes() {
        walk(&node, DMat4::IDENTITY, blob, false, &mut read);
    }
    Ok(read)
}

/// A mesh's pieces that don't touch (triangles joined where they share a corner,
/// to a millimetre), each one's box.
fn islands(tris: &[[DVec3; 3]]) -> Vec<(DVec3, DVec3)> {
    let key = |p: DVec3| ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64, (p.z * 1000.0).round() as i64);
    let mut ids: std::collections::HashMap<(i64, i64, i64), usize> = std::collections::HashMap::new();
    let mut up: Vec<usize> = Vec::new();
    fn root(up: &mut [usize], mut x: usize) -> usize {
        while up[x] != x {
            up[x] = up[up[x]];
            x = up[x];
        }
        x
    }
    let mut corner = |p: DVec3, up: &mut Vec<usize>| *ids.entry(key(p)).or_insert_with(|| {
        up.push(up.len());
        up.len() - 1
    });
    let mut at = Vec::new();
    for t in tris {
        let k = t.map(|p| corner(p, &mut up));
        for &j in &k[1..] {
            let (a, b) = (root(&mut up, k[0]), root(&mut up, j));
            up[b] = a;
        }
        at.push((k[0], t));
    }
    let mut boxes: std::collections::BTreeMap<usize, (DVec3, DVec3)> = std::collections::BTreeMap::new();
    for (k, t) in at {
        let r = root(&mut up, k);
        let b = boxes.entry(r).or_insert((t[0], t[0]));
        for p in t {
            b.0 = b.0.min(*p);
            b.1 = b.1.max(*p);
        }
    }
    boxes.into_values().collect()
}

fn walk(node: &gltf::Node, parent: DMat4, blob: Option<&[u8]>, ramp: bool, read: &mut Read) {
    let m = parent * DMat4::from_cols_array_2d(&node.transform().matrix().map(|c| c.map(f64::from)));
    // (Blender numbers repeated names: "nozzle_main.001".)
    let name = node.name().unwrap_or("").split('.').next().unwrap_or("").to_string();
    if name == "CargoRamp" && node.mesh().is_some() {
        read.ramp_frame = Some((m.transform_point3(DVec3::ZERO), m.transform_vector3(DVec3::X).normalize()));
    }
    let ramp = ramp || name.contains("Ramp");
    if let Some(mesh) = node.mesh() {
        let mut points = Vec::new();
        let mut tris = Vec::new();
        for prim in mesh.primitives() {
            let r = prim.reader(|b| match b.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            });
            if let Some(pos) = r.read_positions() {
                let base = points.len();
                points.extend(pos.map(|p| m.transform_point3(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))));
                let index: Vec<usize> = match r.read_indices() {
                    Some(i) => i.into_u32().map(|i| base + i as usize).collect(),
                    None => (base..points.len()).collect(),
                };
                tris.extend(index.as_chunks::<3>().0.iter().map(|t| [points[t[0]], points[t[1]], points[t[2]]]));
            }
        }
        if let Some((lo, hi)) = points.iter().fold(None, |b: Option<(DVec3, DVec3)>, p| Some(b.map_or((*p, *p), |(l, h)| (l.min(*p), h.max(*p))))) {
            read.pieces.push((name.clone(), lo, hi));
        }
        if name.contains("Glass") || name.contains("Screen") {
            read.islands.extend(islands(&tris).into_iter().map(|(lo, hi)| (name.clone(), lo, hi)));
        }
        if name.starts_with("COL_") {
            read.collision.push((name.clone(), points));
        } else {
            read.visual.extend(points);
            if ramp { read.ramp.extend(tris) } else { read.tris.extend(tris) }
        }
    } else if name == "cockpit" || name == "hatch" || ["nozzle_", "mount_", "gear_", "dock_", "door_"].iter().any(|p| name.starts_with(p)) {
        // (Blender's +Y is glTF's −Z.)
        let dir = m.transform_vector3(DVec3::NEG_Z).normalize_or(DVec3::NEG_Z);
        read.nodes.push((name.clone(), m.transform_point3(DVec3::ZERO), dir));
    }
    for child in node.children() {
        walk(&child, m, blob, ramp, read);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The MC-07 (its record names its model): its mass whole from its parts; its gear fit entries carried
    /// (inert modules: their mass in its dry mass), not dropped; gear and command never fitted by default.
    #[test]
    fn import_keeps_recorded_gear_fit_and_its_mass() {
        let reg = crate::registry::registry();
        assert!(matches!(crate::goods::hull_mass(reg, "hull.mc-07"), Some(crate::goods::HullMass::Parts(m)) if m > 0.0));
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/models/mc07.glb");
        let Ok(bytes) = std::fs::read(path) else { return };
        let spec = hull_from_gltf(&bytes, "assets/models/mc07.glb").unwrap();
        let gear: Vec<(&str, &Module)> = spec.fit.iter().map(|(s, h)| (s.as_str(), content().get(*h))).filter(|(_, m)| m.does.slot() == crate::modules::SlotKind::Gear).collect();
        let mut keys: Vec<&str> = gear.iter().map(|(_, m)| m.key.as_str()).collect();
        keys.sort();
        assert_eq!(keys, ["equipment.gear.front.mc07", "equipment.gear.front.mc07-right", "equipment.gear.rear.mc07", "equipment.gear.rear.mc07-right"]);
        let modules: f64 = spec.fit.iter().map(|(_, h)| content().get(*h).mass).sum();
        assert!((spec.dry_mass - spec.frame_mass - modules).abs() < 1e-6 && gear.iter().map(|g| g.1.mass).sum::<f64>() == 3972.0);
        let slots = vec![("command".to_string(), crate::modules::SlotKind::Command, 1), ("gear_0".to_string(), crate::modules::SlotKind::Gear, 3)];
        assert!(crate::design::stock_fit(&slots).is_err_and(|e| e.contains("power")), "nothing fitted to them; only the plant missing");
    }

    /// A hull's engines as its record binds them: exactly, or refused (a missing nozzle or slot, twice, a share
    /// out of (0, 1], an engine's shares over 1, an engine left driving nothing); other bindings left alone.
    #[test]
    fn import_binds_engine_nozzles_strictly() {
        use crate::modules::SlotKind::{Engine, Lift};
        let slots = vec![("engine_aft".to_string(), Engine, 2), ("engine_down".to_string(), Engine, 2), ("lift".to_string(), Lift, 2)];
        let nozzles = ["nozzle_engine_aft", "nozzle_engine_down", "nozzle_lift_0"];
        let t = |n: &str, s: &str, share: f64| (n.to_string(), s.to_string(), share);
        let good = vec![t("nozzle_engine_aft", "engine_aft", 1.0), t("nozzle_engine_down", "engine_down", 1.0), t("nozzle_lift_0", "lift", 1.0)];
        assert_eq!(bind_engines(&good, &slots, &nozzles).unwrap(), good[..2].to_vec(), "the lift's left to its name");
        let refused = |record: Vec<(String, String, f64)>, why: &str| {
            let e = bind_engines(&record, &slots, &nozzles).unwrap_err();
            assert!(e.contains(why), "{e}");
        };
        refused(vec![t("nozzle_engine_aft", "engine_aft", 1.0), t("nozzle_engine_gone", "engine_down", 1.0)], "no such nozzle");
        refused(vec![t("nozzle_engine_aft", "engine_aft", 1.0), t("nozzle_engine_aft", "engine_down", 0.5), t("nozzle_engine_down", "engine_down", 0.5)], "bound twice");
        refused(vec![t("nozzle_engine_aft", "engine_aft", 0.0), t("nozzle_engine_down", "engine_down", 1.0)], "share 0");
        refused(vec![t("nozzle_engine_aft", "engine_aft", 1.5), t("nozzle_engine_down", "engine_down", 1.0)], "share 1.5");
        refused(vec![t("nozzle_engine_aft", "engine_aft", 0.7), t("nozzle_engine_down", "engine_aft", 0.7)], "sum to 1.4");
        refused(vec![t("nozzle_engine_aft", "engine_aft", 0.5), t("nozzle_engine_down", "engine_down", 1.0)], "sum to 0.5, not 1");
        refused(vec![t("nozzle_engine_aft", "engine_aft", 1.0)], "engine slot 'engine_down': its record binds it no nozzle");
    }

    /// Engines drive as they point: aft-facing ones a main drive, downward ones a lift, each its own thrust; one
    /// pointing elsewhere refused; their throttle floor and gimbal stated as limits, not simulated.
    #[test]
    fn import_engines_drive_as_they_point() {
        use crate::modules::SlotKind;
        use crate::ship::{ClassSpec, HullFrame, NozzleLink, Slot, ThrusterRole};
        // (The CH records carried too, since their efficiencies were derived exactly from their jets (fso e35bd2cb3).)
        assert!(["ch.s1", "ch.s2", "ch.s3", "ch.s4"].iter().all(|k| content().handle::<Module>(&format!("equipment.engine.{k}")).is_some()));
        let engine = content().handle::<Module>("equipment.engine.ca.s1").expect("the ca s1 engine is carried");
        let slot = |name: &str| Slot { name: name.into(), kind: SlotKind::Engine, size: 2, mount: None };
        // (A nozzle's push is opposite its exhaust: aft-firing pushes along the nose, −Z; down-firing pushes up.)
        let link = |n: &str, s: &str, push: DVec3| NozzleLink { nozzle: n.into(), slot: s.into(), share: 1.0, at: DVec3::new(0.0, -1.0, 5.0), push };
        let build = |pushes: [DVec3; 4]| {
            let names = ["engine_aft_p", "engine_aft_s", "engine_down_p", "engine_down_s"];
            let frame = HullFrame { frame_mass: 8000.0, price: 1.0, slots: names.iter().map(|n| slot(n)).collect(), nozzles: names.iter().zip(pushes).map(|(n, p)| link(&format!("nozzle_{n}"), n, p)).collect(), radius: 10.0, drag_area: 10.0, hull_strength: 1e6, bay: (0.0, 0.0) };
            let shape = std::sync::Arc::new(crate::shape::ShapeDef::made("shape.test-engines".into(), (0..8).map(|i| DVec3::new(if i & 1 == 0 { -2.0 } else { 2.0 }, if i & 2 == 0 { -2.0 } else { 2.0 }, if i & 4 == 0 { -6.0 } else { 6.0 })).collect(), vec![], vec![], vec![]).build().unwrap());
            ClassSpec::assemble_uncommissioned_owned("hull.test-engines".into(), "TEST".into(), shape, frame, names.iter().map(|n| (n.to_string(), engine)).collect())
        };
        let spec = build([DVec3::NEG_Z, DVec3::NEG_Z, DVec3::Y, DVec3::Y]).unwrap();
        let roles: Vec<ThrusterRole> = spec.thrusters.iter().map(|t| t.role).collect();
        assert_eq!(roles, [ThrusterRole::Main, ThrusterRole::Main, ThrusterRole::Lift, ThrusterRole::Lift]);
        let rated = content().get(engine).does.engine().unwrap().0;
        assert!(spec.thrusters.iter().all(|t| t.thrust == rated), "each engine its own, at its rating");
        assert!((spec.dry_mass - 8000.0 - 4.0 * content().get(engine).mass).abs() < 1e-6);
        let (throttle, gimbal) = match content().get(engine).does { crate::modules::Does::Engine { throttle, gimbal, .. } => (throttle, gimbal), _ => unreachable!() };
        assert!(spec.limits.iter().any(|l| l.contains(&format!("throttle floor {throttle}")) || (throttle == 0.0 && l.contains("on or off"))));
        assert_eq!(spec.limits.iter().any(|l| l.contains("gimbal")), gimbal > 0.0);
        let sideways = build([DVec3::NEG_Z, DVec3::NEG_Z, DVec3::Y, DVec3::X]).unwrap_err();
        assert!(sideways.contains("pushes neither along the nose nor up"), "{sideways}");
    }

    /// Cold nitrogen beside another fuel: a four-nozzle block's thrust shared (not multiplied), its nozzles attitude
    /// thrusters wherever they point, drawing on its own tanks (weighed full) apart from the ship's fuel; a ship of
    /// that class carries the pool in its device state; with no tank of it, refused.
    #[test]
    fn import_cold_gas_rcs_draws_its_own_propellant() {
        use crate::modules::SlotKind;
        use crate::ship::{ClassSpec, HullFrame, NozzleLink, Slot, ThrusterRole};
        let h = |k: &str| content().handle::<Module>(k).unwrap_or_else(|| panic!("{k} is carried"));
        let (main, block, fuel_tank, gas_tank) = (h("equipment.engine.ca.s1"), h("equipment.engine.cg.s1"), h("equipment.tank.methalox.s1"), h("equipment.tank.nitrogen.s1"));
        let slot = |name: &str, kind| Slot { name: name.into(), kind, size: 2, mount: None };
        let link = |n: &str, s: &str, share: f64, push: DVec3| NozzleLink { nozzle: n.into(), slot: s.into(), share, at: DVec3::new(0.0, 0.0, 4.0), push };
        let build = |with_gas_tank: bool| {
            let mut slots = vec![slot("engine_aft_p", SlotKind::Engine), slot("engine_aft_s", SlotKind::Engine), slot("rcs_fwd", SlotKind::Engine), slot("tank", SlotKind::Tank)];
            let mut fit = vec![("engine_aft_p".to_string(), main), ("engine_aft_s".to_string(), main), ("rcs_fwd".to_string(), block), ("tank".to_string(), fuel_tank)];
            if with_gas_tank {
                slots.push(slot("gas", SlotKind::Tank));
                fit.push(("gas".to_string(), gas_tank));
            }
            let mut nozzles = vec![link("nozzle_aft_p", "engine_aft_p", 1.0, DVec3::NEG_Z), link("nozzle_aft_s", "engine_aft_s", 1.0, DVec3::NEG_Z)];
            nozzles.extend([DVec3::X, DVec3::NEG_X, DVec3::Y, DVec3::NEG_Y].iter().enumerate().map(|(i, p)| link(&format!("nozzle_rcs_{i}"), "rcs_fwd", 0.25, *p)));
            let frame = HullFrame { frame_mass: 8000.0, price: 1.0, slots, nozzles, radius: 10.0, drag_area: 10.0, hull_strength: 1e6, bay: (0.0, 0.0) };
            let shape = std::sync::Arc::new(crate::shape::ShapeDef::made("shape.test-cold".into(), (0..8).map(|i| DVec3::new(if i & 1 == 0 { -2.0 } else { 2.0 }, if i & 2 == 0 { -2.0 } else { 2.0 }, if i & 4 == 0 { -6.0 } else { 6.0 })).collect(), vec![], vec![], vec![]).build().unwrap());
            ClassSpec::assemble_uncommissioned_owned("hull.test-cold".into(), "TEST".into(), shape, frame, fit)
        };
        let spec = build(true).unwrap();
        let rcs: Vec<usize> = (2..6).collect();
        assert!(rcs.iter().all(|&i| spec.thrusters[i].role == ThrusterRole::Rcs && spec.thrusters[i].thrust == 125.0), "0.5 kN shared by four");
        let gas = content().get(gas_tank).does.clone();
        let crate::modules::Does::Tank { capacity: gas_full, .. } = gas else { unreachable!() };
        assert_eq!(spec.propellants, vec![crate::ship_devices::Propellant { material: "material.nitrogen".into(), capacity: gas_full, thrusters: rcs }]);
        assert_eq!(spec.fuel, "material.methalox");
        let crate::modules::Does::Tank { capacity: fuel_full, .. } = content().get(fuel_tank).does.clone() else { unreachable!() };
        assert_eq!(spec.fuel_capacity, fuel_full, "the gas is no fuel");
        let parts: f64 = [main, main, block, fuel_tank, gas_tank].iter().map(|m| content().get(*m).mass).sum();
        assert!((spec.dry_mass - 8000.0 - parts - gas_full).abs() < 1e-6, "its gas weighed full");
        let ship = crate::ship::Ship::with_spec(std::sync::Arc::new(spec), DVec3::ZERO, DVec3::ZERO, glam::DQuat::IDENTITY);
        assert!(ship.devices.as_ref().is_some_and(|d| d.burned() == 0.0), "the pool aboard, full");
        assert!(build(false).unwrap_err().contains("no tank holds it"));
    }
}
