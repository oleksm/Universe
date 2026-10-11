//! Read-only analytic derivative cross-reader agreement: surface.pgs queries.json.
use glam::DVec3;
use serde_json::Value;
use universe_world::worlds::{pgs::Surface, sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("surface.pgs queries.json".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let q: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    assert_eq!(q["surface_sha256"], sha256(&bytes));
    let surface = Surface::read(&bytes)?;
    let radius = q["radius_m"].as_f64().ok_or("radius")?;
    let mut errors = [0.0f64; 3];
    let queries = q["queries"].as_array().ok_or("queries")?;
    for expected in queries {
        let v =
            |key: &str| DVec3::from_array(serde_json::from_value(expected[key].clone()).unwrap());
        let (sample, d) = surface.query_differential(v("direction"), radius)?;
        assert_eq!(expected["face"], sample.face);
        errors[0] = errors[0].max((sample.height_m - expected["height_m"].as_f64().unwrap()).abs());
        errors[1] = errors[1].max((d.slope - expected["slope_rise_run"].as_f64().unwrap()).abs());
        errors[2] = errors[2].max((d.normal - v("terrain_normal")).abs().max_element());
    }
    assert!(
        errors[0] <= 0.001 && errors[1] <= 1e-10 && errors[2] <= 1e-10,
        "{errors:?}"
    );
    println!(
        "{}",
        serde_json::json!({"queries":queries.len(),"height_error_m":errors[0],"slope_error":errors[1],"normal_error":errors[2]})
    );
    Ok(())
}
