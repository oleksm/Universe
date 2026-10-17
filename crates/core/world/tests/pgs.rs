use glam::DVec3;
use serde_json::Value;
use universe_world::worlds::{
    pgs::Surface,
    sha256,
};

const ROOT: &[u8] = include_bytes!("fixtures/pgs1-v1/root.pgs");
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
