//! Read-only reproduction of terrain_lod's settled patch sampling and shader morph.
//! PGS_WALK_HANDOFF selects handoff corridor probes; PGS_WALK_AT_EACH uses a 1.7m eye
//! at every probe. PGS_WALK_TERRAIN_DISTANCE and PGS_WALK_MAX_LEVEL are explicit
//! diagnostic controls, not runtime switches or source terrain changes.
//! Not a capture of asynchronous GPU residency. Usage: surface.pgs lat lon absolute_h heading down
use glam::DVec3;
use serde_json::json;
use std::collections::BTreeMap;
use universe_world::worlds::{
    LonLat, direction,
    pgs::{Surface, Water},
    sha256,
};
const GRID: f64 = 16.;
const SPLIT: f64 = 2.4;
fn face(f: usize, u: f64, v: f64) -> DVec3 {
    let (u, v) = (
        (u * std::f64::consts::FRAC_PI_4).tan(),
        (v * std::f64::consts::FRAC_PI_4).tan(),
    );
    match f {
        0 => DVec3::new(1., -v, -u),
        1 => DVec3::new(-1., -v, u),
        2 => DVec3::new(u, 1., v),
        3 => DVec3::new(u, -1., -v),
        4 => DVec3::new(u, -v, 1.),
        _ => DVec3::new(-u, -v, -1.),
    }
    .normalize()
}
fn cube(d: DVec3) -> (usize, f64, f64) {
    let a = d.abs();
    let (f, u, v) = if a.x >= a.y && a.x >= a.z {
        if d.x > 0. {
            (0, -d.z / a.x, -d.y / a.x)
        } else {
            (1, d.z / a.x, -d.y / a.x)
        }
    } else if a.y >= a.z {
        if d.y > 0. {
            (2, d.x / a.y, d.z / a.y)
        } else {
            (3, d.x / a.y, -d.z / a.y)
        }
    } else if d.z > 0. {
        (4, d.x / a.z, -d.y / a.z)
    } else {
        (5, -d.x / a.z, -d.y / a.z)
    };
    (
        f,
        u.atan() / std::f64::consts::FRAC_PI_4,
        v.atan() / std::f64::consts::FRAC_PI_4,
    )
}
fn visible(s: &Surface, d: DVec3) -> f64 {
    let q = s.query(d).unwrap();
    match q.water {
        Water::Wet { level_m, .. } => level_m,
        _ => q.height_m,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(7..=8).contains(&args.len()) {
        return Err(
            "surface.pgs lat lon absolute_h heading down output.json [render_radius_m]".into(),
        );
    }
    let bytes = std::fs::read(&args[0])?;
    let clock = std::time::Instant::now();
    let s = Surface::read(&bytes)?;
    let decode_ms = clock.elapsed().as_secs_f64()*1000.;
    let query_clock = std::time::Instant::now();
    let r = args
        .get(7)
        .map(|v| v.parse::<f64>())
        .transpose()?
        .unwrap_or(s.radius_m);
    if !r.is_finite() || r <= 0. {
        return Err("invalid render radius".into());
    }
    let v: Vec<f64> = args[1..6].iter().map(|v| v.parse().unwrap()).collect();
    let up = direction(LonLat {
        lat: v[0],
        lon: v[1],
    });
    let north = (DVec3::Y - up * up.y).normalize();
    let east = north.cross(up);
    let ahead = north * v[3].to_radians().cos() + east * v[3].to_radians().sin();
    let forward = (ahead - up * v[4]).normalize();
    let eye = up * (r + v[2]);
    let mut levels = BTreeMap::new();
    let mut height = Vec::<f64>::new();
    let mut normals = Vec::<f64>::new();
    let mut cells = Vec::<f64>::new();
    let mut behind = 0;
    let mut wet = 0;
    let mut hit = None;
    let mut samples = Vec::new();
    // A 20km square on the frozen camera tangent plane, 250m sample spacing.
    let mut probes: Vec<(f64,f64,DVec3)> = (-40..=40).flat_map(|y| (-40..=40).map(move |x| (x as f64*250.,y as f64*250.,(up+(east*x as f64+north*y as f64)*250./r).normalize()))).collect();
    if let Ok(path) = std::env::var("PGS_WALK_HANDOFF") {
        let h: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        probes = h["corridor"].as_array().ok_or("corridor")?.iter().map(|p| {
            let a = p["unit_direction"].as_array().unwrap();
            (p["local_xy_m"][0].as_f64().unwrap(),p["local_xy_m"][1].as_f64().unwrap(),DVec3::new(a[0].as_f64().unwrap(),a[1].as_f64().unwrap(),a[2].as_f64().unwrap()))
        }).collect();
    }
    for (x,y,d) in probes {
        {
            let eye = if std::env::var_os("PGS_WALK_AT_EACH").is_some() { d*(r+visible(&s,d)+1.7) } else { eye };
            let (f, u, w) = cube(d);
            let mut level = 0;
            let (mut ix, mut iy) = (0., 0.);
            loop {
                let n = (1u32 << level) as f64;
                let mid = face(f, (ix + 0.5) / n * 2. - 1., (iy + 0.5) / n * 2. - 1.);
                let size = std::f64::consts::FRAC_PI_2 / n * r;
                let terrain_aware = std::env::var_os("PGS_WALK_TERRAIN_DISTANCE").is_some();
                let max_level = std::env::var("PGS_WALK_MAX_LEVEL").ok().and_then(|s| s.parse().ok()).unwrap_or(15);
                let centre = mid * (r + if terrain_aware { visible(&s,mid) } else { 0. });
                if level == max_level || centre.distance(eye) >= SPLIT * size {
                    break;
                }
                level += 1;
                let n = (1u32 << level) as f64;
                ix = ((u + 1.) * 0.5 * n).floor().min(n - 1.);
                iy = ((w + 1.) * 0.5 * n).floor().min(n - 1.);
            }
            *levels.entry(level).or_insert(0usize) += 1;
            let n = (1u32 << level) as f64;
            let size = std::f64::consts::FRAC_PI_2 / n * r;
            cells.push(size / GRID);
            let mid = face(f, (ix + 0.5) / n * 2. - 1., (iy + 0.5) / n * 2. - 1.);
            let origin = mid * (r + visible(&s, mid));
            let raw = |i: f64, j: f64| {
                let d = face(
                    f,
                    (ix + i / GRID) / n * 2. - 1.,
                    (iy + j / GRID) / n * 2. - 1.,
                );
                (d * (r + visible(&s, d)) - origin).as_vec3()
            };
            let vertex = |i: u32, j: u32| {
                let own = raw(i as f64, j as f64);
                let target = match (i % 2, j % 2) {
                    (0, 0) => own,
                    (1, 0) => (raw(i as f64 - 1., j as f64) + raw(i as f64 + 1., j as f64)) * 0.5,
                    (0, 1) => (raw(i as f64, j as f64 - 1.) + raw(i as f64, j as f64 + 1.)) * 0.5,
                    _ => {
                        (raw(i as f64 - 1., j as f64 - 1.) + raw(i as f64 + 1., j as f64 + 1.))
                            * 0.5
                    }
                };
                let far = (SPLIT * 2. * size * 0.95) as f32;
                let distance = (origin + own.as_dvec3() - eye).length() as f32;
                let t = ((distance - far * 0.6) / (far * 0.4)).clamp(0., 1.);
                origin + (own + (target - own) * (t * t * (3. - 2. * t))).as_dvec3()
            };
            let gx = ((u + 1.) * 0.5 * n - ix) * GRID;
            let gy = ((w + 1.) * 0.5 * n - iy) * GRID;
            let i = (gx.floor() as u32).min(15);
            let j = (gy.floor() as u32).min(15);
            let a = vertex(i, j);
            let c = vertex(i + 1, j + 1);
            let b = if gx - i as f64 >= gy - j as f64 {
                vertex(i + 1, j)
            } else {
                vertex(i, j + 1)
            };
            let mut normal = (b - a).cross(c - a).normalize();
            if normal.dot(d) < 0. {
                normal = -normal
            }
            let radial = normal.dot(a) / normal.dot(d);
            let (q, derivative) = s.query_differential(d, r)?;
            if matches!(q.water, Water::Wet { .. }) {
                wet += 1;
                if std::env::var_os("PGS_WALK_INCLUDE_WATER").is_none() { continue; }
            }
            let contact_height = match q.water { Water::Wet { level_m, .. } => level_m, _ => q.height_m };
            height.push(radial - r - contact_height);
            samples.push(json!({"x_m":x,"y_m":y,"level":level,"canonical_height_m":contact_height,"terrain_height_m":q.height_m,"water":format!("{:?}",q.water),"render_height_m":radial-r,"error_m":radial-r-contact_height}));
            normals.push(normal.angle_between(if matches!(q.water,Water::Wet {..}) {d} else {derivative.normal}).to_degrees());
            if (d * (r + q.height_m) - eye).dot(forward) < 0. {
                behind += 1;
            }
        }
    }
    // Centre sightline against canonical visible height; march then bisect first crossing.
    let mut before = 0.;
    for k in 1..=2000 {
        let t = k as f64 * 100.;
        let p = eye + forward * t;
        if p.length() - r - visible(&s, p.normalize()) <= 0. {
            let mut lo = before;
            let mut hi = t;
            for _ in 0..40 {
                let m = (lo + hi) * 0.5;
                let p = eye + forward * m;
                if p.length() - r - visible(&s, p.normalize()) > 0. {
                    lo = m
                } else {
                    hi = m
                }
            }
            let p = eye + forward * hi;
            let ll = universe_world::worlds::lon_lat(p);
            hit = Some(json!({"range_m":hi,"lat":ll.lat,"lon":ll.lon}));
            break;
        }
        before = t;
    }
    let stats = |v: &Vec<f64>| {
        let mut a = v.iter().map(|x| x.abs()).collect::<Vec<_>>();
        a.sort_by(f64::total_cmp);
        json!({"max":a.last(),"p95":a[a.len()*95/100],"rms":(v.iter().map(|x|x*x).sum::<f64>()/v.len()as f64).sqrt()})
    };
    let result = json!({"terrain_aware_distance":std::env::var_os("PGS_WALK_TERRAIN_DISTANCE").is_some(),"max_level_override":std::env::var("PGS_WALK_MAX_LEVEL").ok(),"decode_index_ms":decode_ms,"audit_ms":query_clock.elapsed().as_secs_f64()*1000.,"samples":samples,"surface_sha256":sha256(&bytes),"radius_m":r,"source_radius_m":s.radius_m,"camera":{"lat":v[0],"lon":v[1],"absolute_h":v[2],"heading":v[3],"down":v[4]},"each_sample_eye_1_7m":std::env::var_os("PGS_WALK_AT_EACH").is_some(),"corridor":std::env::var_os("PGS_WALK_HANDOFF").is_some(),"scope":"settled ideal LOD; default20km square/250m spacing or explicit handoff corridor; f32 patch/morph reconstruction, not actual GPU residency","levels":levels,"nominal_cell_m":{"min":cells.iter().copied().fold(f64::INFINITY,f64::min),"max":cells.iter().copied().fold(0.,f64::max)},"dry_samples":height.len(),"wet_queries":wet,"wet_included":std::env::var_os("PGS_WALK_INCLUDE_WATER").is_some(),"height_error_m":stats(&height),"normal_angle_error_deg":stats(&normals),"behind_camera":behind,"centre_ray_hit":hit});
    std::fs::write(&args[6], serde_json::to_vec_pretty(&result)?)?;
    println!("{}", result);
    Ok(())
}
