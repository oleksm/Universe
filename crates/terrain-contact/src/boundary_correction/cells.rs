use crate::{contact_core::Chart, crease};
use glam::{DVec2, DVec3};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use universe_world::worlds::pgs::Surface;
/// Same centroid triangulator for both controls. Only four optional cells change.
pub fn rebuild(
    s: &Surface,
    c: DVec3,
    base: &crease::Pair,
    coalesce: Option<[f64; 2]>,
    reverse: bool,
) -> Result<crease::Pair, Value> {
    let chart = Chart::new(c, 6281370.);
    // Same representation in both controls; keep the left original tile origin.
    let origin = DVec3::from_array(base.tiles[0].origin);
    let mut faces = base
        .tiles
        .iter()
        .flat_map(|t| t.provenance.iter().map(|p| p[0]))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if reverse {
        faces.reverse()
    }
    let centre = coalesce.unwrap_or([0., 0.]);
    let block = [
        centre[0] - 8.,
        centre[0] + 8.,
        centre[1] - 8.,
        centre[1] + 8.,
    ];
    if coalesce.is_some() && block.iter().any(|x| x.abs() > 64.) {
        return Err(json!("coalescing would cross mandatory outer boundary"));
    }
    // Core/band lines are mandatory: a macro-cell crossing them can interpolate
    // a blended band vertex into the otherwise canonical contact core.
    if coalesce.is_some() && [0usize, 2].iter().any(|&axis| {
        [-32., 32.].iter().any(|&line| block[axis] < line && line < block[axis + 1])
    }) {
        return Err(json!("coalescing would cross mandatory core boundary"));
    }
    let mut cells = vec![];
    for x in -8..8 {
        for y in -8..8 {
            if coalesce.is_some()
                && x as f64 * 8. >= block[0]
                && (x + 1) as f64 * 8. <= block[1]
                && y as f64 * 8. >= block[2]
                && (y + 1) as f64 * 8. <= block[3]
            {
                continue;
            }
            cells.push([
                x as f64 * 8.,
                (x + 1) as f64 * 8.,
                y as f64 * 8.,
                (y + 1) as f64 * 8.,
            ]);
        }
    }
    if coalesce.is_some() {
        cells.push(block)
    }
    if reverse {
        cells.reverse()
    }
    let mut tile = crease::Tile {
        origin: origin.to_array(),
        vertices: vec![],
        uv: vec![],
        triangles: vec![],
        provenance: vec![],
    };
    let mut cache = BTreeMap::new();
    for face in faces {
        let f = s.face_geometry(face).ok_or(json!("face"))?;
        let mut poly = f.positions.map(|p| chart.project(p).unwrap()).to_vec();
        if reverse {
            poly.reverse()
        }
        for &bounds in &cells {
            let cut = crease::rect(poly.clone(), bounds);
            if crease::area(&cut) < 1e-12 {
                continue;
            }
            let mut points = vec![];
            for j in 0..cut.len() {
                let a = cut[j];
                let b = cut[(j + 1) % cut.len()];
                points.push(a);
                if coalesce.is_some() && bounds == block {
                    for axis in 0..2 {
                        if a[axis] == b[axis]
                            && (a[axis] - centre[axis]).abs() == 8.
                            && (a[1 - axis] - centre[1 - axis]) * (b[1 - axis] - centre[1 - axis])
                                < 0.
                        {
                            let mut p = a;
                            p[1 - axis] = centre[1 - axis];
                            points.push(p)
                        }
                    }
                }
            }
            if points.len() < 3 {
                continue;
            }
            let centre = points.iter().sum::<DVec2>() / points.len() as f64;
            let mut ids = vec![];
            for p in std::iter::once(centre).chain(points) {
                let key = ((p.x * 1e8).round() as i64, (p.y * 1e8).round() as i64);
                let i = if let Some(&i) = cache.get(&key) {
                    i
                } else {
                    if tile.vertices.len() >= 32768 {
                        return Err(json!("vertex cap"));
                    }
                    let uv = DVec2::new(key.0 as f64, key.1 as f64) / 1e8;
                    let v =
                        crease::world(&f, chart.direction(uv), chart.r).map_err(|e| json!(e))?;
                    let i = tile.vertices.len() as u32;
                    tile.vertices.push((v - origin).as_vec3().to_array());
                    tile.uv.push(uv.to_array());
                    cache.insert(key, i);
                    i
                };
                ids.push(i);
            }
            for j in 1..ids.len() {
                let mut tri = [
                    ids[0],
                    ids[j],
                    ids[if j + 1 < ids.len() { j + 1 } else { 1 }],
                ];
                let uv = tri.map(|i| DVec2::from_array(tile.uv[i as usize]));
                if (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]) < 0. {
                    tri.swap(1, 2)
                }
                let p = tri.map(|i| glam::Vec3::from_array(tile.vertices[i as usize]).as_dvec3());
                if (p[1] - p[0]).cross(p[2] - p[0]).length_squared() == 0. {
                    return Err(
                        json!({"reason":"cell f32 collapse","coalesced":coalesce,"face":face,"uv":tri.map(|i|tile.uv[i as usize]),"positions":p.map(|p|p.to_array())}),
                    );
                }
                if tile.triangles.len() >= 32768 {
                    return Err(json!("triangle cap"));
                }
                tile.triangles.push(tri);
                tile.provenance.push([face, f.body]);
            }
        }
    }
    let area = tile
        .triangles
        .iter()
        .map(|t| crease::area(&t.map(|i| DVec2::from_array(tile.uv[i as usize]))))
        .sum::<f64>();
    if (area - 16384.).abs() > 1e-4 {
        return Err(json!({"reason":"cell area","area":area}));
    }
    let vertices = tile.vertices.len();
    let triangles = tile.triangles.len();
    Ok(crease::Pair {
        feature: base.feature.clone(),
        status: "built",
        seed_owner: base.seed_owner,
        search_faces: base.search_faces,
        source_faces: base.source_faces,
        tiles: vec![tile],
        vertices,
        triangles,
        seam_max_m: 0.,
        seam_points: 0,
        cpu_ms: 0.,
        gpu_expanded_bytes: triangles * 168 + 40,
        retained_cpu_capacity_bytes: 0,
    })
}
