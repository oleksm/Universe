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
    /// The named empties: (name, where, which way).
    nodes: Vec<(String, DVec3, DVec3)>,
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
    let count = |prefix: &str| read.nodes.iter().filter(|n| n.0.starts_with(prefix)).count() as u8;
    let slots = standard_slots(read.class.unwrap_or(2), count("mount_cargo").max(1), count("mount_hardpoint"), count("mount_utility"));
    // Its nozzles, each driven by a slot by its name.
    let thrusters: Vec<(String, String, f64)> = read
        .nodes
        .iter()
        .filter(|n| n.0.starts_with("nozzle_"))
        .map(|n| {
            let slot = if n.0.starts_with("nozzle_main") { "drive" } else if n.0.starts_with("nozzle_lift") { "lift" } else { "thrusters" };
            (n.0.clone(), slot.to_string(), 1.0)
        })
        .collect();
    if !thrusters.iter().any(|t| t.1 == "drive") {
        return Err("no main drive nozzle (an empty named nozzle_main_*)".into());
    }
    let shape = crate::shape::ShapeDef::made(format!("shape.{key}"), body, parts, Vec::new(), read.nodes).build()?;
    let (lo, hi) = shape.mesh.extent();
    let size = hi - lo;
    let frame_mass = FRAME_PER_AREA * shape.solid.volume.powf(2.0 / 3.0);
    let price = PRICE_PER_KG * frame_mass + PRICE_PER_SLOT_SIZE * slots.iter().map(|s| f64::from(s.2)).sum::<f64>();
    let fit = stock_fit(&slots)?;
    let radius = 0.3 * size.max_element() * 0.5 + 6.0;
    let drag = size.x * size.y * 0.6;
    let name = read.name.unwrap_or_else(|| stem.to_uppercase());
    let def = crate::ship::HullDef::made(key, name, shape.key.clone(), frame_mass, price, slots, fit, thrusters, radius, drag, STRENGTH_PER_KG * frame_mass);
    let shape: &'static crate::shape::Shape = Box::leak(Box::new(shape));
    let any = content().shapes.iter().next().map(|(h, _)| h).expect("the content has shapes");
    let module = |k: &str| content().handle::<Module>(k).map(|h| (h, content().get(h)));
    let mut spec = def.build(any, shape, module)?;
    spec.shape_own = Some(shape);
    spec.visual = Some(visual.to_string());
    Ok(spec)
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
        nodes: Vec::new(),
    };
    for node in scene.nodes() {
        walk(&node, DMat4::IDENTITY, blob, &mut read);
    }
    Ok(read)
}

fn walk(node: &gltf::Node, parent: DMat4, blob: Option<&[u8]>, read: &mut Read) {
    let m = parent * DMat4::from_cols_array_2d(&node.transform().matrix().map(|c| c.map(f64::from)));
    // (Blender numbers repeated names: "nozzle_main.001".)
    let name = node.name().unwrap_or("").split('.').next().unwrap_or("").to_string();
    if let Some(mesh) = node.mesh() {
        let mut points = Vec::new();
        for prim in mesh.primitives() {
            let r = prim.reader(|b| match b.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            });
            if let Some(pos) = r.read_positions() {
                points.extend(pos.map(|p| m.transform_point3(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))));
            }
        }
        if name.starts_with("COL_") {
            read.collision.push((name.clone(), points));
        } else {
            read.visual.extend(points);
        }
    } else if name == "cockpit" || ["nozzle_", "mount_", "gear_", "dock_"].iter().any(|p| name.starts_with(p)) {
        // (Blender's +Y is glTF's −Z.)
        let dir = m.transform_vector3(DVec3::NEG_Z).normalize_or(DVec3::NEG_Z);
        read.nodes.push((name.clone(), m.transform_point3(DVec3::ZERO), dir));
    }
    for child in node.children() {
        walk(&child, m, blob, read);
    }
}
