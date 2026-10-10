//! Process-local, explicit development display settings. Never part of terrain/physics.
use std::collections::BTreeMap;
use universe_sim::{Body, world::worlds::pgs::Sample};

pub struct Palette {
    body: String,
    vocabularies: [String; 2],
    colours: BTreeMap<u16, [u8; 4]>,
}
impl Palette {
    pub fn colour(&self, sample: Sample) -> [u8; 4] {
        // Unknown and explicitly unassigned stay neutral; no lookup by source-array index.
        sample.categories.filter(|(rock, _)| *rock != 0)
            .and_then(|(rock, _)| self.colours.get(&rock).copied()).unwrap_or([0; 4])
    }
}

#[cfg(feature = "dev")]
static PALETTE: std::sync::OnceLock<Option<Palette>> = std::sync::OnceLock::new();

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
    let mode = std::env::var("UNIVERSE_PGS1_COLOURS").unwrap_or_else(|_| "neutral".into());
    let palette = match mode.as_str() {
        "neutral" => None,
        "categories" => {
            if surface.capabilities & 32 == 0 { return Err("category colours require explicit PGS1 category capability".into()); }
            let bytes = std::fs::read(folder.join("surface.json")).map_err(|e| e.to_string())?;
            let entry = &manifest["files"]["surface.json"];
            if entry["sha256"] != universe_sim::world::worlds::sha256(&bytes) || entry["bytes"].as_u64() != Some(bytes.len() as u64) {
                return Err("category legend manifest/hash/size mismatch".into());
            }
            let json = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            Some(parse(body, [&surface.provenance[5], &surface.provenance[6]], surface_hash, &json)?)
        }
        _ => return Err("UNIVERSE_PGS1_COLOURS must be neutral or categories".into()),
    };
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
    Ok(Palette { body: body.to_owned(), vocabularies: vocabularies.map(str::to_owned), colours })
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
