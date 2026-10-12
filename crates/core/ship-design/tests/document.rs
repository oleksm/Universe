use serde_json::json;
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
fn empty_document_is_complete_and_stable() {
    let d = Document::new("new-design", "on");
    assert!(validate(&d).is_empty());
    assert_eq!(parse(&emit(&d).unwrap()).unwrap(), d);
}
#[test]
fn strings_use_yaml_12_core_semantics() {
    for s in ["on", "off", "yes", "no", "2026-10-11"] {
        let input = YAML.replace("All layers schema study", s);
        assert_eq!(parse(&input).unwrap().design.name, s);
    }
    let input = YAML.replace("crew: 2", "crew: 010");
    assert_eq!(parse(&input).unwrap().requirements.unwrap().crew, Some(10.));
}
#[test]
fn restricted_yaml_profile_is_enforced() {
    for input in [
        YAML.replace("name: All layers schema study", "name: one\n  name: two"),
        YAML.replace("name: All layers schema study", "name: &alias hello"),
        YAML.replace("name: All layers schema study", "name: !custom hello"),
        YAML.replace("name: All layers schema study", "name: *alias"),
        YAML.replace("name: All layers schema study", "<<: {}\n  name: hi"),
        YAML.replace("crew: 2", "crew: .nan"),
        YAML.replace("crew: 2", "crew: 1e999"),
        format!("{YAML}\n---\n{{}}"),
        YAML.replace("%YAML 1.2", "%YAML 1.1"),
        YAML.replace("  engine_left:", "  42:"),
    ] {
        assert!(parse(&input).is_err(), "accepted {input}");
    }
}
#[test]
fn diagnostics_have_stable_paths_and_source_locations() {
    let input = YAML.replace("to: cockpit", "to: missing");
    let e = parse(&input).unwrap_err();
    let d = e
        .diagnostics
        .iter()
        .find(|d| d.path == "/access/paths/corridor/to")
        .unwrap();
    assert_eq!(
        d.location.unwrap().line,
        input
            .lines()
            .position(|s| s.contains("to: missing"))
            .unwrap()
            + 1
    );
    let duplicate = YAML.replace(
        "name: All layers schema study",
        "name: first\n  name: second",
    );
    let e = parse(&duplicate).unwrap_err();
    assert_eq!(e.diagnostics[0].path, "/design/name");
    assert!(e.diagnostics[0].location.is_some());
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
#[test]
fn generated_and_draft_schema_agree_on_structural_mutations() {
    let draft = jsonschema::validator_for(
        &serde_json::from_str(include_str!("fixtures/ship.schema.json")).unwrap(),
    )
    .unwrap();
    let generated = jsonschema::validator_for(&schema()).unwrap();
    let good: serde_json::Value = serde_json::from_str(JSON).unwrap();
    for (path, value) in [
        ("/version", json!("later")),
        ("/modules/engine_left/envelope_m/0", json!(0)),
        ("/modules/engine_left/pose/rotation_xyzw", json!([0, 0, 1])),
        ("/decks", json!({"bad id":{}})),
        ("/access/groups/cabin/paths", json!([""])),
        ("/requirements/crew", json!(-1)),
        ("/requirements/crew", json!(null)),
        ("/design/name", json!("")),
        ("/hull", json!(null)),
    ] {
        let mut v = good.clone();
        *v.pointer_mut(path).unwrap() = value;
        assert_eq!(
            draft.is_valid(&v),
            generated.is_valid(&v),
            "schema mismatch at {path}: {v}"
        );
    }
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("hull");
    assert!(!generated.is_valid(&missing));
}
#[test]
fn resource_limits_reject_before_authoring_mutation() {
    assert!(
        parse_with_limits(
            YAML,
            Limits {
                max_bytes: 100,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_with_limits(
            YAML,
            Limits {
                max_depth: 2,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_with_limits(
            YAML,
            Limits {
                max_nodes: 10,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn scalar_looking_strings_emit_without_reinterpretation() {
    for s in [
        "0o17",
        "0x2A",
        "True",
        "NULL",
        ".NaN",
        "1e9",
        "18446744073709551615",
        "on",
        "αβ船",
        "line\nbreak",
        "quote\"slash\\",
    ] {
        let d = Document::new("example", s);
        assert_eq!(parse(&emit(&d).unwrap()).unwrap(), d, "string {s}");
    }
    for s in ["+0o17", "-0x2A"] {
        let input = YAML.replace("All layers schema study", s);
        assert_eq!(parse(&input).unwrap().design.name, s);
    }
}
#[test]
fn field_errors_are_local_to_the_selected_module() {
    let input = YAML.replace("envelope_m: [1.5, 1.5, 2.3]", "envelope_m: [-1, 1.5, 2.3]");
    let e = parse(&input).unwrap_err();
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.path == "/modules/engine_left/envelope_m/0" && d.location.is_some())
    );
}
#[test]
fn deck_connections_and_wall_shapes_are_checked() {
    let mut d = parse(YAML).unwrap();
    let deck = d.decks.values_mut().next().unwrap();
    deck.ladders.insert(
        "dangling".into(),
        Ladder {
            at_xz_m: [0., 0.],
            to_deck: "unknown".into(),
        },
    );
    let errors = validate(&d);
    assert!(
        errors
            .iter()
            .any(|e| e.path.ends_with("/ladders/dangling/to_deck"))
    );
    let mut d = parse(YAML).unwrap();
    let wall = d
        .decks
        .values_mut()
        .flat_map(|d| d.walls.values_mut())
        .next()
        .unwrap();
    wall.bulges_m.clear();
    assert!(validate(&d).iter().any(|e| e.path.ends_with("/bulges_m")));
}
#[test]
fn generated_schema_snapshot_is_current() {
    let frozen: serde_json::Value =
        serde_json::from_str(include_str!("../ship.schema.json")).unwrap();
    assert_eq!(schema(), frozen);
}

#[test]
fn thousand_module_document_roundtrip_and_limits() {
    let baseline = parse(YAML).unwrap();
    let prototype = baseline.modules["engine_left"].clone();
    let mut d = Document::new("load-test", "1000 module parser probe");
    for i in 0..1000 {
        d.modules.insert(format!("engine_{i}"), prototype.clone());
    }
    let start = std::time::Instant::now();
    let text = emit(&d).unwrap();
    let emit_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = std::time::Instant::now();
    let parsed = parse(&text).unwrap();
    let parse_ms = start.elapsed().as_secs_f64() * 1000.;
    assert_eq!(d, parsed);
    eprintln!(
        "1000 modules: {} bytes, emit {emit_ms:.2}ms, parse+validate {parse_ms:.2}ms (dev build)",
        text.len()
    );
    assert!(
        parse_with_limits(
            &text,
            Limits {
                max_bytes: text.len() - 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
}
