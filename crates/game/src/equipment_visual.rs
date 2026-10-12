//! Shared immutable, hash-checked equipment geometry. Consumers decide whether
//! a neutral preview or a fully operational mounted mechanism is permitted.
pub use crate::visual_assets::Visual;
use glam::Vec3;
use std::sync::Arc;

pub fn get(key: &str) -> Option<Arc<Visual>> {
    crate::visual_assets::request(key, 1).filter(|v| v.neutral_error.is_none())
}

#[derive(Clone)]
pub struct Face {
    pub positions: [Vec3; 3],
    pub normals: [Vec3; 3],
    pub colour: universe_engine::Color,
    pub double_sided: bool,
    /// Authored glTF alpha mode; a low base alpha alone does not imply blending.
    pub blend: bool,
    pub alpha_cutoff: Option<f32>,
}

/// One resolved visual, shared by design, walk, balance and test drive. Physical
/// envelopes remain independent. transform maps authored metres into design space.
#[derive(Clone)]
pub struct Placed {
    pub faces: Arc<Vec<Face>>,
    pub transform: glam::Mat4,
    pub installed: bool,
    // Hold the full cache lease whenever faces are retained outside the cache.
    lease: Option<Arc<Visual>>,
    pending: Option<(String, Vec3, glam::Quat)>,
}

impl Placed {
    /// Locally authored structure has no external cache owner or pending asset.
    pub fn structure(faces: Vec<Face>) -> Self {
        Self {
            faces: Arc::new(faces),
            transform: glam::Mat4::IDENTITY,
            installed: false,
            lease: None,
            pending: None,
        }
    }
}

pub fn rotation(thrust: Option<Vec3>) -> glam::Quat {
    thrust
        .and_then(Vec3::try_normalize)
        .map_or(glam::Quat::IDENTITY, |d| {
            glam::Quat::from_rotation_arc(Vec3::Y, d)
        })
}

/// Full authoring orientation, shared by every Studio view.
pub fn placed_rotated(
    key: &str,
    at: Vec3,
    r: glam::Quat,
    fallback: impl FnOnce() -> Vec<[Vec3; 3]>,
) -> Placed {
    if let Some(v) = get(key) {
        return Placed {
            faces: v.faces.clone(),
            transform: glam::Mat4::from_rotation_translation(r, at - r * v.centre),
            installed: true,
            lease: Some(v),
            pending: None,
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
                blend: false,
                alpha_cutoff: None,
            }
        })
        .collect();
    Placed {
        faces: Arc::new(faces),
        transform: glam::Mat4::IDENTITY,
        installed: false,
        lease: None,
        pending: Some((key.into(), at, r)),
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
    pub blend: bool,
    pub alpha_cutoff: Option<f32>,
}
impl Projected {
    pub fn draw(&self, frame: &mut universe_engine::Frame) {
        let alpha = if self.blend {
            universe_engine::CanvasAlpha::Blend
        } else if let Some(cutoff) = self.alpha_cutoff {
            universe_engine::CanvasAlpha::Mask(cutoff)
        } else {
            universe_engine::CanvasAlpha::Opaque
        };
        frame.canvas_triangle_alpha(self.points, self.distances, self.near, self.colours, alpha);
    }
}

/// Camera adaptation, shading, near clipping and ordering have one implementation.
/// parent is identity in design/walk; flight supplies pose * translation(-COM).
#[cfg(test)]
pub fn project(placed: &[Placed], parent: glam::Mat4, camera: &Camera) -> Vec<Projected> {
    let mut output = Vec::new();
    project_each(placed, parent, camera, |triangle| output.push(triangle));
    output
}

/// Stream triangles into the consumer: dense installed meshes need no intermediate
/// projected mesh allocation/copy on every frame. Walk may still collect its labels
/// and geometry for ordering; design, catalogue and flight submit directly.
pub fn draw(
    placed: &[Placed],
    parent: glam::Mat4,
    camera: &Camera,
    frame: &mut universe_engine::Frame,
) {
    project_each(placed, parent, camera, |triangle| triangle.draw(frame));
}

