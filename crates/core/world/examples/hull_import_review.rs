//! Read-only importer audit; no hull is added to content and no save is written.
//! Run from the game root: hull_import_review EXACT_MODEL_PATH HULL_REGISTRY_KEY
//! JSON goes to stdout; exit 2 means a discrepancy or an unresolved input.
use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error};
use universe_world::{content::content, registry::registry, worlds::sha256};

// A multiset preserves duplicate fit/nozzle entries: a set would hide them.
fn difference(expected: &[Value], actual: &[Value]) -> Value {
    fn counts(rows: &[Value]) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for row in rows {
            *out.entry(row.to_string()).or_default() += 1;
        }
        out
    }
    fn subtract(a: &BTreeMap<String, usize>, b: &BTreeMap<String, usize>) -> Vec<Value> {
        a.iter()
            .flat_map(|(k, n)| {
                std::iter::repeat_n(
                    serde_json::from_str::<Value>(k).unwrap(),
                    n.saturating_sub(*b.get(k).unwrap_or(&0)),
                )
            })
            .collect()
    }
    let (a, b) = (counts(expected), counts(actual));
    json!({"missing_or_changed": subtract(&a, &b), "added_or_changed": subtract(&b, &a)})
}
fn differs(diff: &Value) -> bool {
    diff["missing_or_changed"].as_array().unwrap().len()
        + diff["added_or_changed"].as_array().unwrap().len()
        > 0
}
fn close(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-8 * a.abs().max(b.abs()).max(1.0)
}

