//! Diagnostic only: imported MC-07 mass/inertia and candidate sockets, no installation.
//! mc07_rcs_review SOCKETS.json SOCKETS.glb [LP_TARGETS.json]
use glam::{DMat4, DVec3};
use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error};
use universe_world::{import::hull_from_gltf, ship::ThrusterRole, thrusters, worlds::sha256};
fn vector(v: &Value) -> DVec3 { DVec3::from_array(serde_json::from_value(v.clone()).unwrap()) }
fn frames(node: gltf::Node<'_>, parent: DMat4, out: &mut BTreeMap<String, DMat4>) {
    let m = parent * DMat4::from_cols_array_2d(&node.transform().matrix().map(|c| c.map(f64::from)));
    if let Some(n) = node.name().filter(|n| n.starts_with("nozzle_")) { assert!(out.insert(n.into(), m).is_none()); }
    for child in node.children() { frames(child, m, out); }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) { return Err("SOCKETS.json SOCKETS.glb [LP_TARGETS.json]".into()); }
    let candidate_bytes = std::fs::read(&args[0])?;
    let candidate: Value = serde_json::from_slice(&candidate_bytes)?;
    let model = std::fs::read("assets/models/mc07.glb")?;
    assert_eq!(candidate["source_glb_sha256"], sha256(&model));
    let spec = hull_from_gltf(&model, "assets/models/mc07.glb")?;
    let centre = spec.shape().made_centre;
    let fixture_bytes = std::fs::read(&args[1])?;
    let glb = gltf::Gltf::from_slice(&fixture_bytes)?;
    let mut nodes = BTreeMap::new();
    for node in glb.default_scene().ok_or("no scene")?.nodes() { frames(node, DMat4::IDENTITY, &mut nodes); }
    let mut changed = spec.thrusters.clone();
    let mut seen = std::collections::BTreeSet::new();
    let mut frame_error = 0.0f64;
    for n in candidate["nozzles"].as_array().ok_or("missing nozzles")? {
        let name = n["node"].as_str().ok_or("node name")?;
        assert!(seen.insert(name));
        let at = vector(&n["origin_hull_gltf_m"]);
        let exhaust = vector(&n["exhaust_direction_hull_gltf"]);
        assert!((exhaust.length() - 1.0).abs() < 2e-6);
        let cols: [f64;16] = serde_json::from_value(n["matrix_hull_gltf_column_major"].clone())?;
        let m = nodes.get(name).ok_or("missing GLB frame")?;
        for (a,b) in m.to_cols_array().iter().zip(cols) { frame_error = frame_error.max((a-b).abs()); }
        assert!(m.transform_point3(DVec3::ZERO).distance(at) < 2e-5);
        assert!(m.transform_vector3(DVec3::NEG_Z).normalize().distance(exhaust) < 2e-6);
        let t = changed.iter_mut().find(|t| t.nozzle == name).ok_or("unknown candidate nozzle")?;
        if t.role != ThrusterRole::Rcs {
            assert!((at-centre).distance(t.at) < 2e-5, "main/lift location changed: {name}");
            assert!((-exhaust).distance(t.push) < 2e-6, "main/lift axis changed: {name}");
        } else { assert_eq!(n["slot"], "thrusters"); }
        t.at = at-centre;
        t.push = -exhaust.normalize();
    }
    assert_eq!(seen.len(), spec.thrusters.len());
    assert_eq!(nodes.len(), seen.len());
    assert_eq!(changed.iter().filter(|t| t.role == ThrusterRole::Rcs).count(), 20);
    assert!(frame_error < 2e-5, "fixture matrix error {frame_error}");
    let targets: Option<Value> = args.get(2).map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap());
    let mut cases = Vec::new();
    for (layout, all) in [("baseline", &spec.thrusters), ("distributed", &changed)] {
        for rating in [120000.0, 6000.0] {
            let mut rcs: Vec<_> = all.iter().filter(|t| t.role == ThrusterRole::Rcs).cloned().collect();
            for t in &mut rcs { assert!((t.thrust-120000.0).abs()<1e-6); t.thrust=rating; }
            let mut flight = all.clone();
            for t in &mut flight { if t.role == ThrusterRole::Rcs { t.thrust = rating; } }
            for (load_name, fuel, load) in [("dry",0.0,0.0),("full_fuel",spec.fuel_capacity,0.0),("full_hold",spec.fuel_capacity,spec.hold_capacity)] {
                let id = format!("{layout}-{}-{load_name}",rating as u32);
                let mass = spec.dry_mass + fuel + load;
                let com = spec.centre_of_mass(fuel,load);
                let inertia = spec.inertia(fuel,load);
                let columns: Vec<_> = rcs.iter().map(|t| {
                    let force = t.push*t.thrust;
                    let angular = inertia.inverse()*(t.at-com).cross(force);
                    let linear = force/mass;
                    [linear.x,linear.y,linear.z,angular.x,angular.y,angular.z]
                }).collect();
                let mut controls = Vec::new();
                if let Some(targets) = &targets {
                    let data = targets["cases"].as_array().unwrap().iter().find(|c| c["id"] == id).ok_or("missing LP case")?;
                    for target in data["targets"].as_array().unwrap() {
                        let wanted: [f64;6] = serde_json::from_value(target["acceleration"].clone())?;
                        let force = DVec3::from_slice(&wanted[..3])*mass;
                        let torque = inertia*DVec3::from_slice(&wanted[3..]);
                        for (mode, jets) in [("rcs_only", &rcs), ("flight_main_off", &flight)] {
                        let mut u = Vec::new();
                        let mut samples = Vec::new();
                        for step in 0..120 {
                            let (f,q) = thrusters::allocate_with(jets,com,mass,inertia,Some(0.0),force,torque,&mut u);
                            if step==0 || step==119 {
                                let a=f/mass; let b=inertia.inverse()*q;
                                samples.push(json!({"step":step+1,"acceleration":[a.x,a.y,a.z,b.x,b.y,b.z],"duty":u}));
                            }
                        }
                        controls.push(json!({"name":target["name"],"mode":mode,"wanted":wanted,"samples":samples}));
                        }
                    }
                }
                cases.push(json!({"id":id,"layout":layout,"rcs_rating_n":rating,"fuel_kg":fuel,"cargo_kg":load,
                    "mass_kg":mass,"com_shape_m":com.to_array(),"inertia_column_major":inertia.to_cols_array(),
                    "rcs_names":rcs.iter().map(|t| &t.nozzle).collect::<Vec<_>>(),"acceleration_columns":columns,
                    "runtime_turn_envelope":thrusters::turn_envelope(&rcs,com,mass,inertia).to_array(),"controls":controls}));
            }
        }
    }
    println!("{}",serde_json::to_string_pretty(&json!({"scope":"static RCS-only review; current imported mass/inertia, no new hardware mass, no installation",
        "model_sha256":sha256(&model),"sockets_json_sha256":sha256(&candidate_bytes),"sockets_glb_sha256":sha256(&fixture_bytes),
        "max_frame_component_error":frame_error,"made_centre_m":centre.to_array(),"cases":cases}))?);
    Ok(())
}
