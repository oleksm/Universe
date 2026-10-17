use universe_ship_design::*;
const YAML: &str = include_str!("fixtures/all-layers.ship.yaml");

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
