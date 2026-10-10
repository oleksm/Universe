//! Find diagnostic low-sun terrain occlusions near a supplied location; no changes to terrain.
use glam::DVec3;
use std::sync::Arc;
use universe_world::{
    worlds::{direction, lon_lat, pgs::Surface, LonLat},
    World,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let file = args.first().ok_or("surface.pgs [time_s]")?;
    let time = args.get(1).map_or(Ok(2412.0), |s| s.parse::<f64>())?;
    let s = Arc::new(Surface::read(&std::fs::read(file)?)?);
    let seed = universe_world::registry::registry().galaxy().unwrap().seed as u64;
    let world = World::with_surfaces(
        seed,
        Arc::new(
            [("body.treistun.treistun-e".into(), s)]
                .into_iter()
                .collect(),
        ),
    );
    let sys = world.system(world.home_system);
    let (i, b) = sys
        .bodies
        .iter()
        .enumerate()
        .find(|(_, b)| b.key == "body.treistun.treistun-e")
        .unwrap();
    let mut positions = Vec::new();
    sys.positions(time, &mut positions);
    let light = b.rotation(time).inverse() * (positions[0] - positions[i]).normalize();
    let terrain = b.terrain.as_ref().unwrap();
    let r = b.rail.radius;
    let mut hits = Vec::new();
    // Diagnostic search grid, not added detail: 0.1 degree steps around S15's
    // measured high point. Rays sample 250 m steps out to 40 km, a screening only.
    for y in -20..=20 {
        for x in -20..=20 {
            let ll = LonLat {
                lat: -50.927589054482716 + y as f64 * 0.1,
                lon: -19.166702846855568 + x as f64 * 0.1,
            };
            let d = direction(ll);
            let h = terrain.surface(d);
            let p = d * (r + h);
            let e = d.any_orthonormal_vector();
            let f = d.cross(e);
            let step = 100.0 / r;
            let sample = |v: DVec3| {
                let q = (d + v * step).normalize();
                q * (r + terrain.surface(q))
            };
            let n = (sample(e) - sample(-e))
                .cross(sample(f) - sample(-f))
                .normalize();
            let n = if n.dot(d) < 0.0 { -n } else { n };
            if n.dot(light) < 0.002 {
                continue;
            }
            let mut deepest = 0.0f64;
            let mut hit = 0.0;
            for k in 1..=160 {
                let distance = k as f64 * 250.0;
                let ray = p + n * 40.0 + light * distance;
                let delta = r + terrain.surface(ray.normalize()) - ray.length();
                if delta > deepest {
                    deepest = delta;
                    hit = distance;
                }
            }
            if deepest > 10.0 {
                let ll = lon_lat(d);
                hits.push(serde_json::json!({"lat":ll.lat,"lon":ll.lon,"height":h,"sun_elevation":d.dot(light).asin().to_degrees(),"slope_sun_dot":n.dot(light),"occlusion_m":deepest,"ray_distance_m":hit}));
            }
        }
    }
    hits.sort_by(|a, b| {
        b["occlusion_m"]
            .as_f64()
            .unwrap()
            .total_cmp(&a["occlusion_m"].as_f64().unwrap())
    });
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"time_s":time,"sun_direction":light.to_array(),"candidates":hits.into_iter().take(12).collect::<Vec<_>>()})
        )?
    );
    Ok(())
}
