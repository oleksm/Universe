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
    let mut shape = crate::shape::ShapeDef::made(format!("shape.{key}"), body, parts, Vec::new(), read.nodes).build()?;
    // (Its surfaces in its frame, centred as the shape is.)
    let c = shape.made_centre;
    let tris: Vec<[DVec3; 3]> = read.tris.iter().map(|t| t.map(|p| p - c)).collect();
    shape.walk = Some(std::sync::Arc::new(crate::walk::WalkMesh::new(&tris)));
    shape.ramp = ramp(&shape, read.ramp.iter().map(|t| t.map(|p| p - c)).collect());
    shape.pieces = read.pieces.iter().map(|(n, lo, hi)| (n.clone(), *lo - c, *hi - c)).collect();
    shape.islands = read.islands.iter().map(|(n, lo, hi)| (n.clone(), *lo - c, *hi - c)).collect();
    let (lo, hi) = shape.mesh.extent();
    let size = hi - lo;
    // (A hull the registry describes by this model weighs what its parts do, at its price;
    // any other, by its size.)
    let recorded = crate::registry::registry().hulls.iter().find(|h| h.model.as_deref() == Some(visual)).and_then(|h| crate::goods::item(&h.identity.key)).map(|i| &content().stock[i]);
    let frame_mass = recorded.map_or(FRAME_PER_AREA * shape.solid.volume.powf(2.0 / 3.0), |h| h.mass);
    let price = recorded.map_or(PRICE_PER_KG * frame_mass + PRICE_PER_SLOT_SIZE * slots.iter().map(|s| f64::from(s.2)).sum::<f64>(), |h| h.price);
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

/// A ramp hinged at the hull's `hatch` (put at the ramp's top, where it meets
/// the belly), swinging down about the level line across it, its far end
/// (the side its middle is on) going down. None without both.
fn ramp(shape: &crate::shape::Shape, tris: Vec<[DVec3; 3]>) -> Option<crate::shape::Ramp> {
    let hinge = shape.node("hatch")?.at;
    if tris.is_empty() {
        return None;
    }
    let points: Vec<DVec3> = tris.iter().flatten().copied().collect();
    let middle = points.iter().sum::<DVec3>() / points.len() as f64;
    let out = DVec3::new(middle.x - hinge.x, 0.0, middle.z - hinge.z).try_normalize()?;
    let length = points.iter().map(|p| (*p - hinge).dot(out)).fold(0.0, f64::max);
    Some(crate::shape::Ramp { hinge, axis: DVec3::Y.cross(out).normalize(), length, walk: std::sync::Arc::new(crate::walk::WalkMesh::new(&tris)) })
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
