use glam::DVec3;
use serde_json::Value;
use universe_world::worlds::{
    pgs::{Surface, Water},
    sha256,
};

const ROOT: &[u8] = include_bytes!("fixtures/pgs1-v1/root.pgs");
fn check(bytes: &[u8], json: &str) {
    let expected: Value = serde_json::from_str(json).unwrap();
    assert_eq!(sha256(bytes), expected["surface_sha256"]);
    let surface = Surface::read(bytes).unwrap();
    for q in expected["queries"].as_array().unwrap() {
        let dir = DVec3::from_array(std::array::from_fn(|i| q["direction"][i].as_f64().unwrap()));
        let actual = surface.query(dir).unwrap();
        assert_eq!(actual.face as u64, q["face"].as_u64().unwrap(), "{q}");
        assert!(
            (actual.height_m - q["height_m"].as_f64().unwrap()).abs() <= 0.001,
            "{q}: {actual:?}"
        );
        let c = &q["categories"];
        let expected_categories = if c["state"] == "known" {
            Some((c["rock"].as_u64().unwrap() as u16, c["pattern"].as_u64().unwrap() as u16))
        } else { None };
        assert_eq!(actual.categories, expected_categories, "{q}");
        let w = &q["water"];
        match actual.water {
            Water::Unknown => assert_eq!(w["state"], "unknown"),
            Water::Dry => assert_eq!(w["state"], "dry"),
            Water::Wet {
                body,
                level_m,
                depth_m,
            } => {
                assert_eq!(w["state"], "wet");
                assert_eq!(w["body"], body);
                assert!((level_m - w["level_m"].as_f64().unwrap()).abs() <= 0.001);
                assert!((depth_m - w["depth_m"].as_f64().unwrap()).abs() <= 0.001);
            }
        }
    }
}

#[test]
fn malformed_files_and_queries_are_errors() {
    for end in 0..ROOT.len() {
        assert!(Surface::read(&ROOT[..end]).is_err(), "truncation {end}");
    }
    for (offset, byte) in [
        (4, 0),
        (6, 1),
        (8, 0),
        (8, 5),
        (12, 1),
        (16, 1),
        (32, 8),
        (464, 17),
        (959, 1),
    ] {
        let mut bad = ROOT.to_vec();
        bad[offset] = byte;
        assert!(Surface::read(&bad).is_err(), "offset {offset}");
    }
    let s = Surface::read(ROOT).unwrap();
    for q in [
        DVec3::ZERO,
        DVec3::splat(f64::NAN),
        DVec3::splat(f64::INFINITY),
    ] {
        assert!(s.query(q).is_err());
    }
}
#[test]
fn independent_water_domain_goldens() {
    let manifest: Value = serde_json::from_str(include_str!("fixtures/pgs1-water-v1/manifest.json")).unwrap();
    for (name, bytes, queries) in [
        ("ocean_cut", include_bytes!("fixtures/pgs1-water-v1/ocean_cut.pgs").as_slice(), include_str!("fixtures/pgs1-water-v1/ocean_cut.json")),
        ("lake_cut", include_bytes!("fixtures/pgs1-water-v1/lake_cut.pgs").as_slice(), include_str!("fixtures/pgs1-water-v1/lake_cut.json")),
        ("lake_spill_cap", include_bytes!("fixtures/pgs1-water-v1/lake_spill_cap.pgs").as_slice(), include_str!("fixtures/pgs1-water-v1/lake_spill_cap.json")),
    ] {
        for (file, data) in [(format!("{name}.pgs"), bytes), (format!("{name}.json"), queries.as_bytes())] {
            assert_eq!(sha256(data), manifest["files"][&file]["sha256"]);
            assert_eq!(data.len() as u64, manifest["files"][&file]["bytes"].as_u64().unwrap());
        }
        check(bytes, queries);
    }
}

#[test]
fn analytic_derivative_matches_independent_owner_face_goldens() {
    let folder=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pgs1-slope-v1");
    let mut count=0;
    for name in ["root","split","shore","ocean_cut","lake_cut","lake_spill_cap"] {
        let bytes=std::fs::read(folder.join(format!("{name}.pgs"))).unwrap();
        let q:Value=serde_json::from_slice(&std::fs::read(folder.join(format!("{name}.json"))).unwrap()).unwrap();
        assert_eq!(q["surface_sha256"],sha256(&bytes));
        let s=Surface::read(&bytes).unwrap();
        let radius=q["radius_m"].as_f64().unwrap();
        for q in q["queries"].as_array().unwrap() {
            let vec=|key:&str| DVec3::from_array(serde_json::from_value(q[key].clone()).unwrap());
            let dir=vec("direction");
            let (a,d)=s.query_differential(dir,radius).unwrap();
            assert_eq!(q["face"],a.face);
            assert!((a.height_m-q["height_m"].as_f64().unwrap()).abs()<0.001);
            assert!((d.slope-q["slope_rise_run"].as_f64().unwrap()).abs()<1e-10);
            assert!((d.normal-vec("terrain_normal")).abs().max_element()<1e-10);
            let original=s.query(dir).unwrap();
            assert_eq!((a.height_m,a.water,a.categories),(original.height_m,original.water,original.categories));
            let (_,larger)=s.query_differential(dir,radius*2.0).unwrap();
            assert!((larger.gradient-d.gradient).length()<1e-8);
            assert!((larger.slope-d.slope*(radius+a.height_m)/(radius*2.0+a.height_m)).abs()<1e-12);
            count+=1;
        }
        for radius in [0.0,-1.0,f64::INFINITY,f64::NAN] {assert!(s.query_differential(DVec3::X,radius).is_err());}
    }
    assert_eq!(count,330);
}

#[test]
fn fine_oblique_edges_preserve_vertex_ownership() {
    check(
        include_bytes!("fixtures/pgs1-fine-owner/oblique.pgs"),
        include_str!("fixtures/pgs1-fine-owner/oblique.json"),
    );
}
