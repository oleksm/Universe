//! Process-local, explicit development display settings. Never part of terrain/physics.
use std::collections::BTreeMap;
use universe_sim::{Body, world::worlds::pgs::Sample};

pub struct Palette {
    body: String,
    vocabularies: [String; 2],
    colours: BTreeMap<u16, [u8; 4]>,
    materials: bool,
}
impl Palette {
    pub fn materials(&self) -> bool { self.materials }
    pub fn colour(&self, sample: Sample) -> [u8; 4] {
        // Unknown and explicitly unassigned stay neutral; no lookup by source-array index.
        sample.categories.filter(|(rock, _)| *rock != 0)
            .and_then(|(rock, _)| self.colours.get(&rock).copied()).unwrap_or([0; 4])
    }
}

#[cfg(feature = "dev")]
static PALETTE: std::sync::OnceLock<Option<Palette>> = std::sync::OnceLock::new();

#[cfg(feature = "dev")]
static SHADOWS: std::sync::OnceLock<(String, bool)> = std::sync::OnceLock::new();

/// Opt-in canonical mountain casters; legacy worlds keep their existing shadows.
pub fn terrain_shadows(body: &Body) -> bool {
    if !body.terrain.as_ref().is_some_and(|t| t.canonical_surface()) { return true; }
    #[cfg(feature = "dev")]
    { SHADOWS.get().is_some_and(|(key, enabled)| key == &body.key && *enabled) }
    #[cfg(not(feature = "dev"))]
    { false }
}

pub fn palette(body: &Body) -> Option<&'static Palette> {
    fn configured() -> Option<&'static Palette> {
        #[cfg(feature = "dev")]
        { PALETTE.get()?.as_ref() }
        #[cfg(not(feature = "dev"))]
        { None }
    }
    let p = configured()?;
    let namespaces = body.terrain.as_ref()?.category_vocabularies()?;
    (p.body == body.key && namespaces == (p.vocabularies[0].as_str(), p.vocabularies[1].as_str())).then_some(p)
}

#[cfg(feature = "dev")]
pub fn configure(folder: &std::path::Path, body: &str, surface: &universe_sim::world::worlds::pgs::Surface, surface_hash: &str, manifest: &serde_json::Value) -> Result<(), String> {
    crate::pgs_debug::configure(body, surface, surface_hash)?;
    let shadows = match std::env::var("UNIVERSE_PGS1_SHADOWS").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("off") => false,
        Ok("on") => true,
        _ => return Err("UNIVERSE_PGS1_SHADOWS must be on or off".into()),
    };
    let mode = std::env::var("UNIVERSE_PGS1_COLOURS").unwrap_or_else(|_| "neutral".into());
    let palette = match mode.as_str() {
        "neutral" => None,
        "categories" | "materials" => {
            if surface.capabilities & 32 == 0 { return Err("category colours require explicit PGS1 category capability".into()); }
            let bytes = std::fs::read(folder.join("surface.json")).map_err(|e| e.to_string())?;
            let entry = &manifest["files"]["surface.json"];
            if entry["sha256"] != universe_sim::world::worlds::sha256(&bytes) || entry["bytes"].as_u64() != Some(bytes.len() as u64) {
                return Err("category legend manifest/hash/size mismatch".into());
            }
            let json = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let mut palette = parse(body, [&surface.provenance[5], &surface.provenance[6]], surface_hash, &json)?;
            if mode == "materials" { palette.use_materials(&json); }
            Some(palette)
        }
        _ => return Err("UNIVERSE_PGS1_COLOURS must be neutral, categories or materials".into()),
    };
    SHADOWS.set((body.to_owned(), shadows)).map_err(|_| "PGS1 shadows already configured".to_string())?;
    PALETTE.set(palette).map_err(|_| "PGS1 display already configured".to_string())
}

