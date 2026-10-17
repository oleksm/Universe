//! Retained equipment shared by Studio and the world. Installed geometry is the
//! cache's original PBR model; only locally authored structure is built here.
pub use crate::visual_assets::Visual;
use glam::{Mat4, Quat, Vec3};
use std::sync::{Arc, OnceLock};
use universe_engine::{
    PbrModel,
    pbr::{Material, PbrData, PbrVertex, Primitive},
    scene::{Geometry, ObjectId, Pose, Scene},
    viewport::{PixelRect, Projection, View},
};

pub fn get(key: &str) -> Option<Arc<Visual>> {
    crate::visual_assets::request(key, 1).filter(|v| v.neutral_error.is_none())
}

/// Construction input for local structure only, never a copy of installed GLB geometry.
#[derive(Clone)]
pub struct Face {
    pub positions: [Vec3; 3],
    pub normals: [Vec3; 3],
    pub colour: universe_engine::Color,
    pub double_sided: bool,
    pub blend: bool,
    pub alpha_cutoff: Option<f32>,
}

#[derive(Clone)]
struct Resolved {
    model: PbrModel,
    transform: Mat4,
    bounds: Option<[glam::DVec3; 2]>,
}
impl Resolved {
    fn installed(v: Arc<Visual>, at: Vec3, r: Quat) -> Option<Self> {
        if v.neutral_error.is_some() {
            return None;
        }
        Some(Self {
            // Attach the lease to the submitted handle, never to Visual.model:
            // the scene and any in-flight frame must pin the entire cache entry.
            model: v.model.with_lease(v.clone()),
            transform: Mat4::from_rotation_translation(r, at - r * v.centre),
            bounds: Some([v.lo, v.hi]),
        })
    }
}

struct Pending {
    key: String,
    at: Vec3,
    rotation: Quat,
    ready: OnceLock<Resolved>,
}

/// Keep placements alongside stable document IDs. Construction builds local
/// geometry once; clones share it and the pending promotion cell. Physics and
/// the saved placement are never changed by a visual becoming ready.
#[derive(Clone)]
pub struct Placed {
    model: PbrModel,
    pub transform: Mat4,
    /// Whether installed at construction (not an operational/physics gate).
    bounds: Option<[glam::DVec3; 2]>,
    pending: Option<Arc<Pending>>,
}

impl Placed {
    pub fn structure(faces: Vec<Face>) -> Self {
        let mut data = PbrData::default();
        let mut groups = std::collections::BTreeMap::new();
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for face in faces {
            let key = (
                face.colour.0.map(f32::to_bits),
                face.double_sided,
                face.blend,
                face.alpha_cutoff.map(f32::to_bits),
            );
            let index = *groups.entry(key).or_insert_with(|| {
                let index = data.materials.len();
                data.materials.push(Material {
                    base_color: face.colour.0,
                    metallic: 0.,
                    roughness: 0.8,
                    emissive: [0.; 3],
                    normal_scale: 1.,
                    base_tex: None,
                    mr_tex: None,
                    normal_tex: None,
                    emissive_tex: None,
                    occlusion_tex: None,
                    occlusion_strength: 1.,
                    alpha_cutoff: face.alpha_cutoff,
                    blend: face.blend,
                    double_sided: face.double_sided,
                });
                data.primitives.push(Primitive {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    material: index,
                    part: 0,
                });
                index
            });
            let primitive = &mut data.primitives[index];
            for (p, n) in face.positions.into_iter().zip(face.normals) {
                lo = lo.min(p);
                hi = hi.max(p);
                primitive.indices.push(
                    primitive
                        .vertices
                        .len()
                        .try_into()
                        .expect("local structure exceeds u32 vertices"),
                );
                let n = n.normalize_or_zero();
                let tangent = if n.length_squared() > 0. {
                    n.any_orthonormal_vector()
                } else {
                    Vec3::X
                };
                primitive.vertices.push(PbrVertex {
                    pos: p.to_array(),
                    normal: n.to_array(),
                    tangent: tangent.extend(1.).to_array(),
                    uv: [0.; 2],
                });
            }
        }
        Self {
            model: PbrModel::new(data),
            transform: Mat4::IDENTITY,

            bounds: (lo.is_finite() && hi.is_finite()).then(|| [lo.as_dvec3(), hi.as_dvec3()]),
            pending: None,
        }
    }

    fn promote_with(&self, request: impl FnOnce(&str) -> Option<Arc<Visual>>) -> Option<&Resolved> {
        let pending = self.pending.as_ref()?;
        if pending.ready.get().is_none() {
            if let Some(ready) = request(&pending.key)
                .and_then(|v| Resolved::installed(v, pending.at, pending.rotation))
            {
                let _ = pending.ready.set(ready);
            }
        }
        pending.ready.get()
    }

