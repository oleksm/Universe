use glam::DVec3;
use serde_json::Value;
use universe_world::worlds::{
    pgs::{Surface, Water},
    sha256,
};

const ROOT: &[u8] = include_bytes!("fixtures/pgs1-v1/root.pgs");
const SPLIT: &[u8] = include_bytes!("fixtures/pgs1-v1/split.pgs");
const SHORE: &[u8] = include_bytes!("fixtures/pgs1-v1/shore.pgs");
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
fn independent_goldens() {
    check(ROOT, include_str!("fixtures/pgs1-v1/root.json"));
    check(SPLIT, include_str!("fixtures/pgs1-v1/split.json"));
    check(SHORE, include_str!("fixtures/pgs1-v1/shore.json"));
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/pgs1-v1/manifest.json")).unwrap();
    for (name, bytes) in [
        ("root.pgs", ROOT),
        ("split.pgs", SPLIT),
        ("shore.pgs", SHORE),
    ] {
        assert_eq!(sha256(bytes), manifest["files"][name]["sha256"]);
    }
}
#[test]
fn split_preserves_field() {
    let a = Surface::read(ROOT).unwrap();
    let b = Surface::read(SPLIT).unwrap();
    for i in 0..2000 {
        let y = 1.0 - 2.0 * (i as f64 + 0.5) / 2000.0;
        let t = i as f64 * 2.399963229728653;
        let r = (1.0 - y * y).sqrt();
        let q = DVec3::new(r * t.cos(), y, r * t.sin());
        let expected = 1000.0 * q.y / q.abs().element_sum();
        for surface in [&a, &b] {
            assert!((surface.query(q).unwrap().height_m - expected).abs() < 1e-9);
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
// Change a payload and re-sign its section so topology validation is exercised,
// rather than just the checksum gate.
fn changed_face(offset: usize, value: u32) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut bad = ROOT.to_vec();
    bad[704 + offset..708 + offset].copy_from_slice(&value.to_le_bytes());
    let hash = Sha256::digest(&bad[704..960]);
    bad[432..464].copy_from_slice(&hash);
    bad
}
#[test]
fn topology_is_checked_after_hashes() {
    assert!(Surface::read(&changed_face(0, 900)).is_err());
    assert!(Surface::read(&changed_face(12, 0)).is_err());
    let v = u32::from_le_bytes(ROOT[704..708].try_into().unwrap());
    assert!(Surface::read(&changed_face(4, v)).is_err());
}

#[test]
fn terrain_preserves_unknown_water_and_shared_heights() {
    use universe_world::terrain::{Terrain, TerrainKind};
    let mut terrain = Terrain::new(TerrainKind::Terran, 6.0e6, 17);
    terrain.add_pad(DVec3::NEG_Y);
    terrain.from_surface(std::sync::Arc::new(Surface::read(ROOT).unwrap()));
    let q = DVec3::NEG_Y;
    assert_eq!(terrain.surface_sample(q).unwrap().water, Water::Unknown);
    assert_eq!(terrain.surface_sample(q).unwrap().categories, None);
    assert!(!terrain.is_ocean(q));
    assert!(
        (terrain.surface(q) + 1000.0).abs() < 1e-9,
        "unknown water must not fill below zero"
    );
    assert_eq!(terrain.raw_height(q), terrain.surface(q));
    assert_eq!(terrain.surface(q), terrain.surface_coarse(q));
    assert_eq!(terrain.surface_view(q), (terrain.surface(q), true));
    assert_eq!(
        terrain.height_and_crater(q),
        terrain.height_and_crater_coarse(q)
    );
    assert_eq!(terrain.surface_fields_view(q), (None, true));
    terrain.from_surface(std::sync::Arc::new(Surface::read(SHORE).unwrap()));
    assert!(matches!(
        terrain.surface_sample(q).unwrap().water,
        Water::Wet { body: 7, .. }
    ));
    assert_eq!(terrain.surface(q), 0.0);
    assert!((terrain.raw_height(q) + 1000.0).abs() < 1e-9);
    assert_eq!(terrain.surface(q), terrain.surface_coarse(q));
    assert_eq!(terrain.surface_view(q), (0.0, true));
}

fn table(bytes: &[u8]) -> usize {
    let mut at = 36;
    for _ in 0..7 {
        let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        at += 4 + n;
    }
    at.next_multiple_of(8)
}
fn entry(bytes: &[u8], kind: u32) -> usize {
    let t = table(bytes);
    let n = u32::from_le_bytes(bytes[t..t + 4].try_into().unwrap()) as usize;
    (0..n)
        .map(|i| t + 8 + i * 72)
        .find(|&at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) == kind)
        .unwrap()
}
fn resign(bytes: &mut [u8], kind: u32) {
    use sha2::{Digest, Sha256};
    let e = entry(bytes, kind);
    let offset = u64::from_le_bytes(bytes[e + 16..e + 24].try_into().unwrap()) as usize;
    let len = u64::from_le_bytes(bytes[e + 24..e + 32].try_into().unwrap()) as usize;
    let hash = Sha256::digest(&bytes[offset..offset + len]);
    bytes[e + 40..e + 72].copy_from_slice(&hash);
}
#[test]
fn section_layout_and_capability_dependencies_are_checked() {
    let e = entry(ROOT, 1);
    for (off, value) in [
        (e + 4, 2u64),
        (e + 12, 39),
        (e + 16, 0),
        (e + 24, u64::MAX),
        (e + 32, u64::MAX),
    ] {
        let mut b = ROOT.to_vec();
        b[off..off + 8].copy_from_slice(&value.to_le_bytes());
        assert!(Surface::read(&b).is_err(), "offset {off}");
    }
    let mut b = ROOT.to_vec();
    b[8] = 3;
    assert!(
        Surface::read(&b).is_err(),
        "water capability requires its table"
    );
    let mut b = SHORE.to_vec();
    b[8] = 1;
    assert!(
        Surface::read(&b).is_err(),
        "water table requires capability"
    );
    let mut b = SHORE.to_vec();
    let e = entry(&b, 2);
    let start = u64::from_le_bytes(b[e + 16..e + 24].try_into().unwrap()) as usize;
    b[start + 24..start + 28].copy_from_slice(&123u32.to_le_bytes());
    resign(&mut b, 2);
    assert!(
        Surface::read(&b).is_err(),
        "face refers to missing water body"
    );
}
#[test]
fn unknown_optional_sections_are_checked_and_skipped() {
    use sha2::{Digest, Sha256};
    let mut b = ROOT.to_vec();
    let t = table(&b);
    let end = t + 8 + 2 * 72;
    b[t..t + 4].copy_from_slice(&3u32.to_le_bytes());
    for e in [t + 8, t + 80] {
        let old = u64::from_le_bytes(b[e + 16..e + 24].try_into().unwrap());
        b[e + 16..e + 24].copy_from_slice(&(old + 72).to_le_bytes());
    }
    let mut s = vec![0; 72];
    s[0..4].copy_from_slice(&999u32.to_le_bytes());
    s[4..8].copy_from_slice(&1u32.to_le_bytes());
    s[12..16].copy_from_slice(&8u32.to_le_bytes());
    s[16..24].copy_from_slice(&((ROOT.len() + 72) as u64).to_le_bytes());
    s[24..32].copy_from_slice(&8u64.to_le_bytes());
    s[32..40].copy_from_slice(&1u64.to_le_bytes());
    s[40..72].copy_from_slice(&Sha256::digest(b"optional"));
    b.splice(end..end, s);
    b.extend_from_slice(b"optional");
    let root = Surface::read(&b).unwrap();
    assert_eq!(
        root.query(DVec3::Y).unwrap().face,
        Surface::read(ROOT).unwrap().query(DVec3::Y).unwrap().face
    );
    b[end + 8] = 1;
    assert!(Surface::read(&b).is_err(), "required unknown section");
    b[end + 8] = 0;
    *b.last_mut().unwrap() ^= 1;
    assert!(
        Surface::read(&b).is_err(),
        "optional still needs a valid checksum"
    );
}

#[test]
fn world_charts_and_contacts_share_the_explicit_surface() {
    use std::sync::Arc;
    use universe_world::{World, registry::registry};
    let source = Arc::new(Surface::read(ROOT).unwrap());
    let key = "body.treistun.treistun-e";
    let world = World::with_surfaces(
        registry().galaxy().unwrap().seed as u64,
        Arc::new([(key.to_string(), source.clone())].into_iter().collect()),
    );
    let physics = world.system(world.home_system);
    let charts = world.charts();
    let drawn = charts.system(world.home_system);
    let index = physics.bodies.iter().position(|b| b.key == key).unwrap();
    let b = &physics.bodies[index];
    let t = 12345.0;
    let mut positions = Vec::new();
    physics.positions(t, &mut positions);
    for q in [
        DVec3::X,
        DVec3::NEG_Y,
        DVec3::Y,
        DVec3::new(1.0, -0.3, 0.7).normalize(),
    ] {
        let height = source.query(q).unwrap().height_m;
        let render = drawn.bodies[index]
            .terrain
            .as_ref()
            .unwrap()
            .surface_view(q)
            .0;
        assert!((render - height).abs() < 1e-9);
        assert!((b.surface_radius(q) - b.rail.radius - render).abs() < 1e-6);
        let up = b.rotation(t) * q;
        let centre = positions[index];
        let at = |gap| centre + up * (b.rail.radius + height + 1.0 + gap);
        assert!(
            universe_physics::collide::surface_contact(
                &physics.bodies,
                index,
                t,
                &positions,
                at(0.05),
                DVec3::ZERO,
                1.0
            )
            .is_none()
        );
        let hit = universe_physics::collide::surface_contact(
            &physics.bodies,
            index,
            t,
            &positions,
            at(-0.05),
            DVec3::ZERO,
            1.0,
        )
        .unwrap();
        assert!(matches!(
            hit.feature,
            universe_physics::collide::Feature::Surface { liquid: false }
        ));
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
fn independent_category_goldens() {
    let manifest: Value = serde_json::from_str(include_str!("fixtures/pgs1-categories-v1/manifest.json")).unwrap();
    for (name, bytes, queries) in [
        ("area_ties", include_bytes!("fixtures/pgs1-categories-v1/area_ties.pgs").as_slice(), include_str!("fixtures/pgs1-categories-v1/area_ties.json")),
        ("irregular_support", include_bytes!("fixtures/pgs1-categories-v1/irregular_support.pgs").as_slice(), include_str!("fixtures/pgs1-categories-v1/irregular_support.json")),
        ("shore_inheritance", include_bytes!("fixtures/pgs1-categories-v1/shore_inheritance.pgs").as_slice(), include_str!("fixtures/pgs1-categories-v1/shore_inheritance.json")),
    ] {
        for (file, data) in [(format!("{name}.pgs"), bytes), (format!("{name}.json"), queries.as_bytes())] {
            assert_eq!(sha256(data), manifest["files"][&file]["sha256"]);
            assert_eq!(data.len() as u64, manifest["files"][&file]["bytes"].as_u64().unwrap());
        }
        let surface = Surface::read(bytes).unwrap();
        assert_eq!(surface.provenance[5], "ground-vocabulary.rock@1");
        assert_eq!(surface.provenance[6], "ground-vocabulary.pattern@1");
        check(bytes, queries);
        // Clearing bit 5 leaves stored labels intact but makes them unknown. It must
        // change neither the terrain owner/height nor the independent water owner.
        let mut unknown = bytes.to_vec();
        unknown[8] &= !32;
        let unknown = Surface::read(&unknown).unwrap();
        let qs: Value = serde_json::from_str(queries).unwrap();
        for q in qs["queries"].as_array().unwrap() {
            let dir = DVec3::from_array(std::array::from_fn(|i| q["direction"][i].as_f64().unwrap()));
            let known = surface.query(dir).unwrap();
            let absent = unknown.query(dir).unwrap();
            assert!(known.categories.is_some());
            assert_eq!(absent.categories, None);
            assert_eq!((known.face, known.height_m, known.water), (absent.face, absent.height_m, absent.water));
        }
    }
}

#[test]
fn category_zero_is_explicit_none_and_namespaces_are_required() {
    let bytes = include_bytes!("fixtures/pgs1-categories-v1/area_ties.pgs");
    let mut zero = bytes.to_vec();
    let e = entry(bytes, 2);
    let offset = u64::from_le_bytes(bytes[e + 16..e + 24].try_into().unwrap()) as usize;
    let count = u64::from_le_bytes(bytes[e + 32..e + 40].try_into().unwrap()) as usize;
    for face in 0..count { zero[offset + face * 32 + 28..offset + face * 32 + 32].fill(0); }
    resign(&mut zero, 2);
    let surface = Surface::read(&zero).unwrap();
    assert_eq!(surface.query(DVec3::X).unwrap().categories, Some((0, 0)));
    let mut terrain = universe_world::terrain::Terrain::new(universe_world::terrain::TerrainKind::Terran, 6e6, 0);
    terrain.from_surface(std::sync::Arc::new(surface));
    assert_eq!(terrain.category_vocabularies(), Some(("ground-vocabulary.rock@1", "ground-vocabulary.pattern@1")));
    assert_eq!(terrain.surface_sample(DVec3::X).unwrap().categories, Some((0, 0)));
    terrain.from_surface(std::sync::Arc::new(Surface::read(ROOT).unwrap()));
    assert_eq!(terrain.category_vocabularies(), None);
    for name in [b"ground-vocabulary.rock@1".as_slice(), b"ground-vocabulary.pattern@1".as_slice()] {
        let at = bytes.windows(name.len()).position(|w| w == name).unwrap();
        for (off, value) in [(0, b' '), (name.len() - 1, b'0'), (name.len() - 2, b'_')] {
            let mut bad = bytes.to_vec();
            bad[at + off] = value;
            assert!(Surface::read(&bad).unwrap_err().contains("vocabularies"));
        }
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
