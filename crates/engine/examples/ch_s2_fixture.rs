//! Diagnostic consumer for Blender's proposal-v04, not a production motion loader.
//! cargo run -p universe-engine --example ch_s2_fixture -- <fixture.json> <rest.glb>
use glam::{DMat4, DQuat, DVec3, Mat4, Vec3};
use serde::Deserialize;
use std::{collections::HashMap, fs};
use universe_engine::PbrModel;

type Poses = HashMap<String, DMat4>;
#[derive(Deserialize)]
struct Fixture {
    root: String,
    nodes: Vec<Node>,
    gimbal: Gimbal,
    bellows: Vec<Bellows>,
    references: Vec<Reference>,
}
#[derive(Deserialize)]
struct Node {
    node: String,
    parent: Option<String>,
    bind: [f64; 16],
}
#[derive(Deserialize)]
struct Gimbal {
    pitch_node: String,
    yaw_node: String,
    pivot: [f64; 3],
    pitch_axis: [f64; 3],
    yaw_axis: [f64; 3],
    limit: f64,
    normalisation: String,
    order: String,
}
#[derive(Deserialize)]
struct End {
    parent: String,
    point: [f64; 3],
    tangent: [f64; 3],
    hinge_axis: Option<[f64; 3]>,
}
#[derive(Deserialize)]
struct Bellows {
    upstream: End,
    downstream: End,
    tangent_magnitude: f64,
    rings: Vec<Ring>,
}
#[derive(Deserialize)]
struct Ring {
    node: String,
    t: f64,
}
#[derive(Deserialize)]
struct Reference {
    input: [f64; 2],
    transforms: HashMap<String, [f64; 16]>,
}

