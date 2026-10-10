//! Cross-reader acceptance of an external lab export. No installation or registry edits.
//! cargo run -p universe-world --release --example pgs_accept -- EXPORT_DIR [PREVIEW.png]
use glam::DVec3;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Instant};
use universe_world::{
    terrain::{Terrain, TerrainKind},
    worlds::{
        pgs::{Surface, Water},
        sha256,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(1..=2).contains(&args.len()) {
        return Err("usage: pgs_accept EXPORT_DIR [PREVIEW.png]".into());
    }
    let dir = Path::new(&args[0]);
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json"))?)?;
    if manifest["format"] != "planet-graph-surface/1" {
        return Err("unexpected manifest format".into());
    }
    let checked = |name: &str| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(dir.join(name))?;
        let entry = &manifest["files"][name];
        if entry["sha256"] != sha256(&bytes) || entry["bytes"].as_u64() != Some(bytes.len() as u64)
        {
            return Err(format!("manifest mismatch: {name}").into());
        }
        Ok(bytes)
    };
    let bytes = checked("surface.pgs")?;
    let queries: Value = serde_json::from_slice(&checked("query_fixtures.json")?)?;
    if queries["surface_sha256"] != sha256(&bytes) {
        return Err("query fixtures name another surface".into());
    }
    let start = Instant::now();
    let surface = Arc::new(Surface::read(&bytes)?);
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut error = 0.0_f64;
    let qs = queries["queries"].as_array().ok_or("missing queries")?;
    if qs.is_empty() {
        return Err("empty query fixture".into());
    }
    for q in qs {
        let a: [f64; 3] = serde_json::from_value(q["direction"].clone())?;
        let actual = surface.query(DVec3::from_array(a))?;
        let difference = (actual.height_m - q["height_m"].as_f64().ok_or("missing height")?).abs();
        error = error.max(difference);
        if difference > 0.001 || q["face"].as_u64() != Some(actual.face as u64) {
            return Err(format!("height/ownership mismatch: {q}: {actual:?}").into());
        }
        let expected = &q["water"];
        let water_ok = match actual.water {
            Water::Unknown => expected["state"] == "unknown",
            Water::Dry => expected["state"] == "dry",
            Water::Wet {
                body,
                level_m,
                depth_m,
            } => {
                expected["state"] == "wet"
                    && expected["body"] == body
                    && expected["level_m"]
                        .as_f64()
                        .is_some_and(|v| (v - level_m).abs() <= 0.001)
                    && expected["depth_m"]
                        .as_f64()
                        .is_some_and(|v| (v - depth_m).abs() <= 0.001)
            }
        };
        if !water_ok {
            return Err(format!("water mismatch: {q}: {actual:?}").into());
        }
    }
    // Fixed directions, independent of the lab query accelerator. Coherent and
    // distributed batches include water evaluation and the full ownership traversal.
    let mut costs = Vec::new();
    for coherent in [true, false] {
        let directions: Vec<_> = (0..10000)
            .map(|i| {
                if coherent {
                    return DVec3::new(1.0, 0.1 + i as f64 * 1e-6, 0.25).normalize();
                }
                let y = 1.0 - 2.0 * (i as f64 + 0.5) / 10000.0;
                let t = i as f64 * 2.399963229728653;
                let r = (1.0 - y * y).sqrt();
                DVec3::new(r * t.cos(), y, r * t.sin())
            })
            .collect();
        let start = Instant::now();
        for q in &directions {
            std::hint::black_box(surface.query(*q)?);
        }
        costs.push(start.elapsed().as_secs_f64() * 1e6 / directions.len() as f64);
    }
    if let Some(path) = args.get(1) {
        let mut terrain = Terrain::new(TerrainKind::Terran, surface.radius_m, 0);
        terrain.from_surface(surface.clone());
        let (width, height) = (512, 256);
        let mut heights = Vec::with_capacity(width * height);
        for y in 0..height {
            let lat = std::f64::consts::PI * (0.5 - (y as f64 + 0.5) / height as f64);
            for x in 0..width {
                let lon = std::f64::consts::TAU * ((x as f64 + 0.5) / width as f64 - 0.5);
                let q = DVec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
                heights.push(terrain.surface(q));
            }
        }
        let lo = heights.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = heights.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // Neutral height image: there is no blue ocean or material inferred from height.
        let pixels: Vec<u8> = heights
            .iter()
            .map(|h| (255.0 * (h - lo) / (hi - lo).max(1.0)).round() as u8)
            .collect();
        let mut encoder =
            png::Encoder::new(std::fs::File::create(path)?, width as u32, height as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.add_text_chunk("Description".into(), format!("PGS1 development preview via Terrain::surface; height range {lo}..{hi} m; water/categories follow capabilities {}", surface.capabilities))?;
        encoder.write_header()?.write_image_data(&pixels)?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "surface_sha256": sha256(&bytes), "bytes": bytes.len(), "points": surface.point_count(), "faces": surface.face_count(),
            "capabilities": surface.capabilities, "queries_passed": qs.len(), "max_error_m": error,
            "load_ms": load_ms, "coherent_query_us": costs[0], "distributed_query_us": costs[1],
            "preview": args.get(1), "installed": false
        }))?
    );
    Ok(())
}