fn review(path: &str, key: &str) -> Result<Value, Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    let reg = registry();
    let record = reg.hull(key);
    let bound: Vec<_> = reg
        .hulls
        .iter()
        .filter(|h| h.model.as_deref() == Some(path))
        .map(|h| &h.identity.key)
        .collect();
    let mut faults = Vec::<String>::new();
    if record.is_none() {
        faults.push(format!("registry hull missing: {key}"));
    }
    if bound.len() != 1 || bound[0] != key {
        faults.push("exact model path does not uniquely bind requested registry hull".into());
    }
    let mut report = json!({
        "format": "freefall-hull-import-review/1",
        "scope": "Read-only hull_from_gltf result, before commission adds a handle. No flight, physical mount, human clearance or motion acceptance.",
        "model_path_argument": path,
        "model_canonical_path": std::fs::canonicalize(path)?.to_string_lossy(),
        "model_sha256": sha256(&bytes), "model_bytes": bytes.len(),
        "requested_registry_key": key, "exact_path_bindings": bound,
        "registry_encoded_sha256": sha256(&reg.encode()),
        "importer_source_sha256": sha256(include_bytes!("../src/import.rs")),
        "mass_consumer_source_sha256": sha256(include_bytes!("../src/goods.rs")),
        "fit_consumer_source_sha256": sha256(include_bytes!("../src/ship.rs")),
        "module_consumer_source_sha256": sha256(include_bytes!("../src/modules.rs")),
        "operational_acceptance": false,
        "record": record,
    });
    let expected_slots: Vec<_> = record.into_iter().flat_map(|h| &h.slots).map(|s|
        json!({"name": s.name, "kind": format!("{:?}", s.kind), "size": s.size, "mount": s.mount})).collect();
    let expected_fit: Vec<_> = record
        .into_iter()
        .flat_map(|h| &h.fit)
        .map(|f| json!({"slot": f.slot, "item": f.item}))
        .collect();
    let expected_nozzles: Vec<_> = record
        .into_iter()
        .flat_map(|h| &h.thrusters)
        .map(|n| json!({"nozzle": n.nozzle, "slot": n.slot, "share": n.share}))
        .collect();
    let mut missing_mass = Vec::new();
    let mass_basis = universe_world::goods::hull_mass(reg, key);
    let frame_mass = mass_basis.as_ref().and_then(|m| m.whole());
    if let Some(universe_world::goods::HullMass::Partial { unweighed, .. }) = &mass_basis {
        missing_mass.extend(unweighed.iter().cloned());
    }
    if frame_mass.is_none_or(|m| !m.is_finite() || m <= 0.0) {
        faults.push("no complete positive registry bare mass; fallback is not acceptance".into());
    }
    let mut equipment_mass = 0.0;
    for f in record.into_iter().flat_map(|h| &h.fit) {
        match reg
            .equipment(&f.item)
            .and_then(|e| e.physical.mass)
            .filter(|m| m.is_finite() && *m > 0.0)
        {
            Some(m) => equipment_mass += m,
            None => missing_mass.push(format!("{}: no equipment mass for {}", f.slot, f.item)),
        }
    }
    if !missing_mass.is_empty() {
        faults.push("incomplete mass evidence (see mass.missing)".into());
    }
    report["mass"] = json!({"registry_bare_kg": frame_mass, "registry_fit_kg": equipment_mass,
        "registry_dry_kg": frame_mass.map(|m| m + equipment_mass), "missing": missing_mass,
        "basis": format!("{mass_basis:?}"),
        "limit": "Record arithmetic only; geometry composition, duplicate baked equipment, crew, cargo and consumables need independent review."});
    if expected_slots.is_empty() {
        faults.push("registry slots empty: inspect inferred slot behavior".into());
    }
    if expected_fit.is_empty() {
        faults.push("registry fit empty: inspect stock-fit behavior".into());
    }
    let mut limits = Vec::new();
    report["equipment_consumers"] = json!(record.into_iter().flat_map(|h| &h.fit).map(|f| {
        let equipment = reg.equipment(&f.item);
        let module = content().handle::<universe_world::modules::Module>(&f.item).map(|h| content().get(h));
        let inert = module.is_some_and(|m| matches!(m.does, universe_world::modules::Does::Inert { .. }));
        if inert {
            limits.push(format!("{}: {} retained inert; nominal needs/function are not operational", f.slot, f.item));
        } else if module.is_none() {
            limits.push(format!("{}: {} absent from native module catalogue", f.slot, f.item));
        }
        json!({"slot": f.slot, "item": f.item, "native_available": module.is_some(), "native_inert": inert,
            "registry_needs_power_w": equipment.and_then(|e| e.needs.power),
            "registry_function": equipment.map(|e| &e.function),
            "native_power_draw_w": module.map(|m| m.power),
            "native_function": module.map(|m| format!("{:?}", m.does))})
    }).collect::<Vec<_>>());
    report["operational_limits"] = json!(limits);
    // This invokes exactly the production importer, but does not commission/add it.
    match universe_world::import::hull_from_gltf(&bytes, path) {
        Err(error) => {
            faults.push(format!("import refused: {error}"));
            report["import_error"] = json!(error);
        }
        Ok(spec) => {
            let slots: Vec<_> = spec.slots.iter().map(|s| json!({"name": s.name, "kind": format!("{:?}", s.kind), "size": s.size, "mount": s.mount})).collect();
            let fit: Vec<_> = spec
                .fit
                .iter()
                .map(|(s, h)| json!({"slot": s, "item": content().get(*h).key}))
                .collect();
            let nozzles: Vec<_> = spec
                .frame
                .nozzles
                .iter()
                .map(|n| json!({"nozzle": n.nozzle, "slot": n.slot, "share": n.share}))
                .collect();
            for (label, expected, actual) in [
                ("slots", &expected_slots, &slots),
                ("fit", &expected_fit, &fit),
                ("thruster_links", &expected_nozzles, &nozzles),
            ] {
                let delta = difference(expected, actual);
                if differs(&delta) {
                    faults.push(format!("{label} differ from registry"));
                }
                report["comparison"][label] = delta;
            }
            if frame_mass.is_some_and(|m| !close(m, spec.frame_mass)) {
                faults.push("effective bare mass differs from registry".into());
            }
            if frame_mass.is_some_and(|m| !close(m + equipment_mass, spec.dry_mass)) {
                faults.push("effective dry mass differs from full registry fit".into());
            }
            let inactive: Vec<_> = expected_nozzles
                .iter()
                .filter(|n| !spec.thrusters.iter().any(|t| n["nozzle"] == t.nozzle))
                .cloned()
                .collect();
            if !inactive.is_empty() {
                faults.push("registry nozzles missing from active thrusters".into());
            }
            let absent_mounts: Vec<_> = expected_fit
                .iter()
                .filter_map(|f| {
                    let node = format!("mount_{}", f["slot"].as_str()?);
                    spec.shape().node(&node).is_none().then_some(node)
                })
                .collect();
            if !absent_mounts.is_empty() {
                faults.push("fitted slots lack authored primary mount nodes".into());
            }
            if !spec.crowded.is_empty() {
                faults.push("import reports crowded/misfit equipment".into());
            }
            report["effective"] = json!({"key":spec.key,"visual":spec.visual,"slots":slots,"fit":fit,"thruster_links":nozzles,
                "bare_kg":spec.frame_mass,"dry_kg":spec.dry_mass,"fuel_capacity_kg":spec.fuel_capacity,"hold_capacity_kg":spec.hold_capacity,
                "shape_made_centre_m":spec.shape().made_centre.to_array(),"dry_com_m":spec.dry_com.to_array(),
                "power_draw_w":spec.power_draw,"power_output_w":spec.power_output,
                "radius_m":spec.radius,"drag_area_m2":spec.drag_area,"hull_strength_j":spec.hull_strength,
                "crowded":spec.crowded,"missing_mount_nodes":absent_mounts,"inactive_registry_nozzles":inactive,
                "thrusters":spec.thrusters.iter().map(|t| json!({"nozzle":t.nozzle,"role":format!("{:?}",t.role),"thrust_n":t.thrust,"position_m":t.at.to_array(),"push":t.push.to_array()})).collect::<Vec<_>>()});
        }
    }
    report["comparison_clear"] = json!(faults.is_empty());
    report["faults"] = json!(faults);
    Ok(report)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: hull_import_review EXACT_MODEL_PATH HULL_REGISTRY_KEY".into());
    }
    let report = review(&args[0], &args[1])?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report["comparison_clear"] != true {
        std::process::exit(2);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropped_and_auto_added_fittings_are_distinct_and_duplicates_count() {
        let command = json!({"slot":"command","item":"equipment.command.s1"});
        let auto = json!({"slot":"drive","item":"equipment.drive.torch.s1"});
        let diff = difference(
            &[command.clone(), command.clone()],
            &[command.clone(), auto.clone()],
        );
        assert_eq!(diff["missing_or_changed"], json!([command]));
        assert_eq!(diff["added_or_changed"], json!([auto]));
    }
    #[test]
    fn wrong_thruster_share_is_visible_but_order_is_irrelevant() {
        let half = json!({"nozzle":"nozzle_main_1","slot":"drive","share":0.5});
        let full = json!({"nozzle":"nozzle_main_1","slot":"drive","share":1.0});
        assert!(differs(&difference(&[half.clone()], &[full.clone()])));
        assert!(!differs(&difference(
            &[half.clone(), full.clone()],
            &[full, half]
        )));
        assert!(!close(f64::NAN, 1000.0));
        assert!(!close(1000.0, 1001.0));
    }
}