#[cfg(any(feature = "dev", test))]
fn parse(body: &str, vocabularies: [&str; 2], surface_hash: &str, json: &serde_json::Value) -> Result<Palette, String> {
    if json["export"]["sha256"] != surface_hash { return Err("category legend belongs to another surface".into()); }
    let categories = &json["categories"];
    for (kind, name) in ["rock", "pattern"].into_iter().zip(vocabularies) {
        if categories["vocabularies"][kind]["name"] != name { return Err("category legend vocabulary mismatch".into()); }
    }
    let mut colours = BTreeMap::new();
    for entry in categories["legend"].as_array().ok_or("missing category legend")? {
        let id = entry["id"].as_u64().and_then(|v| u16::try_from(v).ok()).filter(|v| *v != 0).ok_or("invalid legend rock ID")?;
        let source = entry["source"].as_str().ok_or("missing legend source")?;
        if categories["vocabularies"]["rock"]["mapping"][source] != id { return Err("legend ID differs from frozen mapping".into()); }
        let rgb: [u8; 3] = serde_json::from_value(entry["colour"].clone()).map_err(|_| "invalid legend RGB")?;
        if colours.insert(id, [rgb[0], rgb[1], rgb[2], 255]).is_some() { return Err("duplicate legend ID".into()); }
    }
    if colours.is_empty() { return Err("empty category legend".into()); }
    Ok(Palette { body: body.to_owned(), vocabularies: vocabularies.map(str::to_owned), colours, materials: false })
}