    /// Upsert one stable session object. Parent must already exist. Promotion
    /// replaces this object's geometry/pose atomically; it does not add children.
    /// Local fallbacks are already in design space, matching placed_rotated's
    /// historical fallback contract. Installed models are centred at `at`.
    pub fn set_node(
        &self,
        scene: &mut Scene,
        id: &ObjectId,
        parent: Option<ObjectId>,
    ) -> Result<(), &'static str> {
        let ready = self.promote_with(get);
        let (model, transform, bounds) = ready
            .map_or((&self.model, self.transform, self.bounds), |r| {
                (&r.model, r.transform, r.bounds)
            });
        let pose = pose(transform)?;
        if let Some(node) = scene.get(*id) {
            if node.parent == parent
                && node.pose.position == pose.position
                && node.pose.rotation == pose.rotation
                && node.pose.scale == pose.scale
                && node.bounds == bounds
                && matches!(&node.geometry, Some(Geometry::Model { model: m, part: 0 }) if m.id() == model.id())
            {
                return Ok(());
            }
        }
        // Preserve the session's visibility/layer choices when a pending visual promotes.
        let mut node = scene.get(*id).cloned().unwrap_or_default();
        node.parent = parent;
        node.pose = pose;
        node.bounds = bounds;
        node.geometry = Some(Geometry::Model {
            model: model.clone(),
            part: 0,
        });
        scene.set(*id, node)
    }
}

fn pose(matrix: Mat4) -> Result<Pose, &'static str> {
    if !matrix.is_finite() || matrix.determinant() <= 0. {
        return Err("equipment transform must be rigid with positive uniform scale");
    }
    let (scale, rotation, position) = matrix.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.min_element() <= 0.
        || !scale.abs_diff_eq(Vec3::splat(scale.x), 1e-5)
        || !rotation.is_finite()
        || !matrix.abs_diff_eq(
            Mat4::from_scale_rotation_translation(scale, rotation, position),
            1e-5,
        )
    {
        return Err("equipment transform must be rigid with positive uniform scale");
    }
    Ok(Pose {
        position: position.as_dvec3(),
        rotation: rotation.as_dquat().normalize(),
        scale: scale.x as f64,
    })
}

pub fn rotation(thrust: Option<Vec3>) -> Quat {
    thrust
        .and_then(Vec3::try_normalize)
        .map_or(Quat::IDENTITY, |d| Quat::from_rotation_arc(Vec3::Y, d))
}

pub fn placed_rotated(
    key: &str,
    at: Vec3,
    r: Quat,
    fallback: impl FnOnce() -> Vec<[Vec3; 3]>,
) -> Placed {
    if let Some(ready) = get(key).and_then(|v| Resolved::installed(v, at, r)) {
        return Placed {
            model: ready.model,
            transform: ready.transform,
            bounds: ready.bounds,

            pending: None,
        };
    }
    let mut placed = Placed::structure(
        fallback()
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
                    blend: false,
                    alpha_cutoff: None,
                }
            })
            .collect(),
    );
    placed.pending = Some(Arc::new(Pending {
        key: key.into(),
        at,
        rotation: r,
        ready: OnceLock::new(),
    }));
    placed
}

