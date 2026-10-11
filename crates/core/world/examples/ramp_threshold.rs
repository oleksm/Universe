//! Walker traversal on an owner's exported triangle fixture; no production install.
//! ramp_threshold fixture.glb from_x,from_y,from_z to_x,to_y,to_z
//! Inputs are approximate feet positions, snapped down <=1m onto the fixture.
use glam::{DMat4, DQuat, DVec3};
use universe_world::walk::{Collider, Stride, WalkMesh, Walker};
fn point(s: &str) -> Result<DVec3, Box<dyn std::error::Error>> {
    let v = s
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<f64>, _>>()?;
    if v.len() != 3 {
        return Err("expected x,y,z".into());
    }
    Ok(DVec3::new(v[0], v[1], v[2]))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("fixture.glb from_x,from_y,from_z to_x,to_y,to_z".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let g = gltf::Gltf::from_slice(&bytes)?;
    let blob = g.blob.as_deref().ok_or("embedded GLB required")?;
    let mut tris = Vec::new();
    fn visit(n: gltf::Node<'_>, parent: DMat4, blob: &[u8], tris: &mut Vec<[DVec3; 3]>) {
        let m =
            parent * DMat4::from_cols_array_2d(&n.transform().matrix().map(|c| c.map(f64::from)));
        if let Some(mesh) = n.mesh() {
            for p in mesh.primitives() {
                assert_eq!(p.mode(), gltf::mesh::Mode::Triangles);
                let r = p.reader(|_| Some(blob));
                let points: Vec<_> = r
                    .read_positions()
                    .unwrap()
                    .map(|p| m.transform_point3(DVec3::from_array(p.map(f64::from))))
                    .collect();
                let ids: Vec<usize> = r.read_indices().map_or_else(
                    || (0..points.len()).collect(),
                    |r| r.into_u32().map(|v| v as usize).collect(),
                );
                tris.extend(
                    ids.chunks_exact(3)
                        .map(|i| [points[i[0]], points[i[1]], points[i[2]]]),
                );
            }
        }
        for child in n.children() {
            visit(child, m, blob, tris);
        }
    }
    for n in g.default_scene().ok_or("scene")?.nodes() {
        visit(n, DMat4::IDENTITY, blob, &mut tris);
    }
    let mesh = WalkMesh::new(&tris);
    let colliders = [Collider::Mesh {
        mesh: &mesh,
        at: DVec3::ZERO,
        rot: DQuat::IDENTITY,
    }];
    let mut results = Vec::new();
    for (start, end) in [
        (point(&args[1])?, point(&args[2])?),
        (point(&args[2])?, point(&args[1])?),
    ] {
        for hz in [30, 60, 120] {
            let floor =
                universe_world::walk::ray(&colliders, start + DVec3::Y * 0.1, DVec3::NEG_Y, 1.1)
                    .ok_or("start has no supporting floor")?;
            let feet = start + DVec3::Y * (0.1 - floor.0);
            let target = DVec3::new(end.x, feet.y, end.z);
            let direction = (target - feet).normalize();
            let distance = (target - feet).length();
            let speed = 1.5; // Diagnostic walking speed, not a new gameplay parameter.
            let dt = 1.0 / hz as f64;
            let frames = (distance / speed / dt).ceil() as usize;
            let mut walker = Walker {
                feet,
                velocity: DVec3::ZERO,
            };
            let mut airborne = 0;
            let mut unsupported = Vec::new();
            let mut min_y = feet.y;
            let mut max_y = feet.y;
            for _ in 0..frames {
                if !walker.step(
                    &colliders,
                    &|_| DVec3::Y,
                    universe_world::units::STANDARD_GRAVITY,
                    &|_| false,
                    &Stride {
                        wish: direction * speed,
                        jump: 0.0,
                        climb: 0.0,
                    },
                    dt,
                ) {
                    airborne += 1;
                    let support = universe_world::walk::ray(
                        &colliders,
                        walker.feet + DVec3::Y * universe_world::walk::STEP,
                        DVec3::NEG_Y,
                        1.0,
                    );
                    unsupported.push(serde_json::json!({"feet":walker.feet.to_array(),"down_ray":support.map(|(distance,normal)|serde_json::json!({"distance":distance,"normal":normal.to_array()}))}));
                }
                min_y = min_y.min(walker.feet.y);
                max_y = max_y.max(walker.feet.y);
            }
            let progress = (walker.feet - feet).dot(direction);
            let reached_end = progress >= distance - 0.05
                && (walker.feet - target).cross(DVec3::Y).length() < 0.06;
            let continuous_support = airborne == 0;
            let pass = reached_end && continuous_support;
            results.push(serde_json::json!({"reached_end":reached_end,"continuous_support":continuous_support,"start":feet.to_array(),"requested_end":end.to_array(),"final":walker.feet.to_array(),"hz":hz,"speed_m_s":speed,"frames":frames,"airborne_frames":airborne,"unsupported":unsupported,"progress_m":progress,"min_feet_y":min_y,"max_feet_y":max_y,"pass":pass}));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"fixture":args[0],"sha256":universe_world::worlds::sha256(&bytes),"triangles":tris.len(),"gravity_m_s2":universe_world::units::STANDARD_GRAVITY,"results":results,"cargo":"unsupported: no cargo traversal body"})
        )?
    );
    if results.iter().any(|r| r["pass"] != true) {
        return Err(
            "continuous supported traversal gate failed; see reached_end separately".into(),
        );
    }
    Ok(())
}
