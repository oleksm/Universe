//! Bounded canonical sightline screen for frozen development cameras.
use glam::DVec3;
use serde_json::{Value, json};
use universe_world::worlds::{
    LonLat, direction,
    pgs::{Surface, Water},
    sha256,
};
fn height(s: &Surface, d: DVec3) -> f64 {
    let q = s.query(d).unwrap();
    match q.water {
        Water::Wet { level_m, .. } => level_m,
        _ => q.height_m,
    }
}
fn ray(s: &Surface, r: f64, eye: DVec3, target: DVec3) -> Value {
    let delta = target - eye;
    let range = delta.length();
    let n = (range / 25.).ceil() as usize;
    let mut min = f64::INFINITY;
    let mut first = None;
    // Exclude the final 25m to avoid counting target self-contact as an obstruction.
    for i in 1..n {
        let t = i as f64 / n as f64;
        if range * (1. - t) < 25. {
            continue;
        }
        let p = eye + delta * t;
        let clearance = p.length() - r - height(s, p.normalize());
        min = min.min(clearance);
        if clearance < -0.001 && first.is_none() {
            first = Some(range * t);
        }
    }
    json!({"range_m":range,"minimum_sampled_clearance_m":min,"first_occlusion_range_m":first,"occluded":first.is_some()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(a.len(), 4, "surface cameras sidecar output");
    let b = std::fs::read(&a[0])?;
    let s = Surface::read(&b)?;
    let cameras: Value = serde_json::from_slice(&std::fs::read(&a[1])?)?;
    let side: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    let r = cameras["render_radius_m"].as_f64().unwrap();
    let mut views = vec![];
    for c in cameras["proposed_cameras"].as_array().unwrap() {
        let num = |k: &str| c[k].as_f64().unwrap();
        let up = direction(LonLat {
            lat: num("lat"),
            lon: num("lon"),
        });
        let eye = up * (r + num("absolute_height_m"));
        let north = (DVec3::Y - up * up.y).normalize();
        let east = north.cross(up);
        let angle = num("heading_deg").to_radians();
        let forward = (north * angle.cos() + east * angle.sin() - up * num("down")).normalize();
        let right = forward.cross(up).normalize();
        let camup = right.cross(forward);
        let in_view = |p: DVec3| {
            let v = p - eye;
            let z = v.dot(forward);
            z > 0.
                && v.dot(right).abs() <= z * (std::f64::consts::PI / 6.).tan() * 16. / 9.
                && v.dot(camup).abs() <= z * (std::f64::consts::PI / 6.).tan()
        };
        let target = cameras["local_camera_targets"]
            .as_object()
            .unwrap()
            .values()
            .find(|v| v["point_id"] == c["target_point_id"])
            .unwrap();
        let td = direction(LonLat {
            lat: target["lat"].as_f64().unwrap(),
            lon: target["lon"].as_f64().unwrap(),
        });
        let mut layers = vec![];
        for layer in side["layers"].as_array().unwrap() {
            let id = layer["id"].as_str().unwrap();
            if !["reference_rivers", "native_rivers", "connector"].contains(&id) {
                continue;
            }
            let mut near = 0;
            let mut front = 0;
            let mut clear = 0;
            let mut samples = vec![];
            for (index, p) in layer["points"].as_array().unwrap().iter().enumerate() {
                let d = DVec3::new(
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                )
                .normalize();
                let xyz = d * (r + height(&s, d));
                if xyz.distance(eye) > 30000. {
                    continue;
                }
                near += 1;
                if !in_view(xyz) {
                    continue;
                }
                front += 1;
                let test = ray(&s, r, eye, xyz);
                if !test["occluded"].as_bool().unwrap() {
                    clear += 1
                }
                samples.push(json!({"index":index,"ray":test}));
            }
            layers.push(json!({"id":id,"points_within_30km":near,"points_in_frustum":front,"points_sampled_unoccluded":clear,"samples":samples}));
        }
        let ground = height(&s, up);
        views.push(json!({"camera":c,"launch_alt_m":num("absolute_height_m")-ground,"ground_at_camera_m":ground,"target_ground_height_m":height(&s,td),"target_ground_in_frustum":in_view(td*(r+height(&s,td))),"target_ground_ray":ray(&s,r,eye,td*(r+height(&s,td))),"fixed_candidate_target_ray":ray(&s,r,eye,td*(r+target["height_m"].as_f64().unwrap())),"channel_layers":layers}));
    }
    std::fs::write(
        &a[3],
        serde_json::to_vec_pretty(
            &json!({"surface_sha256":sha256(&b),"host_radius_m":r,"source_radius_m":s.radius_m,"scope":"25m or finer ray samples, final25m omitted, 1mm occlusion tolerance; nominal ship eye; no continuous clearance guarantee; channels resampled on tested surface","views":views}),
        )?,
    )?;
    Ok(())
}
