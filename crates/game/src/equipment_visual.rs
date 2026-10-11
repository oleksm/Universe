//! Shared immutable, hash-checked equipment geometry. Consumers decide whether
//! a neutral preview or a fully operational mounted mechanism is permitted.
use glam::{DMat4, Vec3};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock},
};
use universe_engine::PbrModel;
use universe_sim::world::assets;

pub struct Visual {
    pub model: PbrModel,
    pub faces: Arc<Vec<Face>>,
    pub centre: Vec3,
    pub nodes: BTreeMap<String, DMat4>,
    pub has_motion: bool,
}

pub fn get(key: &str) -> Option<Arc<Visual>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Visual>>>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache
        .entry(key.into())
        .or_insert_with(|| {
            let record = assets::visual(key)?;
            let read = || -> Result<Visual, String> {
                let package = assets::model(record)?;
                // A sidecar can describe future motion while the GLB remains a
                // valid neutral rigid model. Do not silently ignore skin/morph data.
                assets::mounts::require_static(&package.glb)?;
                let nodes = assets::mounts::frames(&package.glb)?;
                let model = PbrModel::load_gltf(&package.glb)?;
                let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                for v in model.data.primitives.iter().flat_map(|p| &p.vertices) {
                    let p = Vec3::from(v.pos);
                    if !p.is_finite() {
                        return Err("non-finite model vertex".into());
                    }
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
                if !lo.is_finite() || !hi.is_finite() {
                    return Err("empty model".into());
                }
                let mut faces = Vec::new();
                for primitive in &model.data.primitives {
                    let material = model.data.materials.get(primitive.material);
                    for idx in primitive.indices.chunks_exact(3) {
                        let vs = [idx[0], idx[1], idx[2]].map(|i| primitive.vertices[i as usize]);
                        faces.push(Face {
                            positions: vs.map(|v| Vec3::from(v.pos)),
                            normals: vs.map(|v| Vec3::from(v.normal)),
                            colour: universe_engine::Color(
                                material.map_or([0.6, 0.6, 0.6, 1.], |m| m.base_color),
                            ),
                            double_sided: material.is_some_and(|m| m.double_sided),
                        });
                    }
                }
                Ok(Visual {
                    faces: Arc::new(faces),
                    model,
                    centre: (lo + hi) * 0.5,
                    nodes,
                    has_motion: package.has_motion,
                })
            };
            match read() {
                Ok(v) => {
                    log::info!("equipment visual: loaded {key} for neutral drawing");
                    Some(Arc::new(v))
                }
                Err(e) => {
                    log::warn!("equipment visual: {key}: {e}; retaining design envelope");
                    None
                }
            }
        })
        .clone()
}

#[derive(Clone)]
pub struct Face {
    pub positions: [Vec3; 3],
    pub normals: [Vec3; 3],
    pub colour: universe_engine::Color,
    pub double_sided: bool,
}

/// One resolved visual, shared by design, walk, balance and test drive. Physical
/// envelopes remain independent. transform maps authored metres into design space.
#[derive(Clone)]
pub struct Placed {
    pub faces: Arc<Vec<Face>>,
    pub transform: glam::Mat4,
    pub installed: bool,
}

pub fn rotation(thrust: Option<Vec3>) -> glam::Quat {
    thrust
        .and_then(Vec3::try_normalize)
        .map_or(glam::Quat::IDENTITY, |d| {
            glam::Quat::from_rotation_arc(Vec3::Y, d)
        })
}

pub fn placed(
    key: &str,
    at: Vec3,
    thrust: Option<Vec3>,
    fallback: impl FnOnce() -> Vec<[Vec3; 3]>,
) -> Placed {
    if let Some(v) = get(key) {
        let r = rotation(thrust);
        return Placed {
            faces: v.faces.clone(),
            transform: glam::Mat4::from_rotation_translation(r, at - r * v.centre),
            installed: true,
        };
    }
    let faces = fallback()
        .into_iter()
        .map(|positions| {
            let normal = (positions[1] - positions[0])
                .cross(positions[2] - positions[0])
                .normalize_or_zero();
            Face {
                positions,
                normals: [normal; 3],
                colour: universe_engine::Color::rgb(0.55, 0.5, 0.62),
                double_sided: true,
            }
        })
        .collect();
    Placed {
        faces: Arc::new(faces),
        transform: glam::Mat4::IDENTITY,
        installed: false,
    }
}

