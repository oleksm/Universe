//! Independent rigid consumer of the measured cargo-ramp source fixture.
//! Usage: cargo_ramp_fixture source-motion.json model.glb [runtime-fixtures.json]
//! Diagnostic only: no production installation or collision acceptance.
use glam::{DMat4, DQuat, DVec3};
use serde_json::Value;
use std::{collections::HashMap, fs};
use universe_engine::PbrModel;

fn matrix(v: &Value) -> DMat4 {
    let rows: [[f64; 4]; 4] = serde_json::from_value(v.clone()).unwrap();
    DMat4::from_cols_array_2d(&rows).transpose()
}
fn vector(v: &Value) -> DVec3 {
    DVec3::from_array(serde_json::from_value(v.clone()).unwrap())
}
fn error(a: DMat4, b: DMat4) -> f64 {
    a.to_cols_array()
        .into_iter()
        .zip(b.to_cols_array())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
}
fn track(parent: DMat4, at: DVec3, target: DVec3) -> DMat4 {
    let (_, rotation, _) = parent.to_scale_rotation_translation();
    let axis = rotation * DVec3::Y; // Blender TRACK_Y; glTF -Z after basis conversion.
    let direction = (target - at).normalize();
    assert!(
        axis.dot(direction) > -1.0 + 1e-8,
        "ambiguous antiparallel track"
    );
    DMat4::from_rotation_translation(DQuat::from_rotation_arc(axis, direction) * rotation, at)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err("source-motion.json model.glb [runtime-fixtures.json]".into());
    }
    let fixture: Value = serde_json::from_slice(&fs::read(&args[0])?)?;
    let runtime: Option<Value> = args
        .get(2)
        .map(|p| -> Result<_, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&fs::read(p)?)?)
        })
        .transpose()?;
    let basis = DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2);
    let mut full_error = 0.0f64;
    let nodes = &fixture["driver_nodes"];
    let poses = fixture["poses"].as_array().ok_or("poses")?;
    assert_eq!(poses.len(), 65);
    let root = matrix(&nodes["MiningShip_Root"]["rest_matrix_world_blender"]);
    let rest = |name: &str| matrix(&nodes[name]["rest_matrix_parent_blender"]);
    let names = [
        "CargoRamp",
        "CargoRam_L_Body",
        "CargoRam_L_Rod",
        "CargoRam_R_Body",
        "CargoRam_R_Rod",
    ];
    let driver = nodes["CargoRamp"]["drivers"][0]["expression"]
        .as_str()
        .ok_or("driver")?;
    let angle: f64 = driver
        .strip_suffix("*c")
        .ok_or("unsupported ramp driver")?
        .parse()?;
    let mut max_error = 0.0f64;
    for (pose_index, pose) in poses.iter().enumerate() {
        let c = pose["control"].as_f64().ok_or("control")?;
        let ramp = root * rest("CargoRamp") * DMat4::from_rotation_x(angle * c);
        let mut evaluated = HashMap::from([("CargoRamp".to_string(), ramp)]);
        for side in ["L", "R"] {
            let body = format!("CargoRam_{side}_Body");
            let rod = format!("CargoRam_{side}_Rod");
            let anchor = format!("CargoRam_{side}_Attach");
            let moving = ramp * rest(&anchor);
            let fixed = root.transform_point3(vector(&poses[0]["nodes"][&body]["location_parent"]));
            let end = moving.transform_point3(DVec3::ZERO);
            for name in [&body, &rod] {
                assert_eq!(nodes[name]["constraints"][0]["type"], "DAMPED_TRACK");
                assert_eq!(nodes[name]["constraints"][0]["axis"], "TRACK_Y");
            }
            evaluated.insert(anchor, moving);
            evaluated.insert(body, track(root, fixed, end));
            evaluated.insert(rod, track(moving, end, fixed));
        }
        evaluated.insert("MiningShip_Root".into(), root);
        evaluated.insert("mount_cargo_ramp".into(), DMat4::IDENTITY);
        evaluated.insert("door_cargo_ramp".into(), ramp * rest("door_cargo_ramp"));
        if let Some(runtime) = &runtime {
            let reference = &runtime["poses"][pose_index];
            assert_eq!(reference["control"].as_f64(), Some(c));
            assert_eq!(reference["nodes"].as_object().unwrap().len(), 10);
            for (name, expected) in reference["nodes"].as_object().unwrap() {
                let e = error(basis * evaluated[name] * basis.inverse(), matrix(expected));
                full_error = full_error.max(e);
                assert!(e < 1e-5, "full glTF pose {name} at {c}: {e}");
            }
        }
        for name in names {
            let e = error(
                evaluated[name],
                matrix(&pose["nodes"][name]["world_blender"]),
            );
            max_error = max_error.max(e);
            assert!(e < 1e-5, "{name} at {c}: matrix error {e}");
        }
    }
    // The exported mesh coordinates already include the source root turn.
    // Compare those full binds against the source rest pose in glTF basis.
    let bytes = fs::read(&args[1])?;
    let gltf = gltf::Gltf::from_slice(&bytes)?;
    fn visit(node: gltf::Node<'_>, parent: DMat4, out: &mut HashMap<String, DMat4>) {
        let m = parent
            * DMat4::from_cols_array_2d(&node.transform().matrix().map(|c| c.map(f64::from)));
        if let Some(name) = node.name() {
            assert!(out.insert(name.into(), m).is_none());
        }
        for child in node.children() {
            visit(child, m, out);
        }
    }
    let mut binds = HashMap::new();
    for node in gltf.default_scene().ok_or("scene")?.nodes() {
        visit(node, DMat4::IDENTITY, &mut binds);
    }
    let basis = DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2);
    let mut bind_error = 0.0f64;
    for name in names
        .into_iter()
        .chain(["door_cargo_ramp", "mount_cargo_ramp"])
    {
        let source = matrix(&nodes[name]["rest_matrix_world_blender"]);
        let e = error(binds[name], basis * source * basis.inverse());
        bind_error = bind_error.max(e);
        assert!(e < 1e-5, "export bind {name}: {e}");
    }
    let model = PbrModel::load_gltf_parts(&bytes, &names)?;
    for part in 1..=5 {
        assert!(model.data.primitives.iter().any(|p| p.part == part));
    }
    assert!(!model.data.primitives.iter().any(|p| p.part == 0));
    println!(
        "{}",
        serde_json::json!({"poses":poses.len(),"groups":5,"max_pose_matrix_error":max_error,"max_gltf_bind_error":bind_error,"all_node_gltf_pose_error":runtime.as_ref().map(|_|full_error),"status":"diagnostic rigid agreement; hull fit and runtime installation not accepted"})
    );
    Ok(())
}