pub fn project_each(
    placed: &[Placed],
    parent: glam::Mat4,
    camera: &Camera,
    mut emit: impl FnMut(Projected),
) {
    use universe_engine::Color;
    let _scope = universe_prof::scope("draw/studio/project-equipment");
    let light = (-camera.forward + camera.up * 0.7 - camera.right * 0.4).normalize();
    for item in placed {
        // A stand/test craft may have been built while the asset was pending.
        // Resolve again at draw time so it promotes without rebuilding physics.
        let ready = item.pending.as_ref().and_then(|(key, at, r)| {
            get(key).map(|v| {
                let matrix = glam::Mat4::from_rotation_translation(*r, *at - *r * v.centre);
                (v, matrix)
            })
        });
        let (faces, transform) = ready
            .as_ref()
            .map_or((&item.faces, item.transform), |(v, m)| (&v.faces, *m));
        let _lease = &item.lease;
        let matrix = parent * transform;
        for face in faces.iter() {
            let positions = face.positions.map(|p| matrix.transform_point3(p));
            let geometric = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
            if !face.double_sided && geometric.dot(camera.eye - positions[0]) <= 0. {
                continue;
            }
            let view = positions.map(|p| {
                let delta = p - camera.eye;
                Vec3::new(
                    delta.dot(camera.right),
                    delta.dot(camera.up),
                    delta.dot(camera.forward),
                )
            });
            if view.iter().all(|p| p.z < camera.near) {
                continue;
            }
            let vertices = [0, 1, 2].map(|i| {
                let p = view[i];
                let n = matrix
                    .transform_vector3(face.normals[i])
                    .normalize_or_zero();
                let shade = 0.3 + 0.7 * n.dot(light).max(0.);
                let c = glam::Vec4::from_array(face.colour.0)
                    * glam::Vec4::new(shade, shade, shade, 1.);
                (p, c)
            });
            // A triangle clipped by one plane has at most four vertices. The
            // overwhelmingly common all-visible case requires no clipping.
            let mut clipped = [(Vec3::ZERO, glam::Vec4::ZERO); 4];
            let mut count = 0;
            if view.iter().all(|p| p.z >= camera.near) {
                clipped[..3].copy_from_slice(&vertices);
                count = 3;
            } else {
                for i in 0..3 {
                    let (a, b) = (vertices[i], vertices[(i + 1) % 3]);
                    if a.0.z >= camera.near {
                        clipped[count] = a;
                        count += 1;
                    }
                    if (a.0.z >= camera.near) != (b.0.z >= camera.near) {
                        let t = (camera.near - a.0.z) / (b.0.z - a.0.z);
                        clipped[count] = (a.0.lerp(b.0, t), a.1.lerp(b.1, t));
                        count += 1;
                    }
                }
            }
            for i in 1..count.saturating_sub(1) {
                let triangle = [clipped[0], clipped[i], clipped[i + 1]];
                emit(Projected {
                    depth: triangle.iter().map(|v| v.0.z).sum::<f32>() / 3.,
                    distances: triangle.map(|v| v.0.z.max(camera.near)),
                    near: camera.near,
                    points: triangle.map(|(p, _)| {
                        camera.centre + glam::Vec2::new(p.x, -p.y) * (camera.focal / p.z)
                    }),
                    colours: triangle.map(|(_, c)| Color(c.to_array())),
                    blend: face.blend,
                    alpha_cutoff: face.alpha_cutoff,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference_project(placed: &[Placed], parent: glam::Mat4, camera: &Camera) -> Vec<Projected> {
        use universe_engine::Color;
        let light = (-camera.forward + camera.up * 0.7 - camera.right * 0.4).normalize();
        let mut output = Vec::new();
        for item in placed {
            // A stand/test craft may have been built while the asset was pending.
            // Resolve again at draw time so it promotes without rebuilding physics.
            let ready = item.pending.as_ref().and_then(|(key, at, r)| {
                get(key).map(|v| {
                    let matrix = glam::Mat4::from_rotation_translation(*r, *at - *r * v.centre);
                    (v, matrix)
                })
            });
            let (faces, transform) = ready
                .as_ref()
                .map_or((&item.faces, item.transform), |(v, m)| (&v.faces, *m));
            let _lease = &item.lease;
            let matrix = parent * transform;
            for face in faces.iter() {
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
                        blend: face.blend,
                        alpha_cutoff: face.alpha_cutoff,
                    });
                }
            }
        }
        output
    }

    // Independent pre-optimization path: all near-plane classifications, rotated
    // parents/placements, sidedness and material modes must yield identical pixels.
    #[test]
    fn streamed_projection_matches_reference_at_near_plane() {
        let camera = Camera {
            eye: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
            forward: Vec3::Z,
            focal: 120.,
            centre: glam::Vec2::splat(240.),
            near: 0.1,
        };
        let mut faces = Vec::new();
        for a in [-0.4, 0.1, 0.9] {
            for b in [-0.4, 0.1, 0.9] {
                for c in [-0.4, 0.1, 0.9] {
                    for double_sided in [false, true] {
                        for (blend, alpha_cutoff) in
                            [(false, None), (true, None), (false, Some(0.5))]
                        {
                            faces.push(Face {
                                positions: [
                                    Vec3::new(-1., -1., a),
                                    Vec3::new(0., 1., b),
                                    Vec3::new(1., -1., c),
                                ],
                                normals: [Vec3::NEG_Z, Vec3::NEG_Y, Vec3::X],
                                colour: universe_engine::Color([0.2, 0.5, 0.8, 0.4]),
                                double_sided,
                                blend,
                                alpha_cutoff,
                            });
                        }
                    }
                }
            }
        }
        let mut item = Placed::structure(faces);
        for angle in [0., 0.3, 1.5] {
            item.transform = glam::Mat4::from_rotation_y(angle);
            let parent = glam::Mat4::from_rotation_x(angle * 0.5);
            let expected = reference_project(std::slice::from_ref(&item), parent, &camera);
            let actual = project(std::slice::from_ref(&item), parent, &camera);
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(&expected) {
                assert_eq!(a.points, b.points);
                assert_eq!(a.distances, b.distances);
                assert_eq!(a.depth, b.depth);
                assert_eq!(a.colours.map(|c| c.0), b.colours.map(|c| c.0));
                assert_eq!(a.blend, b.blend);
                assert_eq!(a.alpha_cutoff, b.alpha_cutoff);
            }
        }
    }

    #[test]
    fn pending_stand_promotes_and_world_uses_same_model() {
        use std::time::{Duration, Instant};
        if universe_sim::world::assets::store().is_none() {
            return;
        }
        let key = "equipment.engine.ch.s2";
        let at = Vec3::new(3., 2., 10.);
        let r = glam::Quat::from_euler(glam::EulerRot::XYZ, 0.21, 0.37, 0.55);
        // Represent a stand created before loading starts, regardless of other tests.
        let mut pending = placed_rotated("not-installed", at, r, || vec![]);
        pending.pending = Some((key.into(), at, r));
        let until = Instant::now() + Duration::from_secs(15);
        let ready = loop {
            if let Some(v) = get(key) {
                break v;
            }
            assert!(
                Instant::now() < until,
                "{:?}",
                crate::visual_assets::snapshot()
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        let world = crate::visuals::Visuals
            .get(key)
            .expect("world shares ready entry");
        assert!(Arc::ptr_eq(&ready, &world));
        let installed = placed_rotated(key, at, r, || panic!("ready model fell back"));
        assert!(Arc::ptr_eq(installed.lease.as_ref().unwrap(), &ready));
        assert!(
            installed
                .transform
                .transform_vector3(Vec3::X)
                .abs_diff_eq(r * Vec3::X, 1e-6)
        );
        let cam = Camera {
            eye: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
            forward: Vec3::Z,
            focal: 100.,
            centre: glam::Vec2::ZERO,
            near: 0.1,
        };
        // Both identity (walk) and craft parent (balance/drive) must promote.
        for parent in [
            glam::Mat4::IDENTITY,
            glam::Mat4::from_rotation_translation(
                glam::Quat::from_rotation_y(0.15),
                Vec3::new(1., 0., 2.),
            ),
        ] {
            let a = project(std::slice::from_ref(&pending), parent, &cam);
            let b = project(std::slice::from_ref(&installed), parent, &cam);
            assert!(!a.is_empty());
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(&b) {
                assert_eq!(a.points, b.points);
                assert_eq!(a.distances, b.distances);
                assert_eq!(a.blend, b.blend);
                assert_eq!(a.alpha_cutoff, b.alpha_cutoff);
            }
        }
        assert!(
            !pending.installed,
            "promotion does not mutate saved placement or physics"
        );
    }
    #[test]
    fn authored_alpha_survives_clipping_and_placed_transforms() {
        let camera = Camera {
            eye: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
            forward: Vec3::Z,
            focal: 100.,
            centre: glam::Vec2::ZERO,
            near: 0.1,
        };
        // Low factor alpha is valid even for OPAQUE: preserve the mode separately.
        for (blend, alpha_cutoff) in [
            (false, None),
            (false, Some(0.5)),
            (false, Some(1.5)),
            (true, None),
        ] {
            let face = Face {
                positions: [
                    Vec3::new(-1., -1., 0.02),
                    Vec3::new(1., -1., 2.),
                    Vec3::new(0., 1., 2.),
                ],
                normals: [Vec3::NEG_Z; 3],
                colour: universe_engine::Color([0.3, 0.6, 0.9, 0.2]),
                double_sided: true,
                blend,
                alpha_cutoff,
            };
            let item = Placed {
                faces: Arc::new(vec![face]),
                transform: glam::Mat4::from_translation(Vec3::X),
                installed: true,
                lease: None,
                pending: None,
            };
            // A flight parent and placement cancel; every clipped child keeps its material.
            let projected = project(&[item], glam::Mat4::from_translation(Vec3::NEG_X), &camera);
            assert_eq!(projected.len(), 2);
            for triangle in projected {
                assert_eq!(triangle.blend, blend);
                assert_eq!(triangle.alpha_cutoff, alpha_cutoff);
                assert!(triangle.colours.iter().all(|c| (c.0[3] - 0.2).abs() < 1e-6));
                assert!(triangle.distances.iter().all(|z| *z >= camera.near));
            }
        }
    }
    #[test]
    fn fallback_and_near_clipping_use_shared_projection() {
        let triangle = [
            Vec3::new(-1., -1., 0.02),
            Vec3::new(1., -1., 2.),
            Vec3::new(0., 1., 2.),
        ];
        let item = placed_rotated(
            "no-such-equipment",
            Vec3::ZERO,
            glam::Quat::IDENTITY,
            || vec![triangle],
        );
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
        assert!(visible.iter().all(|t| !t.blend && t.alpha_cutoff.is_none()));
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