fn unit(v: DVec3) -> DVec3 {
    assert!(
        v.is_finite() && v.length() > 1e-10,
        "degenerate axis/tangent"
    );
    v.normalize()
}
fn frame(b: &Bellows, t: f64, poses: &Poses) -> DMat4 {
    assert!((0.0..=1.0).contains(&t) && b.tangent_magnitude > 0.0);
    let u = poses[&b.upstream.parent];
    let d = poses[&b.downstream.parent];
    let a = u.transform_point3(b.upstream.point.into());
    let z = d.transform_point3(b.downstream.point.into());
    // Both endpoint tangents point downstream; the second is not an outward normal.
    let ta = unit(u.transform_vector3(b.upstream.tangent.into())) * b.tangent_magnitude;
    let tz = unit(d.transform_vector3(b.downstream.tangent.into())) * b.tangent_magnitude;
    let t2 = t * t;
    let t3 = t2 * t;
    let p = (2.0 * t3 - 3.0 * t2 + 1.0) * a
        + (t3 - 2.0 * t2 + t) * ta
        + (-2.0 * t3 + 3.0 * t2) * z
        + (t3 - t2) * tz;
    let axis_z = unit(
        (6.0 * t2 - 6.0 * t) * a
            + (3.0 * t2 - 4.0 * t + 1.0) * ta
            + (-6.0 * t2 + 6.0 * t) * z
            + (3.0 * t2 - 2.0 * t) * tz,
    );
    let axis_x =
        unit(u.transform_vector3(b.upstream.hinge_axis.expect("upstream hinge axis").into()));
    // This evaluator is restricted to planar, single-hinge bellows.
    assert!(
        axis_x.dot(axis_z).abs() < 1e-6,
        "nonplanar bellows requires another evaluator"
    );
    let axis_y = unit(axis_z.cross(axis_x));
    let axis_x = unit(axis_y.cross(axis_z));
    DMat4::from_cols(
        axis_x.extend(0.0),
        axis_y.extend(0.0),
        axis_z.extend(0.0),
        p.extend(1.0),
    )
}
fn evaluate(f: &Fixture, input: [f64; 2]) -> Poses {
    let bind: Poses = f
        .nodes
        .iter()
        .map(|n| (n.node.clone(), DMat4::from_cols_array(&n.bind)))
        .collect();
    assert_eq!(bind.len(), f.nodes.len(), "duplicate nodes");
    let g = &f.gimbal;
    assert_eq!(g.order, "pitch_then_yaw");
    assert_eq!(g.normalisation, "radial");
    let norm = input[0].hypot(input[1]).max(1.0);
    let pivot = DMat4::from_translation(g.pivot.into());
    let rotate = |axis: [f64; 3], angle| {
        pivot * DMat4::from_quat(DQuat::from_axis_angle(unit(axis.into()), angle)) * pivot.inverse()
    };
    let pitch = rotate(g.pitch_axis, g.limit * input[0] / norm);
    let yaw = pitch * rotate(g.yaw_axis, g.limit * input[1] / norm);
    let mut poses = Poses::new();
    for _ in 0..f.nodes.len() {
        for n in &f.nodes {
            if poses.contains_key(&n.node) {
                continue;
            }
            let current = if n.node == g.pitch_node {
                pitch * bind[&n.node]
            } else if n.node == g.yaw_node {
                yaw * bind[&n.node]
            } else if let Some(parent) = &n.parent {
                let Some(p) = poses.get(parent) else {
                    continue;
                };
                *p * bind[parent].inverse() * bind[&n.node]
            } else {
                assert_eq!(n.node, f.root);
                bind[&n.node]
            };
            poses.insert(n.node.clone(), current);
        }
    }
    assert_eq!(
        poses.len(),
        f.nodes.len(),
        "unknown parent or cyclic hierarchy"
    );
    for b in &f.bellows {
        for r in &b.rings {
            // Solve in root space, then calibrate against the exported neutral basis.
            // This avoids embedding Blender's local-axis conversion in the runtime.
            let current = frame(b, r.t, &poses) * frame(b, r.t, &bind).inverse() * bind[&r.node];
            poses.insert(r.node.clone(), current);
        }
    }
    assert!(poses.values().all(|m| m.is_finite()));
    poses
}
struct Probe {
    name: String,
    bind: Mat4,
    positions: Vec<[f32; 3]>,
    part: u8,
}
fn walk(
    n: gltf::Node,
    parent: Mat4,
    inherited: u8,
    names: &[&str],
    buffers: &[gltf::buffer::Data],
    out: &mut Vec<Probe>,
) {
    let bind = parent * Mat4::from_cols_array_2d(&n.transform().matrix());
    let name = n.name().expect("named fixture node");
    let matches: Vec<_> = names
        .iter()
        .enumerate()
        .filter(|(_, s)| name.contains(**s))
        .collect();
    assert!(matches.len() <= 1, "ambiguous part selector: {name}");
    let part = matches.first().map_or(inherited, |(i, _)| (*i + 1) as u8);
    if let Some(mesh) = n.mesh() {
        for p in mesh.primitives() {
            let positions = p
                .reader(|b| Some(&buffers[b.index()]))
                .read_positions()
                .unwrap()
                .collect();
            out.push(Probe {
                name: name.into(),
                bind,
                positions,
                part,
            });
        }
    }
    for child in n.children() {
        walk(child, bind, part, names, buffers, out);
    }
}
fn max_component(a: DMat4, b: DMat4) -> f64 {
    a.to_cols_array()
        .into_iter()
        .zip(b.to_cols_array())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "usage: ch_s2_fixture fixture.json rest.glb");
    let f: Fixture = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let bytes = fs::read(&args[2]).unwrap();
    let mut names = vec![f.gimbal.pitch_node.as_str(), f.gimbal.yaw_node.as_str()];
    names.extend(
        f.bellows
            .iter()
            .flat_map(|b| b.rings.iter().map(|r| r.node.as_str())),
    );
    assert!(names.len() <= 255);
    let model = PbrModel::load_gltf_parts(&bytes, &names).expect("engine GLB loader");
    let original_id = model.id();
    let (doc, buffers, _) = gltf::import_slice(&bytes).unwrap();
    assert_eq!(doc.animations().count(), 0);
    assert_eq!(doc.skins().count(), 0);
    let mut probes = Vec::new();
    for n in doc.default_scene().unwrap().nodes() {
        walk(n, Mat4::IDENTITY, 0, &names, &buffers, &mut probes);
    }
    assert_eq!(probes.len(), model.data.primitives.len());
    let bind: Poses = f
        .nodes
        .iter()
        .map(|n| (n.node.clone(), DMat4::from_cols_array(&n.bind)))
        .collect();
    let mut bind_error: f64 = 0.0;
    for (p, loaded) in probes.iter().zip(&model.data.primitives) {
        assert_eq!(p.part, loaded.part);
        assert_eq!(p.positions.len(), loaded.vertices.len());
        bind_error = bind_error.max(max_component(p.bind.as_dmat4(), bind[&p.name]));
    }
    assert!(bind_error < 2e-6, "GLB bind mismatch {bind_error}");
    let mut matrix_error: f64 = 0.0;
    let mut vertex_error: f64 = 0.0;
    let placement = Mat4::from_rotation_translation(
        glam::Quat::from_rotation_y(0.37),
        Vec3::new(2.0, -3.0, 5.0),
    );
    for reference in &f.references {
        let poses = evaluate(&f, reference.input);
        assert_eq!(reference.transforms.len(), f.nodes.len());
        for (name, actual) in &poses {
            matrix_error = matrix_error.max(max_component(
                *actual,
                DMat4::from_cols_array(&reference.transforms[name]),
            ));
        }
        for (p, loaded) in probes.iter().zip(&model.data.primitives) {
            // Same per-part delta supplied to Frame::model_pbr_part, including ancestor inheritance.
            let delta = if loaded.part == 0 {
                DMat4::IDENTITY
            } else {
                let name = names[loaded.part as usize - 1];
                poses[name] * bind[name].inverse()
            }
            .as_mat4();
            let expected = DMat4::from_cols_array(&reference.transforms[&p.name]);
            for (local, baked) in p.positions.iter().zip(&loaded.vertices) {
                let actual = placement.transform_point3(delta.transform_point3(baked.pos.into()));
                let expected = placement.transform_point3(
                    expected
                        .transform_point3(Vec3::from(*local).as_dvec3())
                        .as_vec3(),
                );
                vertex_error = vertex_error.max(actual.distance(expected) as f64);
            }
        }
    }
    assert!(matrix_error < 2e-6, "pose mismatch {matrix_error}");
    assert!(
        vertex_error < 3e-6,
        "draw transform mismatch {vertex_error}"
    );
    let mut sweep = 0;
    for radius in [0.0, 0.25, 0.5, 0.75, 1.0] {
        for k in 0..if radius == 0.0 { 1 } else { 128 } {
            let a = k as f64 * std::f64::consts::TAU / 128.0;
            let pose = evaluate(&f, [radius * a.cos(), radius * a.sin()]);
            for m in pose.values() {
                assert!((m.determinant() - 1.0).abs() < 2e-6);
                assert!((m.w_axis.w - 1.0).abs() < 1e-9);
                for v in [m.x_axis, m.y_axis, m.z_axis] {
                    assert!((v.length() - 1.0).abs() < 2e-6);
                }
                assert!(m.x_axis.dot(m.y_axis).abs() < 2e-6);
                assert!(m.y_axis.dot(m.z_axis).abs() < 2e-6);
                assert!(m.z_axis.dot(m.x_axis).abs() < 2e-6);
            }
            sweep += 1;
        }
    }
    assert_eq!(original_id, model.id());
    println!(
        "nodes={} sleeves={} primitives={} parts={} references={} sweep={sweep}",
        f.nodes.len(),
        names.len() - 2,
        probes.len(),
        names.len() + 1,
        f.references.len()
    );
    println!("max bind component error={bind_error:e}; pose component error={matrix_error:e}; placed vertex error={vertex_error:e} m");
    println!("PASS: CPU evaluator + real PBR loader/part transforms; GPU appearance/cost and full assembly clearance untested");
}
