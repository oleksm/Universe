use universe_ship_design::*;
const YAML: &str = include_str!("fixtures/all-layers.ship.yaml");
const JSON: &str = include_str!("fixtures/all-layers.ship.json");

#[test]
fn every_layer_round_trips_yaml_and_json() {
    let source = parse_source(YAML).unwrap();
    let d = &source.document;
    assert_eq!(d, &parse(JSON).unwrap());
    assert_eq!(d.view.as_ref().unwrap().layers.as_ref().unwrap().len(), 26);
    let encoded = emit(d).unwrap();
    assert_eq!(parse(&encoded).unwrap(), *d);
    assert_eq!(emit(&parse(&encoded).unwrap()).unwrap(), encoded);
    assert_eq!(source.source, YAML);
    assert!(source.source.contains("# Local +Y"));
    assert!(
        source
            .locations
            .contains_key("/modules/engine_left/pose/rotation_xyzw/0")
    );
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ship.schema.json")).unwrap();
    assert!(
        jsonschema::validator_for(&original)
            .unwrap()
            .is_valid(&serde_json::to_value(d).unwrap())
    );
}

#[test]
fn semantic_faults_reject_without_silent_repair() {
    let baseline = parse(YAML).unwrap();
    let mut bad = baseline.clone();
    if let Module::Equipment(e) = bad.modules.get_mut("engine_left").unwrap() {
        e.pose.rotation_xyzw = [0.; 4];
    }
    assert!(!validate(&bad).is_empty());
    assert!(emit(&bad).is_err());
    let mut bad = baseline.clone();
    bad.access
        .groups
        .get_mut("cabin")
        .unwrap()
        .paths
        .push("missing".into());
    assert!(!validate(&bad).is_empty());
    let mut bad = baseline.clone();
    bad.structure.plates.get_mut("main_plate").unwrap().max_xz_m = [-100.; 2];
    assert!(!validate(&bad).is_empty());
    let mut bad = baseline.clone();
    bad.requirements.as_mut().unwrap().crew = Some(f64::NAN);
    assert!(!validate(&bad).is_empty());
    let mut bad = baseline.clone();
    bad.access.ends.get_mut("entry_end").unwrap().hatch = None;
    assert!(!validate(&bad).is_empty());
}