pub struct Camera {
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub focal: f32,
    pub centre: glam::Vec2,
    pub near: f32,
}

pub struct Projected {
    pub depth: f32,
    pub points: [glam::Vec2; 3],
    pub distances: [f32; 3],
    pub near: f32,
    pub colours: [universe_engine::Color; 3],
}
impl Projected {
    pub fn draw(&self, frame: &mut universe_engine::Frame) {
        frame.canvas_triangle(self.points, self.distances, self.near, self.colours);
    }
}

/// Camera adaptation, shading, near clipping and ordering have one implementation.
/// parent is identity in design/walk; flight supplies pose * translation(-COM).
pub fn project(placed: &[Placed], parent: glam::Mat4, camera: &Camera) -> Vec<Projected> {
    use universe_engine::Color;
    let light = (-camera.forward + camera.up * 0.7 - camera.right * 0.4).normalize();
    let mut output = Vec::new();
    for item in placed {
        let matrix = parent * item.transform;
        for face in item.faces.iter() {
            let positions = face.positions.map(|p| matrix.transform_point3(p));
            let geometric = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
            if !face.double_sided && geometric.dot(camera.eye - positions[0]) <= 0. {
                continue;
            }
            let vertices: Vec<_> = (0..3)
                .map(|i| {
                    let delta = positions[i] - camera.eye;
                    let p = Vec3::new(
                        delta.dot(camera.right),
                        delta.dot(camera.up),
                        delta.dot(camera.forward),
                    );
                    let n = matrix
                        .transform_vector3(face.normals[i])
                        .normalize_or_zero();
                    let shade = 0.3 + 0.7 * n.dot(light).max(0.);
                    let c = glam::Vec4::from_array(face.colour.0)
                        * glam::Vec4::new(shade, shade, shade, 1.);
                    (p, c)
                })
                .collect();
            let mut clipped = Vec::new();
            for i in 0..3 {
                let (a, b) = (vertices[i], vertices[(i + 1) % 3]);
                if a.0.z >= camera.near {
                    clipped.push(a);
                }
                if (a.0.z >= camera.near) != (b.0.z >= camera.near) {
                    let t = (camera.near - a.0.z) / (b.0.z - a.0.z);
                    clipped.push((a.0.lerp(b.0, t), a.1.lerp(b.1, t)));
                }
            }
            for i in 1..clipped.len().saturating_sub(1) {
                let triangle = [clipped[0], clipped[i], clipped[i + 1]];
                output.push(Projected {
                    depth: triangle.iter().map(|v| v.0.z).sum::<f32>() / 3.,
                    distances: triangle.map(|v| v.0.z.max(camera.near)),
                    near: camera.near,
                    points: triangle.map(|(p, _)| {
                        camera.centre + glam::Vec2::new(p.x, -p.y) * (camera.focal / p.z)
                    }),
                    colours: triangle.map(|(_, c)| Color(c.to_array())),
                });
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_and_near_clipping_use_shared_projection() {
        let triangle = [
            Vec3::new(-1., -1., 0.02),
            Vec3::new(1., -1., 2.),
            Vec3::new(0., 1., 2.),
        ];
        let item = placed("no-such-equipment", Vec3::ZERO, None, || vec![triangle]);
        assert!(!item.installed);
        let camera = Camera {
            eye: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
            forward: Vec3::Z,
            focal: 100.,
            centre: glam::Vec2::ZERO,
            near: 0.1,
        };
        let visible = project(&[item], glam::Mat4::IDENTITY, &camera);
        assert_eq!(
            visible.len(),
            2,
            "triangle crossing near plane becomes clipped quad"
        );
        assert!(
            visible
                .iter()
                .all(|t| t.depth >= camera.near && t.points.iter().all(|p| p.is_finite()))
        );
    }
}