// Existing legacy shader ROCK values: visual tuning, never physical PGS reflectance.
// Explicit frozen source-name mapping; a test below keeps values tied to the shader.
#[cfg(any(feature = "dev", test))]
const ROCK_LOOK: [(&str, [f32; 3]); 20] = [
    ("morb", [0.102, 0.089, 0.076]),
    ("arc_volcanic", [0.195, 0.138, 0.112]),
    ("arc_plutonic", [0.434, 0.352, 0.287]),
    ("basement", [0.392, 0.305, 0.231]),
    ("greenstone", [0.150, 0.156, 0.107]),
    ("schist", [0.305, 0.262, 0.223]),
    ("granite", [0.527, 0.413, 0.352]),
    ("flood_basalt", [0.076, 0.058, 0.045]),
    ("carbonate", [0.672, 0.604, 0.468]),
    ("clastic", [0.617, 0.413, 0.195]),
    ("ophiolite", [0.122, 0.112, 0.080]),
    ("rift", [0.413, 0.122, 0.065]),
    ("iron_formation", [0.262, 0.065, 0.034]),
    ("primary_crust", [0.305, 0.262, 0.223]),
    ("intercrater_plains", [0.238, 0.205, 0.171]),
    ("high_ti_basalt", [0.058, 0.061, 0.076]),
    ("impact_melt", [0.133, 0.114, 0.102]),
    ("impact_breccia", [0.468, 0.423, 0.361]),
    ("anorthosite", [0.552, 0.539, 0.503]),
    ("kreep", [0.195, 0.150, 0.112]),
];
#[cfg(any(feature = "dev", test))]
impl Palette {
    fn use_materials(&mut self, json: &serde_json::Value) {
        self.materials = true;
        let known = self.colours.clone();
        self.colours.clear();
        if self.vocabularies[0] != "ground-vocabulary.rock@1" { return; }
        let srgb = |x: f32| -> u8 {
            let x = if x <= 0.0031308 { 12.92*x } else { 1.055*x.powf(1.0/2.4)-0.055 };
            (x.clamp(0.0,1.0)*255.0).round() as u8
        };
        for (source, linear) in ROCK_LOOK {
            if let Some(id) = json["categories"]["vocabularies"]["rock"]["mapping"][source].as_u64()
                .and_then(|v| u16::try_from(v).ok()).filter(|v| *v != 0) {
                if !known.contains_key(&id) || !json["categories"]["legend"].as_array().is_some_and(|entries| entries.iter().any(|e| e["id"] == id && e["source"] == source)) { continue; }
                self.colours.insert(id, [srgb(linear[0]),srgb(linear[1]),srgb(linear[2]),255]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use universe_sim::world::worlds::pgs::Water;
    const VOCABS: [&str; 2] = ["ground-vocabulary.rock@1", "ground-vocabulary.pattern@1"];
    fn legend() -> Value {
        json!({"export":{"sha256":"test-surface"},"categories":{
            "vocabularies":{"rock":{"name":VOCABS[0],"mapping":{"morb":1}},"pattern":{"name":VOCABS[1]}},
            "legend":[{"id":1,"source":"morb","colour":[62,74,92]}]
        }})
    }
    #[test]
    fn material_names_are_explicit_and_legacy_visual_values_stay_in_sync() {
        let shader = include_str!("../../engine/src/shaders/ground_material.wgsl");
        let table = shader.split("const ROCK:").nth(1).unwrap().split("const SAND").next().unwrap();
        let colours: Vec<[f32;3]> = table.split("vec3<f32>(").skip(1).map(|s| {
            let values: Vec<f32> = s.split(')').next().unwrap().split(',').map(|v| v.trim().parse().unwrap()).collect();
            values.try_into().unwrap()
        }).collect();
        assert_eq!(colours, ROCK_LOOK.iter().map(|(_,c)| *c).collect::<Vec<_>>());
        let mut json = legend();
        // A frozen ID is not a legacy table index: MORB can be assigned another ID.
        json["categories"]["legend"][0]["id"] = json!(17);
        json["categories"]["vocabularies"]["rock"]["mapping"]["morb"] = json!(17);
        let mut p = parse("test", VOCABS, "test-surface", &json).unwrap();
        p.use_materials(&json);
        let mut sample = Sample {face:0,height_m:0.0,water:Water::Unknown,categories:Some((17,0))};
        assert_eq!(p.colour(sample), [90,84,78,255]);
        for id in [0,1,65535] { sample.categories=Some((id,0)); assert_eq!(p.colour(sample),[0;4]); }
        sample.categories=None; assert_eq!(p.colour(sample),[0;4]);
        p.vocabularies[0]="other@1".into(); p.use_materials(&json);
        sample.categories=Some((17,0)); assert_eq!(p.colour(sample),[0;4]);
    }

    #[test]
    fn frozen_id_lookup_preserves_unknown_unassigned_and_unmapped() {
        let palette = parse("test", VOCABS, "test-surface", &legend()).unwrap();
        let mut sample = Sample { face: 0, height_m: -100.0, water: Water::Unknown, categories: None };
        assert_eq!(palette.colour(sample), [0;4]);
        for categories in [Some((0,0)), Some((2,0))] {
            sample.categories = categories;
            assert_eq!(palette.colour(sample), [0;4]);
        }
        sample.categories = Some((1,143)); // Patterns remain independent; this is a rock palette.
        assert_eq!(palette.colour(sample), [62,74,92,255]);
        assert_eq!(sample.water, Water::Unknown);
        assert_eq!(sample.height_m, -100.0);
    }
    #[test]
    fn legend_cannot_change_surface_namespace_mapping_or_colour_type() {
        let valid = legend();
        assert!(parse("test", VOCABS, "another-surface", &valid).is_err());
        for path in ["rock", "pattern"] {
            let mut bad = valid.clone();
            bad["categories"]["vocabularies"][path]["name"] = json!("another@1");
            assert!(parse("test", VOCABS, "test-surface", &bad).is_err());
        }
        for colour in [json!([256,0,0]), json!([1.5,0,0]), json!([0,0]), json!("blue")] {
            let mut bad = valid.clone(); bad["categories"]["legend"][0]["colour"] = colour;
            assert!(parse("test", VOCABS, "test-surface", &bad).is_err());
        }
        for id in [0,2,65536] {
            let mut bad = valid.clone(); bad["categories"]["legend"][0]["id"] = json!(id);
            assert!(parse("test", VOCABS, "test-surface", &bad).is_err());
        }
        let mut duplicate = valid.clone();
        duplicate["categories"]["legend"].as_array_mut().unwrap().push(valid["categories"]["legend"][0].clone());
        assert!(parse("test", VOCABS, "test-surface", &duplicate).is_err());
    }
}