/// Legacy camera input. New sessions can construct engine View directly.
pub struct Camera {
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub focal: f32,
    pub centre: glam::Vec2,
    pub near: f32,
}
impl Camera {
    /// The shared perspective is symmetric: rect must be centred on `centre`.
    pub fn view(&self, rect: PixelRect) -> Result<View, &'static str> {
        if !rect.origin.is_finite()
            || !rect.size.is_finite()
            || rect.size.min_element() <= 0.
            || !self.centre.abs_diff_eq(rect.origin + rect.size * 0.5, 0.01)
            || !self.eye.is_finite()
            || !self.focal.is_finite()
            || self.focal <= 0.
            || !self.near.is_finite()
            || self.near <= 0.
        {
            return Err("invalid equipment viewport or camera centre");
        }
        let basis = glam::Mat3::from_cols(self.right, self.up, -self.forward);
        if !basis.is_finite()
            || (basis.determinant() - 1.).abs() > 1e-4
            || !(basis.transpose() * basis).abs_diff_eq(glam::Mat3::IDENTITY, 1e-4)
        {
            return Err("equipment camera basis must be right-handed and orthonormal");
        }
        Ok(View {
            camera: universe_engine::Camera {
                position: self.eye.as_dvec3(),
                orientation: Quat::from_mat3(&basis).normalize(),
                fov_y: 2. * (rect.size.y * 0.5 / self.focal).atan(),
                near: self.near,
            },
            projection: Projection::Perspective,
            rect,
            layers: u64::MAX,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe_engine::scene::Node;
    use glam::{DQuat, DVec3};

    fn face(blend: bool, cutoff: Option<f32>) -> Face {
        Face {
            positions: [Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: [Vec3::Z; 3],
            colour: universe_engine::Color([0.2, 0.4, 0.6, 0.3]),
            double_sided: true,
            blend,
            alpha_cutoff: cutoff,
        }
    }
    fn visual(neutral_error: Option<String>) -> Arc<Visual> {
        Arc::new(Visual {
            source_sha256: "fixture".into(),
            model: Placed::structure(vec![face(false, None)]).model,
            centre: Vec3::new(2., 3., 4.),
            nodes: Default::default(),
            has_motion: true,
            lo: DVec3::ZERO,
            hi: DVec3::splat(8.),
            neutral_error,
        })
    }
    fn pending(at: Vec3, rotation: Quat) -> Placed {
        let mut item = Placed::structure(vec![face(false, None)]);
        item.pending = Some(Arc::new(Pending {
            key: "fixture".into(),
            at,
            rotation,
            ready: OnceLock::new(),
        }));
        item
    }
    fn model(scene: &Scene, id: ObjectId) -> &PbrModel {
        let Some(Geometry::Model { model, part: 0 }) = &scene.get(id).unwrap().geometry else {
            panic!("expected PBR model")
        };
        model
    }

    #[test]
    fn pending_promotes_same_object_once_with_parent_and_shared_lease() {
        let at = Vec3::new(7., 11., 13.);
        let rotation = Quat::from_euler(glam::EulerRot::XYZ, 0.2, 0.4, 0.6);
        let item = pending(at, rotation);
        let twin = item.clone();
        let mut scene = Scene::default();
        let (root, id) = (ObjectId(10), ObjectId(11));
        let parent = Pose {
            position: DVec3::new(100., -20., 30.),
            rotation: DQuat::from_rotation_y(0.7),
            scale: 2.,
        };
        scene
            .set(
                root,
                Node {
                    pose: parent,
                    ..Node::default()
                },
            )
            .unwrap();
        assert!(item.promote_with(|_| None).is_none());
        item.set_node(&mut scene, &id, Some(root)).unwrap();
        let fallback_id = model(&scene, id).id();
        // Consumer settings survive asynchronous replacement.
        let mut node = scene.get(id).unwrap().clone();
        node.layers = 4;
        node.visible = false;
        scene.set(id, node).unwrap();
        let ready = visual(None);
        let weak = Arc::downgrade(&ready);
        let model_id = ready.model.id();
        item.promote_with(|_| Some(ready.clone())).unwrap();
        twin.promote_with(|_| panic!("promoted clones must never request again"))
            .unwrap();
        item.set_node(&mut scene, &id, Some(root)).unwrap();
        assert_ne!(model(&scene, id).id(), fallback_id);
        assert_eq!(model(&scene, id).id(), model_id);
        assert!(Arc::ptr_eq(&model(&scene, id).data, &ready.model.data));
        assert_eq!(scene.get(id).unwrap().layers, 4);
        assert!(!scene.get(id).unwrap().visible);
        let world = scene.world_pose(id).unwrap();
        let centre = world.position + world.rotation * (ready.centre.as_dvec3() * world.scale);
        let expected = parent.position + parent.rotation * (at.as_dvec3() * parent.scale);
        // Placement matrices are legacy f32; decomposition and parent scaling
        // introduce a few micrometres of rounding at these metre coordinates.
        assert!(
            centre.abs_diff_eq(expected, 5e-6),
            "centre delta {:?}",
            centre - expected
        );
        assert!(
            (world.rotation * DVec3::X)
                .abs_diff_eq(parent.rotation * (rotation * Vec3::X).as_dvec3(), 1e-6)
        );
        let revision = scene.revision();
        item.set_node(&mut scene, &id, Some(root)).unwrap();
        assert_eq!(scene.revision(), revision);
        assert!(
            item.pending.is_some(),
            "promotion must not change saved/physics state"
        );
        // Queued frame submission is another clone of the very same PBR handle.
        let queued_model = model(&scene, id).clone();
        drop(ready);
        drop(item);
        drop(twin);
        assert!(weak.upgrade().is_some(), "scene pins the cache owner");
        scene.remove(root);
        assert!(
            weak.upgrade().is_some(),
            "queued submission also pins the owner"
        );
        drop(queued_model);
        assert!(weak.upgrade().is_none(), "no owner/model reference cycle");
    }
}
