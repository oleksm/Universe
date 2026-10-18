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
    // Wires survive source round trips by stable module ID, and local graph
    // errors are rejected before registry-specific snap checks run.
    let mut wired = baseline.clone();
    let end = WireEnd {
        module: "engine_left".into(),
        port: "power".into(),
        connector: "connector.test".into(),
    };
    wired
        .modules
        .insert("engine_right".into(), wired.modules["engine_left"].clone());
    if let Module::Equipment(e) = wired.modules.get_mut("engine_right").unwrap() {
        e.slot = None;
    }
    let mut wire = Wire {
        from: end.clone(),
        to: WireEnd {
            module: "engine_right".into(),
            ..end
        },
        cable: "run.test".into(),
        waypoints_m: vec![[1., 2., 3.]],
        pins: vec![WirePin {
            conductor: "1".into(),
            from: "1".into(),
            to: "1".into(),
        }],
    };
    wired.wiring = Some([("feed".into(), wire.clone())].into());
    assert_eq!(parse(&emit(&wired).unwrap()).unwrap(), wired);
    let mut nonfinite = wired.clone();
    nonfinite
        .wiring
        .as_mut()
        .unwrap()
        .get_mut("feed")
        .unwrap()
        .waypoints_m = vec![[f64::NAN, 0., 0.]];
    assert!(
        validate(&nonfinite)
            .iter()
            .any(|d| d.path.contains("waypoints_m"))
    );
    assert!(emit(&nonfinite).is_err());
    wire.to.module = "missing".into();
    wired.wiring.as_mut().unwrap().insert("bad".into(), wire);
    let faults = validate(&wired);
    assert!(
        faults
            .iter()
            .any(|f| f.message.contains("equipment instance"))
    );
    assert!(
        faults
            .iter()
            .any(|f| f.message.contains("already occupied"))
    );
    wired.wiring = Some(Default::default());
    assert!(parse(&emit(&wired).unwrap()).unwrap().wiring.is_some());
}
